import init, { Viewer } from './pkg/layerprocgen_web.js';

const canvas = document.getElementById('world');
const status = document.getElementById('status');
const error = document.getElementById('error');
const keys = new Set();
let viewer;
let paused = false;
let stopped = false;
let span = 520;
let minSpan=220,maxSpan=1200;
let routeData=null;
const routeSpan = () => Math.max(85,85*canvas.clientHeight/canvas.clientWidth);
let yaw = Math.atan2(.6,.8);
let pitch = Math.PI / 6;
let previous;
const pointers = new Map();

function fail(reason) {
  error.textContent = String(reason?.stack || reason);
  status.textContent = 'Viewer stopped';
  window.layerprocgenError = error.textContent;
}

function render(time) {
  try {
    const dt = Math.min((time - (previous ?? time)) / 1000, 0.05);
    previous = time;
    const scale = Math.min(devicePixelRatio || 1, 2);
    const width = Math.max(1, Math.round(canvas.clientWidth * scale));
    const height = Math.max(1, Math.round(canvas.clientHeight * scale));
    if (canvas.width !== width || canvas.height !== height) { canvas.width = width; canvas.height = height; }
    const right = Number(keys.has('d') || keys.has('ArrowRight')) - Number(keys.has('a') || keys.has('ArrowLeft'));
    const down = Number(keys.has('s') || keys.has('ArrowDown')) - Number(keys.has('w') || keys.has('ArrowUp'));
    if (right || down) viewer.pan_screen(right * dt * span, down * dt * span);
    viewer.frame(width, height);
    const state = JSON.parse(viewer.stats());
    status.textContent = `${Math.round(state.x)}, ${Math.round(state.y)} · ${state.objects} cubes · ${state.pending} queued${stopped ? ' · stopped' : paused ? ' · paused' : ''}`;
    if (routeData) status.textContent=`${routeData.siteIds.join(' → ')} · width ${routeData.width.surface} · heights ×80${stopped ? ' · stopped' : ''}`;
    window.layerprocgen.state = state;
    requestAnimationFrame(render);
  } catch (reason) { fail(reason); }
}

try {
  await init();
  viewer = new Viewer(canvas);
  window.layerprocgen = { viewer, state: null };
  document.getElementById('home').onclick = () => { viewer.set_position(0, 0); span = routeData ? routeSpan() : 520; yaw = Math.atan2(.6,.8); pitch = routeData ? Math.PI/4 : Math.PI / 6; viewer.set_zoom(span); viewer.set_angles(yaw,pitch); };
  const routeButton = document.getElementById('route');
  routeButton.onclick = async () => {
    routeButton.disabled = true;
    try {
      if (routeData) {viewer.show_points();routeData=null;minSpan=220;maxSpan=1200;span=520;delete window.layerprocgen.route;routeButton.textContent='Climate path';}
      else {
        const response=await fetch('./route-checkpoint.json');if(!response.ok) throw Error('Cannot load fixed climate route');
        const data=await response.json();const g=data.manifest.grid;
        viewer.show_route(new Float64Array(data.ground),new Float64Array(data.points.flat()),new Float64Array([g.width,g.rows,g.spacing,g.minX,g.minZ,data.width.surface]));
        routeData=data;window.layerprocgen.route=data;minSpan=40;maxSpan=800;span=110;routeButton.textContent='Points';
      }
      document.querySelector('h1').textContent=routeData ? 'RFL climate path' : 'LayerProCGen';
      document.getElementById('subtitle').textContent=`Rust · WebGL2 · seed ${routeData ? routeData.manifest.seed : 1234}`;
      canvas.setAttribute('aria-label',routeData ? 'Fixed RFL terrain patch with a path between two building locations. Drag, pinch, twist or tilt to explore.' : 'Procedurally generated cubes. Drag to explore or use WASD and arrow keys.');
      document.getElementById('pause').hidden=Boolean(routeData);
      stopped=false;document.getElementById('stop').textContent='Stop';
      document.getElementById('home').click();
    } catch(reason) {fail(reason);}
    finally {routeButton.disabled=false;}
  };
  document.getElementById('pause').onclick = (event) => { paused = !paused; viewer.set_paused(paused); event.target.textContent = paused ? 'Resume queue' : 'Pause queue'; };
  document.getElementById('stop').onclick = (event) => { stopped = !stopped; if (stopped) viewer.shutdown(); else viewer.restart(); event.target.textContent = stopped ? 'Restart' : 'Stop'; };
  window.addEventListener('keydown', event => { if (event.target instanceof HTMLButtonElement) return; if (['w','a','s','d','ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(event.key)) { keys.add(event.key); event.preventDefault(); } });
  window.addEventListener('keyup', event => keys.delete(event.key));
  window.addEventListener('blur', () => { keys.clear(); pointers.clear(); });
  const gesture = () => {
    const [a, b] = pointers.values();
    return b ? { distance: Math.hypot(b.x-a.x,b.y-a.y), angle: Math.atan2(b.y-a.y,b.x-a.x), y: (a.y+b.y)/2 } : null;
  };
  canvas.addEventListener('pointerdown', event => {
    if (event.pointerType === 'mouse' && event.button !== 0) return;
    pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
    canvas.setPointerCapture(event.pointerId);
    canvas.focus();
  });
  canvas.addEventListener('pointermove', event => {
    const pointer = pointers.get(event.pointerId);
    if (!pointer) return;
    const before = gesture();
    pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
    if (pointers.size > 1) {
      const after = gesture();
      if (before.distance > 0 && after.distance > 0) {
        const delta = after.angle - before.angle;
        yaw += Math.atan2(Math.sin(delta), Math.cos(delta));
        yaw = Math.atan2(Math.sin(yaw),Math.cos(yaw));
        pitch = Math.max(Math.PI/12,Math.min(4*Math.PI/9,pitch-(after.y-before.y)*.005));
        viewer.set_angles(yaw,pitch);
        span = Math.max(minSpan, Math.min(maxSpan, span * before.distance / after.distance));
        viewer.set_zoom(span);
      }
      return;
    }
    const dx = (event.clientX - pointer.x) * span * 2 / canvas.clientHeight;
    const dy = (event.clientY - pointer.y) * span * 2 / canvas.clientHeight;
    viewer.pan_screen(-dx,-dy);
  });
  const release = event => pointers.delete(event.pointerId);
  canvas.addEventListener('pointerup', release);
  canvas.addEventListener('pointercancel', release);
  canvas.addEventListener('lostpointercapture', release);
  canvas.addEventListener('wheel', event => { event.preventDefault(); span = Math.max(minSpan, Math.min(maxSpan, span * Math.exp(event.deltaY * .001))); viewer.set_zoom(span); }, { passive: false });
  canvas.addEventListener('webglcontextlost', event => { event.preventDefault(); fail('WebGL context lost. Reload this page to restore the viewer.'); });
  window.addEventListener('pagehide', event => { if (!event.persisted) viewer.free(); });
  if (new URLSearchParams(location.search).get('scene') === 'route') await routeButton.onclick();
  requestAnimationFrame(render);
} catch (reason) { fail(reason); }
