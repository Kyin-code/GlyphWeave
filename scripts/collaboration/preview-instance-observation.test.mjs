import {test} from 'node:test'
import assert from 'node:assert/strict'
import * as THREE from '../../adapters/html/vendor/three/build/three.module.js'

// Mirrors the collector in adapters/html/app.js. The test uses real Three
// objects so empty groups, zero-count batches and invalid instance indices are
// exercised by the same graph semantics as the browser preview.
function collect(root) {
  const ids = [], sources = []
  const drawableGeometry = object => {
    let meshCount = 0, geometryCount = 0, instanceCount = 0
    object.traverse(child => {
      if (!(child.isMesh || child.isInstancedMesh) || !child.geometry) return
      const count = child.isInstancedMesh ? Number(child.count) : 1
      if (!Number.isFinite(count) || count <= 0) return
      meshCount++; geometryCount++
      if (child.isInstancedMesh) instanceCount += count
    })
    return {meshCount, geometryCount, instanceCount}
  }
  root.traverse(object => {
    if (object.userData?.sourceEntityId) {
      const geometry = drawableGeometry(object)
      const valid = geometry.meshCount > 0 && geometry.geometryCount > 0
      ids.push(object.userData.sourceEntityId)
      sources.push({id: object.userData.sourceEntityId, source: 'object', objectUuid: object.uuid,
        meshCount: geometry.meshCount, geometryCount: geometry.geometryCount, valid,
        reason: valid ? null : 'no-drawable-geometry'})
    }
    if (Array.isArray(object.userData?.entityInstanceBindings)) {
      const meshes = []
      object.traverse(child => { if (child.isInstancedMesh && child.geometry) meshes.push(child) })
      const meshByUuid = new Map(meshes.map(mesh => [mesh.uuid, mesh]))
      for (const binding of object.userData.entityInstanceBindings) {
        const parts = Array.isArray(binding.parts) ? binding.parts : []
        let valid = Number.isSafeInteger(binding.instanceIndex) && binding.instanceIndex >= 0
          && Number.isSafeInteger(binding.count) && binding.instanceIndex < binding.count
          && parts.length > 0
        const checkedParts = parts.map(part => {
          const mesh = meshByUuid.get(part.batchUuid)
          const partValid = Boolean(mesh) && Number.isSafeInteger(part.instanceIndex) && part.instanceIndex >= 0
            && part.instanceIndex < mesh.count && part.geometryUuid === mesh.geometry?.uuid
            && Number.isSafeInteger(part.instanceCount) && part.instanceCount === mesh.count
          if (!partValid) valid = false
          return {...part, partValid}
        })
        ids.push(binding.id)
        sources.push({id: binding.id, source: 'instances', instanceIndex: binding.instanceIndex,
          count: binding.count, parts: checkedParts, valid, reason: valid ? null : 'invalid-instance-binding'})
      }
    }
  })
  return {ids, sources}
}
const mesh = () => new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1), new THREE.MeshBasicMaterial())
const batch = count => new THREE.InstancedMesh(new THREE.BoxGeometry(1, 1, 1), new THREE.MeshBasicMaterial(), count)

test('empty tagged group has no entity existence', () => {
  const root = new THREE.Scene(), group = new THREE.Group()
  group.userData.entityInstanceBindings = [{id: 'synthetic-tree', instanceIndex: 0, count: 1, parts: []}]
  root.add(group)
  const {sources} = collect(root)
  assert.equal(sources[0].valid, false)
})
test('zero-count instance batch has no entity existence', () => {
  const root = new THREE.Scene(), group = new THREE.Group(), inst = batch(0)
  group.add(inst)
  group.userData.entityInstanceBindings = [{id: 'synthetic-tree', instanceIndex: 0, count: 0,
    parts: [{batchUuid: inst.uuid, geometryUuid: inst.geometry.uuid, instanceIndex: 0, instanceCount: 0}]}]
  root.add(group)
  assert.equal(collect(root).sources[0].valid, false)
})
test('compound building with multiple meshes is one valid logical entity', () => {
  const root = new THREE.Scene(), group = new THREE.Group()
  group.userData.sourceEntityId = 'building:1'
  group.add(mesh(), mesh())
  root.add(group)
  const source = collect(root).sources[0]
  assert.equal(source.valid, true)
  assert.equal(source.meshCount, 2)
})
test('valid instanced tree records logical ID, batch UUID and index', () => {
  const root = new THREE.Scene(), group = new THREE.Group(), trunk = batch(2), canopy = batch(2)
  group.add(trunk, canopy)
  group.userData.entityInstanceBindings = [
    {id: 'tree:0', instanceIndex: 0, count: 2, parts: [
      {batchUuid: trunk.uuid, geometryUuid: trunk.geometry.uuid, instanceIndex: 0, instanceCount: 2, part: 'trunk'},
      {batchUuid: canopy.uuid, geometryUuid: canopy.geometry.uuid, instanceIndex: 0, instanceCount: 2, part: 'canopy'},
    ]},
    {id: 'tree:1', instanceIndex: 1, count: 2, parts: [
      {batchUuid: trunk.uuid, geometryUuid: trunk.geometry.uuid, instanceIndex: 1, instanceCount: 2, part: 'trunk'},
      {batchUuid: canopy.uuid, geometryUuid: canopy.geometry.uuid, instanceIndex: 1, instanceCount: 2, part: 'canopy'},
    ]},
  ]
  root.add(group)
  const {sources} = collect(root)
  assert.equal(sources.length, 2)
  assert.ok(sources.every(source => source.valid && source.parts.length === 2))
})
test('out-of-range instance index is invalid', () => {
  const root = new THREE.Scene(), group = new THREE.Group(), inst = batch(1)
  group.add(inst)
  group.userData.entityInstanceBindings = [{id: 'tree:0', instanceIndex: 1, count: 1,
    parts: [{batchUuid: inst.uuid, geometryUuid: inst.geometry.uuid, instanceIndex: 1, instanceCount: 1}]}]
  root.add(group)
  assert.equal(collect(root).sources[0].valid, false)
})
test('deleted actual part while metadata remains is invalid', () => {
  const root = new THREE.Scene(), group = new THREE.Group(), inst = batch(1)
  group.userData.entityInstanceBindings = [{id: 'tree:0', instanceIndex: 0, count: 1,
    parts: [{batchUuid: inst.uuid, geometryUuid: inst.geometry.uuid, instanceIndex: 0, instanceCount: 1}]}]
  root.add(group)
  assert.equal(collect(root).sources[0].valid, false)
})
test('duplicate logical roots are visible to the checker', () => {
  const root = new THREE.Scene(), a = mesh(), b = mesh()
  a.userData.sourceEntityId = 'dup'; b.userData.sourceEntityId = 'dup'
  root.add(a, b)
  const {sources} = collect(root)
  assert.equal(sources.length, 2)
  assert.equal(new Set(sources.map(source => source.id)).size, 1)
})
