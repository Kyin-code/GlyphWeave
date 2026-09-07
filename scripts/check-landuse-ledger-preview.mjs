// Read-only display contract, not independent planning or area acceptance.
// Usage: node scripts/check-landuse-ledger-preview.mjs URL SCREENSHOT.png
import { createRequire } from 'node:module'
import { writeFile } from 'node:fs/promises'
import assert from 'node:assert/strict'
const require = createRequire(import.meta.url)
const { chromium } = require('playwright')
const [url, screenshot] = process.argv.slice(2)
assert(url && screenshot)
const browser = await chromium.launch({ headless: true, channel: process.env.PREVIEW_BROWSER_CHANNEL || 'msedge' })
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } })
  const errors = []
  page.on('pageerror', error => errors.push(String(error)))
  await page.goto(url)
  await page.waitForFunction(() => window.glyphweaveFeedback?.visualChecks?.ownershipRendered)
  const feedback = await page.evaluate(() => window.glyphweaveFeedback)
  assert.equal(feedback.visualChecks.landUseLedgerDisplayed, true)
  const reportUrl = new URL('../scenes/' + feedback.sceneId + '/land-use-ledger.json', url).href
  const expected = await page.request.get(reportUrl).then(r => r.json())
  assert.deepEqual(feedback.landUseLedgerDiagnostic, expected)
  assert(expected.byPurpose.rural.realizedM2 > 0)
  assert(expected.unresolvedM2 > 0)
  assert.equal(expected.status, 'incomplete')
  await page.screenshot({ path: screenshot })
  for (const invalid of ['missing', 'wrong-scene', 'invalid-number']) {
    await page.route('**/land-use-ledger.json', async route => {
      if (invalid === 'missing') return route.fulfill({ status: 404, body: '' })
      const bad = structuredClone(expected)
      if (invalid === 'wrong-scene') bad.sceneId = 'unrelated'
      else bad.byPurpose.rural.realizedM2 = null
      await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(bad) })
    })
    await page.reload()
    await page.waitForFunction(() => window.glyphweaveFeedback?.visualChecks?.ownershipRendered)
    const actual = await page.evaluate(() => window.glyphweaveFeedback)
    assert.notEqual(actual.visualChecks.landUseLedgerDisplayed, true, invalid)
    assert.equal(actual.landUseLedgerDiagnostic, undefined, invalid)
    await page.unroute('**/land-use-ledger.json')
  }
  assert.deepEqual(errors, [])
  const proof = { status: 'pass', displaySource: 'Rust bake diagnostic, not CLI acceptance', expected, invalidReportsHidden: 3, errors, screenshot }
  await writeFile(screenshot + '.json', JSON.stringify(proof, null, 2))
  console.log(JSON.stringify(proof, null, 2))
} finally {
  await browser.close()
}
