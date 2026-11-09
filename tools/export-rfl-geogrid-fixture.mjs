import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
import path from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const args=process.argv.slice(2);
let checking=false,rflPath=path.join(root,'../rasd-fastwalker-league-js');
for(let i=0;i<args.length;i++){
 if(args[i]==='--check'){checking=true;continue;}
 if(args[i]==='--rfl'&&args[i+1]&&!args[i+1].startsWith('--')){rflPath=args[++i];continue;}
 throw Error('Usage: node tools/export-rfl-geogrid-fixture.mjs [--check] [--rfl PATH]');
}
const rfl=path.resolve(rflPath);
const source='packages/interiors/src/';
const {generate}=await import(pathToFileURL(path.join(rfl,source,'generator.mjs')));
const {roomVolumes}=await import(pathToFileURL(path.join(rfl,source,'room-shapes.mjs')));
const {minTerrain}=await import(pathToFileURL(path.join(rfl,source,'terrain.mjs')));
const settings={seed:'GARDEN-07',roomDensity:0,atriums:0};
const lines=['preset\tid\tmin_x\tmin_z\tmax_x\tmax_z\tfloor\troof\tcover\tbase_minimum'];
for(const [preset,id] of [['plate','p0'],['orbital','r8']]){
 const layout=generate({...settings,preset});
 const room=layout.rooms.find(room=>room.id===id);
 assert(room&&!room.openToSky&&!room.hullExtension,`${preset} fixture is an enclosed room`);
 for(const [index,volume] of roomVolumes(room).entries()){
  const xs=volume.polygon.map(p=>p[0]),zs=volume.polygon.map(p=>p[1]);
  const minX=Math.min(...xs),minZ=Math.min(...zs),maxX=Math.max(...xs),maxZ=Math.max(...zs);
  const roof=volume.y+volume.h,cover=layout.config.cover;
  const base=minTerrain(layout.terrain,minX,minZ,maxX-minX,maxZ-minZ);
  assert(base>=roof+cover,`${preset} source obeys roof cover`);
  lines.push([preset,`${id}/${index}`,minX,minZ,maxX,maxZ,volume.y,roof,cover,base].join('\t'));
 }
}
const data=lines.join('\n')+'\n',directory=path.join(root,'fixtures/rfl-structures');
if(checking){
 assert.equal(data,await readFile(path.join(directory,'volumes.tsv'),'utf8'),'current RFL generated room volumes reproduce the frozen fixture');
 console.log('PASS RFL Plate and Orbital room-volume fixture reproduces');
}else{
 await mkdir(directory,{recursive:true});
 await writeFile(path.join(directory,'volumes.tsv'),data);
 const sourceHashes={};
 for(const file of ['generator.mjs','plate.mjs','terrain.mjs','room-shapes.mjs','plate-landscape.mjs'])sourceHashes[source+file]=createHash('sha256').update(await readFile(path.join(rfl,source,file))).digest('hex');
 const provenance={sourceRepository:'https://github.com/KO0I/rasd-fastwalker-league-js',sourceCommit:execFileSync('git',['rev-parse','HEAD'],{cwd:rfl,encoding:'utf8'}).trim(),sourceHashes,settings,units:'metres',axes:'local x/z footprint, y up',footprints:'conservative bounds of each roomVolumes prism',base:'minimum source terrain over each prism bounding rectangle',sha256:createHash('sha256').update(data).digest('hex')};
 await writeFile(path.join(directory,'provenance.json'),JSON.stringify(provenance,null,2)+'\n');
 console.log(`Exported ${lines.length-1} real RFL enclosed prisms`);
}
