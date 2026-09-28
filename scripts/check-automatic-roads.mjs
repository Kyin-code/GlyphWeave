// Independent accepted-road, graph, target and millimetre-raster checks.
// CLI replays candidate selection separately; this does not prove global optimality.
import {readFile,writeFile} from 'node:fs/promises'
import {join,dirname} from 'node:path'
import assert from 'node:assert/strict'
const [root,out]=process.argv.slice(2);assert(root&&out)
const json=async p=>JSON.parse((await readFile(p,'utf8')).replace(/^\uFEFF/,''))
const manifest=await json(join(root,'glyphweave.manifest.json')),world=await json(join(root,'world.json'))
const reports=[]
for(const file of world.scenes){
 const s=await json(join(root,file)),plan=s.roadNetworkPlan,a=plan?.automaticExtensions
 if(!a)continue
 const input=manifest.style.automaticRoadExtensions.scenes.find(p=>p.sceneId===s.sceneId)
 assert.deepEqual(a.program,input);assert(a.acceptedSegments<=input.maxNewSegments);assert(a.attemptedCandidates<=input.maxCandidateAttempts)
 assert.equal(a.attemptedCandidates,a.acceptedSegments+Object.values(a.rejectedByReason).reduce((a,b)=>a+b,0))
 const autoIds=new Set(a.generatedRoadIds);assert.equal(autoIds.size,a.acceptedSegments)
 const nodes=new Map(plan.nodes.map(n=>[n.id,n])),entities=new Map(s.entities.map(e=>[e.entityId,e])),edges=new Map(plan.edges.map(e=>[e.id,e]))
 assert.equal(nodes.size,plan.nodes.length);assert.equal(edges.size,plan.edges.length)
 const rect=e=>e.rotationYDeg===0?[e.worldX-e.widthM/2,e.worldZ-4,e.worldX+e.widthM/2,e.worldZ+4]:[e.worldX-4,e.worldZ-e.widthM/2,e.worldX+4,e.worldZ+e.widthM/2]
 const overlap=(a,b)=>a[0]<b[2]&&a[2]>b[0]&&a[1]<b[3]&&a[3]>b[1]
 const dist=(p,r)=>Math.max(0,r[0]-p[0],p[0]-r[2])+Math.max(0,r[1]-p[1],p[1]-r[3])
 const roads=plan.edges.map(e=>entities.get(e.id)),before=plan.edges.filter(e=>!autoIds.has(e.id)).map(e=>entities.get(e.id))
 assert(roads.every(Boolean))
 const zones=s.landUseZones.filter(z=>input.priorityPurposes.includes(z.kind)).sort((a,b)=>a.id.localeCompare(b.id))
 assert.deepEqual([...a.targets.map(t=>t.zoneId)].sort(),zones.map(z=>z.id).sort())
 let unserved=0
 for(const target of a.targets){
   const zone=zones.find(z=>z.id===target.zoneId),bounds=[zone.worldX,zone.worldZ,zone.worldX+zone.widthM,zone.worldZ+zone.depthM],centre=[zone.worldX+Math.floor(zone.widthM/2),zone.worldZ+Math.floor(zone.depthM/2)]
   assert.deepEqual(target.zoneBounds,bounds);assert.deepEqual(target.centre,centre);assert.equal(target.purpose,zone.kind)
   for(const [set,prefix] of [[before,'before'],[roads,'after']]){
     const d=set.length?Math.min(...set.map(e=>dist(centre,rect(e)))):null,has=set.some(e=>overlap(bounds,rect(e)))
     assert.equal(target[prefix+'DistanceM'],d);assert.equal(target[prefix+'HasRoad'],has)
     if(prefix==='after'&&(!has||d===null||d>input.reachDistanceM))unserved++
   }
 }
 assert.equal(unserved,a.unservedTargets)
 if(a.stopReason==='segment_budget_exhausted')assert.equal(a.acceptedSegments,input.maxNewSegments)
 if(a.stopReason==='candidate_budget_exhausted')assert.equal(a.attemptedCandidates,input.maxCandidateAttempts)
 if(a.stopReason==='all_target_zones_entered_and_centres_near_road')assert.equal(unserved,0)
 const folder=join(root,dirname(file)),chunks=new Map(s.chunks.map(c=>[c.chunkX+':'+c.chunkZ,c])),buffers=new Map()
 async function height(x,z,natural){
   const c=chunks.get(Math.floor((x-s.originX)/512)+':'+Math.floor((z-s.originZ)/512));assert(c)
   assert(x>=c.worldX&&x<c.worldX+c.validWidthM&&z>=c.worldZ&&z<c.worldZ+c.validDepthM)
   const f=natural?c.naturalHeightFile:c.heightFile;assert(/^chunk-\d+-\d+\.(natural\.)?height\.bin$/.test(f))
   if(!buffers.has(f)){const b=await readFile(join(folder,f));assert.equal(b.toString('ascii',0,4),'GWHM');assert.equal(b.readUInt32LE(4),1);assert.equal(b.readUInt32LE(8),c.validWidthM);assert.equal(b.readUInt32LE(12),c.validDepthM);assert.equal(b.length,16+4*c.validWidthM*c.validDepthM);buffers.set(f,b)}
   return buffers.get(f).readInt32LE(16+4*((z-c.worldZ)*c.validWidthM+x-c.worldX))
 }
 const protectedZones=s.landUseZones.filter(z=>!['urban','rural'].includes(z.kind))
 function cap(x,z){
   assert(s.landUseZones.some(q=>['urban','rural'].includes(q.kind)&&x>=q.worldX&&x<q.worldX+q.widthM&&z>=q.worldZ&&z<q.worldZ+q.depthM))
   let fill=6000
   for(const q of protectedZones)fill=Math.min(fill,500*Math.max(0,q.worldX-x,x-(q.worldX+q.widthM-1),q.worldZ-z,z-(q.worldZ+q.depthM-1)))
   return fill
 }
 const mmAt=(e,x,z)=>{const p=e.grounding?.roadProfile;if(!p)return Math.round((e.worldY+(e.grounding?.bottomOffsetM??0)+(e.grounding?.roadbedOffsetM??0))*1000);const dx=Math.sign(p.end[0]-p.start[0]),dz=Math.sign(p.end[1]-p.start[1]),i=Math.max(0,Math.min(p.elevationsMm.length-1,dx?(x-p.start[0])*dx:(z-p.start[1])*dz));assert(Number.isInteger(i));return p.elevationsMm[i]}
 const known=plan.edges.filter(e=>!autoIds.has(e.id));const checked=[]
 for(const id of a.generatedRoadIds){
   const ext=plan.extensions.find(e=>e.entityId===id),edge=edges.get(id),entity=entities.get(id);assert(ext&&edge&&entity)
   const d=ext.design,r=d.request,p=entity.grounding.roadProfile,n=p.elevationsMm.length-1,dx=Math.sign(p.end[0]-p.start[0]),dz=Math.sign(p.end[1]-p.start[1])
   assert.deepEqual(p.elevationsMm,d.profileMm);assert.deepEqual(p.start,r.start);assert.deepEqual(p.end,r.end);assert.equal(r.maxFillMm,6000);assert.equal(r.maxRiseMmPerM,80);assert.equal(r.junctionLengthM,8)
   const from=nodes.get(edge.from),to=nodes.get(edge.to);assert.deepEqual([from.worldX,from.worldZ],r.start);assert.deepEqual([to.worldX,to.worldZ],r.end)
   const parents=known.filter(e=>e.from===edge.from||e.to===edge.from);assert(parents.length);assert.deepEqual(parents.map(e=>e.id).sort(),[...ext.parentRoadIds].sort())
   for(const e of parents)assert.equal(mmAt(entities.get(e.id),...r.start),r.startLevelMm)
   const priorEnd=known.filter(e=>e.from===edge.to||e.to===edge.to)
   if(priorEnd.length)for(const e of priorEnd)assert.equal(mmAt(entities.get(e.id),...r.end),r.endLevelMm)
   const rows=[],lower=[],upper=[];let maxFill=0,maxRise=0
   for(let i=0;i<=n;i++){
     const x=r.start[0]+dx*i,z=r.start[1]+dz*i,row=[];let lo=-Infinity,hi=Infinity
     for(let j=-7;j<=7;j++){const xx=x+(dx===0?j:0),zz=z+(dz===0?j:0),h=await height(xx,zz,true),f=await height(xx,zz,false);row.push(h);lo=Math.max(lo,h);hi=Math.min(hi,h+cap(xx,zz));assert.equal(f,p.elevationsMm[i]);assert(f>=h&&f-h<=cap(xx,zz));maxFill=Math.max(maxFill,f-h)}
     rows.push(row);lower.push(i<=8?Math.max(lo,r.startLevelMm):lo);upper.push(i<=8?Math.min(hi,r.startLevelMm):hi)
     if(i>0)maxRise=Math.max(maxRise,Math.abs(p.elevationsMm[i]-p.elevationsMm[i-1]))
   }
   if(!priorEnd.length){
     let endLo=-Infinity,endHi=Infinity
     for(let z=r.end[1]-8;z<=r.end[1]+8;z++)for(let x=r.end[0]-8;x<=r.end[0]+8;x++){const h=await height(x,z,true);endLo=Math.max(endLo,h);endHi=Math.min(endHi,h+cap(x,z))}
     const inferred=Math.max(endLo,...lower.map((h,j)=>h-80*Math.max(0,n-8-j)))
     assert.equal(r.endLevelMm,inferred,'endpoint must be the inferred minimum, not a rounded/manual value');assert(inferred<=endHi)
   }
   for(let i=n-8;i<=n;i++){lower[i]=Math.max(lower[i],r.endLevelMm);upper[i]=Math.min(upper[i],r.endLevelMm)}
   for(let i=0;i<=n;i++){const oracle=Math.max(...lower.map((h,j)=>h-80*Math.abs(i-j)));assert.equal(p.elevationsMm[i],oracle);assert(oracle<=upper[i])}
   let volume=0
   for(let i=0;i<n;i++)for(let j=0;j<14;j++)volume+=(2*p.elevationsMm[i]+2*p.elevationsMm[i+1]-rows[i][j]-rows[i][j+1]-rows[i+1][j]-rows[i+1][j+1])/4000
   assert.equal(volume,d.proposedFillVolumeM3);assert.equal(maxFill,d.maximumFillMm);assert.equal(maxRise,d.maximumRiseMmPerM)
   // Include the applied padded endpoints, not just the design-only mesh.
   for(let i=-4;i<=n+4;i++)for(let j=-7;j<=7;j++){const x=r.start[0]+dx*i+(dx===0?j:0),z=r.start[1]+dz*i+(dz===0?j:0),f=await height(x,z,false),h=await height(x,z,true);assert.equal(f,p.elevationsMm[Math.max(0,Math.min(n,i))]);assert(f>=h&&f-h<=cap(x,z))}
   known.push(edge);checked.push({id,lengthM:n,startMm:r.startLevelMm,endMm:r.endLevelMm,maxFillMm:maxFill,volumeM3:volume,checkedSupportSamples:(n+9)*15})
 }
 // Every actual contact in the road plan must correspond to shared graph nodes.
 let intersections=0
 for(let i=0;i<roads.length;i++)for(let j=i+1;j<roads.length;j++){
   const e=plan.edges[i],f=plan.edges[j],a=rect(roads[i]),b=rect(roads[j]),q=[Math.max(a[0],b[0]),Math.max(a[1],b[1]),Math.min(a[2],b[2]),Math.min(a[3],b[3])]
   if(q[0]>q[2]||q[1]>q[3])continue
   const common=[e.from,e.to].filter(id=>id===f.from||id===f.to).map(id=>nodes.get(id));assert(common.length,'contact without shared graph node')
   for(let z=q[1];z<=q[3];z++)for(let x=q[0];x<=q[2];x++){assert(common.some(n=>Math.abs(x-n.worldX)<=8&&Math.abs(z-n.worldZ)<=8));assert.equal(mmAt(roads[i],x,z),mmAt(roads[j],x,z))}
   intersections++
 }
 reports.push({sceneId:s.sceneId,accepted:a.acceptedSegments,attempted:a.attemptedCandidates,unserved:a.unservedTargets,stop:a.stopReason,checked,roadIntersections:intersections,heightFiles:buffers.size,targets:a.targets.map(t=>({id:t.zoneId,purpose:t.purpose,entered:t.afterHasRoad,distanceM:t.afterDistanceM}))})
}
assert(reports.length)
await writeFile(out,JSON.stringify({status:'pass',scope:'accepted geometry/endpoint inference/graph/target evidence; CLI separately replays choices; not full map completion',scenes:reports},null,2))
console.log(JSON.stringify(reports.map(r=>({sceneId:r.sceneId,accepted:r.accepted,attempted:r.attempted,unserved:r.unserved,stop:r.stop,intersections:r.roadIntersections,heightFiles:r.heightFiles})),null,2))
