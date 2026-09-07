// Independent integer-raster/profile/mesh check, plus a read-only HTML study.
// Does not apply engineering or award any realized map area.
import { readFile,writeFile } from 'node:fs/promises'
import { resolve,dirname,join } from 'node:path'
import assert from 'node:assert/strict'
const [worldDir,requestPath,designPath,reportPath,htmlPath]=process.argv.slice(2)
assert(worldDir&&requestPath&&designPath&&reportPath)
const json=async p=>JSON.parse((await readFile(p,'utf8')).replace(/^\uFEFF/,''))
const request={startLevelMm:null,endLevelMm:null,...await json(requestPath)},d=await json(designPath),world=await json(join(worldDir,'world.json'))
assert.equal(d.status,'design_only_not_applied');assert.deepEqual(d.request,request)
let scene,base
for(const p of world.scenes){const s=await json(join(worldDir,p));if(s.sceneId===request.sceneId){assert(!scene);scene=s;base=dirname(resolve(worldDir,p))}}
assert(scene);assert.equal(d.naturalHeightfieldHash,scene.naturalTerrain.heightfieldHash)
const chunks=new Map(scene.chunks.map(c=>[c.chunkX+':'+c.chunkZ,c])),cache=new Map()
async function height(x,z){
  assert(x>=scene.originX&&z>=scene.originZ&&x<scene.originX+scene.widthM&&z<scene.originZ+scene.depthM)
  const key=Math.floor((x-scene.originX)/512)+':'+Math.floor((z-scene.originZ)/512),c=chunks.get(key);assert(c)
  if(!cache.has(key)){
    const b=await readFile(join(base,c.naturalHeightFile)),n=c.validWidthM*c.validDepthM
    const mm=b.length===16+n*4
    if(mm){assert.equal(b.toString('ascii',0,4),'GWHM');assert.equal(b.readUInt32LE(4),1);assert.equal(b.readUInt32LE(8),c.validWidthM);assert.equal(b.readUInt32LE(12),c.validDepthM)}else assert.equal(b.length,n*2)
    cache.set(key,{b,mm})
  }
  const {b,mm}=cache.get(key),i=(z-c.worldZ)*c.validWidthM+x-c.worldX
  return mm?b.readInt32LE(16+i*4):b.readInt16LE(i*2)*250
}
const contains=(q,x,z)=>x>=q.worldX&&x<q.worldX+q.widthM&&z>=q.worldZ&&z<q.worldZ+q.depthM
const allowed=scene.landUseZones.filter(q=>['urban','rural'].includes(q.kind)),protectedZones=scene.landUseZones.filter(q=>!['urban','rural'].includes(q.kind))
const [x0,z0]=request.start,[x1,z1]=request.end,dx=Math.sign(x1-x0),dz=Math.sign(z1-z0),n=Math.abs(x1-x0)+Math.abs(z1-z0),half=request.roadWidthM/2+request.shoulderWidthM
assert((dx===0)!==(dz===0));assert(request.maxFillMm<=6000&&request.maxRiseMmPerM>0&&request.maxRiseMmPerM<=80)
const rows=[],lower=[],upper=[],points=[]
for(let i=0;i<=n;i++){
  const x=x0+dx*i,z=z0+dz*i,row=[];let lo=-Infinity,hi=Infinity
  for(let t=-half;t<=half;t++){
    const xx=x+(dx===0?t:0),zz=z+(dz===0?t:0)
    assert.equal(allowed.filter(q=>contains(q,xx,zz)).length,1)
    assert(!protectedZones.some(q=>contains(q,xx,zz)))
    let cap=request.maxFillMm
    for(const q of protectedZones){const distance=Math.max(0,q.worldX-xx,xx-(q.worldX+q.widthM-1),q.worldZ-zz,zz-(q.worldZ+q.depthM-1));cap=Math.min(cap,distance*500)}
    const h=await height(xx,zz);row.push(h);lo=Math.max(lo,h);hi=Math.min(hi,h+cap)
  }
  if(request.startLevelMm!==null&&i<=request.junctionLengthM){lo=Math.max(lo,request.startLevelMm);hi=Math.min(hi,request.startLevelMm)}
  if(request.endLevelMm!==null&&n-i<=request.junctionLengthM){lo=Math.max(lo,request.endLevelMm);hi=Math.min(hi,request.endLevelMm)}
  rows.push(row);lower.push(lo);upper.push(hi);points.push([x,z])
}
assert.equal(d.profileMm.length,n+1)
// Deliberately different O(N²) oracle from the Rust O(N) two-sweep solver.
for(let i=0;i<=n;i++){
  const expected=lower.reduce((best,h,j)=>Math.max(best,h-request.maxRiseMmPerM*Math.abs(i-j)),-Infinity)
  assert.equal(d.profileMm[i],expected);assert(expected<=upper[i])
}
for(const [mesh,width] of [[d.roadMesh,request.roadWidthM],[d.supportMesh,half*2]]){
  assert.equal(mesh.vertices.length,2*(n+1));assert.equal(mesh.triangles.length,2*n)
  for(let i=0;i<=n;i++)for(const [k,t] of [-width/2,width/2].entries()){
    assert.deepEqual(mesh.vertices[2*i+k],[points[i][0]+(dx===0?t:0),d.profileMm[i]/1000,points[i][1]+(dz===0?t:0)])
  }
  let area=0
  for(let i=0;i<n;i++){
    const indices=mesh.triangles.slice(i*2,i*2+2)
    assert.deepEqual(indices.map(t=>[...t].sort((a,b)=>a-b)),[[i*2,i*2+1,i*2+2],[i*2+1,i*2+2,i*2+3]])
    assert.deepEqual([...new Set(indices.flat())].sort((a,b)=>a-b),[i*2,i*2+1,i*2+2,i*2+3])
    for(const tri of indices){const [a,b,c]=tri.map(j=>mesh.vertices[j]);const twice=(b[2]-a[2])*(c[0]-a[0])-(b[0]-a[0])*(c[2]-a[2]);assert(twice>0);area+=twice/2}
  }
  assert.equal(area,n*width)
}
let volume=0,maxFill=0,maxRise=0,maxFillStation=0
for(let i=0;i<=n;i++)for(const h of rows[i]){const fill=d.profileMm[i]-h;assert(fill>=0);if(fill>maxFill){maxFill=fill;maxFillStation=i}}
for(let i=0;i<n;i++){
  maxRise=Math.max(maxRise,Math.abs(d.profileMm[i+1]-d.profileMm[i]))
  for(let j=0;j<rows[i].length-1;j++)volume+=(2*d.profileMm[i]+2*d.profileMm[i+1]-rows[i][j]-rows[i][j+1]-rows[i+1][j]-rows[i+1][j+1])/4000
}
assert(Math.abs(volume-d.proposedFillVolumeM3)<1e-6);assert.equal(maxFill,d.maximumFillMm);assert.equal(maxRise,d.maximumRiseMmPerM);assert.equal(d.supportAreaM2,n*half*2)
const allMin=Math.min(...rows.map(row=>Math.min(...row))),allMax=Math.max(...rows.map(row=>Math.max(...row)))
const report={status:'pass',scope:'natural-stage engineering design only, not applied road/terrain/ownership',sourceScene:scene.sceneId,sourceHash:d.naturalHeightfieldHash,lengthM:n,widthM:request.roadWidthM,supportWidthM:half*2,samples:(n+1)*(half*2+1),naturalMinM:allMin/1000,naturalMaxM:allMax/1000,minimumIntegerFlatPlatformFillM:(Math.ceil(allMax/1000)*1000-allMin)/1000,maximumFillM:maxFill/1000,maximumRiseMmPerM:maxRise,proposedFillVolumeM3:volume,supportAreaM2:d.supportAreaM2,trianglesPerMesh:2*n,maxFillStation,decodedChunks:cache.size,html:htmlPath??null}
await writeFile(reportPath,JSON.stringify(report,null,2))
if(htmlPath){
  const escape=value=>String(value).replace(/[&<>"]/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}[c]))
  const W=1140,H=330,pad=55,min=allMin/1000-1,max=Math.max(...d.profileMm)/1000+1
  const X=i=>pad+i/n*(W-pad*2),Y=h=>H-pad-(h-min)/(max-min)*(H-pad*2)
  const line=arr=>arr.map((h,i)=>(i?'L':'M')+X(i).toFixed(2)+','+Y(h).toFixed(2)).join(' ')
  const design=d.profileMm.map(h=>h/1000),groundLow=rows.map(r=>Math.min(...r)/1000),groundHigh=rows.map(r=>Math.max(...r)/1000)
  const fillPath=line(design)+' '+groundLow.map((h,i)=>'L'+X(n-i).toFixed(2)+','+Y(groundLow[n-i]).toFixed(2)).join(' ')+' Z'
  const section=rows[maxFillStation],roadY=d.profileMm[maxFillStation]/1000,secMin=Math.min(...section)/1000-0.5,secMax=roadY+0.5
  const SX=i=>55+i/(section.length-1)*1030,SY=h=>240-(h-secMin)/(secMax-secMin)*190
  const sectionPath=section.map((h,i)=>(i?'L':'M')+SX(i)+','+SY(h/1000)).join(' ')
  const ticks=Array.from({length:6},(_,i)=>{const h=min+(max-min)*i/5;return `<line x1="55" x2="1085" y1="${Y(h)}" y2="${Y(h)}" stroke="#29404a"/><text x="6" y="${Y(h)+4}">${h.toFixed(1)}m</text>`}).join('')
  await writeFile(htmlPath,`<!doctype html><html lang="zh-CN"><meta charset="utf-8"><title>道路纵断面研究 · 未应用</title><style>body{margin:0;background:#101d23;color:#e2eee9;font:16px/1.7 system-ui}main{max-width:1140px;margin:auto;padding:40px 24px}h1{font-size:32px;margin-bottom:8px}h2{margin-top:32px}p{color:#adc1c6}strong{color:#ffd388}.grid{display:grid;grid-template-columns:repeat(4,1fr);gap:14px}.card{padding:16px;background:#1a3039;border:1px solid #35505b;border-radius:10px}.card b{font-size:24px;display:block;color:#81dcc3}.card small{color:#aec3c8}svg{width:100%;background:#152831;border:1px solid #35505b;border-radius:10px}text{fill:#aec3c8;font:12px system-ui}.caption{font-size:14px}</style><main><p>GLYPHWEAVE / RUST ENGINEERING STUDY / R45</p><h1>让道路纵断面跟随自然起伏</h1><p><strong>设计方案，尚未应用到地图。</strong>固定水平走廊，在 6m 填土上限、8% 纵坡与保护域限制内求解。没有删减用地需求，也没有给地图新增虚构面积。</p><div class="grid"><div class="card"><b>${n} m</b><small>走廊长度 / 路面 ${request.roadWidthM}m</small></div><div class="card"><b>${(maxRise/10).toFixed(1)}%</b><small>最大纵坡</small></div><div class="card"><b>${(maxFill/1000).toFixed(3)} m</b><small>最大设计填土</small></div><div class="card"><b>${volume.toFixed(1)} m³</b><small>走廊内预计填土量</small></div></div><h2>自然地形与道路纵断面</h2><p class="caption">青线：Rust 路面标高；绿色：全宽自然地形上包络；灰线：下包络；阴影：标高差示意，不是已施工区域。</p><svg viewBox="0 0 ${W} ${H}" role="img" aria-label="自然地形和设计道路纵断面对照">${ticks}<path d="${fillPath}" fill="#4b7879" opacity=".3"/><path d="${line(groundLow)}" fill="none" stroke="#859899"/><path d="${line(groundHigh)}" fill="none" stroke="#8fab66" stroke-width="2"/><path d="${line(design)}" fill="none" stroke="#69e6d0" stroke-width="2"/><text x="55" y="315">起点 ${request.start.join(', ')}</text><text x="860" y="315">终点 ${request.end.join(', ')}</text></svg><h2>最大填土处的全宽横断面 · ${maxFillStation}m</h2><svg viewBox="0 0 1140 280" role="img" aria-label="最大填土处横断面"><path d="${sectionPath}" fill="none" stroke="#a1b971" stroke-width="3"/><line x1="55" x2="1085" y1="${SY(roadY)}" y2="${SY(roadY)}" stroke="#839795" stroke-width="4"/><line x1="${SX(request.shoulderWidthM)}" x2="${SX(section.length-1-request.shoulderWidthM)}" y1="${SY(roadY)}" y2="${SY(roadY)}" stroke="#69e6d0" stroke-width="6"/><text x="55" y="265">自然地面 / 道路及路肩支撑宽 ${half*2}m</text><text x="750" y="265">道路设计高程 ${roadY.toFixed(3)}m</text></svg><h2>为何不能继续要求整张路网等高</h2><p>这条走廊自然高程为 ${(allMin/1000).toFixed(3)}–${(allMax/1000).toFixed(3)}m。原整数等高平台至少需要 ${report.minimumIntegerFlatPlatformFillM.toFixed(3)}m 填土；当前纵断面最大为 ${(maxFill/1000).toFixed(3)}m。两者都以同一份自然栅格计算。</p><p>下一步必须把纵断面接入共享路口、连通路网、最终高度场与入口通行审计。横断面外边坡、排水、土源与压实、竖曲线舒适性尚未实现；这里的体积只覆盖明确的路面/路肩支撑走廊。</p><p class="caption">数据：${escape(scene.sceneId)} · ${report.samples.toLocaleString('en-US')} 个原始整数毫米样本 · 每个 mesh ${2*n} 个三角形 · 独立 Node 全宽复核通过</p></main></html>`)
}
console.log(JSON.stringify(report,null,2))
