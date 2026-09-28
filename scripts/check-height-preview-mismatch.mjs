// Inject a downgrade in an isolated browser request; do not edit actual data.
import {createRequire} from 'node:module'
import assert from 'node:assert/strict'
const require=createRequire(import.meta.url)
const {chromium}=require('playwright')
const browser=await chromium.launch({headless:true,channel:process.env.PREVIEW_BROWSER_CHANNEL||'msedge'})
try{
 const page=await browser.newPage();let downgraded=0
 await page.route('**/*.height.bin',async route=>{
  const response=await route.fetch(),body=await response.body()
  if(body.toString('ascii',0,4)!=='GWHM'){await route.fulfill({response});return}
  const count=body.readUInt32LE(8)*body.readUInt32LE(12),raw=Buffer.alloc(count*2)
  for(let i=0;i<count;i++)raw.writeInt16LE(Math.trunc(body.readInt32LE(16+i*4)/250),i*2)
  downgraded++;await route.fulfill({response,body:raw})
 })
 await page.goto(process.argv[2])
 await page.waitForFunction(()=>document.querySelector('#info')?.textContent.includes('Height encoding disagrees with world sidecar'),{}, {timeout:30000})
 assert(downgraded>0)
 const result=await page.evaluate(()=>({message:document.querySelector('#info').textContent,rendered:window.glyphweaveFeedback?.visualChecks?.nearSceneRendered??false}))
 assert.equal(result.rendered,false)
 console.log(JSON.stringify({downgradedRequests:downgraded,...result}))
}finally{await browser.close()}
