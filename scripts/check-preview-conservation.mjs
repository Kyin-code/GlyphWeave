// Verify actual Rust-authored geometry/identity, not only rendered counts.
import { createRequire } from 'node:module'
import { writeFile } from 'node:fs/promises'
import assert from 'node:assert/strict'
const require = createRequire(import.meta.url)
const { chromium } = require('playwright')
const [url, out] = process.argv.slice(2)
assert(url && out, 'usage: URL SCREENSHOT_PATH (whole single-chunk scene)')
const browser = await chromium.launch({ headless: true, channel: process.env.PREVIEW_BROWSER_CHANNEL || 'msedge' })
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } })
  const errors = []
  page.on('pageerror', e => errors.push(String(e)))
  await page.goto(url)
  await page.waitForFunction(() => window.glyphweaveFeedback?.visualChecks?.nearSceneRendered, {}, { timeout: 90000 })
  const inspect = async () => page.evaluate(async () => {
    const THREE = await import('/preview/vendor/three/build/three.module.js')
    const scene = await fetch('../scenes/' + window.glyphweaveFeedback.sceneId + '/scene.json').then(r => r.json())
    const generation = await fetch('../generation-report.json').then(r => r.json())
    const materials = ['conservation_ground', 'conservation_forest_floor', 'conservation_wetland']
    const expected = scene.landcoverSurfaces.patches.filter(p => materials.includes(p.material))
    const patchById = new Map(expected.map(p => [p.id, p]))
    const plants = scene.landcoverSurfaces.ecology.plants
    const plantById = new Map(plants.map(p => [p.id, p]))
    const found = [], roots = [], mismatches = []
    let verts = 0
    const near = (a, b) => Number.isFinite(a) && Math.abs(a - b) <= 1e-5
    window.__nearScene.updateMatrixWorld(true)
    let batches = 0
    window.__nearScene.traverse(object => {
      if (object.isMesh && object.name.startsWith('landcover:')) {
        const id = object.name.slice('landcover:'.length), patch = patchById.get(id)
        if (patch && materials.includes(object.userData.landcoverMaterial)) {
          found.push(id); verts += object.geometry.getAttribute('position').count
        }
      }
      if (object.name.startsWith('ecology-plant:')) { mismatches.push({ reason: 'legacy per-plant root remains' }); return }
      if (!object.name.startsWith('ecology-batch:')) return
      batches++
      if (!object.isInstancedMesh || object.count !== object.userData.ecologyInstances?.length) { mismatches.push({ reason: 'invalid instance batch' }); return }
      const geometry = object.geometry.parameters
      const instance = new THREE.Matrix4(), combined = new THREE.Matrix4()
      for (let i = 0; i < object.count; i++) {
        const record = object.userData.ecologyInstances[i], id = record.id, plant = plantById.get(id)
        roots.push(id)
        if (!plant) { mismatches.push({ id, reason: 'unplanned rendered plant' }); continue }
        object.getMatrixAt(i, instance)
        combined.multiplyMatrices(object.matrixWorld, instance)
        const base = new THREE.Vector3(0, -.5, 0).applyMatrix4(combined)
        const top = new THREE.Vector3(0, .5, 0).applyMatrix4(combined)
        // Float32 instance matrices are chunk-local; origin must be applied once.
        const tolerance = 1e-4 + Math.max(Math.abs(base.y), Math.abs(top.y)) * 2 ** -23
        const close = (a, b) => Number.isFinite(a) && Math.abs(a - b) <= tolerance
        if (record.sourcePatchId !== plant.sourcePatchId || record.kind !== plant.kind || record.layer !== plant.layer
          || !close(base.x, plant.worldX - scene.originX) || !close(base.y, plant.worldY) || !close(base.z, plant.worldZ - scene.originZ)
          || !close(top.y - base.y, plant.heightM) || !close(instance.elements[0], plant.widthM) || !close(instance.elements[10], plant.widthM)
          || !near(geometry.height, 1) || !near(geometry.radius, .5)) {
          mismatches.push({ id, reason: 'instance source, world base or dimensions differ from Rust' })
        }
      }
    })
    return {
      generatorRevision: generation.generatorRevision,
      proof: { expected: expected.map(p => p.id).sort(), found: found.sort(), verts,
        area: expected.reduce((sum, p) => sum + p.areaM2, 0), terrainPatches: expected.length },
      plantProof: { expected: plants.map(p => p.id).sort(), rendered: roots.sort(), planned: plants.length, batches, mismatches },
      instancing: window.glyphweaveFeedback.visualChecks.ecologyInstancing
    }
  })
  const result = await inspect()
  assert(result.plantProof.batches > 0 && result.plantProof.batches < result.plantProof.planned)
  assert.equal(result.instancing.activePlants, result.plantProof.planned)
  const initialBuilds = result.instancing.builds
  for (let redraw = 0; redraw < 3; redraw++) {
    await page.evaluate(() => window.__focusAt(200, 200))
    await page.waitForFunction(() => window.glyphweaveFeedback?.visualChecks?.nearSceneRendered, {}, { timeout: 90000 })
    const next = await inspect()
    assert.deepEqual(next.plantProof, result.plantProof, 'redraw must not duplicate or transform plants')
    assert.equal(next.instancing.builds, initialBuilds, 'same active batches should reuse matrices')
    assert.deepEqual(next.proof, result.proof, 'transient groups must not duplicate surfaces')
  }
  assert.deepEqual(result.proof.found, result.proof.expected)
  assert(result.proof.area > 0)
  assert.equal(result.proof.verts, result.proof.terrainPatches * 4)
  assert(result.plantProof.planned > 0)
  assert.deepEqual(result.plantProof.rendered, result.plantProof.expected, 'every rendered ID must match Rust')
  assert.equal(new Set(result.plantProof.rendered).size, result.plantProof.rendered.length)
  assert.deepEqual(result.plantProof.mismatches, [])
  assert.deepEqual(errors, [])
  await page.screenshot({ path: out })
  await writeFile(out + '.json', JSON.stringify({ status: 'pass', ...result, errors, screenshot: out }, null, 2))
  console.log(JSON.stringify({ status: 'pass', generatorRevision: result.generatorRevision,
    patches: result.proof.terrainPatches, area: result.proof.area, plants: result.plantProof.planned, batches: result.plantProof.batches, instancing: result.instancing,
    mismatches: result.plantProof.mismatches, errors, screenshot: out }, null, 2))
} finally {
  await browser.close()
}
