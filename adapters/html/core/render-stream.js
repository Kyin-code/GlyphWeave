// Read-only render projection. Full scene.json stays CLI audit authority.
// New exports fail closed on corrupt/missing chunks, never silently fetch the
// giant full scene as a repair. Only missing directories support legacy worlds.
export async function loadPreviewScene(scenePath, worldRevision, fetcher = fetch) {
  const base = '../' + scenePath.slice(0, scenePath.lastIndexOf('/') + 1)
  const response = await fetcher(base + 'preview-scene.json')
  if (response.status === 404) {
    const legacy = await fetcher('../' + scenePath)
    if (!legacy.ok) throw new Error('Missing legacy scene')
    return legacy.json()
  }
  if (!response.ok) throw new Error('Cannot load render directory')
  const scene = await response.json()
  const stream = scene.renderStream
  if (stream?.format !== 'glyphweave.render-directory' || stream.version !== 1 || stream.worldRevision !== worldRevision
    || !Array.isArray(scene.chunks) || !Array.isArray(stream.chunks) || stream.chunks.length !== scene.chunks.length
    || !Number.isSafeInteger(stream.totalPatches) || stream.totalPatches < 0 || !Number.isSafeInteger(stream.totalPlants) || stream.totalPlants < 0) throw new Error('Invalid render directory revision or coverage')
  scene.renderBase = base
  // Validate the full descriptor set before any render chunk is requested.
  const keys = new Set(scene.chunks.map(c => c.chunkX + ':' + c.chunkZ))
  if (keys.size !== scene.chunks.length) throw new Error('Duplicate scene chunk')
  for (const d of stream.chunks) {
    const key = d.chunkX + ':' + d.chunkZ
    if (!Number.isSafeInteger(d.chunkX) || d.chunkX < 0 || !Number.isSafeInteger(d.chunkZ) || d.chunkZ < 0 || !keys.delete(key) || !Number.isSafeInteger(d.bytes) || d.bytes < 0 || !Number.isSafeInteger(d.patchCount) || d.patchCount < 0
      || !Number.isSafeInteger(d.plantCount) || d.plantCount < 0 || !/^[a-f0-9]{64}$/.test(d.sha256)
      || d.file !== 'render-chunks/' + d.chunkX + '-' + d.chunkZ + '-' + d.sha256 + '.json') throw new Error('Invalid render chunk descriptor')
  }
  return scene
}
export function createRenderStream(scene, fetcher = fetch) {
  const descriptors = new Map(scene.renderStream.chunks.map(d => [d.chunkX + ':' + d.chunkZ, d]))
  let cache = new Map(), lastKey = '', lastResult = null, serial = 0
  let requests = 0, bytesFetched = 0, attempts = 0, peakPending = 0, discarded = 0, failures = 0
  let lastError = null, disposed = false
  const pending = new Map()
  function stats() {
    return { version: 1, generation: serial, pending: pending.size, peakPending, cacheEntries: cache.size,
      loadedChunks: [...cache.keys()], residentBytes: [...cache.keys()].reduce((n, key) => n + descriptors.get(key).bytes, 0),
      attempts, requests, bytesFetched, discarded, failures, lastError }
  }
  async function read(d) {
    const key = d.chunkX + ':' + d.chunkZ
    if (pending.has(key)) return pending.get(key).promise
    // The pending budget is a CONCURRENCY limit, not a failure. `load` starts
    // every active chunk in one Promise.all, so a world with more than two
    // chunks in view used to throw 'pending budget exceeded' on the third
    // request even though nothing was wrong with the data. Wait for a slot
    // instead; the accounting (peakPending, requests, bytesFetched) is
    // unchanged, and `dispose` still aborts everything in flight.
    while (pending.size >= 2) {
      await Promise.race([...pending.values()].map(entry => entry.promise.catch(() => {})))
      if (disposed) throw new Error('Render stream disposed')
    }
    const controller = new AbortController()
    attempts++
    const promise = readPayload(d, controller.signal).finally(() => pending.delete(key))
    pending.set(key, {promise, controller}); peakPending = Math.max(peakPending, pending.size)
    return promise
  }
  async function readPayload(d, signal) {
    const response = await fetcher(scene.renderBase + d.file, {signal})
    if (!response.ok) throw new Error('Missing render chunk ' + d.file)
    const buffer = await response.arrayBuffer()
    if (buffer.byteLength !== d.bytes) throw new Error('Render chunk length mismatch')
    const digest = [...new Uint8Array(await crypto.subtle.digest('SHA-256', buffer))].map(x => x.toString(16).padStart(2, '0')).join('')
    if (digest !== d.sha256) throw new Error('Render chunk hash mismatch')
    const data = JSON.parse(new TextDecoder().decode(buffer))
    if (data.format !== 'glyphweave.render-chunk' || data.version !== 1 || data.worldRevision !== scene.renderStream.worldRevision
      || data.sceneId !== scene.sceneId || data.chunkX !== d.chunkX || data.chunkZ !== d.chunkZ
      || !Array.isArray(data.patches) || !Array.isArray(data.plants) || data.patches?.length !== d.patchCount || data.plants?.length !== d.plantCount) throw new Error('Render chunk identity/count mismatch')
    requests++; bytesFetched += buffer.byteLength
    return data
  }
  async function load(chunks) {
    const keys = [...new Set(chunks.map(c => c.chunkX + ':' + c.chunkZ))].sort()
    const signature = keys.join('|')
    const ticket = ++serial
    if (disposed) throw new Error('Render stream disposed')
    lastError = null
    try {
    if (lastResult && signature === lastKey) return lastResult
    const selected = await Promise.all(keys.map(async key => {
      const descriptor = descriptors.get(key)
      if (!descriptor) throw new Error('Unknown active render chunk')
      return [key, cache.get(key) ?? await read(descriptor)]
    }))
    const patches = new Map(), plants = new Map()
    for (const [, data] of selected) {
      for (const patch of data.patches) {
        const old = patches.get(patch.id)
        if (old && JSON.stringify(old) !== JSON.stringify(patch)) throw new Error('Conflicting replicated patch')
        patches.set(patch.id, patch)
      }
      for (const plant of data.plants) {
        if (plants.has(plant.id)) throw new Error('Duplicate render plant')
        plants.set(plant.id, plant)
      }
    }
    if ([...plants.values()].some(p => !patches.has(p.sourcePatchId))) throw new Error('Render plant source missing')
    const surfaces = scene.landcoverSurfaces ? { ...scene.landcoverSurfaces, patches: [...patches.values()],
      ecology: { ...scene.landcoverSurfaces.ecology, plants: [...plants.values()] } } : null
    const patchIds = [...patches.keys()]
    const nonMeshPatchIds = [...patches.values()].filter(patch => patch.material === 'access_pavement').map(patch => patch.id)
    const result = { surfaces, stats: { loadedChunks: keys, residentBytes: keys.reduce((sum,k) => sum + descriptors.get(k).bytes, 0),
      loadedPatches: patches.size, loadedPlants: plants.size, patchIds, nonMeshPatchIds,
      totalPatches: scene.renderStream.totalPatches, totalPlants: scene.renderStream.totalPlants,
      requests, bytesFetched, format: 'r44-render-stream' } }
    // Late results cannot repopulate evicted chunks or replace a newer cache.
    if (ticket === serial && !disposed) { cache = new Map(selected); lastKey = signature; lastResult = result }
    else discarded++
    return result
    } catch (error) {
      failures++
      if (ticket === serial && !disposed) lastError = String(error.message || error)
      throw error
    }
  }
  function dispose() {
    disposed = true; serial++
    for (const entry of pending.values()) entry.controller.abort()
    cache.clear(); lastResult = null; lastKey = ''
  }
  return { load, stats, dispose }
}

// A patch can span an interior chunk without any vertex lying inside it.
// Conservative closed AABB intersection matches Rust partitioning; no clipping.
export function verticesOverlapChunks(vertices, chunks) {
  if (!vertices.length) return false
  let minX=Infinity,minZ=Infinity,maxX=-Infinity,maxZ=-Infinity
  for (const [x,,z] of vertices) { minX=Math.min(minX,x);minZ=Math.min(minZ,z);maxX=Math.max(maxX,x);maxZ=Math.max(maxZ,z) }
  return chunks.some(c=>minX<=c.worldX+c.validWidthM && maxX>=c.worldX && minZ<=c.worldZ+c.validDepthM && maxZ>=c.worldZ)
}

// Lifecycle-only tracker: count actual resource objects and dispose a retired
// frame once even when its caches, clones and prototypes share resources.
// These are object counts, NOT estimates of driver/GPU allocation bytes.
export function createPreviewResourceTracker() {
  const known = {geometries: new WeakSet(), materials: new WeakSet(), textures: new WeakSet(), instances: new WeakSet()}
  const freed = {geometries: new WeakSet(), materials: new WeakSet(), textures: new WeakSet(), instances: new WeakSet()}
  const counts = {created: {geometries: 0, materials: 0, textures: 0, instances: 0}, released: {geometries: 0, materials: 0, textures: 0, instances: 0}}
  function collect(roots) {
    const resources = {geometries: new Set(), materials: new Set(), textures: new Set(), instances: new Set()}
    const addTexture = value => { if (value?.isTexture) resources.textures.add(value) }
    for (const root of roots) root?.traverse?.(object => {
      if (object.userData?.ecology) return // owned and counted by ecologyRenderer
      if (object.geometry) resources.geometries.add(object.geometry)
      if (object.isInstancedMesh) resources.instances.add(object)
      for (const material of [object.material].flat().filter(Boolean)) {
        resources.materials.add(material)
        for (const value of Object.values(material)) addTexture(value)
        for (const uniform of Object.values(material.uniforms ?? {})) {
          for (const value of [uniform?.value].flat()) addTexture(value)
        }
      }
    })
    for (const type of Object.keys(resources)) for (const object of resources[type]) {
      if (!known[type].has(object)) { known[type].add(object); counts.created[type]++ }
    }
    return resources
  }
  function release(roots) {
    const resources = collect(roots)
    for (const type of Object.keys(resources)) for (const object of resources[type]) {
      if (!freed[type].has(object)) { object.dispose(); freed[type].add(object); counts.released[type]++ }
    }
  }
  return {observe: collect, release, stats: () => structuredClone(counts)}
}
