// Independently compare every derived chunk against authoritative scene data.
// Not a semantic quality audit: the CLI quality gate remains authoritative.
import { readFile, writeFile, stat } from 'node:fs/promises'
import { join, dirname, resolve, relative } from 'node:path'
import { createHash } from 'node:crypto'
import assert from 'node:assert/strict'
const [directory, output] = process.argv.slice(2)
assert(directory && output, 'usage: node check-render-stream.mjs WORLD_DIR REPORT.json')
const root=resolve(directory), world=JSON.parse(await readFile(join(root,'world.json'),'utf8'))
const report={status:'running',scope:'exact derived projection parity, not semantic quality gate',scenes:[]}
try {
  for(const sceneFile of world.scenes) {
    const path=resolve(root,sceneFile);assert(!relative(root,path).startsWith('..'))
    const base=dirname(path), source=JSON.parse(await readFile(path,'utf8'))
    const headerBytes=await readFile(join(base,'preview-scene.json')),header=JSON.parse(headerBytes)
    assert.equal(header.renderStream.worldRevision,world.revision)
    assert.deepEqual(header.chunks,source.chunks)
    assert.deepEqual(header.entities,source.entities)
    assert.deepEqual(header.accessSurfaces,source.accessSurfaces)
    const patches=new Map(source.landcoverSurfaces.patches.map(p=>[p.id,p]))
    const plants=new Map(source.landcoverSurfaces.ecology.plants.map(p=>[p.id,p]))
    const seenPatches=new Set(),seenPlants=new Set(),seenChunks=new Set(),chunks=[]
    let payloadBytes=0,replicatedPatches=0
    const bounds=new Map([...patches].map(([id,p])=>[id,p.vertices.reduce((a,v)=>[Math.min(a[0],v[0]),Math.min(a[1],v[2]),Math.max(a[2],v[0]),Math.max(a[3],v[2])],[Infinity,Infinity,-Infinity,-Infinity])]))
    for(const d of header.renderStream.chunks) {
      const key=d.chunkX+':'+d.chunkZ;assert(!seenChunks.has(key));seenChunks.add(key)
      const chunk=source.chunks.find(c=>c.chunkX===d.chunkX&&c.chunkZ===d.chunkZ);assert(chunk)
      assert.equal(d.file,`render-chunks/${d.chunkX}-${d.chunkZ}-${d.sha256}.json`)
      const bytes=await readFile(join(base,d.file));assert.equal(bytes.length,d.bytes)
      assert.equal(createHash('sha256').update(bytes).digest('hex'),d.sha256)
      const data=JSON.parse(bytes);assert.equal(data.sceneId,source.sceneId);assert.equal(data.worldRevision,world.revision)
      assert.equal(data.chunkX,d.chunkX);assert.equal(data.chunkZ,d.chunkZ)
      assert.equal(data.patches.length,d.patchCount);assert.equal(data.plants.length,d.plantCount)
      const localPatches=new Set()
      for(const p of data.patches) { assert.deepEqual(p,patches.get(p.id));assert(!localPatches.has(p.id));localPatches.add(p.id);seenPatches.add(p.id) }
      // Independent half-open minimum/inclusive maximum bounding interval.
      const expectedPatches=[...bounds].filter(([,b])=>b[0]<chunk.worldX+chunk.validWidthM && b[2]>=chunk.worldX && b[1]<chunk.worldZ+chunk.validDepthM && b[3]>=chunk.worldZ).map(([id])=>id).sort()
      assert.deepEqual([...localPatches].sort(),expectedPatches)
      for(const p of data.plants) {
        assert.deepEqual(p,plants.get(p.id));assert(!seenPlants.has(p.id));seenPlants.add(p.id);assert(localPatches.has(p.sourcePatchId))
        assert(p.worldX>=chunk.worldX&&p.worldX<chunk.worldX+chunk.validWidthM&&p.worldZ>=chunk.worldZ&&p.worldZ<chunk.worldZ+chunk.validDepthM)
      }
      // Compact per-chunk expected identity digest, derived from the full source.
      const plantIds=data.plants.map(p=>p.id).sort()
      chunks.push({key,bytes:bytes.length,patches:d.patchCount,plants:d.plantCount,plantIdsSha256:createHash('sha256').update(JSON.stringify(plantIds)).digest('hex')})
      payloadBytes+=bytes.length;replicatedPatches+=data.patches.length
    }
    assert.equal(seenChunks.size,source.chunks.length);assert.equal(seenPatches.size,patches.size);assert.equal(seenPlants.size,plants.size)
    assert.equal(header.renderStream.totalPatches,patches.size);assert.equal(header.renderStream.totalPlants,plants.size)
    assert.deepEqual(header.landcoverSurfaces.patches,[]);assert.deepEqual(header.landcoverSurfaces.ecology.plants,[])
    report.scenes.push({sceneId:source.sceneId,sourceBytes:(await stat(path)).size,headerBytes:headerBytes.length,payloadBytes,uniquePatches:patches.size,replicatedPatches,uniquePlants:plants.size,chunks})
  }
  report.status='pass'
} catch(error) { report.status='fail';report.failure=String(error);process.exitCode=1 }
await writeFile(output,JSON.stringify(report,null,2))
console.log(JSON.stringify({...report,scenes:report.scenes.map(({chunks,...scene})=>({...scene,chunks:chunks.length}))},null,2))
