// Verify full payload coverage and summarize actual data, not a synthetic map.
// Prefer `--audit-dir` (compact source produced by `glyphweave audit-preview-stream`)
// so the 460MB scene is never loaded as one string.
import { readFile, stat, open, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'
import assert from 'node:assert/strict'
import {loadAuditInput} from './collaboration/audit-input.mjs'

export const MAX_SOURCE_BYTES = 64 * 1024 * 1024

function parseArgs(argv) {
  const positional = []
  let auditDir = null
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--audit-dir') {
      auditDir = argv[++i]
      continue
    }
    positional.push(argv[i])
  }
  return { positional, auditDir }
}

const { positional, auditDir } = parseArgs(process.argv.slice(2))
const [directory, reportPath] = positional
assert(directory && reportPath, 'usage: node check-maximum-world.mjs WORLD_DIR REPORT.json [--audit-dir DIR]')
const world = JSON.parse(await readFile(join(directory, 'world.json'), 'utf8'))
assert.equal(world.scenes.length, 1)

let scene
let sceneJsonBytes
if (auditDir) {
  const parity = JSON.parse(await readFile(join(resolve(auditDir), 'parity.json'), 'utf8'))
  assert.equal(parity.status, 'pass')
  assert.equal(parity.worldRevision, world.revision)
  const sceneId = parity.scenes[0].sceneId
  scene = await loadAuditInput(resolve(auditDir), parity, 0)
  assert.equal(scene.sceneId, sceneId)
  sceneJsonBytes = scene.source.bytes
} else {
  const scenePath = join(directory, world.scenes[0])
  sceneJsonBytes = (await stat(scenePath)).size
  assert(
    sceneJsonBytes <= MAX_SOURCE_BYTES,
    `scene is ${sceneJsonBytes} bytes > ${MAX_SOURCE_BYTES}; run ` +
      '`glyphweave audit-preview-stream WORLD_DIR OUT_DIR` and pass --audit-dir OUT_DIR',
  )
  scene = JSON.parse(await readFile(scenePath, 'utf8'))
}

assert.equal(scene.widthM, 6000)
assert.equal(scene.depthM, 10000)
assert.equal(scene.chunkSizeM, 512)
assert.equal(scene.chunks.length, 240)
const ids = new Set()
const heightFiles = new Set()
let samples = 0
let totalHeightBytes = 0
for (const c of scene.chunks) {
  assert(Number.isInteger(c.chunkX) && Number.isInteger(c.chunkZ))
  assert(c.chunkX >= 0 && c.chunkX < 12 && c.chunkZ >= 0 && c.chunkZ < 20)
  const key = c.chunkX + ':' + c.chunkZ
  assert(!ids.has(key))
  ids.add(key)
  const w = Math.min(512, 6000 - c.chunkX * 512)
  const d = Math.min(512, 10000 - c.chunkZ * 512)
  assert.equal(c.validWidthM, w)
  assert.equal(c.validDepthM, d)
  assert.equal(c.worldX, scene.originX + c.chunkX * 512)
  assert.equal(c.worldZ, scene.originZ + c.chunkZ * 512)
  samples += w * d
  for (const f of [c.heightFile, c.naturalHeightFile]) {
    assert(typeof f === 'string')
    assert(!heightFiles.has(f))
    heightFiles.add(f)
    const path = join(directory, 'scenes', scene.sceneId, f)
    const metadata = await stat(path)
    assert.equal(metadata.size, 16 + w * d * 4)
    totalHeightBytes += metadata.size
    const handle = await open(path, 'r')
    try {
      const header = Buffer.alloc(16)
      const { bytesRead } = await handle.read(header, 0, 16, 0)
      assert.equal(bytesRead, 16)
      assert.equal(header.toString('ascii', 0, 4), 'GWHM')
      assert.equal(header.readUInt32LE(4), 1)
      assert.equal(header.readUInt32LE(8), w)
      assert.equal(header.readUInt32LE(12), d)
    } finally {
      await handle.close()
    }
  }
  assert.equal((await stat(join(directory, 'scenes', scene.sceneId, c.surfaceFile))).size, w * d)
  assert.equal((await stat(join(directory, 'scenes', scene.sceneId, c.terrainFile))).size, w * d * 4)
}
assert.equal(samples, 60000000)
assert(scene.entities.length > 0)

const ecologyPlants = auditDir ? scene.ecologyPlantCount : scene.landcoverSurfaces.ecology.plants.length
const ecologyCommunities = auditDir ? scene.ecologyCommunityCount : scene.landcoverSurfaces.ecology.communities.length
const ecologyUnfilled = auditDir ? scene.ecologyRejectionCount : scene.landcoverSurfaces.ecology.rejections.length
const agriculturalFields = auditDir ? scene.agriculturalFieldCount : scene.landcoverSurfaces.agriculture.fields.length
const surfaces = auditDir ? scene.surfacePatchCount : scene.landcoverSurfaces.patches.length
assert(ecologyPlants > 0)

const report = {
  status: 'pass',
  scope: 'full-domain dimensions, payload headers and lengths; not hydraulic/visual safety',
  sizeM: [scene.widthM, scene.depthM],
  chunks: ids.size,
  samplesPerHeightLayer: samples,
  heightFiles: heightFiles.size,
  totalHeightBytes,
  lastChunk: scene.chunks.find((c) => c.chunkX === 11 && c.chunkZ === 19),
  entities: scene.entities.length,
  entityKinds: auditDir
    ? scene.entityKinds
    : scene.entities.reduce((a, e) => ((a[e.kind] = (a[e.kind] || 0) + 1), a), {}),
  ecologyPlants,
  ecologyCommunities,
  ecologyUnfilled,
  agriculturalFields,
  surfaces,
  sceneJsonBytes,
}
await writeFile(reportPath, JSON.stringify(report, null, 2))
console.log(JSON.stringify(report, null, 2))
