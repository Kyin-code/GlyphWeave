// Isolated WebGL submission comparison using the actual baked plant records.
// This does NOT measure full-world FPS, GPU time or peak process memory.
import { chromium } from 'playwright'
import { writeFile } from 'node:fs/promises'
import assert from 'node:assert/strict'
const [url, output] = process.argv.slice(2)
assert(url && output)
const browser = await chromium.launch({ headless: true, channel: process.env.PREVIEW_BROWSER_CHANNEL || 'msedge' })
try {
  const page = await browser.newPage()
  await page.goto(url)
  await page.waitForFunction(() => window.glyphweaveFeedback?.visualChecks?.nearSceneRendered, {}, { timeout: 90000 })
  const evidence = await page.evaluate(async () => {
    const THREE = await import('/preview/vendor/three/build/three.module.js')
    const { createEcologyRenderer } = await import('/preview/render/ecology.js')
    const scene = await fetch('../scenes/' + window.glyphweaveFeedback.sceneId + '/scene.json').then(r => r.json())
    const plants = scene.landcoverSurfaces.ecology.plants
    const renderer = new THREE.WebGLRenderer({ antialias: false })
    renderer.setSize(256, 256)
    const view = new THREE.Scene(), root = new THREE.Group()
    root.position.set(-scene.originX, 0, -scene.originZ); view.add(root)
    view.add(new THREE.AmbientLight(0xffffff, 1))
    const span = Math.max(scene.widthM, scene.depthM) * .6
    const camera = new THREE.OrthographicCamera(-span, span, span, -span, .1, 10000)
    camera.position.set(scene.widthM / 2, 2000, scene.depthM / 2)
    camera.up.set(0, 0, -1); camera.lookAt(scene.widthM / 2, 0, scene.depthM / 2)
    const colors = { tree: '#315f35', reed: '#6f8c50', bush: '#4d713f' }
    const old = []
    const metrics = () => { renderer.render(view, camera); return { calls: renderer.info.render.calls, triangles: renderer.info.render.triangles, geometries: renderer.info.memory.geometries } }
    const batcher = createEcologyRenderer(THREE)
    try {
      for (const p of plants) {
        const mesh = new THREE.Mesh(new THREE.ConeGeometry(p.widthM * .5, p.heightM, p.kind === 'tree' ? 6 : 5), new THREE.MeshStandardMaterial({ color: colors[p.kind], roughness: .95 }))
        mesh.position.set(p.worldX, p.worldY + p.heightM * .5, p.worldZ)
        root.add(mesh); old.push(mesh)
      }
      const legacy = metrics()
      for (const mesh of old) { mesh.removeFromParent(); mesh.geometry.dispose(); mesh.material.dispose() }
      old.length = 0
      const stats = batcher.update(scene, scene.chunks, root)
      const instanced = metrics()
      return { scope: 'isolated-ecology-draw-submissions-not-full-map-performance', plants: plants.length, legacy, instanced, stats }
    } finally {
      for (const mesh of old) { mesh.removeFromParent(); mesh.geometry.dispose(); mesh.material.dispose() }
      batcher.dispose(); renderer.dispose(); renderer.forceContextLoss()
    }
  })
  assert(evidence.plants > 0)
  assert.equal(evidence.legacy.calls, evidence.plants)
  assert.equal(evidence.instanced.triangles, evidence.legacy.triangles, 'must not reduce geometry to improve counters')
  assert.equal(evidence.instanced.calls, evidence.stats.activeBatches)
  assert(evidence.instanced.calls < evidence.legacy.calls)
  await writeFile(output, JSON.stringify({ status: 'pass', ...evidence }, null, 2))
  console.log(JSON.stringify({ status: 'pass', ...evidence }, null, 2))
} finally { await browser.close() }
