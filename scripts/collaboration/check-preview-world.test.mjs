import {test} from 'node:test'
import assert from 'node:assert/strict'
import {mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, symlinkSync} from 'node:fs'
import {resolve, join} from 'node:path'
import {BUDGET, MAX_JSON_BYTES, digest, sha256Ids, validateParity, navigationPlan,
  verifyBindings, evaluateEvidence, readJsonLimited, runInspection, boundedDeadline} from './check-preview-world.mjs'
import {loadPreviewScene, createRenderStream, createPreviewResourceTracker} from '../../adapters/html/core/render-stream.js'

// All fixtures/evidence stay in the explicitly assigned work tree. No browser,
// HTTP server, Rust, authoritative scene, or real-world parity is used.
const work = resolve('work/collaboration/correct-luna-d2-r1')
mkdirSync(work, {recursive: true})
const dir = mkdtempSync(join(work, 'r1-unit-'))
function jsonBytes(v) { return Buffer.from(JSON.stringify(v)) }
function heightBytes(width, depth) {
  const bytes = Buffer.alloc(16 + width * depth * 4)
  bytes.write('GWHM', 0, 'ascii'); bytes.writeUInt32LE(1, 4); bytes.writeUInt32LE(width, 8); bytes.writeUInt32LE(depth, 12)
  return bytes
}
function fixture(widthM = 1030, depthM = 600, originX = -1000, originZ = 73) {
  const url = 'http://fixture.invalid/world/preview/index.html', root = new URL('../', url).href
  const worldPath = join(dir, 'world', 'world.json'), scenePath = join(dir, 'world', 'scenes', 's', 'scene.json')
  const world = {format: 'glyphweave-world', version: 1, revision: 'current-r49-synthetic', scenes: ['scenes/s/scene.json']}
  const chunks = [], descriptors = [], records = new Map(), files = new Map()
  const patch = {id: 'patch:source', material: 'managed_grass', vertices: [[originX, 0, originZ], [originX + widthM, 0, originZ], [originX, 0, originZ + depthM]], triangles: [[0, 2, 1]]}
  const accessPatch = {...patch, id: 'patch:pavement', material: 'access_pavement'}
  const accessSurface = {version: 1, entityId: 'source-building', anchorId: 'front', vertices: patch.vertices, triangles: patch.triangles}
  const entityId = accessSurface.entityId
  const naturalId = 'tree:source'
  const accessId = 'access:' + JSON.stringify(['s', accessSurface.entityId, accessSurface.anchorId])
  const source = {sceneId: 's', plants: [], patches: [patch, accessPatch], accessSurfaces: [accessSurface]}
  for (let z = 0; z < Math.ceil(depthM / 512); z++) for (let x = 0; x < Math.ceil(widthM / 512); x++) {
    const heightFile = `chunk-${x}-${z}.height.bin`
    const c = {chunkX: x, chunkZ: z, worldX: originX + x * 512, worldZ: originZ + z * 512,
      validWidthM: Math.min(512, widthM - x * 512), validDepthM: Math.min(512, depthM - z * 512)}
    const height = heightBytes(c.validWidthM, c.validDepthM)
    chunks.push(c)
    const plant = {id: `plant:${x}:${z}`, sourcePatchId: patch.id, kind: 'tree', layer: 'canopy', worldX: c.worldX + 1, worldZ: c.worldZ + 1, worldY: 1, widthM: 1, heightM: 2}
    source.plants.push(plant)
    const data = {format: 'glyphweave.render-chunk', version: 1, worldRevision: world.revision, sceneId: 's', chunkX: x, chunkZ: z, patches: [patch, accessPatch], plants: [plant]}
    const bytes = jsonBytes(data), sha256 = digest(bytes), file = `render-chunks/${x}-${z}-${sha256}.json`
    const d = {...c, key: `${x}:${z}`, file, sha256, bytes: bytes.length, patches: 2, plants: 1, entities: 1, entityIds: [entityId],
      entityIdsSha256: sha256Ids([entityId]), heightFile, heightBytes: height.length, heightSha256: digest(height), heightEncoding: 'millimetre',
      naturalHeightFile: null, naturalHeightBytes: null, naturalHeightSha256: null, naturalHeightEncoding: null,
      naturalEntities: 1, naturalEntityIds: [naturalId], naturalEntityIdsSha256: sha256Ids([naturalId]),
      nonMeshPatches: 1, nonMeshPatchIdsSha256: sha256Ids([accessPatch.id]), accessSurfaces: 1, accessSurfaceIdsSha256: sha256Ids([accessId]),
      plantIdsSha256: sha256Ids([plant.id]), patchIdsSha256: sha256Ids([patch.id, accessPatch.id])}
    descriptors.push(d); records.set(d.key, data)
    files.set(new URL(`scenes/s/${file}`, root).href, bytes)
    files.set(new URL(`scenes/s/${heightFile}`, root).href, height)
  }
  const header = {sceneId: 's', widthM, depthM, originX, originZ, chunkSizeM: 512, chunks, accessSurfaces: [accessSurface],
    landcoverSurfaces: {patches: [], ecology: {plants: []}}, renderStream: {format: 'glyphweave.render-directory', version: 1,
      worldRevision: world.revision, totalPlants: descriptors.length, totalPatches: 2,
      chunks: descriptors.map(c => ({chunkX: c.chunkX, chunkZ: c.chunkZ, file: c.file, sha256: c.sha256, bytes: c.bytes, plantCount: c.plants, patchCount: c.patches}))}}
  const headerBytes = jsonBytes(header), sourceBytes = jsonBytes(source), worldBytes = jsonBytes(world)
  const headerUrl = new URL('scenes/s/preview-scene.json', root).href
  files.set(new URL('world.json', root).href, worldBytes); files.set(headerUrl, headerBytes)
  const parity = {format: 'glyphweave.render-parity', version: 1, status: 'pass', worldRevision: world.revision,
    worldFile: {path: worldPath, sha256: digest(worldBytes)}, scenes: [{sceneId: 's', widthM, depthM, originX, originZ, chunkSizeM: 512,
      source: {path: scenePath, sha256: digest(sourceBytes), bytes: sourceBytes.length},
      header: {path: join(dir, 'world', 'scenes', 's', 'preview-scene.json'), sha256: digest(headerBytes), bytes: headerBytes.length},
      uniquePlants: descriptors.length, uniquePatches: 2, nonEcologyEntityIds: [entityId], naturalEntityIds: [naturalId], chunks: descriptors}]}
  const calls = []
  const fetcher = async value => {
    const address = String(value); calls.push(address)
    const key = address.startsWith('../') ? new URL(address, url).href : address
    return files.has(key) ? new Response(files.get(key)) : new Response('not found', {status: 404})
  }
  return {url, root, parity, header, headerUrl, records, files, fetcher, calls, accessId, naturalId}
}
function observations(f) {
  const plan = navigationPlan(f.parity), views = []; let token = 0
  function view(row, phase, round = null) {
    const c = f.parity.scenes.find(s => s.sceneId === row.sceneId).chunks.find(c => c.key === row.key), ids = f.records.get(c.key)
    token++
    const v = {...row, format: 'glyphweave.preview-observation', version: 1, phase, round, token, elapsedMs: 10, observedAtMs: token * 10,
      pageId: 'one-persistent-page', worldRevision: f.parity.worldRevision, status: 'ready', requestedGeneration: token, completedGeneration: token,
      highDetailChunk: c.key.replace(':', ','), loadedChunks: [c.key.replace(':', ',')],
      stream: {version: 1, generation: token, loadedChunks: [c.key], pending: 0, peakPending: 2, cacheEntries: 1, residentBytes: c.bytes},
      payload: {residentBytes: c.bytes, loadedPlants: c.plants, loadedPatches: c.patches}, plantIds: ids.plants.map(p => p.id), patchIds: ids.patches.map(p => p.id),
      renderedPatchIds: ids.patches.filter(p => p.material !== 'access_pavement').map(p => p.id),
      nonMeshPatchIds: ids.patches.filter(p => p.material === 'access_pavement').map(p => p.id), accessSurfaceIds: [f.accessId],
      renderedEntityInstanceIds: [c.entityIds[0], f.naturalId],
      renderedEntityIds: [c.entityIds[0], f.naturalId].sort(),
      renderedEntityInstanceSources: [
        {id: c.entityIds[0], source: 'object', objectUuid: `object:${c.entityIds[0]}`, meshCount: 1, geometryCount: 1, valid: true, reason: null},
        {id: f.naturalId, source: 'instances', instanceIndex: 0, count: 1,
          parts: [{batchUuid: 'batch-natural', geometryUuid: 'geometry-natural', instanceIndex: 0, instanceCount: 1, part: 'canopy'}],
          valid: true, reason: null},
      ],
      heightSource: {url: new URL('scenes/s/' + c.heightFile, f.root).href, bytes: c.heightBytes, sha256: c.heightSha256},
      renderer: {geometries: 100, textures: 20, drawCalls: 30},
      heap: {usedBytes: 12 * 1024 * 1024, gc: true, source: 'CDP.Runtime.getHeapUsage'},
      caches: {terrain: 1, entityMeshes: 12, entityGroups: 4}, ecology: {activePlants: c.plants, builds: token, evictions: token - 1},
      resources: {created: {geometries: token * 10, materials: token * 10, textures: token * 10}, released: {geometries: (token - 1) * 10, materials: (token - 1) * 10, textures: (token - 1) * 10}}}
    views.push(v); return v
  }
  plan.coverage.forEach(row => view(row, 'coverage'))
  plan.critical.forEach(row => view(row, 'warm', 0))
  for (let round = 1; round <= 3; round++) plan.critical.forEach(row => view(row, 'revisit', round))
  const oldRetry = ++token, retry = view(plan.coverage[0], 'retry')
  const oldLate = ++token, late = view(plan.coverage.at(-1), 'late')
  return {format: 'glyphweave.preview-traversal', version: 1, status: 'complete', pageId: 'one-persistent-page',
    budget: {...BUDGET}, totalDeadlineMs: plan.totalDeadlineMs, elapsedMs: token * 10, rootUrl: f.root,
    worldRevision: f.parity.worldRevision, worldSha256: f.parity.worldFile.sha256, errors: [], views,
    faults: {
      networkRetry: {version: 1, status: 'pass', phase: 'retry', pageId: 'one-persistent-page', worldRevision: f.parity.worldRevision,
        target: {sceneId: retry.sceneId, key: retry.key, url: new URL('scenes/s/' + f.parity.scenes[0].chunks.find(c => c.key === retry.key).file, f.root).href},
        recovery: {sceneId: retry.sceneId, key: retry.key}, oldToken: oldRetry, newToken: retry.token,
        diagnostic: 'Failed to fetch', injectedAtMs: oldRetry * 10 - 5, failedAtMs: oldRetry * 10,
        newRequestedAtMs: oldRetry * 10 + 1, completedAtMs: retry.observedAtMs,
        failedState: {status: 'failed', requestedGeneration: oldRetry, error: 'Failed to fetch', sceneId: retry.sceneId,
          pageId: retry.pageId, worldRevision: f.parity.worldRevision, observedAtMs: oldRetry * 10}},
      lateResponse: {version: 1, status: 'pass', phase: 'late', pageId: 'one-persistent-page', worldRevision: f.parity.worldRevision,
        target: {sceneId: 's', key: plan.coverage[0].key, url: new URL('scenes/s/' + f.parity.scenes[0].chunks[0].file, f.root).href},
        recovery: {sceneId: late.sceneId, key: late.key}, oldToken: oldLate, newToken: late.token,
        injectedAtMs: oldLate * 10 - 5, heldAtMs: oldLate * 10 - 1, newRequestedAtMs: oldLate * 10,
        releasedAtMs: oldLate * 10 + 1, deliveredAtMs: oldLate * 10 + 2, completedAtMs: late.observedAtMs,
        delivery: {requestFinished: true, url: new URL('scenes/s/' + f.parity.scenes[0].chunks[0].file, f.root).href,
          status: 200, bytes: f.parity.scenes[0].chunks[0].bytes, sha256: f.parity.scenes[0].chunks[0].sha256}},
    }}

}
function assertComponentPass(result) {
  assert.equal(result.componentStatus, 'pass', JSON.stringify(result))
  assert.equal(result.status, 'pass', JSON.stringify(result))
  assert.deepEqual(result.coverageFlags, {entityCoverage: 'pass', heightCoverage: 'pass'})
}
function rejected(f, e, id, status = 'fail') {
  const result = evaluateEvidence(f.parity, e)
  assert.equal(result.status, status, JSON.stringify(result))
  assert.equal(result.componentStatus, status, JSON.stringify(result))
  assert.ok(result.checks.some(c => c.id === id && c.status === status), JSON.stringify(result))
}
test('complete independent synthetic control passes compact HTTP bindings and traversal', async () => {
  const f = fixture(), proof = await verifyBindings(f.parity, f.url, f.fetcher)
  assert.equal(proof.status, 'pass')
  assert.equal(f.calls.length, 2, 'only compact world/header are read before a browser RUN')
  assert.ok(f.calls.every(url => !url.endsWith('/scene.json') && !url.includes('/render-chunks/')))
  assertComponentPass(evaluateEvidence(f.parity, observations(f)))
})
test('240 descriptors and partial chunks at negative/nonzero origins define coverage and local focus', () => {
  const f = fixture(6000, 10000, -1234, 5678), plan = navigationPlan(f.parity)
  assert.equal(plan.coverage.length, 240)
  assert.equal(plan.totalDeadlineMs, 240 * 120_000)
  assert.deepEqual(plan.coverage.at(-1).focus, {x: 5816, z: 9864})
  assertComponentPass(evaluateEvidence(f.parity, observations(f)))
})
test('ID digest uses UTF8 order for Unicode and rejects duplicates without deduplication', () => {
  const ids = ['\u{10000}', '\uE000', 'A']
  assert.equal(sha256Ids(ids), digest(JSON.stringify(['A', '\uE000', '\u{10000}'])))
  assert.throws(() => sha256Ids(['same', 'same']), e => e.id === 'ids.duplicate')
})
test('R2 runtime cap bounds the per-run deadline without changing plan budget', () => {
  const start = 1000
  assert.equal(boundedDeadline(start, 10_000, 4000), 5000)
  assert.equal(boundedDeadline(start, 3000, 4000), 4000)
  assert.equal(boundedDeadline(start, 3000, 0), 4000)
})
for (const [name, mutate, id, status] of [
  ['legacy r47 format', p => { p.format = 'legacy-projection' }, 'parity.schema', 'incomplete'],
  ['missing source hash', p => { delete p.scenes[0].source.sha256 }, 'parity.binding', 'incomplete'],
  ['failed parity', p => { p.status = 'fail' }, 'parity.status', 'fail'],
  ['bad partial width', p => { p.scenes[0].chunks.at(-1).validWidthM++ }, 'parity.chunk-geometry', 'fail'],
  ['missing chunk', p => { p.scenes[0].chunks.pop() }, 'parity.coverage', 'fail'],
  ['duplicate descriptor', p => { p.scenes[0].chunks[1] = structuredClone(p.scenes[0].chunks[0]) }, 'parity.chunk-key', 'fail'],
  ['unsafe chunk path', p => { p.scenes[0].chunks[0].file = '../scene.json' }, 'parity.chunk-file', 'fail'],
  ['zero chunkSize', p => { p.scenes[0].chunkSizeM = 0 }, 'parity.geometry', 'fail'],
]) test(`parity rejects ${name}`, () => {
  const f = fixture(); mutate(f.parity)
  assert.throws(() => validateParity(f.parity), e => e.id === id && e.status === status)
})
for (const [name, mutate, id, status = 'fail'] of [
  ['empty coverage', e => { e.views = e.views.filter(v => v.phase !== 'coverage') }, 'coverage.missing', 'incomplete'],
  ['missing revisit', e => { e.views = e.views.filter(v => v.round !== 3) }, 'revisit.missing', 'incomplete'],
  ['out-of-order revisit', e => { e.views.find(v => v.round === 1).round = 2 }, 'revisit.order'],
  ['reload page', e => { e.views[1].pageId = 'new-page' }, 'view.page'],
  ['old feedback', e => { e.views[1].completedGeneration = 1 }, 'view.generation'],
  ['failed navigation', e => { e.views[1].status = 'failed' }, 'view.generation'],
  ['timeout', e => { e.views[1].elapsedMs = 120001 }, 'budget.step'],
  ['global deadline', e => { e.elapsedMs = e.totalDeadlineMs + 1 }, 'budget.total'],
  ['widened budget', e => { e.budget.pending = 3 }, 'budget.changed'],
  ['two active chunks', e => { e.views[1].loadedChunks.push('0,0') }, 'view.active'],
  ['cache history', e => { e.views[1].stream.cacheEntries = 2 }, 'budget.stream'],
  ['resident bytes off by one', e => { e.views[1].stream.residentBytes++ }, 'budget.stream'],
  ['pending peak three', e => { e.views[1].stream.peakPending = 3 }, 'budget.stream'],
  ['old stream cache key', e => { e.views[1].stream.loadedChunks = ['0:0'] }, 'budget.stream'],
  ['plant omission', e => { e.views[1].plantIds = [] }, 'view.plants'],
  ['duplicated instance', e => { e.views[1].plantIds.push(e.views[1].plantIds[0]) }, 'ids.duplicate'],
  ['patch omission', e => { e.views[1].patchIds = [] }, 'view.patches'],
  ['missing mesh', e => { e.views[1].renderedPatchIds = [] }, 'view.patch-mesh'],
  ['geometry growth', e => { e.views.find(v => v.round === 3).renderer.geometries += 9 }, 'budget.gpu-growth'],
  ['texture growth', e => { e.views.find(v => v.round === 3).renderer.textures += 5 }, 'budget.gpu-growth'],
  ['monotonic growth', e => { e.views.filter(v => v.phase === 'revisit').forEach(v => { v.renderer.geometries += v.round }) }, 'budget.gpu-monotonic'],
  ['heap 65MiB', e => { e.views.find(v => v.round === 3).heap.usedBytes += 65 * 1024 * 1024 }, 'budget.heap'],
  ['heap last rounds 9MiB', e => { e.views.find(v => v.round === 3).heap.usedBytes += 9 * 1024 * 1024 }, 'budget.heap-late'],
  ['unmeasurable heap', e => { e.views.find(v => v.round === 3).heap = null }, 'metrics.heap', 'incomplete'],
  ['unmeasurable renderer', e => { e.views[1].renderer = null }, 'metrics.renderer', 'incomplete'],
  ['missing lifecycle counters', e => { delete e.views[1].resources }, 'metrics.lifecycle', 'incomplete'],
  ['missing fault exercise', e => { e.faults = {} }, 'faults.missing', 'incomplete'],
  ['fault stale token', e => { e.faults.networkRetry.newToken = e.faults.networkRetry.oldToken }, 'faults.tokens'],
  ['browser error', e => { e.errors.push({type: 'pageerror', message: 'crashed'}) }, 'browser.errors'],
]) test(`observation rejects ${name}`, () => { const f = fixture(), e = observations(f); mutate(e); rejected(f, e, id, status) })
test('cross-chunk duplicate plants fail even if every local ID digest is internally consistent', () => {
  const f = fixture(), e = observations(f), reused = e.views[0].plantIds[0], key = e.views[1].key
  f.parity.scenes[0].chunks.find(c => c.key === key).plantIdsSha256 = sha256Ids([reused])
  e.views.filter(v => v.key === key).forEach(v => { v.plantIds = [reused] })
  rejected(f, e, 'coverage.duplicate-plant')
})
test('patched HTTP bytes cannot substitute for bound world/header', async () => {
  const f = fixture(); f.files.set(new URL('world.json', f.root).href, jsonBytes({revision: 'r47'}))
  await assert.rejects(verifyBindings(f.parity, f.url, f.fetcher), e => e.id === 'binding.world-hash')
  const g = fixture(); g.files.set(g.headerUrl, Buffer.from('{truncated'))
  await assert.rejects(verifyBindings(g.parity, g.url, g.fetcher), e => e.id === 'binding.header-hash')
})
test('rehashed header geometry mismatch is rejected independently of hash validation', async () => {
  const f = fixture(); f.header.originX++
  const bytes = jsonBytes(f.header); f.files.set(f.headerUrl, bytes)
  Object.assign(f.parity.scenes[0].header, {bytes: bytes.length, sha256: digest(bytes)})
  await assert.rejects(verifyBindings(f.parity, f.url, f.fetcher), e => e.id === 'binding.header-geometry')
})
test('rehashed duplicate header descriptor is rejected', async () => {
  const f = fixture(); f.header.renderStream.chunks[1] = f.header.renderStream.chunks[0]
  const bytes = jsonBytes(f.header); f.files.set(f.headerUrl, bytes)
  Object.assign(f.parity.scenes[0].header, {bytes: bytes.length, sha256: digest(bytes)})
  await assert.rejects(verifyBindings(f.parity, f.url, f.fetcher), e => e.id === 'binding.descriptor-duplicate')
})
test('bounded regular input read rejects oversized JSON', async () => {
  const file = join(dir, 'oversize.json'); writeFileSync(file, JSON.stringify({padding: 'x'.repeat(1024)}))
  await assert.rejects(readJsonLimited(file, 100), e => e.id === 'input.budget')
})
test('executable no-browser route produces incomplete, out wx refuses overwrite before HTTP', async () => {
  const f = fixture(), parityPath = join(dir, 'parity.json'), outPath = join(dir, 'incomplete.json')
  writeFileSync(parityPath, JSON.stringify(f.parity))
  const r = await runInspection({url: f.url, parityPath, outPath, fetcher: f.fetcher})
  assert.equal(r.result.status, 'incomplete'); assert.equal(r.exitCode, 1)
  assert.equal(JSON.parse(readFileSync(outPath)).checks[0].id, 'browser.not-run')
  const n = f.calls.length
  await assert.rejects(runInspection({url: f.url, parityPath, outPath, fetcher: f.fetcher}), e => e.code === 'EEXIST')
  assert.equal(f.calls.length, n)
})
test('missing parity returns incomplete without creating any output or starting browser/HTTP', async () => {
  const f = fixture()
  const result = await runInspection({url: f.url, parityPath: join(dir, 'missing-parity.json'), outPath: join(dir, 'missing-result.json'), fetcher: f.fetcher})
  assert.equal(result.exitCode, 1); assert.equal(result.result.status, 'incomplete'); assert.equal(result.result.checks[0].id, 'parity.missing'); assert.equal(f.calls.length, 0)
  assert.equal(result.result.outputWritten, false); assert.equal(existsSync(join(dir, 'missing-result.json')), false)
})

// Actual production loader (not a second mock checker): fault, hash/revision,
// pending cap and late cache tests use controlled Promises, never timers/server.
async function loaderFixture() {
  const f = fixture(), scene = await loadPreviewScene('scenes/s/scene.json', f.parity.worldRevision, f.fetcher)
  return {...f, scene}
}
test('one chunk HTTP failure can retry same focus with live versioned stats', async () => {
  const f = await loaderFixture(); let fail = true
  const loader = createRenderStream(f.scene, url => { if (fail) { fail = false; return Promise.resolve(new Response('', {status: 503})) } return f.fetcher(url) })
  await assert.rejects(loader.load([f.scene.chunks[0]]), /Missing render chunk/)
  assert.equal(loader.stats().pending, 0); assert.equal(loader.stats().cacheEntries, 0)
  const result = await loader.load([f.scene.chunks[0]])
  assert.equal(loader.stats().version, 1); assert.equal(loader.stats().residentBytes, result.stats.residentBytes)
  assert.equal(loader.stats().cacheEntries, 1); assert.equal(loader.stats().lastError, null)
})
test('render stream binds non-mesh IDs to loaded access-pavement patches', async () => {
  const f = await loaderFixture(), loader = createRenderStream(f.scene, f.fetcher)
  const result = await loader.load([f.scene.chunks[0]])
  assert.deepEqual(new Set(result.stats.patchIds), new Set(['patch:source', 'patch:pavement']))
  assert.deepEqual(result.stats.nonMeshPatchIds, ['patch:pavement'])
})
test('late response cannot replace newer focus cache; peak pending remains two', async () => {
  const f = await loaderFixture(); let release
  const wait = new Promise(r => { release = r })
  const loader = createRenderStream(f.scene, async url => { if (url.includes('/0-0-')) await wait; return f.fetcher(url) })
  const old = loader.load([f.scene.chunks[0]])
  const current = await loader.load([f.scene.chunks[1]])
  assert.equal(loader.stats().peakPending, 2)
  release(); await old
  assert.deepEqual(loader.stats().loadedChunks, ['1:0']); assert.equal(loader.stats().discarded, 1)
  assert.equal(await loader.load([f.scene.chunks[1]]), current)
  const detached = loader.stats(); detached.loadedChunks.push('bad')
  assert.deepEqual(loader.stats().loadedChunks, ['1:0'])
})
test('third pending chunk is explicitly rejected rather than raising frozen cap', async () => {
  const f = await loaderFixture(); let release
  const wait = new Promise(r => { release = r })
  const loader = createRenderStream(f.scene, async url => { await wait; return f.fetcher(url) })
  const a = loader.load([f.scene.chunks[0]]), b = loader.load([f.scene.chunks[1]])
  await assert.rejects(loader.load([f.scene.chunks[2]]), /pending budget exceeded \(2\)/)
  release(); await Promise.all([a, b]); assert.equal(loader.stats().peakPending, 2)
})
for (const mode of ['truncated', 'wrong-hash', 'wrong-revision']) test(`actual loader rejects ${mode} without giant-scene fallback`, async () => {
  const f = await loaderFixture(), descriptor = f.scene.renderStream.chunks[0]
  const url = new URL(`scenes/s/${descriptor.file}`, f.root).href
  if (mode === 'truncated') f.files.set(url, Buffer.from('{'))
  if (mode === 'wrong-hash') { const b = Buffer.from(f.files.get(url)); b[0] = 32; f.files.set(url, b) }
  if (mode === 'wrong-revision') {
    const data = JSON.parse(f.files.get(url)); data.worldRevision = 'old-r47'
    const b = jsonBytes(data); f.files.set(url, b); descriptor.bytes = b.length; descriptor.sha256 = digest(b)
  }
  const loader = createRenderStream(f.scene, f.fetcher)
  await assert.rejects(loader.load([f.scene.chunks[0]]), new RegExp(mode === 'truncated' ? 'length mismatch' : mode === 'wrong-hash' ? 'hash mismatch' : 'identity/count mismatch'))
  assert.equal(loader.stats().cacheEntries, 0)
  assert.ok(f.calls.every(url => !url.endsWith('/scene.json')))
})
test('240 loader loads and repeated focus cycles retain only one payload cache entry', async () => {
  const f = fixture(6000, 10000), scene = await loadPreviewScene('scenes/s/scene.json', f.parity.worldRevision, f.fetcher)
  const loader = createRenderStream(scene, f.fetcher)
  for (const c of [...scene.chunks, ...Array.from({length: 3}, () => [scene.chunks[0], scene.chunks.at(-1)]).flat()]) {
    const r = await loader.load([c])
    assert.equal(loader.stats().cacheEntries, 1); assert.equal(loader.stats().residentBytes, r.stats.residentBytes); assert.equal(loader.stats().pending, 0)
  }
  loader.dispose(); assert.equal(loader.stats().cacheEntries, 0)
  await assert.rejects(loader.load([scene.chunks[0]]), /disposed/)
})
test('frame resource ownership releases shared geometry/material/uniform textures once', () => {
  const tracker = createPreviewResourceTracker(); let disposals = 0
  const disposable = props => ({...props, dispose() { disposals++ }})
  const texture = disposable({isTexture: true}), geometry = disposable({}), material = disposable({map: texture, uniforms: {normal: {value: texture}}})
  const mesh = {geometry, material}, root = {traverse(fn) { fn(mesh); fn(mesh) }}
  tracker.observe([root]); tracker.release([root, root]); tracker.release([root])
  assert.equal(disposals, 3)
  assert.deepEqual(tracker.stats(), {created: {geometries: 1, materials: 1, textures: 1, instances: 0}, released: {geometries: 1, materials: 1, textures: 1, instances: 0}})
})

test('retired non-ecology instance matrix buffers are explicitly disposed once', () => {
  let matricesDisposed = 0
  const tracker = createPreviewResourceTracker()
  const instance = {isInstancedMesh: true, dispose() { matricesDisposed++ }}
  const root = {traverse(fn) { fn(instance) }}
  tracker.release([root, root]); tracker.release([root])
  assert.equal(matricesDisposed, 1)
  assert.equal(tracker.stats().created.instances, 1)
  assert.equal(tracker.stats().released.instances, 1)
})

// R1 review regressions. Full controls retain an independently allowed pavement
// exception AND its observed access mesh; mutations must reject for their cause.
for (const field of ['nonMeshPatches', 'nonMeshPatchIdsSha256', 'accessSurfaces', 'accessSurfaceIdsSha256']) {
  test(`R1 missing independent ${field} is incomplete, not silently legacy pass`, () => {
    const f = fixture(); delete f.parity.scenes[0].chunks[0][field]
    assert.throws(() => validateParity(f.parity), e => e.id === 'parity.render-bindings' && e.status === 'incomplete')
  })
}
for (const [name, mutate, id, status = 'fail'] of [
  ['managed grass declared non-mesh', e => { for (const v of e.views) { v.nonMeshPatchIds = [...v.patchIds]; v.renderedPatchIds = [] } }, 'view.non-mesh'],
  ['non-mesh digest substitution', e => { e.views[0].nonMeshPatchIds = ['other-pavement'] }, 'view.non-mesh'],
  ['actual access mesh missing', e => { e.views[0].accessSurfaceIds = [] }, 'view.access-surfaces'],
  ['wrong access mesh identity', e => { e.views[0].accessSurfaceIds = ['access:["s","wrong","front"]'] }, 'view.access-surfaces'],
  ['negative total elapsed', e => { e.elapsedMs = -1 }, 'budget.total'],
  ['nonfinite total elapsed', e => { e.elapsedMs = Infinity }, 'budget.total'],
  ['negative navigation elapsed', e => { e.views[0].elapsedMs = -1 }, 'budget.step'],
  ['negative observation timestamp', e => { e.views[0].observedAtMs = -1 }, 'view.time'],
  ['observation beyond traversal end', e => { e.views[0].observedAtMs = e.elapsedMs + 1 }, 'view.time'],
  ['pass plus ordinary tokens only', e => { const f = e.faults.networkRetry; e.faults.networkRetry = {status: 'pass', phase: 'retry', oldToken: f.oldToken, newToken: f.newToken} }, 'faults.structure', 'incomplete'],
  ['network diagnostic missing', e => { delete e.faults.networkRetry.diagnostic }, 'faults.diagnostic'],
  ['empty network diagnostic', e => { e.faults.networkRetry.diagnostic = '' }, 'faults.diagnostic'],
  ['network failed-state missing', e => { delete e.faults.networkRetry.failedState }, 'faults.failed-state'],
  ['network failed-state wrong token', e => { e.faults.networkRetry.failedState.requestedGeneration++ }, 'faults.failed-state'],
  ['network target URL mismatch', e => { e.faults.networkRetry.target.url = 'http://fixture.invalid/wrong.json' }, 'faults.url'],
  ['network target scene mismatch', e => { e.faults.networkRetry.target.sceneId = 'other' }, 'faults.url'],
  ['network diagnostic disagrees with observed error', e => { e.faults.networkRetry.failedState.error = 'different error' }, 'faults.failed-state'],
  ['network failure is a ready snapshot', e => { e.faults.networkRetry.failedState.status = 'ready' }, 'faults.failed-state'],
  ['fault from another page', e => { e.faults.networkRetry.pageId = 'other' }, 'faults.structure', 'incomplete'],
  ['fault from stale world', e => { e.faults.lateResponse.worldRevision = 'old-r47' }, 'faults.structure', 'incomplete'],
  ['late delivery missing', e => { delete e.faults.lateResponse.delivery }, 'faults.delivery'],
  ['late response not actually delivered', e => { e.faults.lateResponse.delivery.requestFinished = false }, 'faults.delivery'],
  ['late delivery hash mismatch', e => { e.faults.lateResponse.delivery.sha256 = '0'.repeat(64) }, 'faults.delivery'],
  ['late delivery wrong URL', e => { e.faults.lateResponse.delivery.url = 'http://fixture.invalid/other.json' }, 'faults.delivery'],
  ['late delivery truncated', e => { e.faults.lateResponse.delivery.bytes-- }, 'faults.delivery'],
  ['late delivery after final observation', e => { e.faults.lateResponse.deliveredAtMs = e.faults.lateResponse.completedAtMs + 1 }, 'faults.time'],
  ['release before newer navigation', e => { e.faults.lateResponse.releasedAtMs = e.faults.lateResponse.newRequestedAtMs - 1 }, 'faults.time'],
  ['late final focus does not match recovery', e => { e.faults.lateResponse.recovery.key = e.faults.lateResponse.target.key }, 'faults.tokens'],
]) test(`R1 rejects ${name}`, () => { const f = fixture(), e = observations(f); mutate(e); rejected(f, e, id, status) })
for (const kind of ['networkRetry', 'lateResponse']) {
  for (const field of kind === 'networkRetry'
    ? ['injectedAtMs', 'failedAtMs', 'newRequestedAtMs', 'completedAtMs']
    : ['injectedAtMs', 'heldAtMs', 'newRequestedAtMs', 'releasedAtMs', 'deliveredAtMs', 'completedAtMs']) {
    test(`R1 ${kind}.${field} must be finite and non-negative`, () => {
      const f = fixture(), e = observations(f); e.faults[kind][field] = -1
      rejected(f, e, 'faults.time')
    })
  }
}
test('R1 entity/height evidence cannot be replaced by self-attested coverage flags', () => {
  const f = fixture(), e = observations(f)
  e.coverageFlags = {entityCoverage: 'pass', heightCoverage: 'pass'}
  delete e.views.find(v => v.phase === 'coverage').renderedEntityIds
  rejected(f, e, 'view.entity-instance-binding', 'incomplete')
})
test('R1 revisit cannot clear rendered entities', () => {
  const f = fixture(), e = observations(f)
  for (const view of e.views.filter(view => view.phase === 'revisit')) {
    view.renderedEntityIds = []
    view.renderedEntityInstanceIds = []
    view.renderedEntityInstanceSources = []
  }
  rejected(f, e, 'view.entity-missing')
})
test('R1 revisit cannot replace the bound height source', () => {
  const f = fixture(), e = observations(f)
  for (const view of e.views.filter(view => view.phase === 'revisit')) {
    view.heightSource = {url: 'http://fixture.invalid/wrong.height.bin', bytes: 0, sha256: '0'.repeat(64)}
  }
  rejected(f, e, 'view.height-source')
})
test('R1 duplicate actual entity instances cannot be hidden by a Set', () => {
  const f = fixture(), e = observations(f)
  e.views.find(view => view.phase === 'coverage').renderedEntityInstanceSources.push({
    ...e.views.find(view => view.phase === 'coverage').renderedEntityInstanceSources[1]
  })
  rejected(f, e, 'view.entity-duplicate-source')
})
test('R1 missing authoritative natural instance is rejected', () => {
  const f = fixture(), e = observations(f)
  for (const view of e.views.filter(view => view.phase === 'coverage')) {
    view.renderedEntityIds = view.renderedEntityIds.filter(id => id !== f.naturalId)
    view.renderedEntityInstanceIds = view.renderedEntityInstanceIds.filter(id => id !== f.naturalId)
    view.renderedEntityInstanceSources = view.renderedEntityInstanceSources.filter(source => source.id !== f.naturalId)
  }
  rejected(f, e, 'view.natural-entity-missing')
})
test('R2 empty tagged group is rejected', () => {
  const f = fixture(), e = observations(f)
  e.views[0].renderedEntityInstanceSources = [
    {id: 'synthetic-tree', source: 'object', objectUuid: 'empty', meshCount: 0, geometryCount: 0, valid: false, reason: 'no-drawable-geometry'},
  ]
  e.views[0].renderedEntityInstanceIds = ['synthetic-tree']
  e.views[0].renderedEntityIds = ['synthetic-tree']
  rejected(f, e, 'view.entity-source-invalid')
})
test('R2 zero-count instance batch is rejected', () => {
  const f = fixture(), e = observations(f)
  e.views[0].renderedEntityInstanceSources = [
    {id: 'synthetic-tree', source: 'instances', instanceIndex: 0, count: 0, valid: false, reason: 'invalid-instance-binding', parts: []},
  ]
  e.views[0].renderedEntityInstanceIds = ['synthetic-tree']
  e.views[0].renderedEntityIds = ['synthetic-tree']
  rejected(f, e, 'view.entity-source-invalid')
})
test('R2 out-of-range instance index is rejected', () => {
  const f = fixture(), e = observations(f)
  e.views[0].renderedEntityInstanceSources[1] = {id: f.naturalId, source: 'instances', instanceIndex: 1, count: 1,
    parts: [{batchUuid: 'batch-natural', geometryUuid: 'geometry-natural', instanceIndex: 1, instanceCount: 1, part: 'canopy'}], valid: true}
  rejected(f, e, 'view.entity-instance-index')
})
test('R2 missing instance part is rejected', () => {
  const f = fixture(), e = observations(f)
  e.views[0].renderedEntityInstanceSources[1] = {id: f.naturalId, source: 'instances', instanceIndex: 0, count: 1,
    parts: [], valid: true}
  rejected(f, e, 'view.entity-instance-parts')
})
test('R2 duplicate logical root is rejected', () => {
  const f = fixture(), e = observations(f)
  e.views[0].renderedEntityInstanceSources.push({...e.views[0].renderedEntityInstanceSources[1]})
  rejected(f, e, 'view.entity-duplicate-source')
})
test('R2 missing source records are rejected', () => {
  const f = fixture(), e = observations(f)
  delete e.views[0].renderedEntityInstanceSources
  rejected(f, e, 'view.entity-source-observation', 'incomplete')
})
test('R1 source-bound access surfaces must agree with rehashed header overlap subset', async () => {
  const f = fixture(); f.header.accessSurfaces[0].entityId = 'other-source'
  const bytes = jsonBytes(f.header); f.files.set(f.headerUrl, bytes)
  Object.assign(f.parity.scenes[0].header, {bytes: bytes.length, sha256: digest(bytes)})
  await assert.rejects(verifyBindings(f.parity, f.url, f.fetcher), e => e.id === 'binding.access-ids')
})
test('R1 HTTP header access surface array is required, not defaulted to empty', async () => {
  const f = fixture(); delete f.header.accessSurfaces
  const bytes = jsonBytes(f.header); f.files.set(f.headerUrl, bytes)
  Object.assign(f.parity.scenes[0].header, {bytes: bytes.length, sha256: digest(bytes)})
  await assert.rejects(verifyBindings(f.parity, f.url, f.fetcher), e => e.id === 'binding.access-surfaces' && e.status === 'incomplete')
})
async function boundaryCase(f, outPath, label, observationPath = null) {
  const parityPath = join(dir, `boundary-${label}.json`)
  writeFileSync(parityPath, JSON.stringify(f.parity))
  const r = await runInspection({url: f.url, parityPath, outPath, observationPath, fetcher: f.fetcher})
  assert.equal(r.result.status, 'fail', JSON.stringify(r)); assert.equal(r.result.outputWritten, false)
  assert.equal(r.result.checks[0].id, 'output.protected', JSON.stringify(r))
  assert.equal(f.calls.length, 0, 'output admission must happen before HTTP')
  return r
}
test('R1 output within protected world is refused without creating any file/parent', async () => {
  const f = fixture(), out = join(dir, 'world', 'forbidden-new-dir', 'result.json')
  await boundaryCase(f, out, 'world')
  assert.equal(existsSync(out), false); assert.equal(existsSync(join(dir, 'world', 'forbidden-new-dir')), false)
})
test('R1 output source/header sibling is protected even before directory exists', async () => {
  const f = fixture(), out = join(dir, 'world', 'scenes', 's', 'forbidden-output.json')
  await boundaryCase(f, out, 'scene')
  assert.equal(existsSync(out), false)
})
test('R1 lexical world alias with .. is still protected', async () => {
  const f = fixture(), out = join(dir, 'uncreated', '..', 'world', 'lexical-result.json')
  await boundaryCase(f, out, 'lexical'); assert.equal(existsSync(out), false)
})
test('R1 junction ancestor into input tree cannot bypass output boundary', async t => {
  const f = fixture(), world = join(dir, 'world'), alias = join(dir, 'world-output-alias')
  mkdirSync(world, {recursive: true})
  try { symlinkSync(world, alias, 'junction') } catch (error) { t.skip(`junction unavailable: ${error.code}`); return }
  const out = join(alias, 'new-parent', 'result.json')
  await boundaryCase(f, out, 'junction')
  assert.equal(existsSync(join(world, 'new-parent')), false)
})
test('R1 output cannot equal an observation input, and input bytes stay intact', async () => {
  const f = fixture(), observationPath = join(dir, 'protected-observation.json')
  writeFileSync(observationPath, JSON.stringify(observations(f)))
  const before = readFileSync(observationPath)
  await boundaryCase(f, observationPath, 'observation', observationPath)
  assert.deepEqual(readFileSync(observationPath), before)
})
test('R1 unknown boundaries from malformed parity cause no output or directory creation', async () => {
  const f = fixture(), parityPath = join(dir, 'malformed-parity.json'), outPath = join(dir, 'unknown-parent', 'result.json')
  writeFileSync(parityPath, '{')
  const r = await runInspection({url: f.url, parityPath, outPath, fetcher: f.fetcher})
  assert.equal(r.result.outputWritten, false); assert.equal(existsSync(join(dir, 'unknown-parent')), false); assert.equal(f.calls.length, 0)
})
test('R1 valid work output records a passing full coverage result', async () => {
  const f = fixture(), parityPath = join(dir, 'full-parity.json'), observationPath = join(dir, 'full-observation.json'), outPath = join(dir, 'safe-work-output.json')
  writeFileSync(parityPath, JSON.stringify(f.parity)); writeFileSync(observationPath, JSON.stringify(observations(f)))
  const r = await runInspection({url: f.url, parityPath, observationPath, outPath, fetcher: f.fetcher})
  assertComponentPass(r.result); assert.equal(r.exitCode, 0); assert.equal(r.result.outputWritten, true)
  assert.equal(JSON.parse(readFileSync(outPath)).componentStatus, 'pass')
})
