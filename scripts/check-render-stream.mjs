// Independently compare every derived chunk against an independent parity
// document produced by `glyphweave audit-preview-stream` (preferred, `--audit-dir`),
// or against the full authoritative scene for small worlds. Never loads a scene
// larger than the 64MiB budget as a single string.
// Not a semantic quality audit: the CLI quality gate remains authoritative.
import { readFile, writeFile, stat } from 'node:fs/promises'
import { createReadStream } from 'node:fs'
import { join, dirname, resolve, relative } from 'node:path'
import { createHash } from 'node:crypto'
import assert from 'node:assert/strict'
import {verifyAuditInputBinding} from './collaboration/audit-input.mjs'

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

async function sha256File(path) {
  const hash = createHash('sha256')
  for await (const chunk of createReadStream(path)) hash.update(chunk)
  return hash.digest('hex')
}

function idDigest(ids) {
  return createHash('sha256').update(JSON.stringify([...ids].sort())).digest('hex')
}

const { positional, auditDir } = parseArgs(process.argv.slice(2))
const [directory, output] = positional
assert(directory && output, 'usage: node check-render-stream.mjs WORLD_DIR REPORT.json [--audit-dir DIR]')
const root = resolve(directory)
const world = JSON.parse(await readFile(join(root, 'world.json'), 'utf8'))
const report = {
  status: 'running',
  scope: auditDir
    ? 'independent parity document, not semantic quality gate'
    : 'exact derived projection parity, not semantic quality gate',
  scenes: [],
}
try {
  const parity = auditDir
    ? JSON.parse(await readFile(join(resolve(auditDir), 'parity.json'), 'utf8'))
    : null
  if (parity) {
    assert.equal(parity.format, 'glyphweave.render-parity')
    assert.equal(parity.version, 1)
    assert.equal(parity.status, 'pass', 'parity document must report pass')
    assert.equal(parity.worldRevision, world.revision)
    assert.equal(parity.scenes.length, world.scenes.length)
    for (let index = 0; index < parity.scenes.length; index++) {
      await verifyAuditInputBinding(resolve(auditDir), parity, index)
    }
  }
  for (const [sceneIndex, sceneFile] of world.scenes.entries()) {
    const path = resolve(root, sceneFile)
    assert(!relative(root, path).startsWith('..'))
    const base = dirname(path)
    const headerBytes = await readFile(join(base, 'preview-scene.json'))
    const header = JSON.parse(headerBytes)
    assert.equal(header.renderStream.worldRevision, world.revision)

    if (parity) {
      // Consume the independently produced, source-bound parity. Hash the
      // authoritative scene by streaming so we never build a giant string.
      const scene = parity.scenes[sceneIndex]
      assert.equal(scene.sceneId, header.sceneId)
      const sourceSha = await sha256File(path)
      const sourceBytes = (await stat(path)).size
      assert.equal(sourceSha, scene.source.sha256, 'parity source hash must bind the on-disk scene')
      assert.equal(sourceBytes, scene.source.bytes)
      const byKey = new Map(scene.chunks.map((c) => [c.key, c]))
      assert.equal(byKey.size, scene.chunks.length)
      let payloadBytes = 0
      const chunks = []
      for (const d of header.renderStream.chunks) {
        const key = d.chunkX + ':' + d.chunkZ
        const expected = byKey.get(key)
        assert(expected, `parity missing chunk ${key}`)
        assert.equal(d.file, expected.file)
        assert.equal(d.sha256, expected.sha256)
        assert.equal(d.bytes, expected.bytes)
        const bytes = await readFile(join(base, d.file))
        assert.equal(bytes.length, expected.bytes)
        assert.equal(createHash('sha256').update(bytes).digest('hex'), expected.sha256)
        const data = JSON.parse(bytes)
        assert.equal(data.sceneId, header.sceneId)
        assert.equal(data.worldRevision, world.revision)
        assert.equal(data.chunkX, d.chunkX)
        assert.equal(data.chunkZ, d.chunkZ)
        assert.equal(data.patches.length, expected.patches)
        assert.equal(data.plants.length, expected.plants)
        assert.equal(idDigest(data.patches.map((p) => p.id)), expected.patchIdsSha256)
        assert.equal(idDigest(data.plants.map((p) => p.id)), expected.plantIdsSha256)
        payloadBytes += bytes.length
        chunks.push({ key, bytes: bytes.length, patches: data.patches.length, plants: data.plants.length })
      }
      assert.equal(byKey.size, header.renderStream.chunks.length)
      report.scenes.push({
        sceneId: header.sceneId,
        sourceBytes,
        headerBytes: headerBytes.length,
        payloadBytes,
        uniquePatches: scene.uniquePatches,
        uniquePlants: scene.uniquePlants,
        chunks,
      })
      continue
    }

    // Legacy small-world path. Reject an oversized source instead of reading it
    // as one string.
    const sourceBytes = (await stat(path)).size
    assert(
      sourceBytes <= MAX_SOURCE_BYTES,
      `scene ${sceneFile} is ${sourceBytes} bytes > ${MAX_SOURCE_BYTES}; run ` +
        '`glyphweave audit-preview-stream WORLD_DIR OUT_DIR` and pass --audit-dir OUT_DIR',
    )
    const source = JSON.parse(await readFile(path, 'utf8'))
    assert.deepEqual(header.chunks, source.chunks)
    assert.deepEqual(header.entities, source.entities)
    assert.deepEqual(header.accessSurfaces, source.accessSurfaces)
    const patches = new Map(source.landcoverSurfaces.patches.map((p) => [p.id, p]))
    const plants = new Map(source.landcoverSurfaces.ecology.plants.map((p) => [p.id, p]))
    const seenPatches = new Set()
    const seenPlants = new Set()
    const seenChunks = new Set()
    const chunks = []
    let payloadBytes = 0
    let replicatedPatches = 0
    const bounds = new Map(
      [...patches].map(([id, p]) => [
        id,
        p.vertices.reduce(
          (a, v) => [Math.min(a[0], v[0]), Math.min(a[1], v[2]), Math.max(a[2], v[0]), Math.max(a[3], v[2])],
          [Infinity, Infinity, -Infinity, -Infinity],
        ),
      ]),
    )
    for (const d of header.renderStream.chunks) {
      const key = d.chunkX + ':' + d.chunkZ
      assert(!seenChunks.has(key))
      seenChunks.add(key)
      const chunk = source.chunks.find((c) => c.chunkX === d.chunkX && c.chunkZ === d.chunkZ)
      assert(chunk)
      assert.equal(d.file, `render-chunks/${d.chunkX}-${d.chunkZ}-${d.sha256}.json`)
      const bytes = await readFile(join(base, d.file))
      assert.equal(bytes.length, d.bytes)
      assert.equal(createHash('sha256').update(bytes).digest('hex'), d.sha256)
      const data = JSON.parse(bytes)
      assert.equal(data.sceneId, source.sceneId)
      assert.equal(data.worldRevision, world.revision)
      assert.equal(data.chunkX, d.chunkX)
      assert.equal(data.chunkZ, d.chunkZ)
      assert.equal(data.patches.length, d.patchCount)
      assert.equal(data.plants.length, d.plantCount)
      const localPatches = new Set()
      for (const p of data.patches) {
        assert.deepEqual(p, patches.get(p.id))
        assert(!localPatches.has(p.id))
        localPatches.add(p.id)
        seenPatches.add(p.id)
      }
      const expectedPatches = [...bounds]
        .filter(
          ([, b]) =>
            b[0] < chunk.worldX + chunk.validWidthM &&
            b[2] >= chunk.worldX &&
            b[1] < chunk.worldZ + chunk.validDepthM &&
            b[3] >= chunk.worldZ,
        )
        .map(([id]) => id)
        .sort()
      assert.deepEqual([...localPatches].sort(), expectedPatches)
      for (const p of data.plants) {
        assert.deepEqual(p, plants.get(p.id))
        assert(!seenPlants.has(p.id))
        seenPlants.add(p.id)
        assert(localPatches.has(p.sourcePatchId))
        assert(
          p.worldX >= chunk.worldX &&
            p.worldX < chunk.worldX + chunk.validWidthM &&
            p.worldZ >= chunk.worldZ &&
            p.worldZ < chunk.worldZ + chunk.validDepthM,
        )
      }
      const plantIds = data.plants.map((p) => p.id).sort()
      chunks.push({
        key,
        bytes: bytes.length,
        patches: d.patchCount,
        plants: d.plantCount,
        plantIdsSha256: createHash('sha256').update(JSON.stringify(plantIds)).digest('hex'),
      })
      payloadBytes += bytes.length
      replicatedPatches += data.patches.length
    }
    assert.equal(seenChunks.size, source.chunks.length)
    assert.equal(seenPatches.size, patches.size)
    assert.equal(seenPlants.size, plants.size)
    assert.equal(header.renderStream.totalPatches, patches.size)
    assert.equal(header.renderStream.totalPlants, plants.size)
    assert.deepEqual(header.landcoverSurfaces.patches, [])
    assert.deepEqual(header.landcoverSurfaces.ecology.plants, [])
    report.scenes.push({
      sceneId: source.sceneId,
      sourceBytes,
      headerBytes: headerBytes.length,
      payloadBytes,
      uniquePatches: patches.size,
      replicatedPatches,
      uniquePlants: plants.size,
      chunks,
    })
  }
  report.status = 'pass'
} catch (error) {
  report.status = 'fail'
  report.failure = String(error)
  process.exitCode = 1
}
await writeFile(output, JSON.stringify(report, null, 2))
console.log(
  JSON.stringify(
    { ...report, scenes: report.scenes.map(({ chunks, ...scene }) => ({ ...scene, chunks: chunks.length })) },
    null,
    2,
  ),
)
