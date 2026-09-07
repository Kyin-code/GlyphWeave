// Actual v2 housing meshes and the read-only ownership summary, not pass flags.
// Usage: node scripts/check-preview-joint.mjs URL SCREENSHOT.png
import { createRequire } from 'node:module'
import { writeFile } from 'node:fs/promises'
import assert from 'node:assert/strict'
const require=createRequire(import.meta.url)
const {chromium}=require('playwright')
const [url,screenshot]=process.argv.slice(2)
assert(url&&screenshot)
const browser=await chromium.launch({headless:true,channel:process.env.PREVIEW_BROWSER_CHANNEL||'msedge'})
try {
  const page=await browser.newPage({viewport:{width:1440,height:1000}})
  const errors=[];page.on('pageerror',e=>errors.push(String(e)))
  await page.goto(url)
  await page.waitForFunction(()=>window.glyphweaveFeedback?.visualChecks?.nearSceneRendered,{}, {timeout:90000})
  const proof=await page.evaluate(async()=>{
    const THREE=await import('./vendor/three/build/three.module.js')
    const s=await fetch('../scenes/'+window.glyphweaveFeedback.sceneId+'/scene.json').then(r=>r.json())
    const expected=s.roadNetworkPlan.roadsideSites.filter(s=>!s.program).map(s=>s.id).sort()
    const found=[];let maxEnvelopeError=0,maxDoorError=0
    window.__nearScene.traverse(g=>{
      if(!g.name.startsWith('planned-building:'))return
      const id=g.name.slice('planned-building:'.length),e=s.entities.find(e=>e.entityId===id)
      if(!e)throw new Error('unowned residential mesh')
      found.push(id)
      const x=e.worldX-s.originX,z=e.worldZ-s.originZ,angle=e.rotationYDeg*Math.PI/180
      const hw=(e.rotationYDeg%180?e.depthM:e.widthM)/2,hd=(e.rotationYDeg%180?e.widthM:e.depthM)/2
      const box=new THREE.Box3().setFromObject(g)
      maxEnvelopeError=Math.max(maxEnvelopeError,Math.abs(box.min.x-(x-hw)),Math.abs(box.max.x-(x+hw)),Math.abs(box.min.z-(z-hd)),Math.abs(box.max.z-(z+hd)),Math.abs(box.min.y-e.worldY),Math.abs(box.max.y-e.worldY-e.heightM))
      const door=g.getObjectByName('front-door').getWorldPosition(new THREE.Vector3())
      maxDoorError=Math.max(maxDoorError,Math.abs(door.x-(x-Math.sin(angle)*(e.depthM/2+.002))),Math.abs(door.z-(z-Math.cos(angle)*(e.depthM/2+.002))))
    })
    return {expected,found:found.sort(),maxEnvelopeError,maxDoorError,joint:s.roadNetworkPlan.jointLayout}
  })
  assert(proof.joint)
  assert.equal(proof.joint.residentialMinimum,18)
  assert.equal(proof.expected.length,18)
  assert.deepEqual(proof.found,proof.expected)
  for(const id of proof.joint.replacementResidentialIds)assert(proof.found.includes(id))
  assert(proof.joint.replacementResidentialIds.length>0)
  assert(proof.maxEnvelopeError<.003)
  assert(proof.maxDoorError<1e-5)
  const ownership=new URL(url);ownership.searchParams.set('view','ownership')
  await page.goto(ownership.href)
  await page.waitForFunction(()=>window.glyphweaveFeedback?.visualChecks?.jointLayoutDisplayed)
  assert.deepEqual(errors,[])
  await page.screenshot({path:screenshot})
  const result={status:'pass',proof,errors,screenshot}
  await writeFile(screenshot+'.json',JSON.stringify(result,null,2))
  console.log(JSON.stringify(result,null,2))
}finally{await browser.close()}
