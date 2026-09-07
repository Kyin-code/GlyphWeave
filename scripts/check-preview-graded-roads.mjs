// Compare live graded road geometry with Rust profiles AND final/natural binary
// samples. No full scene.json request is needed, even for maximum worlds.
import {chromium} from 'playwright'
import {writeFile} from 'node:fs/promises'
import assert from 'node:assert/strict'
const [url,output,focusX,focusZ]=process.argv.slice(2);assert(url&&output)
const browser=await chromium.launch({headless:true,channel:process.env.PREVIEW_BROWSER_CHANNEL||'msedge'})
try {
  const page=await browser.newPage({viewport:{width:1440,height:1000}}),errors=[]
  page.on('pageerror',e=>errors.push(String(e)))
  await page.goto(url);await page.waitForFunction(()=>window.glyphweaveFeedback?.visualChecks?.nearSceneRendered,{}, {timeout:120000})
  if(focusX!==undefined||focusZ!==undefined){
    assert(Number.isFinite(Number(focusX))&&Number.isFinite(Number(focusZ)))
    await page.evaluate(([x,z])=>window.__focusAt(x,z),[Number(focusX),Number(focusZ)])
    await page.waitForFunction(key=>window.glyphweaveFeedback?.visualChecks?.nearSceneRendered&&window.glyphweaveFeedback.visualChecks.highDetailChunk===key,Math.floor(Number(focusX)/512)+','+Math.floor(Number(focusZ)/512),{timeout:120000})
  }
  const proof=await page.evaluate(async()=>{
    const THREE=await import('/preview/vendor/three/build/three.module.js'),id=window.glyphweaveFeedback.sceneId
    const base='../scenes/'+id+'/',scene=await fetch(base+'preview-scene.json').then(r=>r.json()),chunks=new Map(scene.chunks.map(c=>[c.chunkX+':'+c.chunkZ,c])),files=new Map()
    const active=new Set(window.glyphweaveFeedback.visualChecks.loadedChunks)
    const expected=scene.entities.filter(e=>e.grounding?.roadProfile&&scene.chunks.some(c=>active.has(c.chunkX+','+c.chunkZ)&&e.bounds.minX<=c.worldX+c.validWidthM&&e.bounds.maxX>=c.worldX&&e.bounds.minZ<=c.worldZ+c.validDepthM&&e.bounds.maxZ>=c.worldZ))
    async function sample(x,z,natural=false){
      const key=Math.floor((x-scene.originX)/512)+':'+Math.floor((z-scene.originZ)/512),c=chunks.get(key);if(!c)throw Error('missing terrain chunk')
      const f=natural?c.naturalHeightFile:c.heightFile
      if(!files.has(f)){const response=await fetch(base+f);if(!response.ok)throw Error('missing height file');const b=await response.arrayBuffer(),v=new DataView(b);if(String.fromCharCode(...new Uint8Array(b,0,4))!=='GWHM'||v.getUint32(4,true)!==1||v.getUint32(8,true)!==c.validWidthM||v.getUint32(12,true)!==c.validDepthM)throw Error('height contract mismatch');files.set(f,v)}
      return files.get(f).getInt32(16+((z-c.worldZ)*c.validWidthM+x-c.worldX)*4,true)
    }
    window.__nearScene.updateMatrixWorld(true)
    const rendered=new Map();window.__nearScene.traverse(o=>{if(o.name.startsWith('graded-road:')){const id=o.name.slice('graded-road:'.length);if(rendered.has(id))throw Error('duplicate graded road');rendered.set(id,o)}})
    const roads=[],mismatches=[]
    for(const e of expected){
      const mesh=rendered.get(e.entityId),p=e.grounding.roadProfile;if(!mesh){mismatches.push('missing '+e.entityId);continue}
      const pos=mesh.geometry.getAttribute('position'),dx=Math.sign(p.end[0]-p.start[0]),dz=Math.sign(p.end[1]-p.start[1]),n=p.elevationsMm.length-1
      if(pos.count!==2*(n+1)||mesh.geometry.index.count!==n*6)throw Error('wrong road geometry count')
      let checked=0,maxFillMm=0
      for(let i=0;i<=n;i++){
        for(const [k,t] of [-4,4].entries()){
          const v=new THREE.Vector3().fromBufferAttribute(pos,i*2+k).applyMatrix4(mesh.matrixWorld)
          const x=p.start[0]+dx*i+(dx===0?t:0)-scene.originX,z=p.start[1]+dz*i+(dz===0?t:0)-scene.originZ,y=p.elevationsMm[i]/1000
          if(Math.abs(v.x-x)>1e-5||Math.abs(v.z-z)>1e-5||Math.abs(v.y-y)>0.00003)mismatches.push({id:e.entityId,station:i,vertex:k,expected:[x,y,z],actual:v.toArray()})
        }
        for(let t=-7;t<=7;t++){
          const x=p.start[0]+dx*i+(dx===0?t:0),z=p.start[1]+dz*i+(dz===0?t:0),f=await sample(x,z),natural=await sample(x,z,true)
          if(f!==p.elevationsMm[i]||f<natural||f-natural>p.maxFillMm)mismatches.push({id:e.entityId,station:i,lateral:t,finalMm:f,naturalMm:natural,expectedMm:p.elevationsMm[i]})
          maxFillMm=Math.max(maxFillMm,f-natural);checked++
        }
      }
      roads.push({id:e.entityId,stations:n+1,vertices:pos.count,triangles:mesh.geometry.index.count/3,checkedSupportSamples:checked,maxFillMm,startMm:p.elevationsMm[0],endMm:p.elevationsMm[n]})
    }
    return {roads,mismatches,expected:expected.map(e=>e.entityId).sort(),rendered:[...rendered.keys()].sort(),heightFiles:files.size,feedback:window.glyphweaveFeedback}
  })
  await writeFile(output+'.json',JSON.stringify({status:'checking',...proof,errors},null,2))
  assert(proof.roads.length>0);assert.deepEqual(proof.expected,proof.rendered);assert.deepEqual(proof.mismatches,[]);assert.deepEqual(errors,[])
  await page.screenshot({path:output})
  await writeFile(output+'.json',JSON.stringify({status:'pass',...proof,errors},null,2));console.log(JSON.stringify({status:'pass',roads:proof.roads,errors},null,2))
} finally {await browser.close()}
