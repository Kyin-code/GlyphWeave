// A-D2: executable all-chunk / same-page revisit inspection. No scene.json reads.
// Usage (ONLY in an authorized browser RUN slot):
// node check-preview-world.mjs --url http://localhost:PORT/preview/index.html --parity ABS.json --out ABS_NEW.json --browser
// Without --browser, verify compact HTTP bindings and emit incomplete.
// --observations ABS.json evaluates a separately collected observation receipt.
// Parity producer provenance is authenticated externally by the coordinator.
import {createHash} from 'node:crypto'
import {open, realpath} from 'node:fs/promises'
import {dirname, basename, resolve, relative, isAbsolute, sep} from 'node:path'
import {fileURLToPath} from 'node:url'

export const MAX_JSON_BYTES = 64 * 1024 * 1024
export const BUDGET = Object.freeze({activeChunks: 1, cacheEntries: 1, pending: 2,
  geometryGrowth: 8, textureGrowth: 4, heapGrowth: 64 * 1024 * 1024,
  heapLateGrowth: 8 * 1024 * 1024, stepMs: 120_000, rounds: 3})
export function boundedDeadline(start, planDeadlineMs, runtimeCapMs) {
  return Math.min(start + planDeadlineMs, runtimeCapMs > 0 ? start + runtimeCapMs : Infinity)
}
export class EvidenceError extends Error {
  constructor(id, reason, status = 'fail') { super(reason); this.id = id; this.status = status }
}
function need(ok, id, reason, status = 'fail') { if (!ok) throw new EvidenceError(id, reason, status) }
const object = v => v !== null && typeof v === 'object' && !Array.isArray(v)
const count = v => Number.isSafeInteger(v) && v >= 0
const hex = v => typeof v === 'string' && /^[0-9a-f]{64}$/i.test(v)
export const digest = bytes => createHash('sha256').update(bytes).digest('hex')
export function sha256Ids(ids) {
  need(Array.isArray(ids) && ids.every(id => typeof id === 'string' && id.length > 0), 'ids.shape', 'IDs must be nonempty strings')
  need(ids.length === new Set(ids).size, 'ids.duplicate', 'duplicate IDs cannot be removed before hashing')
  return digest(JSON.stringify([...ids].sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)))))
}
const keyOf = c => `${c.chunkX}:${c.chunkZ}`
const identity = (scene, key) => JSON.stringify([scene, key])
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b)
function binding(file, label, bytes = true) {
  need(object(file) && typeof file.path === 'string' && isAbsolute(file.path) && hex(file.sha256)
    && (!bytes || count(file.bytes)), 'parity.binding', `${label} needs absolute path, SHA256${bytes ? ' and bytes' : ''}`, 'incomplete')
}
function relativeArtifact(worldFile, artifact) {
  const rel = relative(dirname(worldFile), artifact)
  need(rel && !isAbsolute(rel) && rel !== '..' && !rel.startsWith('..' + sep), 'parity.path', 'source/header must be within the bound world directory')
  return rel.split(sep).join('/')
}
function httpPath(rootUrl, path) {
  need(typeof path === 'string' && path && !path.includes('\\') && !path.split('/').some(p => p === '..' || p === '.') && !/[:?#%]/.test(path), 'http.path', 'unsafe artifact path')
  return new URL(path.split('/').map(encodeURIComponent).join('/'), rootUrl).href
}
export function validateParity(parity) {
  need(object(parity) && parity.format === 'glyphweave.render-parity' && parity.version === 1,
    'parity.schema', 'requires current glyphweave.render-parity/v1; legacy unbound parity is insufficient', 'incomplete')
  need(parity.status === 'pass', 'parity.status', 'independent parity must report pass', parity.status === 'fail' ? 'fail' : 'incomplete')
  need(typeof parity.worldRevision === 'string' && parity.worldRevision.length > 0, 'parity.revision', 'worldRevision is missing', 'incomplete')
  binding(parity.worldFile, 'worldFile', false)
  need(Array.isArray(parity.scenes) && parity.scenes.length > 0, 'parity.scenes', 'parity scenes are missing', 'incomplete')
  const scenes = new Set()
  for (const s of parity.scenes) {
    need(object(s) && typeof s.sceneId === 'string' && s.sceneId && !scenes.has(s.sceneId), 'parity.scene-id', 'duplicate or invalid sceneId')
    scenes.add(s.sceneId)
    for (const f of ['widthM', 'depthM', 'chunkSizeM']) need(Number.isSafeInteger(s[f]) && s[f] > 0, 'parity.geometry', `invalid ${f}`)
    for (const f of ['originX', 'originZ']) need(Number.isSafeInteger(s[f]), 'parity.geometry', `invalid ${f}`)
    for (const f of ['source', 'header']) binding(s[f], `${s.sceneId}.${f}`)
    const src = relativeArtifact(parity.worldFile.path, s.source.path)
    const hdr = relativeArtifact(parity.worldFile.path, s.header.path)
    need(src.endsWith('/scene.json') && hdr === src.replace(/scene\.json$/, 'preview-scene.json'), 'parity.path', 'source/header must be scene siblings')
    need(count(s.uniquePlants) && count(s.uniquePatches), 'parity.counts', 'missing independent unique counts', 'incomplete')
    need(Array.isArray(s.nonEcologyEntityIds) && s.nonEcologyEntityIds.every(id => typeof id === 'string' && id.length > 0)
      && new Set(s.nonEcologyEntityIds).size === s.nonEcologyEntityIds.length,
      'parity.entity-scope', 'authoritative non-ecology entity IDs are missing or duplicated', 'incomplete')
    need(Array.isArray(s.naturalEntityIds) && s.naturalEntityIds.every(id => typeof id === 'string' && id.length > 0)
      && new Set(s.naturalEntityIds).size === s.naturalEntityIds.length,
      'parity.natural-entity-scope', 'authoritative natural entity IDs are missing or duplicated', 'incomplete')
    const nx = Math.ceil(s.widthM / s.chunkSizeM), nz = Math.ceil(s.depthM / s.chunkSizeM)
    need(Array.isArray(s.chunks) && s.chunks.length === nx * nz, 'parity.coverage', 'incomplete descriptor coverage')
    const seen = new Set()
    for (const c of s.chunks) {
      need(object(c) && count(c.chunkX) && count(c.chunkZ) && c.chunkX < nx && c.chunkZ < nz
        && c.key === keyOf(c) && !seen.has(c.key), 'parity.chunk-key', 'duplicate/out-of-range descriptor')
      seen.add(c.key)
      need(c.worldX === s.originX + c.chunkX * s.chunkSizeM && c.worldZ === s.originZ + c.chunkZ * s.chunkSizeM
        && c.validWidthM === Math.min(s.chunkSizeM, s.widthM - c.chunkX * s.chunkSizeM)
        && c.validDepthM === Math.min(s.chunkSizeM, s.depthM - c.chunkZ * s.chunkSizeM), 'parity.chunk-geometry', 'descriptor geometry/origin/partial chunk mismatch')
      need(count(c.bytes) && c.bytes > 0 && count(c.plants) && count(c.patches)
        && [c.sha256, c.plantIdsSha256, c.patchIdsSha256].every(hex), 'parity.chunk-binding', 'missing chunk/ID bindings', 'incomplete')
      need(count(c.nonMeshPatches) && c.nonMeshPatches <= c.patches && hex(c.nonMeshPatchIdsSha256)
        && count(c.accessSurfaces) && hex(c.accessSurfaceIdsSha256),
        'parity.render-bindings', 'nonMesh/accessSurfaces count and independent ID hashes are required', 'incomplete')
      need(count(c.entities) && Array.isArray(c.entityIds) && c.entityIds.length === c.entities
        && c.entityIds.every(id => typeof id === 'string' && id.length > 0)
        && new Set(c.entityIds).size === c.entityIds.length && hex(c.entityIdsSha256)
        && sha256Ids(c.entityIds) === c.entityIdsSha256.toLowerCase(),
        'parity.entity-bindings', 'per-chunk non-ecology entity IDs and digest are required', 'incomplete')
      need(count(c.naturalEntities) && Array.isArray(c.naturalEntityIds) && c.naturalEntityIds.length === c.naturalEntities
        && c.naturalEntityIds.every(id => typeof id === 'string' && id.length > 0)
        && new Set(c.naturalEntityIds).size === c.naturalEntityIds.length && hex(c.naturalEntityIdsSha256)
        && sha256Ids(c.naturalEntityIds) === c.naturalEntityIdsSha256.toLowerCase(),
        'parity.natural-entity-bindings', 'per-chunk natural entity IDs and digest are required', 'incomplete')
      need(typeof c.heightFile === 'string' && c.heightFile.length > 0 && !c.heightFile.includes('..')
        && count(c.heightBytes) && c.heightBytes > 0 && hex(c.heightSha256)
        && ['millimetre', 'quarter-metre'].includes(c.heightEncoding),
        'parity.height-binding', 'per-chunk final height file bytes/hash/encoding are required', 'incomplete')
      if (c.naturalHeightFile !== null && c.naturalHeightFile !== undefined) {
        need(typeof c.naturalHeightFile === 'string' && c.naturalHeightFile.length > 0 && !c.naturalHeightFile.includes('..')
          && count(c.naturalHeightBytes) && c.naturalHeightBytes > 0 && hex(c.naturalHeightSha256)
          && ['millimetre', 'quarter-metre'].includes(c.naturalHeightEncoding),
          'parity.natural-height-binding', 'natural height file bytes/hash/encoding are required when present', 'incomplete')
      }
      need(c.nonMeshPatches === 0 || c.accessSurfaces > 0, 'parity.access-coverage', 'non-mesh pavement needs at least one independently bound access surface')
      need(c.file === `render-chunks/${c.chunkX}-${c.chunkZ}-${c.sha256.toLowerCase()}.json`, 'parity.chunk-file', 'invalid content-addressed chunk path')
    }
    need(s.chunks.reduce((n, c) => n + c.plants, 0) === s.uniquePlants, 'parity.plant-count', 'cross-chunk plant totals disagree')
  }
  return parity
}
export function navigationPlan(parity) {
  validateParity(parity)
  const rows = parity.scenes.flatMap(s => [...s.chunks].sort((a, b) => a.chunkZ - b.chunkZ || a.chunkX - b.chunkX)
    .map(c => ({sceneId: s.sceneId, key: c.key,
      focus: {x: c.worldX - s.originX + c.validWidthM / 2, z: c.worldZ - s.originZ + c.validDepthM / 2}})))
  const critical = parity.scenes.flatMap(s => {
    const own = rows.filter(r => r.sceneId === s.sceneId)
    return [...new Set([0, Math.floor(own.length / 2), own.length - 1])].map(i => own[i])
  })
  return {coverage: rows, critical, totalDeadlineMs: rows.length * BUDGET.stepMs, budget: {...BUDGET}}
}
export async function readJsonLimited(path, max = MAX_JSON_BYTES) {
  const handle = await open(path, 'r')
  try {
    const info = await handle.stat()
    need(info.isFile() && info.size <= max, 'input.budget', 'input is not a small regular JSON file', 'incomplete')
    const buffer = Buffer.alloc(Math.min(info.size + 1, max + 1)); let offset = 0
    while (offset < buffer.length) { const r = await handle.read(buffer, offset, buffer.length - offset, null); if (!r.bytesRead) break; offset += r.bytesRead }
    need(offset === info.size, 'input.changed', 'input grew or truncated during bounded read')
    return {data: JSON.parse(buffer.subarray(0, offset).toString('utf8')), sha256: digest(buffer.subarray(0, offset))}
  } finally { await handle.close() }
}
async function httpBytes(url, fetcher, deadline) {
  const response = await fetcher(url, {signal: AbortSignal.timeout(Math.max(1, Math.min(BUDGET.stepMs, deadline - Date.now())))})
  need(response.ok, 'http.status', `HTTP ${response.status}: ${url}`)
  need(Number(response.headers.get('content-length') ?? 0) <= MAX_JSON_BYTES, 'http.budget', 'HTTP JSON exceeds read budget', 'incomplete')
  const pieces = []; let n = 0
  for await (const piece of response.body) { n += piece.length; need(n <= MAX_JSON_BYTES, 'http.budget', 'HTTP JSON stream exceeds budget', 'incomplete'); pieces.push(piece) }
  return Buffer.concat(pieces)
}
export async function verifyBindings(parity, pageUrl, fetcher = fetch) {
  validateParity(parity)
  const rootUrl = new URL('../', pageUrl).href
  const deadline = Date.now() + navigationPlan(parity).totalDeadlineMs
  const worldUrl = new URL('world.json', rootUrl).href
  const worldBytes = await httpBytes(worldUrl, fetcher, deadline)
  need(digest(worldBytes) === parity.worldFile.sha256.toLowerCase(), 'binding.world-hash', 'HTTP world bytes differ from parity')
  const world = JSON.parse(worldBytes)
  need(world.format === 'glyphweave-world' && world.version === 1 && world.revision === parity.worldRevision,
    'binding.world-identity', 'HTTP world format/version/revision mismatch')
  need(Array.isArray(world.scenes) && world.scenes.length === parity.scenes.length, 'binding.world-scenes', 'world scene coverage differs')
  const artifacts = [{url: worldUrl, sha256: digest(worldBytes), bytes: worldBytes.length}], headers = []
  for (const s of parity.scenes) {
    const src = relativeArtifact(parity.worldFile.path, s.source.path)
    need(world.scenes.filter(p => p === src).length === 1, 'binding.source-path', 'world source scene does not match parity source path')
    const headerUrl = httpPath(rootUrl, relativeArtifact(parity.worldFile.path, s.header.path))
    const bytes = await httpBytes(headerUrl, fetcher, deadline)
    need(bytes.length === s.header.bytes && digest(bytes) === s.header.sha256.toLowerCase(), 'binding.header-hash', 'HTTP preview header bytes differ from parity')
    const h = JSON.parse(bytes)
    for (const f of ['sceneId', 'widthM', 'depthM', 'originX', 'originZ', 'chunkSizeM']) need(h[f] === s[f], 'binding.header-geometry', `header ${f} differs`)
    need(h.renderStream?.format === 'glyphweave.render-directory' && h.renderStream.version === 1
      && h.renderStream.worldRevision === parity.worldRevision && h.renderStream.totalPlants === s.uniquePlants
      && h.renderStream.totalPatches === s.uniquePatches, 'binding.header-identity', 'header stream identity/counts differ')
    need(Array.isArray(h.chunks) && Array.isArray(h.renderStream.chunks) && h.chunks.length === s.chunks.length
      && h.renderStream.chunks.length === s.chunks.length, 'binding.header-coverage', 'header descriptor coverage differs')
    for (const [list, fields] of [[h.chunks, ['chunkX', 'chunkZ', 'worldX', 'worldZ', 'validWidthM', 'validDepthM']],
      [h.renderStream.chunks, ['file', 'sha256', 'bytes']]]) {
      const keys = new Set()
      for (const d of list) {
        const c = s.chunks.find(c => c.key === keyOf(d))
        need(c && !keys.has(c.key), 'binding.descriptor-duplicate', 'unknown or duplicate header descriptor'); keys.add(c.key)
        for (const f of fields) need(d[f] === c[f], 'binding.descriptor', `header ${keyOf(d)} ${f} differs from independent parity`)
        if (list === h.renderStream.chunks) need(d.plantCount === c.plants && d.patchCount === c.patches, 'binding.descriptor-count', 'header patch/plant counts differ')
      }
    }
    need(Array.isArray(h.accessSurfaces), 'binding.access-surfaces', 'header access surface array missing', 'incomplete')
    for (const c of s.chunks) {
      const ids = h.accessSurfaces.filter(surface => {
        need(typeof surface.entityId === 'string' && typeof surface.anchorId === 'string'
          && Array.isArray(surface.vertices) && surface.vertices.length > 0
          && surface.vertices.every(v => Array.isArray(v) && v.length === 3 && v.every(Number.isFinite)),
          'binding.access-geometry', 'invalid access surface source geometry')
        const bounds = surface.vertices.reduce((b, v) => [Math.min(b[0], v[0]), Math.min(b[1], v[2]), Math.max(b[2], v[0]), Math.max(b[3], v[2])], [Infinity, Infinity, -Infinity, -Infinity])
        return bounds[0] <= c.worldX + c.validWidthM && bounds[2] >= c.worldX
          && bounds[1] <= c.worldZ + c.validDepthM && bounds[3] >= c.worldZ
      }).map(surface => 'access:' + JSON.stringify([s.sceneId, surface.entityId, surface.anchorId]))
      need(ids.length === c.accessSurfaces && sha256Ids(ids) === c.accessSurfaceIdsSha256.toLowerCase(),
        'binding.access-ids', 'header access surface subset differs from independent parity')
    }
    artifacts.push({url: headerUrl, sha256: digest(bytes), bytes: bytes.length})
    headers.push({sceneId: s.sceneId, index: world.scenes.indexOf(src), url: headerUrl})
  }
  return {format: 'glyphweave.preview-bindings', version: 1, status: 'pass', worldRevision: parity.worldRevision, rootUrl, artifacts, headers}
}
function resultError(error) { return {id: error.id ?? 'unexpected.error', status: error.status ?? 'fail', reason: String(error.message || error)} }
const coverageFlags = (entityCoverage = false, heightCoverage = false) => ({
  entityCoverage: entityCoverage ? 'pass' : 'unverified',
  heightCoverage: heightCoverage ? 'pass' : 'unverified',
})
const coverageChecks = (entityCoverage = false, heightCoverage = false) => [
  entityCoverage
    ? {id: 'coverage.entities', status: 'pass', reason: 'authoritative non-ecology and natural entity IDs are present in actual browser scene-graph/instance observations in every ready phase'}
    : {id: 'coverage.entities', status: 'incomplete', reason: 'non-ecology entity representations are not independently verified'},
  heightCoverage
    ? {id: 'coverage.heights', status: 'pass', reason: 'browser height requests match independent per-chunk bytes/hash/encoding'}
    : {id: 'coverage.heights', status: 'incomplete', reason: 'final height transport/source coverage is not independently verified'},
]
const finiteTime = v => typeof v === 'number' && Number.isFinite(v) && v >= 0
function validateFaults(parity, evidence) {
  for (const [kind, phase] of [['networkRetry', 'retry'], ['lateResponse', 'late']]) {
    const f = evidence.faults?.[kind]
    need(f?.status === 'pass', 'faults.missing', `${kind} exercise missing`, 'incomplete')
    need(f.version === 1 && f.phase === phase && f.pageId === evidence.pageId
      && f.worldRevision === parity.worldRevision && object(f.target) && object(f.recovery),
      'faults.structure', `${kind} lacks versioned page/source/target evidence`, 'incomplete')
    const s = parity.scenes.find(s => s.sceneId === f.target.sceneId)
    const c = s?.chunks.find(c => c.key === f.target.key)
    need(c && f.target.url === new URL(c.file, httpPath(evidence.rootUrl, relativeArtifact(parity.worldFile.path, s.header.path))).href,
      'faults.url', 'injected URL is not the bound scene/chunk payload')
    need(count(f.oldToken) && count(f.newToken) && f.newToken > f.oldToken, 'faults.tokens', 'fault tokens are not ordered')
    const final = evidence.views.find(v => v.token === f.newToken && v.phase === phase)
    need(final && final.sceneId === f.recovery.sceneId && final.key === f.recovery.key
      && final.completedGeneration === f.newToken && final.status === 'ready', 'faults.tokens', 'fault recovery lacks corresponding fresh final observation')
    const ordered = kind === 'networkRetry'
      ? [f.injectedAtMs, f.failedAtMs, f.newRequestedAtMs, f.completedAtMs]
      : [f.injectedAtMs, f.heldAtMs, f.newRequestedAtMs, f.releasedAtMs, f.deliveredAtMs, f.completedAtMs]
    need(ordered.every(finiteTime) && ordered.every(t => t <= evidence.elapsedMs)
      && ordered.every((t, i) => i === 0 || ordered[i - 1] <= t)
      && f.completedAtMs === final.observedAtMs && f.completedAtMs - f.injectedAtMs <= BUDGET.stepMs,
      'faults.time', `${kind} has missing, negative, out-of-order or over-budget event times`)
    if (kind === 'networkRetry') {
      need(typeof f.diagnostic === 'string' && f.diagnostic.trim().length > 0, 'faults.diagnostic', 'network failure needs its actual diagnostic')
      const failed = f.failedState
      need(failed?.status === 'failed' && failed.requestedGeneration === f.oldToken && failed.error === f.diagnostic
        && failed.pageId === f.pageId && failed.worldRevision === f.worldRevision && failed.sceneId === s.sceneId
        && failed.observedAtMs === f.failedAtMs && f.recovery.sceneId === s.sceneId && f.recovery.key === c.key,
        'faults.failed-state', 'network failure/retry must refer to the injected focus and actual failed generation')
    } else {
      need(f.recovery.sceneId !== s.sceneId || f.recovery.key !== c.key, 'faults.new-focus', 'late response must be superseded by a different focus')
      need(f.delivery?.requestFinished === true && f.delivery.url === f.target.url && f.delivery.status === 200
        && f.delivery.bytes === c.bytes && f.delivery.sha256 === c.sha256.toLowerCase(),
        'faults.delivery', 'late payload must be hash-bound, released and actually delivered before the final observation')
    }
  }
}

// Resolve existing ancestors as well as lexical names, so a not-yet-created
// output beneath a junction cannot bypass the protected input directory tree.
async function prospectiveRealpath(path) {
  let existing = resolve(path)
  const tail = []
  while (true) {
    try { return resolve(await realpath(existing), ...tail.reverse()) } catch (error) {
      if (error.code !== 'ENOENT') throw error
      const parent = dirname(existing)
      if (parent === existing) throw error
      tail.push(basename(existing)); existing = parent
    }
  }
}
function inside(child, parent) {
  const rel = relative(parent, child)
  return rel === '' || (!isAbsolute(rel) && rel !== '..' && !rel.startsWith('..' + sep))
}
export async function validateOutputPath(parity, outPath, inputs = []) {
  validateParity(parity) // unknown input boundaries must never result in writes
  const roots = [dirname(parity.worldFile.path), ...parity.scenes.flatMap(s => [dirname(s.source.path), dirname(s.header.path)])]
  const files = [parity.worldFile.path, ...parity.scenes.flatMap(s => [s.source.path, s.header.path]), ...inputs.filter(Boolean)]
  const output = resolve(outPath), canonicalOutput = await prospectiveRealpath(output)
  for (const path of [...roots, ...files]) {
    const lexical = resolve(path), canonical = await prospectiveRealpath(path)
    need(!inside(output, lexical) && !inside(canonicalOutput, canonical), 'output.protected', `refusing output within protected input: ${path}`)
  }
}

export function evaluateEvidence(parity, evidence) {
  const checks = [], unverified = ['Parity producer provenance is external; no semantic quality-gate verdict.', 'GPU byte allocation is not exposed; only actual renderer object counts are checked.']
  let entityCoverage = false, heightCoverage = false
  try {
    const plan = navigationPlan(parity)
    need(object(evidence) && evidence.format === 'glyphweave.preview-traversal' && evidence.version === 1, 'observation.schema', 'versioned browser traversal is missing', 'incomplete')
    need(same(evidence.budget, {...BUDGET}) && evidence.totalDeadlineMs === plan.totalDeadlineMs, 'budget.changed', 'frozen budgets may not be widened')
    need(typeof evidence.rootUrl === 'string' && /^https?:$/.test(new URL(evidence.rootUrl).protocol), 'observation.root', 'HTTP world root missing', 'incomplete')
    need(evidence.worldRevision === parity.worldRevision && evidence.worldSha256 === parity.worldFile.sha256.toLowerCase(), 'observation.binding', 'browser traversal is not bound to current world')
    need(Array.isArray(evidence.errors) && evidence.errors.length === 0, 'browser.errors', 'browser/HTTP/content errors recorded')
    need(evidence.status === 'complete', 'browser.state', 'browser traversal did not complete', 'incomplete')
    need(finiteTime(evidence.elapsedMs) && evidence.elapsedMs <= plan.totalDeadlineMs, 'budget.total', 'global fault deadline exceeded')
    need(typeof evidence.pageId === 'string' && evidence.pageId.length > 0 && Array.isArray(evidence.views), 'observation.page', 'same-page navigation evidence missing', 'incomplete')
    const coverage = new Set(), plants = new Map(), patches = new Map(), visits = new Map()
    let previousToken = -1
    for (const v of evidence.views) {
      const s = parity.scenes.find(s => s.sceneId === v.sceneId), c = s?.chunks.find(c => c.key === v.key)
      need(c, 'view.identity', 'unknown scene/chunk view')
      need(v.format === 'glyphweave.preview-observation' && v.version === 1, 'view.schema', 'versioned browser observation required', 'incomplete')
      need(v.pageId === evidence.pageId && v.worldRevision === parity.worldRevision, 'view.page', 'reload or stale world observed')
      need(v.status === 'ready' && count(v.requestedGeneration) && v.requestedGeneration > previousToken
        && v.completedGeneration === v.requestedGeneration && v.token === v.requestedGeneration, 'view.generation', 'stale feedback or unfinished navigation')
      previousToken = v.requestedGeneration
      const expected = plan.coverage.find(r => r.sceneId === v.sceneId && r.key === v.key)
      need(same(v.focus, expected.focus), 'view.focus', 'focus must be descriptor center in scene-local coordinates')
      need(Number.isFinite(v.elapsedMs) && v.elapsedMs >= 0 && v.elapsedMs <= BUDGET.stepMs, 'budget.step', 'navigation exceeds 120s')
      need(finiteTime(v.observedAtMs) && v.observedAtMs <= evidence.elapsedMs && v.elapsedMs <= v.observedAtMs, 'view.time', 'invalid or out-of-run observation timestamp')
      need(v.highDetailChunk === v.key.replace(':', ',') && same(v.loadedChunks, [v.highDetailChunk]), 'view.active', 'exactly one current high-detail chunk required')
      const stream = v.stream, payload = v.payload
      need(stream?.version === 1 && same(stream.loadedChunks, [v.key]) && stream.cacheEntries === 1
        && stream.residentBytes === c.bytes && stream.pending === 0 && count(stream.peakPending) && stream.peakPending <= BUDGET.pending,
        'budget.stream', 'cache/resident/loaded/pending violates fixed 1/1/bytes/2 contract')
      need(payload?.loadedPlants === c.plants && payload.loadedPatches === c.patches && payload.residentBytes === c.bytes,
        'view.counts', 'loaded payload counts/bytes disagree with parity')
      need(sha256Ids(v.plantIds) === c.plantIdsSha256.toLowerCase() && v.plantIds.length === c.plants, 'view.plants', 'actual instance IDs/count differ from independent parity')
      need(sha256Ids(v.patchIds) === c.patchIdsSha256.toLowerCase() && v.patchIds.length === c.patches, 'view.patches', 'loaded patch IDs/count differ from independent parity')
      need(Array.isArray(v.nonMeshPatchIds) && v.nonMeshPatchIds.length === c.nonMeshPatches
        && sha256Ids(v.nonMeshPatchIds) === c.nonMeshPatchIdsSha256.toLowerCase(), 'view.non-mesh', 'non-mesh exceptions differ from independent parity')
      need(Array.isArray(v.accessSurfaceIds) && v.accessSurfaceIds.length === c.accessSurfaces
        && sha256Ids(v.accessSurfaceIds) === c.accessSurfaceIdsSha256.toLowerCase(), 'view.access-surfaces', 'actual access mesh IDs/count differ from independent parity')
      need(Array.isArray(v.renderedEntityInstanceSources), 'view.entity-source-observation',
        'browser rendered-entity source records are missing', 'incomplete')
      const sourceIds = new Set()
      for (const [sourceIndex, entry] of v.renderedEntityInstanceSources.entries()) {
        need(object(entry) && typeof entry.id === 'string' && entry.id.length > 0 && entry.valid === true,
          'view.entity-source-invalid', `entity source ${sourceIndex} is not backed by drawable geometry: ${entry?.reason ?? 'invalid'}`)
        need(!sourceIds.has(entry.id), 'view.entity-duplicate-source', `duplicate logical entity root ${entry.id}`)
        sourceIds.add(entry.id)
        if (entry.source === 'object') {
          need(typeof entry.objectUuid === 'string' && count(entry.meshCount) && entry.meshCount > 0
            && count(entry.geometryCount) && entry.geometryCount > 0,
            'view.entity-object-geometry', `object source ${entry.id} has no drawable geometry`)
        } else {
          need(entry.source === 'instances', 'view.entity-source-kind',
            `entity source ${entry.id} has unknown source kind`)
          need(count(entry.instanceIndex) && count(entry.count) && entry.instanceIndex < entry.count,
            'view.entity-instance-index', `instance source ${entry.id} index is outside count`)
          need(Array.isArray(entry.parts) && entry.parts.length > 0, 'view.entity-instance-parts',
            `instance source ${entry.id} has no actual batch parts`)
          for (const part of entry.parts) {
            need(object(part) && typeof part.batchUuid === 'string' && typeof part.geometryUuid === 'string'
              && count(part.instanceIndex) && count(part.instanceCount) && part.instanceIndex < part.instanceCount,
              'view.entity-instance-part', `instance source ${entry.id} has an invalid batch/index binding`)
          }
        }
      }
      need(Array.isArray(v.renderedEntityInstanceIds) && v.renderedEntityInstanceIds.every(id => typeof id === 'string' && id.length > 0)
        && same([...sourceIds].sort(), [...new Set(v.renderedEntityInstanceIds)].sort()),
        'view.entity-observation', 'browser rendered-entity instance IDs do not match source records', 'incomplete')
      need(Array.isArray(v.renderedEntityIds) && v.renderedEntityIds.every(id => typeof id === 'string' && id.length > 0)
        && same([...sourceIds].sort(), v.renderedEntityIds),
        'view.entity-instance-binding', 'rendered entity IDs must be the unique actual scene-graph/instance IDs', 'incomplete')
      const renderedEntities = new Set(v.renderedEntityIds)
      const authoritativeEntities = new Set([...s.nonEcologyEntityIds, ...s.naturalEntityIds])
      for (const id of c.entityIds) need(renderedEntities.has(id), 'view.entity-missing', `authoritative entity ${id} has no rendered representation in ${v.key}`)
      for (const id of c.naturalEntityIds) need(renderedEntities.has(id), 'view.natural-entity-missing', `authoritative natural entity ${id} has no rendered instance in ${v.key}`)
      for (const id of renderedEntities) need(authoritativeEntities.has(id), 'view.entity-unknown', `rendered entity ${id} is not authoritative in scene ${v.sceneId}`)
      const headerUrl = new URL(relativeArtifact(parity.worldFile.path, s.header.path), evidence.rootUrl).href
      const expectedHeightUrl = new URL(c.heightFile, headerUrl).href
      need(v.heightSource && v.heightSource.url === expectedHeightUrl && v.heightSource.bytes === c.heightBytes
        && v.heightSource.sha256 === c.heightSha256.toLowerCase(), 'view.height-source',
      `chunk ${v.key} did not request the independently bound final height source`)
      need(sha256Ids([...v.renderedPatchIds, ...v.nonMeshPatchIds]) === c.patchIdsSha256.toLowerCase(), 'view.patch-mesh', 'patch meshes plus declared access-pavement accounting differ')
      need(v.ecology?.activePlants === c.plants && count(v.ecology.builds) && count(v.ecology.evictions), 'view.ecology', 'ecology instance lifecycle observation missing', 'incomplete')
      need(count(v.caches?.terrain) && v.caches.terrain <= 1 && count(v.caches.entityMeshes) && count(v.caches.entityGroups), 'view.caches', 'terrain/entity caches missing or unbounded')
      for (const f of ['geometries', 'textures', 'drawCalls']) need(count(v.renderer?.[f]), 'metrics.renderer', `actual renderer ${f} unavailable`, 'incomplete')
      for (const f of ['geometries', 'materials', 'textures']) {
        need(count(v.resources?.created?.[f]) && count(v.resources?.released?.[f]) && v.resources.created[f] >= v.resources.released[f], 'metrics.lifecycle', 'resource creation/release counters unavailable', 'incomplete')
      }
      const k = identity(v.sceneId, v.key)
      if (v.phase === 'coverage') {
        need(!coverage.has(k), 'coverage.duplicate', 'duplicate coverage view cannot replace a missing chunk'); coverage.add(k)
        const owned = plants.get(s.sceneId) ?? new Set(), seenPatches = patches.get(s.sceneId) ?? new Set()
        for (const id of v.plantIds) { need(!owned.has(id), 'coverage.duplicate-plant', 'plant ID appears in more than one chunk'); owned.add(id) }
        v.patchIds.forEach(id => seenPatches.add(id)); plants.set(s.sceneId, owned); patches.set(s.sceneId, seenPatches)
      } else if (v.phase === 'warm' || v.phase === 'revisit') {
        const own = visits.get(k) ?? []
        need(v.round === own.length && v.round <= BUDGET.rounds && v.phase === (v.round === 0 ? 'warm' : 'revisit'), 'revisit.order', 'warm and three ordered rounds required')
        own.push(v); visits.set(k, own)
      } else need(v.phase === 'retry' || v.phase === 'late', 'view.phase', 'unknown observation phase')
    }
    need(coverage.size === plan.coverage.length, 'coverage.missing', `coverage ${coverage.size}/${plan.coverage.length}`, 'incomplete')
    for (const s of parity.scenes) {
      need(plants.get(s.sceneId)?.size === s.uniquePlants && patches.get(s.sceneId)?.size === s.uniquePatches, 'coverage.unique', 'missing or duplicate global plants/patches')
    }
    entityCoverage = true
    heightCoverage = true
    for (const r of plan.critical) {
      const rows = visits.get(identity(r.sceneId, r.key))
      need(rows?.length === 4, 'revisit.missing', 'each critical focus needs warm + three revisits', 'incomplete')
      for (const metric of ['geometries', 'textures']) {
        const n = rows.map(v => v.renderer[metric]), limit = metric === 'geometries' ? BUDGET.geometryGrowth : BUDGET.textureGrowth
        need(n.slice(1).every(x => x - n[0] <= limit), 'budget.gpu-growth', `${metric} exceeds fixed revisit growth`)
        need(!(n[1] > n[0] && n[2] > n[1] && n[3] > n[2]), 'budget.gpu-monotonic', `${metric} grows on every revisit`)
      }
      if (!rows.every(v => count(v.heap?.usedBytes) && v.heap.source === 'CDP.Runtime.getHeapUsage' && v.heap.gc === true)) {
        checks.push({id: 'metrics.heap', status: 'incomplete', reason: 'comparable post-GC CDP heap measurements unavailable'})
        unverified.push('JS heap growth not verified; no zero substitution.')
      } else {
        const h = rows.map(v => v.heap.usedBytes)
        need(h.slice(1).every(n => n - h[0] <= BUDGET.heapGrowth), 'budget.heap', 'heap increase exceeds warm baseline +64MiB')
        need(h[3] - h[2] <= BUDGET.heapLateGrowth, 'budget.heap-late', 'last two rounds grow by more than 8MiB')
      }
    }
    validateFaults(parity, evidence)
    checks.push({id: 'coverage-and-budgets', status: 'pass', reason: `${coverage.size} chunks, three same-page revisits, entity/height source identities and fixed budgets verified`})
  } catch (error) { checks.push(resultError(error)) }
  const componentStatus = checks.some(c => c.status === 'fail') ? 'fail' : checks.some(c => c.status === 'incomplete') ? 'incomplete' : 'pass'
  return {status: componentStatus === 'fail' ? 'fail' : componentStatus === 'incomplete' ? 'incomplete' : 'pass', componentStatus,
    coverageFlags: coverageFlags(entityCoverage, heightCoverage), checks: [...checks, ...coverageChecks(entityCoverage, heightCoverage)],
    unverified}
}

// Launched only with explicit --browser after the coordinator grants a RUN slot.
// Dynamic import keeps Node unit tests and compact binding checks browser-free.
export async function collectBrowser(parity, pageUrl, bindings) {
  const {chromium} = await import('playwright')
  const plan = navigationPlan(parity), start = Date.now()
  const runtimeCapMs = Number(process.env.PREVIEW_MAX_RUNTIME_MS || 0)
  const deadline = boundedDeadline(start, plan.totalDeadlineMs, runtimeCapMs)
  const evidence = {format: 'glyphweave.preview-traversal', version: 1, status: 'running', budget: {...BUDGET},
    totalDeadlineMs: plan.totalDeadlineMs, worldRevision: parity.worldRevision,
    runtimeCapMs: runtimeCapMs > 0 ? runtimeCapMs : null,
    worldSha256: parity.worldFile.sha256.toLowerCase(), rootUrl: bindings.rootUrl, pageId: null, views: [], faults: {}, errors: [], elapsedMs: 0}
  const browser = await chromium.launch({headless: true, channel: process.env.PREVIEW_BROWSER_CHANNEL || 'msedge', timeout: BUDGET.stepMs})
  let releaseLate = null
  const watchdog = setTimeout(() => {
    evidence.errors.push({id: runtimeCapMs > 0 && deadline < start + plan.totalDeadlineMs ? 'budget.runtime-cap' : 'budget.total',
      status: 'fail', reason: 'inspection deadline reached; closing browser'})
    void browser.close().catch(error => evidence.errors.push(resultError(error)))
  }, Math.max(1, deadline - Date.now()))
  watchdog.unref()
  try {
    const page = await browser.newPage({viewport: {width: 1440, height: 1000}})
    const cdp = await page.context().newCDPSession(page)
    const boundUrls = new Map(bindings.artifacts.map(a => [a.url, a]))
    const heightSources = new Map(), observedHeights = new Map()
    for (const scene of parity.scenes) {
      const header = bindings.headers.find(h => h.sceneId === scene.sceneId)
      for (const chunk of scene.chunks) heightSources.set(new URL(chunk.heightFile, header.url).href, chunk)
    }
    let fault = null
    const allowedFailure = new Set(), lateRequests = new Map()
    page.on('requestfinished', request => {
      const exercise = lateRequests.get(request)
      if (exercise) {
        exercise.deliveredAtMs = Date.now() - start
        exercise.delivery.requestFinished = true
        lateRequests.delete(request); exercise.delivered()
      }
    })
    page.on('pageerror', e => evidence.errors.push({type: 'pageerror', message: String(e)}))
    page.on('requestfailed', r => {
      if (!allowedFailure.has(r.url())) evidence.errors.push({type: 'requestfailed', url: r.url(), error: r.failure()?.errorText})
      else allowedFailure.delete(r.url())
    })
    page.on('response', response => {
      if (response.status() >= 400) evidence.errors.push({type: 'http', url: response.url(), status: response.status()})
    })
    await page.route('**/*', async route => {
      const url = route.request().url()
      if (new URL(url).pathname.endsWith('/scene.json')) {
        evidence.errors.push({type: 'forbidden-full-scene', url}); await route.abort(); return
      }
      if (heightSources.has(url)) {
        try {
          const response = await route.fetch({timeout: remaining()})
          need(response.ok() && Number(response.headers()['content-length'] ?? 0) <= MAX_JSON_BYTES,
            'height.http', 'final height response is unavailable or oversized')
          const bytes = await response.body(), expected = heightSources.get(url)
          need(bytes.length === expected.heightBytes && digest(bytes) === expected.heightSha256.toLowerCase(),
            'height.binding', 'browser height bytes differ from independent parity')
          observedHeights.set(url, {url, bytes: bytes.length, sha256: digest(bytes)})
          await route.fulfill({response, body: bytes})
        } catch (error) { evidence.errors.push(resultError(error)); await route.abort() }
        return
      }
      if (fault?.url === url) {
        const exercise = fault; fault = null
        exercise.injectedAtMs = Date.now() - start
        if (exercise.type === 'fail') { exercise.started(); allowedFailure.add(url); await route.abort('failed'); return }
        try {
          const response = await route.fetch({timeout: remaining()})
          need(response.status() === 200 && Number(response.headers()['content-length'] ?? 0) <= MAX_JSON_BYTES,
            'faults.response', 'delayed chunk response is not a bounded HTTP 200')
          const bytes = await response.body()
          need(bytes.length === exercise.descriptor.bytes && digest(bytes) === exercise.descriptor.sha256.toLowerCase(),
            'faults.response', 'delayed payload differs from bound descriptor')
          exercise.delivery = {url, status: response.status(), bytes: bytes.length, sha256: digest(bytes), requestFinished: false}
          exercise.heldAtMs = Date.now() - start; exercise.started()
          await exercise.wait
          lateRequests.set(route.request(), exercise)
          await route.fulfill({response, body: bytes})
        } catch (error) {
          evidence.errors.push(resultError(error)); exercise.started(); exercise.delivered(); await route.abort()
        }
        return
      }
      if (boundUrls.has(url)) {
        try {
          const response = await route.fetch({timeout: remaining()})
          const expected = boundUrls.get(url)
          need(response.ok() && Number(response.headers()['content-length'] ?? 0) <= MAX_JSON_BYTES, 'browser.binding-http', 'bound browser response is unavailable or oversized')
          const bytes = await response.body()
          need(bytes.length === expected.bytes && digest(bytes) === expected.sha256, 'browser.binding', 'browser world/header bytes changed after preflight')
          await route.fulfill({response, body: bytes})
        } catch (error) { evidence.errors.push(resultError(error)); await route.abort() }
      } else await route.continue()
    })
    function remaining(stepStart = Date.now()) {
      const time = Math.min(deadline - Date.now(), BUDGET.stepMs - (Date.now() - stepStart))
      need(time > 0, 'budget.deadline', 'inspection deadline exhausted')
      return time
    }
    async function waitState(token, key, wanted, stepStart) {
      await page.waitForFunction(({token, key, wanted}) => {
        const s = window.__previewStats?.()
        return s && s.requestedGeneration === token && s.status === wanted
          && (wanted !== 'ready' || (s.completedGeneration === token && s.highDetailChunk === key.replace(':', ',') && s.stream?.pending === 0))
      }, {token, key, wanted}, {timeout: remaining(stepStart)})
    }
    async function selectScene(sceneId) {
      const header = bindings.headers.find(h => h.sceneId === sceneId)
      const current = await page.evaluate(() => window.__previewStats?.().sceneId)
      if (current !== sceneId) {
        await page.selectOption('#scene', {index: header.index})
        await page.waitForFunction(id => window.__previewStats?.().sceneId === id && window.__previewStats?.().status === 'ready', sceneId, {timeout: remaining()})
      }
    }
    async function focus(row) {
      await selectScene(row.sceneId)
      return page.evaluate(f => window.__focusAt(f.x, f.z), row.focus)
    }
    async function sample(row, phase, round, token, stepStart) {
      await waitState(token, row.key, 'ready', stepStart)
      let heap = null
      try {
        await cdp.send('HeapProfiler.collectGarbage')
        const h = await cdp.send('Runtime.getHeapUsage')
        heap = {usedBytes: h.usedSize, source: 'CDP.Runtime.getHeapUsage', gc: true}
      } catch { /* explicitly unverified below; never synthesize zero */ }
      const v = await page.evaluate(() => window.__previewStats())
      if (evidence.pageId === null) evidence.pageId = v.pageId
      const scene = parity.scenes.find(s => s.sceneId === row.sceneId)
      const chunk = scene.chunks.find(c => c.key === row.key)
      const header = bindings.headers.find(h => h.sceneId === row.sceneId)
      const heightSource = observedHeights.get(new URL(chunk.heightFile, header.url).href) ?? null
      const observation = {...v, ...row, phase, round, token, heap: heap ?? v.heap,
        heightSource, elapsedMs: Date.now() - stepStart, observedAtMs: Date.now() - start}
      evidence.views.push(observation)
      return observation
    }
    async function visit(row, phase, round = null) {
      const stepStart = Date.now(), token = await focus(row)
      await sample(row, phase, round, token, stepStart)
    }
    const target = new URL(pageUrl)
    target.searchParams.set('view', 'near'); target.searchParams.set('ring', '0'); target.searchParams.set('nofx', '1')
    await page.addInitScript(f => { window.__nearFocusOverride = f }, plan.coverage[0].focus)
    await page.goto(target.href, {waitUntil: 'domcontentloaded', timeout: remaining()})
    await page.waitForFunction(() => typeof window.__focusAt === 'function' && window.__previewStats?.().status === 'ready', null, {timeout: remaining()})
    for (const row of plan.coverage) await visit(row, 'coverage')
    for (const row of plan.critical) await visit(row, 'warm', 0)
    for (let round = 1; round <= BUDGET.rounds; round++) for (const row of plan.critical) await visit(row, 'revisit', round)
    // Exercise one real network failure and a held old response on this page.
    const firstSceneRows = plan.coverage.filter(r => r.sceneId === plan.coverage[0].sceneId)
    if (firstSceneRows.length < 2) {
      evidence.faults = {networkRetry: {status: 'incomplete'}, lateResponse: {status: 'incomplete'}}
    } else {
      const a = firstSceneRows[0], b = firstSceneRows.at(-1)
      const scene = parity.scenes.find(s => s.sceneId === a.sceneId)
      const header = bindings.headers.find(h => h.sceneId === a.sceneId)
      const url = new URL(scene.chunks.find(c => c.key === a.key).file, header.url).href
      await visit(b, 'retry') // evict A before injection
      async function until(promise, stepStart) {
        let timer
        try {
          return await Promise.race([promise, new Promise((_, reject) => {
            timer = setTimeout(() => reject(new EvidenceError('faults.deadline', 'fault injection/delivery deadline exceeded')), remaining(stepStart))
          })])
        } finally { clearTimeout(timer) }
      }
      let started
      let startSignal = new Promise(r => { started = r })
      const failure = {type: 'fail', url, started}
      fault = failure
      const failedStart = Date.now(), oldToken = await focus(a)
      await until(startSignal, failedStart)
      await waitState(oldToken, a.key, 'failed', failedStart)
      const failedState = await page.evaluate(() => window.__previewStats())
      failedState.observedAtMs = Date.now() - start
      need(typeof failedState.error === 'string' && failedState.error.length > 0, 'fault.network', 'network failure lacked a diagnostic')
      const retryStart = Date.now(), retryRequestedAt = Date.now() - start, newToken = await focus(a)
      const retryFinal = await sample(a, 'retry', null, newToken, retryStart)
      evidence.faults.networkRetry = {version: 1, status: 'pass', pageId: evidence.pageId, worldRevision: parity.worldRevision,
        target: {sceneId: a.sceneId, key: a.key, url}, recovery: {sceneId: a.sceneId, key: a.key},
        oldToken, newToken, phase: 'retry', diagnostic: failedState.error, failedState,
        injectedAtMs: failure.injectedAtMs, failedAtMs: failedState.observedAtMs,
        newRequestedAtMs: retryRequestedAt, completedAtMs: retryFinal.observedAtMs}
      await visit(b, 'late')
      startSignal = new Promise(r => { started = r })
      const wait = new Promise(r => { releaseLate = r })
      let delivered
      const deliverySignal = new Promise(r => { delivered = r })
      const delayed = {type: 'late', url, started, wait, delivered, descriptor: scene.chunks.find(c => c.key === a.key)}
      fault = delayed
      const lateStart = Date.now(), delayedToken = await focus(a)
      await until(startSignal, lateStart)
      const newRequestedAtMs = Date.now() - start, currentToken = await focus(b)
      const releasedAtMs = Date.now() - start
      releaseLate(); releaseLate = null
      // requestfinished confirms the held response arrived, not merely that
      // route.continue/fulfill was called. Only then observe the latest focus.
      await until(deliverySignal, lateStart)
      const lateFinal = await sample(b, 'late', null, currentToken, lateStart)
      evidence.faults.lateResponse = {version: 1, status: 'pass', pageId: evidence.pageId, worldRevision: parity.worldRevision,
        target: {sceneId: a.sceneId, key: a.key, url}, recovery: {sceneId: b.sceneId, key: b.key},
        oldToken: delayedToken, newToken: currentToken, phase: 'late',
        injectedAtMs: delayed.injectedAtMs, heldAtMs: delayed.heldAtMs, newRequestedAtMs, releasedAtMs,
        deliveredAtMs: delayed.deliveredAtMs, completedAtMs: lateFinal.observedAtMs, delivery: delayed.delivery}
    }
    evidence.status = 'complete'
  } catch (error) { evidence.errors.push(resultError(error)); evidence.status = 'failed' }
  finally { clearTimeout(watchdog); releaseLate?.(); await browser.close(); evidence.elapsedMs = Date.now() - start }
  return evidence
}
export async function runInspection({url, parityPath, outPath, browser = false, observationPath = null, fetcher = fetch} = {}) {
  need(url && parityPath && outPath && !(browser && observationPath), 'cli.usage', '--url --parity --out required; choose --browser OR --observations')
  need(isAbsolute(parityPath) && isAbsolute(outPath) && (!observationPath || isAbsolute(observationPath)), 'cli.paths', 'CLI evidence paths must be absolute')
  let input
  try {
    try { input = await readJsonLimited(parityPath) } catch (error) {
      if (error.code === 'ENOENT') throw new EvidenceError('parity.missing', 'current independent parity file is missing', 'incomplete')
      throw error
    }
    await validateOutputPath(input.data, outPath, [parityPath, observationPath])
  } catch (error) {
    const check = resultError(error)
    return {result: {format: 'glyphweave.preview-world-check', version: 1, status: check.status,
      componentStatus: check.status, coverageFlags: coverageFlags(), checks: [check], outputWritten: false,
      worldQualityStatus: 'not_run', runningHandles: []}, exitCode: 1}
  }
  const output = await open(outPath, 'wx') // after lexical + existing-ancestor realpath admission
  let result
  try {
    const {data: parity, sha256} = input
    const bindings = await verifyBindings(parity, url, fetcher)
    const observation = browser ? await collectBrowser(parity, url, bindings)
      : observationPath ? (await readJsonLimited(observationPath)).data : null
    if (observation) need(observation.rootUrl === bindings.rootUrl, 'observation.root-binding', 'observation HTTP world root differs from inspected world')
    const evaluated = observation ? evaluateEvidence(parity, observation)
      : {status: 'incomplete', componentStatus: 'incomplete', coverageFlags: coverageFlags(), checks: [{id: 'browser.not-run', status: 'incomplete', reason: 'browser RUN slot/observations not supplied'}], unverified: ['browser coverage, residency and faults not run']}
    result = {format: 'glyphweave.preview-world-check', version: 1, ...evaluated, parity: {path: parityPath, sha256},
      bindings, observation, worldQualityStatus: 'not_run', runningHandles: []}
  } catch (error) {
    const check = resultError(error)
    result = {format: 'glyphweave.preview-world-check', version: 1, status: check.status, componentStatus: check.status, coverageFlags: coverageFlags(), checks: [check],
      worldQualityStatus: 'not_run', runningHandles: []}
  }
  result.outputWritten = true
  try { await output.writeFile(JSON.stringify(result, null, 2) + '\n') } finally { await output.close() }
  return {result, exitCode: result.status === 'pass' ? 0 : 1}
}
function parseArgs(args) {
  const out = {}; const allowed = new Set(['--url', '--parity', '--out', '--observations'])
  for (let i = 0; i < args.length; i++) {
    const key = args[i]
    if (key === '--browser') { need(!out.browser, 'cli.duplicate', 'duplicate --browser'); out.browser = true; continue }
    need(allowed.has(key) && args[i + 1] && !args[i + 1].startsWith('--') && !Object.hasOwn(out, key), 'cli.args', `unknown/duplicate/missing argument ${key}`)
    out[key] = args[++i]
  }
  return {url: out['--url'], parityPath: out['--parity'], outPath: out['--out'], observationPath: out['--observations'], browser: out.browser}
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const {result, exitCode} = await runInspection(parseArgs(process.argv.slice(2)))
    console.log(JSON.stringify({status: result.status, checks: result.checks}, null, 2)); process.exitCode = exitCode
  } catch (error) { console.error(String(error.message || error)); process.exitCode = 2 }
}
