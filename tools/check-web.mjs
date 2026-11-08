import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {createServer} from 'node:http';
import {mkdtemp, mkdir, readFile, writeFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const output = path.join(root, 'target/browser');
await mkdir(output, {recursive: true});
const server = createServer(async (request, response) => {
  try {
    const name = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
    const file = path.resolve(root, 'web', name === '/' ? 'index.html' : '.' + name);
    assert(file.startsWith(path.join(root, 'web') + path.sep));
    const body = await readFile(file);
    const types = {'.html':'text/html', '.js':'text/javascript', '.wasm':'application/wasm'};
    response.writeHead(200, {'Content-Type': types[path.extname(file)] || 'application/octet-stream'});
    response.end(body);
  } catch { response.writeHead(404); response.end('Not found'); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const profile = await mkdtemp(path.join(tmpdir(), 'layerprocgen-web-'));
const mode = process.env.WEBGL_GPU || 'swiftshader';
assert(['swiftshader','vulkan'].includes(mode));
const chrome = spawn(process.env.CHROMIUM || 'chromium', [
  '--headless=new', '--no-first-run', '--no-default-browser-check', '--disable-background-networking',
  '--disable-renderer-backgrounding', '--disable-background-timer-throttling', '--disable-gpu-sandbox',
  ...(mode === 'vulkan' ? ['--use-gl=angle','--use-angle=vulkan','--enable-features=Vulkan','--disable-vulkan-surface'] : ['--use-angle=swiftshader','--enable-unsafe-swiftshader']),
  `--user-data-dir=${profile}`, '--remote-debugging-port=0', '--window-size=1100,760', 'about:blank',
], {stdio: ['ignore','ignore','pipe']});
let diagnostics = '';
chrome.stderr.on('data', data => diagnostics += data);
chrome.on('error', error => diagnostics += error.message);
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
async function until(read, condition, label, timeout = 30000) {
  const end = Date.now() + timeout;
  let last;
  while (Date.now() < end) { last = await read(); if (condition(last)) return last; await pause(80); }
  throw Error(`${label}: ${JSON.stringify(last)}`);
}
class Cdp {
  constructor(url) { this.socket = new WebSocket(url); this.id = 0; this.pending = new Map(); this.errors = []; }
  async connect() {
    await new Promise((resolve,reject) => { this.socket.onopen=resolve; this.socket.onerror=reject; });
    this.socket.onmessage = event => {
      const message=JSON.parse(event.data);
      if (message.method === 'Runtime.exceptionThrown') this.errors.push(message.params.exceptionDetails);
      const pending=this.pending.get(message.id);
      if (pending) { this.pending.delete(message.id); clearTimeout(pending.timer); message.error ? pending.reject(Error(message.error.message)) : pending.resolve(message.result); }
    };
  }
  send(method, params={}) {
    return new Promise((resolve,reject) => {
      const id=++this.id;
      const timer=setTimeout(() => { this.pending.delete(id); reject(Error(`CDP timeout ${method}`)); },30000);
      this.pending.set(id,{resolve,reject,timer});
      this.socket.send(JSON.stringify({id,method,params}));
    });
  }
  async evaluate(expression) {
    const result=await this.send('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true,userGesture:true});
    if (result.exceptionDetails) throw Error(result.exceptionDetails.exception?.description || result.exceptionDetails.text);
    return result.result.value;
  }
  close() { this.socket.close(); }
}
let browser, page;
const report = {gpuMode: mode, checks: []};
try {
  const endpoint = await until(() => /DevTools listening on (ws:\/\/\S+)/.exec(diagnostics)?.[1], Boolean, 'Chromium startup');
  browser = new Cdp(endpoint); await browser.connect();
  report.browser = await browser.send('Browser.getVersion');
  const {targetId} = await browser.send('Target.createTarget', {url:'about:blank'});
  const targets = await fetch(`http://${new URL(endpoint).host}/json/list`).then(r => r.json());
  page = new Cdp(targets.find(t => t.id===targetId).webSocketDebuggerUrl); await page.connect();
  await page.send('Page.enable'); await page.send('Runtime.enable');
  await page.send('Emulation.setDeviceMetricsOverride', {width:1100,height:760,deviceScaleFactor:1,mobile:false});
  await page.send('Page.addScriptToEvaluateOnNewDocument', {source: 'Object.defineProperty(navigator,"gpu",{value:undefined,configurable:true});'});
  await page.send('Page.navigate', {url:`http://127.0.0.1:${server.address().port}/`});
  await until(() => page.evaluate('({state:window.layerprocgen?.state,error:window.layerprocgenError})'), r => { assert(!r.error,r.error); return r.state?.objects>0; },'Rust viewer readiness');
  const state = () => page.evaluate('JSON.parse(window.layerprocgen.viewer.stats())');
  const initial = await state();
  assert.equal(initial.renderer,'webgl2');
  for (const point of [[92,167],[130,196],[247,2]]) assert(initial.positions.some(p => p[0]===point[0] && p[1]===point[1]),`C++ origin point ${point}`);
  report.initial = initial;
  report.checks.push('Rust Wasm starts with WebGPU unavailable and contains C++ point fixtures');
  const pixels = await page.evaluate(`(() => {
    const canvas=document.getElementById('world'), viewer=window.layerprocgen.viewer;
    viewer.frame(canvas.width,canvas.height);
    const gl=canvas.getContext('webgl2');
    const data=new Uint8Array(canvas.width*canvas.height*4);
    gl.readPixels(0,0,canvas.width,canvas.height,gl.RGBA,gl.UNSIGNED_BYTE,data);
    const colors=new Set(); let bright=0;
    for(let i=0;i<data.length;i+=4){ colors.add(data[i]+','+data[i+1]+','+data[i+2]); if(data[i+1]>70)bright++; }
    const ext=gl.getExtension('WEBGL_debug_renderer_info');
    return {colors:colors.size,bright,version:gl.getParameter(gl.VERSION),renderer:gl.getParameter(ext?ext.UNMASKED_RENDERER_WEBGL:gl.RENDERER),error:gl.getError()};
  })()`);
  assert.match(pixels.version,/WebGL 2/); assert(pixels.colors>10); assert(pixels.bright>100); assert.equal(pixels.error,0);
  if(mode==='vulkan') assert(!/swiftshader|llvmpipe/i.test(pixels.renderer),'Hardware mode must use the GPU');
  report.pixels=pixels;
  report.checks.push('WebGL2 produces colored cube pixels with no GL error');
  const screenshot=await page.send('Page.captureScreenshot',{format:'png'});
  await writeFile(path.join(output,'viewer.png'),Buffer.from(screenshot.data,'base64'));
  await page.send('Input.dispatchKeyEvent',{type:'keyDown',key:'d',code:'KeyD'});
  await pause(300);
  await page.send('Input.dispatchKeyEvent',{type:'keyUp',key:'d',code:'KeyD'});
  assert((await state()).x>initial.x+1,'Keyboard moves camera');
  await page.send('Input.dispatchMouseEvent',{type:'mousePressed',x:550,y:400,button:'left',clickCount:1});
  const beforeDrag=await state();
  await page.send('Input.dispatchMouseEvent',{type:'mouseMoved',x:650,y:440,button:'left',buttons:1});
  await page.send('Input.dispatchMouseEvent',{type:'mouseReleased',x:650,y:440,button:'left',clickCount:1});
  assert.notEqual((await state()).x,beforeDrag.x,'Drag moves camera');
  report.checks.push('Real keyboard and pointer input move the camera');
  await page.evaluate("document.getElementById('pause').click(); window.layerprocgen.viewer.set_position(-950000,-950000)");
  const queued=await until(state,s=>s.pending>0,'Paused queue accumulates movement actions');
  assert.deepEqual(queued.positions,(await state()).positions,'Paused queue preserves scene');
  await page.evaluate("document.getElementById('pause').click()");
  const moved=await until(state,s=>s.pending===0 && s.destroyed>0 && s.positions.every(p=>p[0]<-900000),'Move retires old objects');
  assert(moved.created>initial.created); report.moved={objects:moved.objects,created:moved.created,destroyed:moved.destroyed};
  await page.evaluate("document.getElementById('home').click()");
  const home=await until(state,s=>s.x===0 && s.y===0 && s.pending===0 && s.positions.some(p=>p[0]===92 && p[1]===167),'Return to origin');
  const sorted=positions=>positions.map(p=>p.join(',')).sort();
  assert.deepEqual(sorted(home.positions),sorted(initial.positions),'Return regenerates exact world');
  report.checks.push('Paused movement, retirement, and far-coordinate return preserve deterministic positions');
  await page.evaluate("document.getElementById('stop').click()");
  const stopped=await until(state,s=>s.stopped && s.objects===0 && s.pending===0,'Shutdown drains scene');
  assert.equal(stopped.created,stopped.destroyed,'Every created object is destroyed');
  await page.evaluate("document.getElementById('stop').click()");
  await until(state,s=>!s.stopped && s.objects>0,'Restart');
  await page.send('Emulation.setDeviceMetricsOverride',{width:390,height:844,deviceScaleFactor:1,mobile:true});
  await until(()=>page.evaluate('document.getElementById("world").width'),width=>width===390,'Mobile resize');
  await page.send('Emulation.setTouchEmulationEnabled',{enabled:true,maxTouchPoints:5});
  const fingers = distance => [{id:1,x:195-distance/2,y:400},{id:2,x:195+distance/2,y:400}];
  await page.evaluate("document.getElementById('home').click()");
  const touchBase=await state();
  await page.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:fingers(100)});
  await page.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:fingers(200)});
  const zoomed=await until(state,s=>Math.abs(s.span-260)<1,'Pinch apart zooms in');
  assert.equal(zoomed.x,touchBase.x); assert.equal(zoomed.y,touchBase.y);
  await page.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:fingers(50)});
  await until(state,s=>Math.abs(s.span-1040)<1,'Pinch together zooms out');
  await page.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:fingers(20)});
  await until(state,s=>s.span===1200,'Pinch clamps zoom out');
  await page.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:fingers(360)});
  await until(state,s=>s.span===220,'Pinch clamps zoom in');
  await page.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
  const beforeTouchDrag=await state();
  await page.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{id:3,x:180,y:400}]});
  await page.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{id:3,x:210,y:420}]});
  const dragged=await until(state,s=>s.x!==beforeTouchDrag.x,'One-finger drag works after pinch');
  assert.equal(dragged.span,beforeTouchDrag.span);
  await page.send('Input.dispatchTouchEvent',{type:'touchCancel',touchPoints:[]});
  await page.send('Input.dispatchMouseEvent',{type:'mouseWheel',x:195,y:400,deltaX:0,deltaY:200});
  await until(state,s=>s.span>220,'Wheel zoom remains available');
  report.checks.push('Real touch pinch zooms in and out, clamps both limits without panning, and preserves dragging and wheel zoom');
  await page.evaluate("document.getElementById('home').click()");
  const oriented=await state();
  const angled=(angle,y=400)=>[{id:4,x:195-50*Math.cos(angle),y:y-50*Math.sin(angle)},{id:5,x:195+50*Math.cos(angle),y:y+50*Math.sin(angle)}];
  await page.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:angled(0)});
  await page.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:angled(Math.PI/4)});
  const turned=await until(state,s=>Math.abs(s.yaw-oriented.yaw-Math.PI/4)<.01,'Two-finger twist changes yaw');
  assert(Math.abs(turned.span-oriented.span)<1,'Twist preserves zoom');
  for(let offset=10;offset<=40;offset+=10) await page.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:angled(Math.PI/4,400+offset)});
  await until(state,s=>Math.abs(s.pitch-(oriented.pitch-.2))<.01,'Two-finger slide changes tilt');
  for(let offset=50;offset<=120;offset+=10) await page.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:angled(Math.PI/4,400+offset)});
  await until(state,s=>Math.abs(s.pitch-Math.PI/12)<.01,'Tilt clamps lower angle');
  for(let offset=110;offset>=-180;offset-=10) await page.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:angled(Math.PI/4,400+offset)});
  const tilted=await until(state,s=>Math.abs(s.pitch-4*Math.PI/9)<.01,'Tilt clamps upper angle');
  assert(Math.abs(tilted.span-oriented.span)<1,'Tilt preserves zoom');
  assert.equal(tilted.x,oriented.x); assert.equal(tilted.y,oriented.y);
  await page.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
  await page.evaluate("document.getElementById('home').click()");
  const resetAngle=await state();
  assert.equal(resetAngle.yaw,oriented.yaw);assert.equal(resetAngle.pitch,oriented.pitch);
  report.checks.push('Real two-finger twist rotates, vertical slide tilts within limits, and Home resets orientation');

  await page.evaluate("document.getElementById('route').click()");
  await until(state,s=>s.mode==='route','Climate route loads in Rust viewer');
  const route = await page.evaluate('window.layerprocgen.route');
  assert.equal(route.sampler,'rfl_world::SurfaceTerrain::elevation');
  assert.deepEqual(route.siteIds,['0:21:21:0:0','0:18:22:0:4']);
  assert.deepEqual(route.width,{surface:4,shoulder:1,material:5});
  for (const [i,point] of [[0,route.points[0]],[1,route.points.at(-1)]]) {
    assert.equal(point[0],route.manifest.sites[i].chart.x);
    assert.equal(point[2],route.manifest.sites[i].chart.z);
  }
  const routePixels = () => page.evaluate(`(() => {
    const canvas=document.getElementById('world');
    window.layerprocgen.viewer.frame(canvas.width,canvas.height);
    const gl=canvas.getContext('webgl2'),data=new Uint8Array(canvas.width*canvas.height*4);
    gl.readPixels(0,0,canvas.width,canvas.height,gl.RGBA,gl.UNSIGNED_BYTE,data);
    const isGold=i=>data[i]>200 && data[i+1]>140 && data[i+1]<220 && data[i+2]<100;
    let gold=0,start=0,goal=0;
    for(let i=0;i<data.length;i+=4){
      if(data[i]>200 && data[i+1]>140 && data[i+1]<220 && data[i+2]<100)gold++;
      if(data[i]<100 && data[i+1]>220 && data[i+2]>220)start++;
      if(data[i]>230 && data[i+1]>60 && data[i+1]<140 && data[i+2]<110)goal++;
    }
    const state=JSON.parse(window.layerprocgen.viewer.stats()),points=window.layerprocgen.route.points.slice(12,-12);
    let covered=0;
    for(const [x,h,z] of points){
      const dx=x-state.x,dz=z-state.y,rx=dx*Math.cos(state.yaw)-dz*Math.sin(state.yaw),ry=dx*Math.sin(state.yaw)+dz*Math.cos(state.yaw);
      const px=Math.round((rx/(state.span*canvas.width/canvas.height)+1)*canvas.width/2);
      const py=Math.round((((h*80+.5)*Math.cos(state.pitch)-ry*Math.sin(state.pitch))/state.span+1)*canvas.height/2);
      let found=false;
      for(let y=py-1;y<=py+1;y++)for(let x=px-1;x<=px+1;x++)if(x>=0&&y>=0&&x<canvas.width&&y<canvas.height&&isGold((y*canvas.width+x)*4))found=true;
      if(found)covered++;
    }
    return {gold,start,goal,centerlineCoverage:covered/points.length,error:gl.getError()};
  })()`);
  const mobileRoutePixels=await routePixels();
  assert(mobileRoutePixels.gold>100,'Route surface renders gold pixels');
  assert(mobileRoutePixels.start>5 && mobileRoutePixels.goal>5,'Both real site markers fit on mobile');
  assert.equal(mobileRoutePixels.error,0);
  assert(mobileRoutePixels.centerlineCoverage>.98,`Route surface must be continuous at 45-degree tilt: ${mobileRoutePixels.centerlineCoverage}`);
  for(const angle of [Math.PI/12,4*Math.PI/9]) {
    await page.evaluate(`window.layerprocgen.viewer.set_angles(${(await state()).yaw},${angle})`);
    assert((await routePixels()).centerlineCoverage>.98,`Route remains continuous at tilt ${angle}`);
  }
  await page.evaluate("document.getElementById('home').click()");
  const routeBase=await state();
  await page.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:fingers(100)});
  await page.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:fingers(200)});
  await until(state,s=>Math.abs(s.span-routeBase.span/2)<1,'Climate route pinch zoom');
  await page.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
  await page.evaluate("document.getElementById('home').click();document.getElementById('stop').click()");
  await until(state,s=>s.stopped,'Climate route stops');
  assert.equal((await routePixels()).gold,0);
  await until(()=>page.evaluate("document.getElementById('status').textContent"),s=>s.includes('stopped'),'Stopped route status updates');
  await page.evaluate("document.getElementById('stop').click()");
  await until(state,s=>!s.stopped,'Climate route restarts');
  assert((await routePixels()).gold>100);
  await page.send('Emulation.setDeviceMetricsOverride',{width:1100,height:760,deviceScaleFactor:1,mobile:false});
  await until(()=>page.evaluate('document.getElementById("world").width'),width=>width===1100,'Route desktop resize');
  await page.evaluate("document.getElementById('home').click()");
  const routeScreenshot=await page.send('Page.captureScreenshot',{format:'png'});
  await writeFile(path.join(output,'route.png'),Buffer.from(routeScreenshot.data,'base64'));
  report.route={siteIds:route.siteIds,points:route.points.length,width:route.width,mobilePixels:mobileRoutePixels};
  await page.evaluate("document.getElementById('route').click()");
  await until(state,s=>s.mode==='points' && s.objects>0,'Return to deterministic points scene');
  report.checks.push('Climate route loads exact site endpoints and widths, renders terrain, path and both markers on mobile, supports pinch, stops/restarts, and switches back to points');

  await page.evaluate("window.layerprocgen.viewer.frame(document.getElementById('world').width,document.getElementById('world').height)");
  await page.evaluate("window.dispatchEvent(new PageTransitionEvent('pagehide',{persisted:true})); window.dispatchEvent(new PageTransitionEvent('pageshow',{persisted:true}));");
  await pause(100);
  await page.evaluate("window.layerprocgen.viewer.frame(document.getElementById('world').width,document.getElementById('world').height)");
  report.checks.push('Stop destroys every object, restart succeeds, mobile viewport renders, and persisted pagehide retains the viewer');
  await page.send('Page.navigate',{url:`http://127.0.0.1:${server.address().port}/?scene=route`});
  await until(()=>page.evaluate('window.layerprocgen?.state'),s=>s?.mode==='route','Direct climate route link');
  assert((await routePixels()).centerlineCoverage>.98,'Direct link renders a continuous route');
  report.checks.push('Direct route link starts the Rust WebGL2 terrain preview');
  assert.deepEqual(page.errors,[],'No browser exceptions');
  assert.equal(await page.evaluate('window.layerprocgenError ?? null'),null);
  report.result='PASS';
  await writeFile(path.join(output,'report.json'),JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify({result:report.result,checks:report.checks,pixels:report.pixels,screenshot:path.join(output,'viewer.png')},null,2));
} finally {
  await writeFile(path.join(output,'chromium.log'),diagnostics);
  page?.close();
  if(browser){try{await browser.send('Browser.close');}catch{} browser.close();}
  chrome.kill('SIGTERM');
  await new Promise(resolve=>server.close(resolve));
  await rm(profile,{recursive:true,force:true,maxRetries:5,retryDelay:100});
}
