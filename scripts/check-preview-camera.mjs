// Runtime regression check; starts an isolated browser, never a user session.
// Usage: node scripts/check-preview-camera.mjs URL OUTPUT.png
import { createRequire } from 'node:module'
import { writeFile } from 'node:fs/promises'
import assert from 'node:assert/strict'
const require = createRequire(import.meta.url)
const { chromium } = require('playwright')
const [url, output] = process.argv.slice(2)
assert(url && output, 'requires preview URL and screenshot output path')
const browser = await chromium.launch({ headless: true, channel: process.env.PREVIEW_BROWSER_CHANNEL || 'msedge' })
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } })
  const errors = []
  page.on('pageerror', error => errors.push(String(error)))
  const response = await page.goto(url)
  assert.equal(response.status(), 200)
  const ownership = new URL(url).searchParams.get('view') === 'ownership'
  let alignment = null
  if (ownership) {
    await page.waitForFunction(() => window.glyphweaveFeedback?.visualChecks?.ownershipRendered, {}, { timeout: 90000 })
    const state = await page.evaluate(() => window.glyphweaveFeedback)
    assert(state.visualChecks.ownershipCells > 0, 'missing ownership cells')
    assert.equal(await page.locator('canvas').count(), 1, 'ownership view must not invent 3D geometry')
  } else {
    await page.waitForFunction(() => window.glyphweaveFeedback?.visualChecks?.nearSceneRendered, {}, { timeout: 90000 })
    alignment = await page.evaluate(() => {
      const camera = window.__nearCamera
      const target = window.__nearControls.target
      const direction = camera.getWorldDirection(camera.position.clone())
      return direction.dot(target.clone().sub(camera.position).normalize())
    })
    assert(alignment > .9999, `camera is not aimed at configured target: ${alignment}`)
    const heightProof=await page.evaluate(async()=>{
      const world=await fetch('../world.json').then(r=>r.json())
      const scene=await fetch('../scenes/'+window.glyphweaveFeedback.sceneId+'/scene.json').then(r=>r.json())
      const {decodeHeightPayload}=await import('./core/height.js')
      let meshes=0,samples=0,nonQuarter=0,maxError=0,maxPositionResidue=0
      const encodings=new Set()
      const objects=[];window.__nearScene.traverse(o=>{if(o.isMesh&&o.name.startsWith('terrain:'))objects.push(o)})
      for(const mesh of objects){
        const c=scene.chunks.find(c=>c.chunkX===mesh.userData.chunkX&&c.chunkZ===mesh.userData.chunkZ)
        const [bytes,surface]=await Promise.all([fetch('../scenes/'+scene.sceneId+'/'+c.heightFile).then(r=>r.arrayBuffer()),fetch('../scenes/'+scene.sceneId+'/'+c.surfaceFile).then(r=>r.arrayBuffer()).then(b=>new Uint8Array(b))])
        const p=decodeHeightPayload(bytes,c.validWidthM,c.validDepthM),pos=mesh.geometry.getAttribute('position')
        encodings.add(p.encoding);meshes++
        for(let i=0;i<pos.count;i++){
          const x=pos.getX(i)+c.validWidthM/2,z=pos.getZ(i)+c.validDepthM/2
          maxPositionResidue=Math.max(maxPositionResidue,Math.abs(x-Math.round(x)),Math.abs(z-Math.round(z)))
          const ix=Math.min(c.validWidthM-1,Math.max(0,Math.round(x))),iz=Math.min(c.validDepthM-1,Math.max(0,Math.round(z))),index=iz*c.validWidthM+ix
          if(surface[index]===3)continue
          const expected=p.readMetres(index)
          maxError=Math.max(maxError,Math.abs(pos.getY(i)-expected));samples++
          if(Math.abs(expected*4-Math.round(expected*4))>.0001)nonQuarter++
        }
      }
      return {meshes,samples,nonQuarter,maxError,maxPositionResidue,encodings:[...encodings],declared:world.sidecar.heightEncoding}
    })
    assert(heightProof.meshes>0&&heightProof.samples>0,'no real terrain geometry inspected')
    assert(heightProof.maxError<.0001,'actual rendered height differs from authoritative payload')
    assert(heightProof.maxPositionResidue<1e-6,'terrain vertices stretch integer-metre grid')
    if(heightProof.declared==='gwhm-v1-i32le-millimeter'){
      assert.deepEqual(heightProof.encodings,['millimeter'])
      assert(heightProof.nonQuarter>0,'precision data was replaced with rescaled legacy values')
    }
    console.log(JSON.stringify({heightRenderContract:heightProof}))
    if(new URL(url).searchParams.get('view')==='farm') {
      const proof=await page.evaluate(async()=>{
        const data=await fetch('../scenes/'+window.glyphweaveFeedback.sceneId+'/scene.json').then(r=>r.json())
        let prepared=0,tracks=0,maxVertexError=0
        window.__nearScene.traverse(o=>{
          if(!o.isMesh||!o.name.startsWith('landcover:'))return
          if(o.userData.landcoverMaterial==='cultivated_soil')prepared++
          if(o.userData.landcoverMaterial==='farm_track')tracks++
          const p=data.landcoverSurfaces.patches.find(p=>'landcover:'+p.id===o.name)
          const actual=o.geometry.getAttribute('position').array,expected=p.vertices.flat()
          if(actual.length!==expected.length)maxVertexError=Infinity
          for(let i=0;i<actual.length;i++)maxVertexError=Math.max(maxVertexError,Math.abs(actual[i]-expected[i]))
        })
        return {fields:data.landcoverSurfaces.agriculture.fields.length,prepared,tracks,maxVertexError,legacyFarmBoxes:data.entities.filter(e=>['farmland','pasture'].includes(e.kind)).length}
      })
      assert(proof.fields>0&&proof.prepared>0&&proof.tracks>0,'no real agriculture/access surfaces')
      assert.equal(proof.legacyFarmBoxes,0,'legacy farm boxes survived road-first migration')
      assert(proof.maxVertexError<.0001,'renderer changed authoritative field vertices')
      console.log(JSON.stringify({farmRenderContract:proof}))
    }
    if(new URL(url).searchParams.get('view')==='yard') {
      const state=await page.evaluate(()=>window.glyphweaveFeedback)
      assert(state.visualChecks.landcoverGrassPatches>0,'missing actual Rust yard meshes')
      const meshCount=await page.evaluate(()=>{let n=0;window.__nearScene.traverse(o=>{if(o.isMesh&&o.userData.landcoverMaterial==='managed_grass')n++});return n})
      assert.equal(meshCount,state.visualChecks.landcoverGrassPatches)
      const contract=await page.evaluate(async()=>{
        const THREE=await import('./vendor/three/build/three.module.js')
        const data=await fetch('../scenes/'+window.glyphweaveFeedback.sceneId+'/scene.json').then(r=>r.json())
        let houses=0,wrongChildren=0,maxDoorError=0,maxFootprintExcess=0,maxVertexError=0
        window.__nearScene.updateMatrixWorld(true)
        window.__nearScene.traverse(o=>{
          if(o.name?.startsWith('planned-building:')) {
            const id=o.name.slice('planned-building:'.length);const e=data.entities.find(e=>e.entityId===id)
            if(!e)return
            houses+=1;if(o.children.length!==5||!o.userData.footprintOnly)wrongChildren+=1
            const door=o.getObjectByName('front-door').getWorldPosition(new THREE.Vector3())
            const angle=e.rotationYDeg*Math.PI/180
            const x=e.worldX-data.originX,z=e.worldZ-data.originZ
            maxDoorError=Math.max(maxDoorError,Math.abs(door.x-(x-Math.sin(angle)*(e.depthM/2+.002))),Math.abs(door.z-(z-Math.cos(angle)*(e.depthM/2+.002))))
            const box=new THREE.Box3().setFromObject(o)
            maxFootprintExcess=Math.max(maxFootprintExcess,Math.abs(box.min.x-x)-e.widthM/2,Math.abs(box.max.x-x)-e.widthM/2,Math.abs(box.min.z-z)-e.depthM/2,Math.abs(box.max.z-z)-e.depthM/2,box.max.y-e.worldY-e.heightM,e.worldY-box.min.y)
          }
          if(o.isMesh&&o.name.startsWith('landcover:')) {
            const patch=data.landcoverSurfaces.patches.find(p=>'landcover:'+p.id===o.name)
            const positions=o.geometry.getAttribute('position').array
            const expected=patch.vertices.flat()
            if(positions.length!==expected.length)maxVertexError=Infinity
            for(let i=0;i<positions.length;i++)maxVertexError=Math.max(maxVertexError,Math.abs(positions[i]-expected[i]))
          }
        })
        return {houses,wrongChildren,maxDoorError,maxFootprintExcess,maxVertexError}
      })
      assert(contract.houses>0,'missing planned residential building geometry')
      assert.equal(contract.wrongChildren,0,'renderer invented compound contents')
      assert(contract.maxDoorError<1e-5,'visible door disagrees with planned facade')
      assert(contract.maxFootprintExcess<.003,'visible building exceeds footprint/height')
      assert(contract.maxVertexError<.0001,'renderer changed Rust landcover vertices beyond Float32 rounding')
      console.log(JSON.stringify({yardRenderContract:contract}))
    }
  }
  assert.deepEqual(errors, [])
  await page.screenshot({ path: output })
  const report = { url, alignment, errors, feedback: await page.evaluate(() => window.glyphweaveFeedback), screenshot: output }
  await writeFile(`${output}.json`, JSON.stringify(report, null, 2))
  console.log(JSON.stringify(report, null, 2))
} finally {
  await browser.close()
}
