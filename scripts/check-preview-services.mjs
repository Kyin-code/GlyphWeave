// Verify actual service meshes against Rust entities, not renderer summaries.
// Usage: node scripts/check-preview-services.mjs URL SCREENSHOT.png
import { createRequire } from 'node:module'
import { writeFile } from 'node:fs/promises'
import assert from 'node:assert/strict'
const require=createRequire(import.meta.url)
const {chromium}=require('playwright')
const [url,screenshot,expectation="unfilled"]=process.argv.slice(2)
assert(["unfilled","complete"].includes(expectation))
assert(url&&screenshot)
const browser=await chromium.launch({headless:true,channel:process.env.PREVIEW_BROWSER_CHANNEL||'msedge'})
try {
  const page=await browser.newPage({viewport:{width:1440,height:1000}})
  const errors=[];page.on('pageerror',e=>errors.push(String(e)))
  await page.goto(url)
  await page.waitForFunction(()=>window.glyphweaveFeedback?.visualChecks?.nearSceneRendered,{}, {timeout:90000})
  const proof=await page.evaluate(async()=>{
    const THREE=await import('./vendor/three/build/three.module.js')
    const scene=await fetch('../scenes/'+window.glyphweaveFeedback.sceneId+'/scene.json').then(r=>r.json())
    const sites=scene.roadNetworkPlan.roadsideSites.filter(s=>s.program)
    const found=[];const sourceRoots=[];let maxEnvelopeError=0,maxDoorError=0,invalidChildren=0
    window.__nearScene.traverse(group=>{
      if(sites.some(s=>s.id===group.userData?.sourceEntityId))sourceRoots.push(group.userData.sourceEntityId)
      if(!group.name.startsWith('planned-service:'))return
      const id=group.name.slice('planned-service:'.length),entity=scene.entities.find(e=>e.entityId===id)
      if(!entity)throw new Error('service mesh without source')
      found.push(id)
      if(group.children.length!==5||!group.userData.footprintOnly||group.userData.programKind!==entity.kind)invalidChildren++
      const angle=entity.rotationYDeg*Math.PI/180
      const hw=(entity.rotationYDeg%180?entity.depthM:entity.widthM)/2
      const hd=(entity.rotationYDeg%180?entity.widthM:entity.depthM)/2
      const x=entity.worldX-scene.originX,z=entity.worldZ-scene.originZ
      const box=new THREE.Box3().setFromObject(group)
      maxEnvelopeError=Math.max(maxEnvelopeError,Math.abs(box.min.x-(x-hw)),Math.abs(box.max.x-(x+hw)),Math.abs(box.min.z-(z-hd)),Math.abs(box.max.z-(z+hd)),Math.abs(box.min.y-entity.worldY),Math.abs(box.max.y-entity.worldY-entity.heightM))
      const door=group.getObjectByName('front-door').getWorldPosition(new THREE.Vector3())
      maxDoorError=Math.max(maxDoorError,Math.abs(door.x-(x-Math.sin(angle)*(entity.depthM/2+.002))),Math.abs(door.z-(z-Math.cos(angle)*(entity.depthM/2+.002))))
    })
    return {expected:sites.map(s=>s.id).sort(),found:found.sort(),sourceRoots:sourceRoots.sort(),maxEnvelopeError,maxDoorError,invalidChildren,
      requests:scene.roadNetworkPlan.serviceOutcomes.map(o=>({id:o.request.id,requested:o.request.count,planned:o.plannedSiteIds.length,unfilled:o.unfilledCount}))}
  })
  assert(proof.expected.length>0)
  assert.deepEqual(proof.found,proof.expected)
  assert.deepEqual(proof.sourceRoots,proof.expected,'duplicate coastal/prototype rendering for a planned service')
  assert.equal(proof.invalidChildren,0)
  assert(proof.maxEnvelopeError<.003)
  assert(proof.maxDoorError<1e-5)
  if(expectation==='unfilled') assert(proof.requests.some(r=>r.unfilled>0),'fixture must retain unmet demand')
  else assert(proof.requests.every(r=>r.unfilled===0&&r.planned===r.requested),'complete fixture must meet every facility request')
  assert.deepEqual(errors,[])
  await page.screenshot({path:screenshot})
  // Reproduce the old second-pass fallback in this isolated browser only.
  // The source-root test must detect the duplicates the earlier mesh-only
  // check missed; never edit the baked artifact for this negative control.
  await page.route('**/app.js',async route=>{
    const response=await route.fetch()
    const text=await response.text()
    assert(text.includes('if (plannedBuildingIds.has(entity.entityId)) continue'))
    await route.fulfill({response,body:text.replace('if (plannedBuildingIds.has(entity.entityId)) continue','/* negative control: old duplicate fallback */')})
  })
  await page.reload()
  await page.waitForFunction(()=>window.glyphweaveFeedback?.visualChecks?.nearSceneRendered,{}, {timeout:90000})
  const duplicatedRoots=await page.evaluate(ids=>{let n=0;window.__nearScene.traverse(o=>{if(ids.includes(o.userData?.sourceEntityId))n++});return n},proof.expected)
  assert(duplicatedRoots>proof.expected.length,'negative control must reproduce duplicate service rendering')
  assert.deepEqual(errors,[])
  const result={status:'pass',proof,duplicateRegressionDetected:true,negativeControlRoots:duplicatedRoots,errors,screenshot}
  await writeFile(screenshot+'.json',JSON.stringify(result,null,2))
  console.log(JSON.stringify(result,null,2))
}finally{await browser.close()}
