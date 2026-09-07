import { test } from 'node:test'
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { loadPreviewScene, createRenderStream, verticesOverlapChunks } from '../adapters/html/core/render-stream.js'
function fixture() {
  const patch = { id: 'seam', vertices: [[500,0,0],[520,0,0],[500,0,16]], triangles: [[0,2,1]] }
  const files = new Map(), descriptors = []
  for (let x=0;x<2;x++) {
    const data = { format:'glyphweave.render-chunk',version:1,sceneId:'s',worldRevision:'rev',chunkX:x,chunkZ:0,patches:[patch],plants:[{id:'p'+x,sourcePatchId:'seam'}] }
    const bytes = Buffer.from(JSON.stringify(data)), sha256 = createHash('sha256').update(bytes).digest('hex'), file='render-chunks/'+x+'-0-'+sha256+'.json'
    descriptors.push({chunkX:x,chunkZ:0,file,sha256,bytes:bytes.length,patchCount:1,plantCount:1}); files.set('../scenes/s/'+file,bytes)
  }
  const scene={sceneId:'s',chunks:[{chunkX:0,chunkZ:0},{chunkX:1,chunkZ:0}],landcoverSurfaces:{patches:[],ecology:{plants:[]}},renderStream:{format:'glyphweave.render-directory',version:1,worldRevision:'rev',chunks:descriptors,totalPlants:2,totalPatches:1}}
  files.set('../scenes/s/preview-scene.json',Buffer.from(JSON.stringify(scene)))
  const calls=[]
  const fetcher=async url=>{calls.push(url);const bytes=files.get(url);return bytes?new Response(bytes):new Response('missing',{status:404})}
  return {scene,files,fetcher,calls}
}
test('directory avoids full scene; chunks hash-check and seam replicas deduplicate',async()=>{
  const f=fixture(),scene=await loadPreviewScene('scenes/s/scene.json','rev',f.fetcher),loader=createRenderStream(scene,f.fetcher)
  const a=await loader.load(scene.chunks);assert.equal(a.surfaces.patches.length,1);assert.equal(a.surfaces.ecology.plants.length,2)
  const b=await loader.load(scene.chunks);assert.equal(a,b);assert.equal(f.calls.length,3)
  assert(!f.calls.some(c=>c.endsWith('/scene.json')))
  await loader.load([scene.chunks[0]])
  await loader.load([scene.chunks[1]])
  assert.equal(f.calls.length,4,'evicted chunk must be read again, not retained forever')
})
test('bad hashes, identity or missing payload never trigger giant-scene fallback',async()=>{
  for (const mode of ['hash','missing','identity']) {
    const f=fixture(),scene=await loadPreviewScene('scenes/s/scene.json','rev',f.fetcher),d=scene.renderStream.chunks[0],key='../scenes/s/'+d.file
    if(mode==='missing')f.files.delete(key)
    else if(mode==='hash'){const b=Buffer.from(f.files.get(key));b[0]=32;f.files.set(key,b)}
    else {const data=JSON.parse(f.files.get(key));data.sceneId='other';const b=Buffer.from(JSON.stringify(data));d.sha256=createHash('sha256').update(b).digest('hex');d.bytes=b.length;f.files.set(key,b)}
    await assert.rejects(createRenderStream(scene,f.fetcher).load([scene.chunks[0]]))
    assert(!f.calls.some(c=>c.endsWith('/scene.json')))
  }
})
test('only missing directory enables legacy compatibility; invalid revision rejected',async()=>{
  const f=fixture();f.files.set('../scenes/s/scene.json',Buffer.from('{"sceneId":"legacy"}'));f.files.delete('../scenes/s/preview-scene.json')
  assert.equal((await loadPreviewScene('scenes/s/scene.json','rev',f.fetcher)).sceneId,'legacy')
  const g=fixture();await assert.rejects(loadPreviewScene('scenes/s/scene.json','wrong',g.fetcher))
  assert.equal(g.calls.length,1)
})
test('late chunk request cannot overwrite newer active cache',async()=>{
  const f=fixture(),scene=await loadPreviewScene('scenes/s/scene.json','rev',f.fetcher)
  let release;const wait=new Promise(r=>{release=r});let delayed=true
  const loader=createRenderStream(scene,async url=>{if(delayed&&url.includes('0-0-')){delayed=false;await wait}return f.fetcher(url)})
  const old=loader.load([scene.chunks[0]])
  const current=await loader.load([scene.chunks[1]])
  release();await old
  assert.equal(await loader.load([scene.chunks[1]]),current)
})

test('cached selection invalidates an older pending navigation',async()=>{
  const f=fixture(),scene=await loadPreviewScene('scenes/s/scene.json','rev',f.fetcher)
  let release;const wait=new Promise(r=>{release=r})
  const loader=createRenderStream(scene,async url=>{if(url.includes('1-0-'))await wait;return f.fetcher(url)})
  const first=await loader.load([scene.chunks[0]])
  const pending=loader.load([scene.chunks[1]])
  assert.equal(await loader.load([scene.chunks[0]]),first)
  release();await pending
  assert.equal(await loader.load([scene.chunks[0]]),first)
})
test('interior and seam chunk overlap does not require a vertex inside',()=>{
  const c=[{worldX:512,worldZ:512,validWidthM:512,validDepthM:512}]
  assert(verticesOverlapChunks([[0,0,0],[1600,0,0],[1600,0,1600],[0,0,1600]],c))
  assert(verticesOverlapChunks([[0,0,500],[512,0,500],[512,0,520]],c))
  assert(!verticesOverlapChunks([[0,0,0],[100,0,0],[100,0,100]],c))
  assert(!verticesOverlapChunks([],c))
})
test('invalid directory coverage and descriptors fail closed',async()=>{
  for(const mutate of [s=>s.chunks.push(s.chunks[0]),s=>s.renderStream.chunks[0].file='../escape',s=>s.renderStream.totalPlants=-1,s=>s.renderStream.chunks[0].chunkX=0.5]) {
    const f=fixture();mutate(f.scene);f.files.set('../scenes/s/preview-scene.json',Buffer.from(JSON.stringify(f.scene)))
    await assert.rejects(loadPreviewScene('scenes/s/scene.json','rev',f.fetcher));assert.equal(f.calls.length,1)
  }
})
