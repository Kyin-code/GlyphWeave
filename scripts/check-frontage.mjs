// Independent full-footprint/station/entrance checks against original payloads.
// Does not prove routing optimality, source replay, or whole-world completion.
import {readFile,writeFile} from 'node:fs/promises'
import {join,dirname} from 'node:path'
import assert from 'node:assert/strict'
const [root,out]=process.argv.slice(2);assert(root&&out)
const json=async p=>JSON.parse((await readFile(p,'utf8')).replace(/^\uFEFF/,''))
const world=await json(join(root,'world.json')),manifest=await json(join(root,'glyphweave.manifest.json')),results=[]
for(const file of world.scenes){
 const s=await json(join(root,file)),plan=s.roadNetworkPlan,report=plan?.frontage;if(!report)continue
 const requests=manifest.style.frontageProgram.requests.filter(r=>r.sceneId===s.sceneId)
 assert.deepEqual(report.outcomes.map(o=>o.request),requests)
 const entities=new Map(s.entities.map(e=>[e.entityId,e])),sites=new Map(plan.roadsideSites.map(s=>[s.id,s]))
 const chunks=new Map(s.chunks.map(c=>[c.chunkX+':'+c.chunkZ,c])),buffers=new Map(),folder=join(root,dirname(file))
 async function mm(x,z,natural=false){
  const c=chunks.get(Math.floor((x-s.originX)/512)+':'+Math.floor((z-s.originZ)/512));assert(c)
  const name=natural?c.naturalHeightFile:c.heightFile;assert(/^chunk-\d+-\d+\.(natural\.)?height\.bin$/.test(name))
  if(!buffers.has(name)){const b=await readFile(join(folder,name));assert.equal(b.toString('ascii',0,4),'GWHM');assert.equal(b.readUInt32LE(4),1);assert.equal(b.readUInt32LE(8),c.validWidthM);assert.equal(b.readUInt32LE(12),c.validDepthM);assert.equal(b.length,16+4*c.validWidthM*c.validDepthM);buffers.set(name,b)}
  return buffers.get(name).readInt32LE(16+4*((z-c.worldZ)*c.validWidthM+x-c.worldX))
 }
 async function height(x,z){const ix=Math.floor(x),iz=Math.floor(z),fx=x-ix,fz=z-iz;return ((await mm(ix,iz))*(1-fx)*(1-fz)+(await mm(ix+1,iz))*fx*(1-fz)+(await mm(ix,iz+1))*(1-fx)*fz+(await mm(ix+1,iz+1))*fx*fz)/1000}
 const verified=[],seen=new Set();let requested=0,unfilled=0,buildingAreaM2=0,supportSamples=0,accessSamples=0
 for(const o of report.outcomes){
  requested+=o.request.count;unfilled+=o.unfilled
  assert.equal(o.request.count,o.siteIds.length+o.unfilled);assert(o.attempted<=o.request.maxCandidateAttempts)
  assert.equal(o.attempted,o.siteIds.length+Object.values(o.rejected).reduce((a,b)=>a+b,0))
  for(const id of o.siteIds){
   assert(!seen.has(id));seen.add(id);const e=entities.get(id),site=sites.get(id);assert(e&&site,id)
   assert.equal(s.entities.filter(e=>e.entityId===id).length,1)
   assert.equal(e.kind,o.request.kind);assert.equal(e.widthM,o.request.widthM);assert.equal(e.depthM,o.request.depthM);assert.equal(e.heightM,o.request.heightM)
   assert.deepEqual([e.worldX,e.worldZ,e.rotationYDeg],[site.worldX,site.worldZ,site.rotationYDeg])
   const level=site.supportLevelMm;assert(Number.isInteger(level));assert.equal(e.worldY,Math.floor(level/1000));assert.equal(Math.round((e.worldY+e.grounding.bottomOffsetM)*1000),level)
   assert.deepEqual(e.grounding.frontageRoadPoint,site.frontageRoadPoint)
   const road=entities.get(site.roadId),p=road?.grounding.roadProfile;assert(p)
   const [rx,rz]=site.frontageRoadPoint;const station=Math.abs(rx-p.start[0])+Math.abs(rz-p.start[1]);assert.equal(p.elevationsMm[station],level)
   assert(p.start[0]===p.end[0]?rx===p.start[0]:rz===p.start[1])
   const hw=(e.rotationYDeg%180?e.depthM:e.widthM)/2,hd=(e.rotationYDeg%180?e.widthM:e.depthM)/2
   const box=[e.worldX-hw-2,e.worldZ-hd-2,e.worldX+hw+3,e.worldZ+hd+3]
   let coverage=0;for(const z of s.landUseZones.filter(z=>z.kind===o.request.purpose)){coverage+=Math.max(0,Math.min(box[2],z.worldX+z.widthM)-Math.max(box[0],z.worldX))*Math.max(0,Math.min(box[3],z.worldZ+z.depthM)-Math.max(box[1],z.worldZ))}
   assert.equal(coverage,(box[2]-box[0])*(box[3]-box[1]),id+' support purpose')
   for(let z=box[1];z<box[3];z++)for(let x=box[0];x<box[2];x++){const n=await mm(x,z,true),f=await mm(x,z);assert.equal(f,level,`${id} support ${x},${z}`);assert(f>=n&&f-n<=plan.maxRoadbedFillM*1000);supportSamples++}
   const links=s.accessGraph.links.filter(l=>l.entityId===id&&l.connected&&l.roadId===site.roadId);assert(links.length>0,id+' real entrance')
   let length=0;for(const pair of links[0].path.slice(1).map((q,i)=>[links[0].path[i],q])){
    const [a,b]=pair,dx=b.x-a.x,dz=b.z-a.z,d=Math.hypot(dx,dz);length+=d;if(d<1e-8)continue;const n=Math.max(1,Math.ceil(d/.5))
    for(const lateral of [-.75,0,.75]){let old;for(let i=0;i<=n;i++){const h=await height(a.x+dx*i/n-dz/d*lateral,a.z+dz*i/n+dx/d*lateral);if(old!==undefined)assert(Math.abs(h-old)<=.25*d/n+1e-6,`${id} access grade`);old=h;accessSamples++}}
   }
   assert(length<=120);assert(Math.abs(length-links[0].distanceM)<1e-6)
   buildingAreaM2+=e.widthM*e.depthM;verified.push({id,kind:e.kind,purpose:o.request.purpose,roadId:site.roadId,station,levelMm:level,areaM2:e.widthM*e.depthM,accessLengthM:length})
  }
 }
 assert(verified.length>0,'empty realization cannot pass this positive evidence check')
 results.push({sceneId:s.sceneId,requested,realized:verified.length,unfilled,buildingAreaM2,supportSamples,accessSamples,heightFiles:buffers.size,verified})
}
assert(results.length>0)
await writeFile(out,JSON.stringify({status:'pass',scope:'actual typed source station, full pad, purpose and raster path slopes; not whole quality gate or independent global route reconstruction',scenes:results},null,2)+'\n')
console.log(JSON.stringify(results.map(({verified,...s})=>s)))
