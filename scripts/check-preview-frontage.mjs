// Check rendered frontage meshes against compact Rust authority, without full scene requests.
import {chromium} from 'playwright'
import {writeFile} from 'node:fs/promises'
import assert from 'node:assert/strict'
const [url,output]=process.argv.slice(2);assert(url&&output)
const browser=await chromium.launch({headless:true,channel:process.env.PREVIEW_BROWSER_CHANNEL||'msedge'})
try {
 const page=await browser.newPage({viewport:{width:1440,height:1000}}),errors=[],fullSceneRequests=[]
 page.on('pageerror',e=>errors.push(String(e)))
 page.on('request',r=>{if(/\/scenes\/[^/]+\/scene\.json(?:\?|$)/.test(r.url()))fullSceneRequests.push(r.url())})
 await page.goto(url);await page.waitForFunction(()=>window.glyphweaveFeedback?.visualChecks?.nearSceneRendered,{}, {timeout:120000})
 const proof=await page.evaluate(async()=>{
  const THREE=await import('/preview/vendor/three/build/three.module.js'),id=window.glyphweaveFeedback.sceneId
  const s=await fetch('../scenes/'+id+'/preview-scene.json').then(r=>r.json()),loaded=new Set(window.glyphweaveFeedback.visualChecks.loadedChunks)
  const expected=s.entities.filter(e=>e.grounding?.frontageRoadPoint&&loaded.has(Math.floor((e.worldX-s.originX)/512)+','+Math.floor((e.worldZ-s.originZ)/512)))
  const roots=new Map();window.__nearScene.traverse(g=>{if(expected.some(e=>e.entityId===g.userData?.sourceEntityId)){if(roots.has(g.userData.sourceEntityId))throw Error('duplicate frontage mesh');roots.set(g.userData.sourceEntityId,g)}})
  const checks=[]
  for(const e of expected){
   const g=roots.get(e.entityId);if(!g)throw Error('missing actual frontage mesh '+e.entityId)
   const b=new THREE.Box3().setFromObject(g),base=e.worldY+(e.grounding.bottomOffsetM??0)+(e.grounding.roadbedOffsetM??0)
   const hw=(e.rotationYDeg%180?e.depthM:e.widthM)/2,hd=(e.rotationYDeg%180?e.widthM:e.depthM)/2
   const target=[e.worldX-s.originX-hw,base,e.worldZ-s.originZ-hd,e.worldX-s.originX+hw,base+e.heightM,e.worldZ-s.originZ+hd]
   const actual=[b.min.x,b.min.y,b.min.z,b.max.x,b.max.y,b.max.z],error=Math.max(...actual.map((n,i)=>Math.abs(n-target[i])))
   if(error>.0031)throw Error(e.entityId+' rendered envelope error '+error)
   const heightError=Math.max(Math.abs(actual[1]-target[1]),Math.abs(actual[4]-target[4]))
   if(heightError>.00005)throw Error(e.entityId+' rendered height error '+heightError)
   const door=g.getObjectByName('front-door');if(!door)throw Error('missing frontage door');const pos=door.getWorldPosition(new THREE.Vector3()),a=e.anchors[0]
   const ex=a.worldX+(a.direction==='east'?-1:a.direction==='west'?1:0)-s.originX,ez=a.worldZ+(a.direction==='south'?-1:a.direction==='north'?1:0)-s.originZ
   if(Math.hypot(pos.x-ex,pos.z-ez)>.0031)throw Error('door faces away from authored entrance')
   checks.push({id:e.entityId,kind:e.kind,levelMm:Math.round(base*1000),envelopeErrorM:error,heightErrorM:heightError,door:[pos.x,pos.y,pos.z]})
  }
  let detectsOneMillimetreShift=false
  if(expected.length){const e=expected[0],g=roots.get(e.entityId),y=g.position.y
   g.position.y+=.001;g.updateMatrixWorld(true);const b=new THREE.Box3().setFromObject(g),base=e.worldY+(e.grounding.bottomOffsetM??0)+(e.grounding.roadbedOffsetM??0)
   detectsOneMillimetreShift=Math.abs(b.min.y-base)>.00005
   g.position.y=y;g.updateMatrixWorld(true)
  }
  if(!detectsOneMillimetreShift)throw Error('height checker did not detect a 1mm mesh shift')
  return {checks,detectsOneMillimetreShift,expected:expected.map(e=>e.entityId).sort(),rendered:[...roots.keys()].sort(),feedback:window.glyphweaveFeedback}
 })
 await writeFile(output+'.json',JSON.stringify({status:'checking',...proof,errors,fullSceneRequests},null,2))
 assert(proof.checks.length>0);assert.deepEqual(proof.expected,proof.rendered);assert.deepEqual(errors,[]);assert.deepEqual(fullSceneRequests,[])
 await page.screenshot({path:output});await writeFile(output+'.json',JSON.stringify({status:'pass',scope:'active-chunk actual frontage meshes and doors; not whole-world visual acceptance',...proof,errors,fullSceneRequests},null,2))
 console.log(JSON.stringify({status:'pass',checked:proof.checks.length,errors}))
} finally {await browser.close()}
