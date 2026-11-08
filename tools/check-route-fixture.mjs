import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
const fixture=new URL('../fixtures/rfl-route/',import.meta.url);
const json=async name=>JSON.parse(await readFile(new URL(name,fixture),'utf8'));
const manifest=await json('manifest.json');
const route=await json('route.json');
assert.equal(manifest.schema,1);
assert.equal(route.schema,1);
assert.equal(manifest.snapshotSha256,'e9225cfd01683391d78899510e89d213351d563184dc77d95c9b27d2dee4db0a');
assert.equal(route.snapshotSha256,manifest.snapshotSha256);
assert.equal(route.sampler,'rfl_world::SurfaceTerrain::elevation');
assert.equal(route.coordinateSystem,'local-gnomonic-x-height-z');
assert.deepEqual(route.siteIds,['0:21:21:0:0','0:18:22:0:4']);
assert.deepEqual(route.siteIds,manifest.sites.map(s=>s.id));
assert.deepEqual(route.width,{surface:4,shoulder:1,material:5});
for(const [name,key,length] of [
 ['heights.f32','heightsSha256',manifest.width*manifest.rows*4],
 ['ground.f64','groundSha256',manifest.grid.width*manifest.grid.rows*8],
 ['basins.u8','basinsSha256',64*33],
]) {
 const data=await readFile(new URL(name,fixture));
 assert.equal(data.length,length,`${name} dimensions`);
 assert.equal(createHash('sha256').update(data).digest('hex'),manifest[key],`${name} integrity`);
}
assert(route.points.length>2 && route.raw.length>2);
assert.equal(route.bodyLocalPoints.length,route.points.length);
const near=(a,b,message)=>assert(Math.abs(a-b)<1e-8,`${message}: ${a} vs ${b}`);
for(const [i,p] of route.points.entries()) {
 assert.equal(p.length,3); assert(p.every(Number.isFinite));
 assert(p[1]>manifest.seaLevel,'dry centerline');
 const {center,right,forward}=manifest.frame;
 const q=center.map((v,j)=>v+(right[j]*p[0]+forward[j]*p[2])/manifest.radius);
 const n=Math.hypot(...q);
 for(let j=0;j<3;j++) near(route.bodyLocalPoints[i][j],q[j]/n*(manifest.radius+p[1]),'body-local conversion');
 if(i) {
  const a=route.points[i-1],d=Math.hypot(a[0]-p[0],a[2]-p[2]);
  assert(d>0 && d<=route.options.sampleSpacing+1e-8,'centerline sample spacing');
  assert(Math.abs(a[1]-p[1])<=route.options.maxGrade*d+1e-8,'centerline grade');
 }
}
for(const [i,index] of [[0,0],[1,route.points.length-1]]) {
 const site=manifest.sites[i],p=route.points[index];
 assert.equal(p[0],site.chart.x); assert.equal(p[2],site.chart.z);
 near(p[1],site.h,'site elevation');
 for(let j=0;j<3;j++) near(route.bodyLocalPoints[index][j],site.p[j],'existing site endpoint');
}
const groundBytes=await readFile(new URL('ground.f64',fixture));
const ground=Array.from({length:groundBytes.length/8},(_,i)=>groundBytes.readDoubleLE(i*8));
const web=JSON.parse(await readFile(new URL('../web/route-checkpoint.json',import.meta.url),'utf8'));
assert.deepEqual(web,{...route,manifest,ground},'WebGL2 preview uses the verified native export');
console.log(`PASS frozen route: ${route.siteIds.join(' -> ')}, ${route.points.length} points, explicit widths, body-local coordinates and preview integrity`);
