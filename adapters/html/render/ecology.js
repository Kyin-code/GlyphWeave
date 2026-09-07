// Rust supplies every plant. This adapter only indexes and batches immutable
// scene snapshots; it never invents positions, scales, density or culling rules.
export const ECOLOGY_RENDERER_REVISION = 'r39-chunk-instancing-v1'
const palette = { tree: '#315f35', reed: '#6f8c50', bush: '#4d713f' }
const chunkKey = c => c.chunkX + ':' + c.chunkZ

export function indexEcology(scene) {
  const size = scene.chunkSizeM
  if (!Number.isFinite(size) || size <= 0 || !Number.isFinite(scene.originX) || !Number.isFinite(scene.originZ) || !Number.isFinite(scene.widthM) || !Number.isFinite(scene.depthM) || scene.widthM <= 0 || scene.depthM <= 0) throw new Error('Invalid ecology scene domain')
  const chunks = new Map()
  for (const chunk of scene.chunks) {
    const key = chunkKey(chunk)
    if (chunks.has(key) || !Number.isInteger(chunk.chunkX) || !Number.isInteger(chunk.chunkZ) || chunk.chunkX < 0 || chunk.chunkZ < 0
      || chunk.worldX !== scene.originX + chunk.chunkX * size || chunk.worldZ !== scene.originZ + chunk.chunkZ * size
      || !Number.isFinite(chunk.validWidthM) || !Number.isFinite(chunk.validDepthM)
      || chunk.validWidthM <= 0 || chunk.validWidthM > size || chunk.validDepthM <= 0 || chunk.validDepthM > size
      || chunk.worldX + chunk.validWidthM > scene.originX + scene.widthM || chunk.worldZ + chunk.validDepthM > scene.originZ + scene.depthM) throw new Error('Invalid ecology chunk index')
    chunks.set(key, { chunk, groups: new Map() })
  }
  const sources = new Set((scene.landcoverSurfaces?.patches ?? []).map(p => p.id))
  const plants = scene.landcoverSurfaces?.ecology?.plants ?? []
  const ids = new Set()
  for (const plant of plants) {
    if (typeof plant.id !== 'string' || !plant.id || ids.has(plant.id) || !sources.has(plant.sourcePatchId)
      || !Object.hasOwn(palette, plant.kind)
      || ![plant.worldX, plant.worldY, plant.worldZ, plant.heightM, plant.widthM].every(Number.isFinite)
      || plant.heightM <= 0 || plant.widthM <= 0) throw new Error('Invalid Rust ecology plant: ' + plant.id)
    ids.add(plant.id)
    const key = Math.floor((plant.worldX - scene.originX) / size) + ':' + Math.floor((plant.worldZ - scene.originZ) / size)
    const row = chunks.get(key), chunk = row?.chunk
    if (!chunk || plant.worldX < chunk.worldX || plant.worldX >= chunk.worldX + chunk.validWidthM
      || plant.worldZ < chunk.worldZ || plant.worldZ >= chunk.worldZ + chunk.validDepthM) throw new Error('Ecology plant outside chunk: ' + plant.id)
    if (!row.groups.has(plant.kind)) row.groups.set(plant.kind, [])
    row.groups.get(plant.kind).push(plant)
  }
  for (const { groups } of chunks.values()) for (const plants of groups.values()) plants.sort((a, b) => a.id.localeCompare(b.id))
  return { chunks, total: plants.length }
}

// One renderer owns its materials/geometries and ONLY the active chunk matrix
// buffers. Revisiting a chunk recreates its buffers; no unbounded GPU history.
export function createEcologyRenderer(THREE) {
  let source = null, sourceSurfaces = null, sourcePlants = null, index = null
  const meshes = new Map(), templates = new Map()
  let builds = 0, evictions = 0, indexBuilds = 0
  function releaseMesh(key) {
    const mesh = meshes.get(key)
    mesh.removeFromParent()
    mesh.dispose()
    meshes.delete(key)
    evictions++
  }
  function dispose() {
    for (const key of [...meshes.keys()]) releaseMesh(key)
    for (const template of templates.values()) { template.geometry.dispose(); template.material.dispose() }
    templates.clear(); index = null; source = null; sourceSurfaces = null; sourcePlants = null
  }
  function templateFor(kind) {
    if (!templates.has(kind)) {
      const geometry = new THREE.ConeGeometry(0.5, 1, kind === 'tree' ? 6 : 5)
      const material = new THREE.MeshStandardMaterial({ color: palette[kind], roughness: .95 })
      templates.set(kind, { geometry, material })
    }
    return templates.get(kind)
  }
  function update(scene, activeChunks, parent) {
    if (source !== scene || sourceSurfaces !== scene.landcoverSurfaces || sourcePlants !== scene.landcoverSurfaces?.ecology?.plants) {
      const next = indexEcology(scene) // validate before replacing a valid snapshot
      dispose(); index = next; source = scene; sourceSurfaces = scene.landcoverSurfaces; sourcePlants = sourceSurfaces?.ecology?.plants; indexBuilds++
    }
    const wanted = new Map()
    for (const chunk of activeChunks) {
      const key = chunkKey(chunk), row = index.chunks.get(key)
      if (!row || row.chunk.worldX !== chunk.worldX || row.chunk.worldZ !== chunk.worldZ
        || row.chunk.validWidthM !== chunk.validWidthM || row.chunk.validDepthM !== chunk.validDepthM) throw new Error('Unknown active ecology chunk')
      for (const [kind, plants] of row.groups) wanted.set(key + ':' + kind, { chunk: row.chunk, kind, plants })
    }
    for (const key of [...meshes.keys()]) if (!wanted.has(key)) releaseMesh(key)
    const transform = new THREE.Matrix4(), position = new THREE.Vector3(), scale = new THREE.Vector3(), rotation = new THREE.Quaternion()
    for (const [key, { chunk, kind, plants }] of wanted) {
      let mesh = meshes.get(key)
      if (!mesh) {
        const template = templateFor(kind)
        mesh = new THREE.InstancedMesh(template.geometry, template.material, plants.length)
        mesh.name = 'ecology-batch:' + key
        // Matrices remain chunk-local (Float32); world origin is applied in the
        // mesh (double precision), then the shared parent subtracts scene origin
        // ONCE. Do not subtract origin in each instance as the old path did.
        mesh.position.set(chunk.worldX, 0, chunk.worldZ)
        mesh.userData.ecology = true
        mesh.userData.ecologyRendererRevision = ECOLOGY_RENDERER_REVISION
        mesh.userData.ecologyKind = kind
        mesh.userData.ecologyChunk = [chunk.chunkX, chunk.chunkZ]
        mesh.userData.ecologyInstances = plants.map(p => ({ id: p.id, sourcePatchId: p.sourcePatchId, kind: p.kind, layer: p.layer }))
        for (let i = 0; i < plants.length; i++) {
          const p = plants[i]
          position.set(p.worldX - chunk.worldX, p.worldY + p.heightM * .5, p.worldZ - chunk.worldZ)
          scale.set(p.widthM, p.heightM, p.widthM)
          transform.compose(position, rotation, scale)
          mesh.setMatrixAt(i, transform)
        }
        mesh.instanceMatrix.needsUpdate = true
        mesh.computeBoundingBox(); mesh.computeBoundingSphere()
        meshes.set(key, mesh); builds++
      }
      parent.add(mesh) // reparent cached active meshes; never duplicate a batch
    }
    return stats()
  }
  function stats() {
    const activePlants = [...meshes.values()].reduce((sum, mesh) => sum + mesh.count, 0)
    return { revision: ECOLOGY_RENDERER_REVISION, totalPlanned: index?.total ?? 0, activePlants,
      activeBatches: meshes.size, matrixBytes: activePlants * 16 * 4, templates: templates.size, builds, evictions, indexBuilds }
  }
  return { update, dispose, stats }
}
