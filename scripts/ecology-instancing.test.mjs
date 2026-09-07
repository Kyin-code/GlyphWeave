import { test } from 'node:test'
import assert from 'node:assert/strict'
import * as THREE from '../adapters/html/vendor/three/build/three.module.js'
import { indexEcology, createEcologyRenderer } from '../adapters/html/render/ecology.js'

function fixture(originX = -1000, originZ = 2000, widthM = 1030, depthM = 600) {
  const chunks = [], plants = []
  for (let z = 0; z < Math.ceil(depthM / 512); z++) for (let x = 0; x < Math.ceil(widthM / 512); x++) {
    const chunk = { chunkX: x, chunkZ: z, worldX: originX + x * 512, worldZ: originZ + z * 512, validWidthM: Math.min(512, widthM - x * 512), validDepthM: Math.min(512, depthM - z * 512) }
    chunks.push(chunk)
    for (const [i, kind] of ['tree', 'reed', 'bush'].entries()) plants.push({ id: x + ':' + z + ':' + kind, sourcePatchId: 'patch', kind, layer: 'test', worldX: chunk.worldX + (i + 1) * chunk.validWidthM / 4, worldZ: chunk.worldZ + chunk.validDepthM / 2, worldY: 12.345, widthM: 1.5, heightM: 2.4 + i })
  }
  return { sceneId: 'test', originX, originZ, widthM, depthM, chunkSizeM: 512, chunks, landcoverSurfaces: { patches: [{ id: 'patch' }], ecology: { plants } } }
}
function parentFor(scene) { const group = new THREE.Group(); group.position.set(-scene.originX, 0, -scene.originZ); return group }
function assertGeometry(parent, scene, count) {
  parent.updateMatrixWorld(true)
  const expected = new Map(scene.landcoverSurfaces.ecology.plants.map(p => [p.id, p]))
  const seen = new Set()
  parent.traverse(mesh => {
    if (!mesh.isInstancedMesh) return
    assert.equal(mesh.geometry.parameters.radius, .5)
    assert.equal(mesh.geometry.parameters.height, 1)
    assert(Number.isFinite(mesh.boundingSphere.radius) && mesh.boundingSphere.radius > 0)
    assert(mesh.boundingBox && !mesh.boundingBox.isEmpty())
    assert.equal(mesh.count, mesh.userData.ecologyInstances.length)
    for (let i = 0; i < mesh.count; i++) {
      const record = mesh.userData.ecologyInstances[i], p = expected.get(record.id)
      assert(p); assert(!seen.has(p.id)); seen.add(p.id)
      assert.equal(record.sourcePatchId, p.sourcePatchId)
      assert.equal(record.kind, p.kind)
      const matrix = new THREE.Matrix4(); mesh.getMatrixAt(i, matrix)
      const base = new THREE.Vector3(0, -.5, 0).applyMatrix4(matrix).applyMatrix4(mesh.matrixWorld)
      const top = new THREE.Vector3(0, .5, 0).applyMatrix4(matrix).applyMatrix4(mesh.matrixWorld)
      assert(Math.abs(base.x - (p.worldX - scene.originX)) < 1e-4)
      assert(Math.abs(base.y - p.worldY) < 1e-4)
      assert(Math.abs(base.z - (p.worldZ - scene.originZ)) < 1e-4)
      assert(Math.abs(top.y - base.y - p.heightM) < 1e-4)
      assert(Math.abs(matrix.elements[0] - p.widthM) < 1e-4)
      assert(Math.abs(matrix.elements[10] - p.widthM) < 1e-4)
    }
  })
  assert.equal(seen.size, count)
}
for (const origin of [0, -1000, 2_000_000_000]) test('authoritative transforms and partial chunks at origin ' + origin, () => {
  const scene = fixture(origin, -origin), parent = parentFor(scene), renderer = createEcologyRenderer(THREE)
  try { const stats = renderer.update(scene, scene.chunks, parent); assert.equal(stats.activePlants, 18); assertGeometry(parent, scene, 18) } finally { renderer.dispose() }
})
test('half-open chunk borders and invalid sources fail closed', () => {
  const scene = fixture()
  scene.landcoverSurfaces.ecology.plants[0].worldX = scene.originX + 512
  const index = indexEcology(scene)
  assert(index.chunks.get('1:0').groups.get('tree').some(p => p.id === '0:0:tree'))
  for (const mutate of [s => s.landcoverSurfaces.ecology.plants.push(s.landcoverSurfaces.ecology.plants[0]), s => { s.landcoverSurfaces.ecology.plants[0].worldX = s.originX + s.widthM }, s => { s.landcoverSurfaces.ecology.plants[0].sourcePatchId = 'missing' }, s => { s.landcoverSurfaces.ecology.plants[0].heightM = NaN }, s => { s.landcoverSurfaces.ecology.plants[0].kind = 'unknown' }]) {
    const bad = structuredClone(scene); mutate(bad); assert.throws(() => indexEcology(bad))
  }
})
test('active batches reuse and evict buffers without duplicate instances', () => {
  const scene = fixture(), renderer = createEcologyRenderer(THREE), first = parentFor(scene), second = parentFor(scene)
  try {
    const a = renderer.update(scene, [scene.chunks[0]], first)
    const mesh = first.children[0]; let disposed = 0; mesh.addEventListener('dispose', () => disposed++)
    const b = renderer.update(scene, [scene.chunks[0], scene.chunks[0]], second)
    assert.equal(a.builds, b.builds); assert.equal(b.indexBuilds, 1); assert.equal(first.children.length, 0)
    assertGeometry(second, scene, 3)
    const c = renderer.update(scene, [scene.chunks[1]], second)
    assert.equal(disposed, 1); assert.equal(c.activePlants, 3); assert.equal(c.activeBatches, 3); assert.equal(c.evictions, 3)
    assertGeometry(second, scene, 3)
    const d = renderer.update(scene, [scene.chunks[0]], second)
    assert.equal(d.builds, 9); assert.equal(d.indexBuilds, 1); assertGeometry(second, scene, 3)
    renderer.update(scene, [], second); assert.equal(second.children.length, 0); assert.equal(renderer.stats().matrixBytes, 0)
  } finally { renderer.dispose() }
})
test('snapshot replacement invalidates stale matrices; disposal releases shared resources', () => {
  const scene = fixture(), renderer = createEcologyRenderer(THREE), parent = parentFor(scene)
  renderer.update(scene, [scene.chunks[0]], parent)
  const mesh = parent.children[0]; let geometryDisposed = 0, materialDisposed = 0
  mesh.geometry.addEventListener('dispose', () => geometryDisposed++)
  mesh.material.addEventListener('dispose', () => materialDisposed++)
  const next = structuredClone(scene); next.landcoverSurfaces.ecology.plants[0].worldY += 5
  renderer.update(next, [next.chunks[0]], parent)
  assert.equal(geometryDisposed, 1); assert.equal(materialDisposed, 1); assert.equal(renderer.stats().indexBuilds, 2)
  assertGeometry(parent, next, 3)
  renderer.dispose(); renderer.dispose(); assert.equal(parent.children.length, 0); assert.equal(renderer.stats().activeBatches, 0)
})
test('synthetic 6km by 10km chunk traversal keeps GPU batch residency bounded', () => {
  const scene = fixture(-1234, 5678, 6000, 10000), parent = parentFor(scene), renderer = createEcologyRenderer(THREE)
  try {
    assert.equal(scene.chunks.length, 240)
    for (const chunk of scene.chunks) {
      const s = renderer.update(scene, [chunk], parent)
      assert.equal(s.activeBatches, 3); assert.equal(s.activePlants, 3); assert.equal(s.matrixBytes, 192); assert.equal(s.indexBuilds, 1)
      assertGeometry(parent, scene, 3)
    }
  } finally { renderer.dispose() }
})
