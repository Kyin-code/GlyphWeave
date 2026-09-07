// Verify full payload coverage and summarize actual data, not a synthetic map.
import { readFile, stat, open, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import assert from 'node:assert/strict'
const [directory, reportPath] = process.argv.slice(2)
assert(directory && reportPath)
const world = JSON.parse(await readFile(join(directory, 'world.json'), 'utf8'))
assert.equal(world.scenes.length, 1)
const scene = JSON.parse(await readFile(join(directory, world.scenes[0]), 'utf8'))
assert.equal(scene.widthM, 6000); assert.equal(scene.depthM, 10000)
assert.equal(scene.chunkSizeM, 512); assert.equal(scene.chunks.length, 240)
const ids = new Set(), heightFiles = new Set()
let samples = 0, totalHeightBytes = 0
for (const c of scene.chunks) {
  assert(Number.isInteger(c.chunkX) && Number.isInteger(c.chunkZ))
  assert(c.chunkX >= 0 && c.chunkX < 12 && c.chunkZ >= 0 && c.chunkZ < 20)
  const key = c.chunkX + ':' + c.chunkZ
  assert(!ids.has(key)); ids.add(key)
  const w = Math.min(512, 6000 - c.chunkX * 512), d = Math.min(512, 10000 - c.chunkZ * 512)
  assert.equal(c.validWidthM, w); assert.equal(c.validDepthM, d)
  assert.equal(c.worldX, scene.originX + c.chunkX * 512); assert.equal(c.worldZ, scene.originZ + c.chunkZ * 512)
  samples += w * d
  for (const f of [c.heightFile, c.naturalHeightFile]) {
    assert(typeof f === 'string'); assert(!heightFiles.has(f)); heightFiles.add(f)
    const path = join(directory, 'scenes', scene.sceneId, f), metadata = await stat(path)
    assert.equal(metadata.size, 16 + w * d * 4); totalHeightBytes += metadata.size
    const handle = await open(path, 'r')
    try {
      const header = Buffer.alloc(16); const { bytesRead } = await handle.read(header, 0, 16, 0)
      assert.equal(bytesRead, 16); assert.equal(header.toString('ascii', 0, 4), 'GWHM')
      assert.equal(header.readUInt32LE(4), 1); assert.equal(header.readUInt32LE(8), w); assert.equal(header.readUInt32LE(12), d)
    } finally { await handle.close() }
  }
  assert.equal((await stat(join(directory,'scenes',scene.sceneId,c.surfaceFile))).size, w * d)
  assert.equal((await stat(join(directory,'scenes',scene.sceneId,c.terrainFile))).size, w * d * 4)
}
assert.equal(samples, 60000000)
assert(scene.entities.length > 0)
const ecology = scene.landcoverSurfaces.ecology
assert(ecology.plants.length > 0)
const fields = scene.landcoverSurfaces.agriculture.fields
const report = { status: 'pass', scope: 'full-domain dimensions, payload headers and lengths; not hydraulic/visual safety',
  sizeM: [scene.widthM,scene.depthM], chunks: ids.size, samplesPerHeightLayer: samples, heightFiles: heightFiles.size, totalHeightBytes,
  lastChunk: scene.chunks.find(c => c.chunkX === 11 && c.chunkZ === 19), entities: scene.entities.length,
  entityKinds: scene.entities.reduce((a,e) => (a[e.kind] = (a[e.kind] || 0) + 1,a), {}),
  ecologyPlants: ecology.plants.length, ecologyCommunities: ecology.communities.length,
  ecologyUnfilled: ecology.rejections.length, agriculturalFields: fields.length,
  surfaces: scene.landcoverSurfaces.patches.length, sceneJsonBytes: (await stat(join(directory,world.scenes[0]))).size }
await writeFile(reportPath, JSON.stringify(report, null, 2))
console.log(JSON.stringify(report, null, 2))
