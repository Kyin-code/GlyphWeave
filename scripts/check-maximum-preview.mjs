// Real maximum-world streaming acceptance. No authoritative full scene is
// fetched into the browser. Reference identities come from disk parity proof.
import { chromium } from 'playwright'
import { readFile, writeFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import assert from 'node:assert/strict'
const [url,output,parityPath]=process.argv.slice(2)
assert(url&&output&&parityPath,'usage: URL OUTPUT.json PROJECTION_PARITY.json')
const parity=JSON.parse(await readFile(parityPath,'utf8'));assert.equal(parity.status,'pass')
const reference=parity.scenes[0],expectedByKey=new Map(reference.chunks.map(c=>[c.key,c]))
const browser=await chromium.launch({headless:true,channel:process.env.PREVIEW_BROWSER_CHANNEL||'msedge'})
const evidence={status:'running',scope:'real maximum map: first/last reload and persistent first-last-first navigation, not all-240-chunk or visual acceptance',errors:[],failedRequests:[],requests:[],views:[]}
try {
  const p=await browser.newPage({viewport:{width:1440,height:1000}})
  p.on('pageerror',e=>evidence.errors.push(String(e)))
  p.on('requestfailed',r=>evidence.failedRequests.push({url:r.url(),error:r.failure()?.errorText}))
  p.on('request',r=>{if(/\/(scene\.json|preview-scene\.json|render-chunks\/)/.test(r.url()))evidence.requests.push(r.url())})
  await p.addInitScript(()=>{const q=new URLSearchParams(location.search);window.__nearFocusOverride={x:Number(q.get('benchmarkX')),z:Number(q.get('benchmarkZ'))}})
  for(const [step,focus] of [
    ['reload-first',{x:256,z:256}],['reload-last',{x:5816,z:9864}],
    ['navigate-first',{x:256,z:256}],['navigate-last',{x:5816,z:9864}],['return-first',{x:256,z:256}]
  ]) {
    const start=Date.now(),expected=[Math.floor(focus.x/512),Math.floor(focus.z/512)],key=expected.join(':')
    if(step.startsWith('reload')) {
      const target=new URL(url);target.searchParams.set('benchmarkX',focus.x);target.searchParams.set('benchmarkZ',focus.z)
      await p.goto(target.href,{waitUntil:'domcontentloaded',timeout:120000})
    } else { await p.evaluate(f=>window.__focusAt(f.x,f.z),focus) }
    await p.waitForFunction(k=>window.glyphweaveFeedback?.visualChecks?.nearSceneRendered&&window.glyphweaveFeedback.visualChecks.highDetailChunk===k,expected.join(','),{timeout:120000})
    const view=await p.evaluate(()=>{
      const feedback=window.glyphweaveFeedback,batches=[],ids=[],landcover=[]
      window.__nearScene.traverse(o=>{
        if(o.name.startsWith('ecology-batch:')) { batches.push({key:o.userData.ecologyChunk,count:o.count});ids.push(...o.userData.ecologyInstances.map(p=>p.id)) }
        if(o.name.startsWith('landcover:'))landcover.push(o.name)
      })
      return {feedback,batches,ids:ids.sort(),landcover,jsHeap:performance.memory?{used:performance.memory.usedJSHeapSize,total:performance.memory.totalJSHeapSize,limit:performance.memory.jsHeapSizeLimit}:null,
        resources:performance.getEntriesByType('resource').filter(r=>/\/(scene\.json|preview-scene\.json|render-chunks\/)/.test(r.name)).map(r=>({name:r.name,durationMs:r.duration,transferBytes:r.transferSize,decodedBytes:r.decodedBodySize}))}
    })
    assert.deepEqual(view.feedback.sizeM,[6000,10000]);assert.equal(view.feedback.chunks,240)
    const stream=view.feedback.visualChecks.renderStream,ref=expectedByKey.get(key)
    assert.equal(stream.format,'r44-render-stream');assert.deepEqual(stream.loadedChunks,[key]);assert.equal(stream.residentBytes,ref.bytes)
    assert.equal(stream.loadedPatches,ref.patches);assert.equal(stream.loadedPlants,ref.plants)
    assert.equal(stream.totalPlants,reference.uniquePlants);assert.equal(stream.totalPatches,reference.uniquePatches)
    assert(view.batches.every(b=>b.key[0]===expected[0]&&b.key[1]===expected[1]));assert(view.batches.length>0&&view.batches.length<=3)
    assert.equal(view.ids.length,ref.plants);assert.equal(new Set(view.ids).size,ref.plants)
    assert.equal(createHash('sha256').update(JSON.stringify(view.ids)).digest('hex'),ref.plantIdsSha256)
    assert.equal(new Set(view.landcover).size,view.landcover.length)
    assert.deepEqual(view.feedback.visualChecks.loadedChunks,[expected.join(',')])
    assert.equal(view.feedback.visualChecks.ecologyInstancing.activePlants,ref.plants)
    assert(!evidence.requests.some(r=>new URL(r).pathname.endsWith('/scene.json')),'full scene must never enter browser')
    delete view.ids;delete view.landcover
    evidence.views.push({step,focus,elapsedMs:Date.now()-start,...view})
    await p.screenshot({path:output+'.'+step+'.png'})
    await writeFile(output,JSON.stringify(evidence,null,2))
  }
  const navigations=evidence.views.slice(1).map(v=>v.feedback.visualChecks.renderStream.requests)
  assert.deepEqual(navigations,[1,2,3,4],'evicted chunks must be fetched again, not retained in JS loader')
  assert.deepEqual(evidence.errors,[]);assert.deepEqual(evidence.failedRequests,[])
  evidence.status='pass'
} catch(error) { evidence.status='fail';evidence.failure=String(error);process.exitCode=1 }
finally {
  await writeFile(output,JSON.stringify(evidence,null,2))
  console.log(JSON.stringify({status:evidence.status,failure:evidence.failure,errors:evidence.errors,views:evidence.views.map(v=>({step:v.step,elapsedMs:v.elapsedMs,jsHeap:v.jsHeap,stream:v.feedback.visualChecks.renderStream}))},null,2))
  await browser.close()
}
