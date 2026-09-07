// Single-chunk farm surface contract. Real route eligibility is rebuilt by CLI.
import { chromium } from 'playwright'
import { writeFile } from 'node:fs/promises'
import assert from 'node:assert/strict'
const [url, output] = process.argv.slice(2)
assert(url && output)
const b = await chromium.launch({ headless: true, channel: process.env.PREVIEW_BROWSER_CHANNEL || 'msedge' })
try {
  const p = await b.newPage({ viewport: { width: 1440, height: 1000 } }), errors = []
  p.on('pageerror', e => errors.push(String(e)))
  await p.goto(url)
  await p.waitForFunction(() => window.glyphweaveFeedback?.visualChecks?.nearSceneRendered, {}, { timeout: 90000 })
  const proof = await p.evaluate(async () => {
    const scene = await fetch('../scenes/' + window.glyphweaveFeedback.sceneId + '/scene.json').then(r => r.json())
    const surfaces = scene.landcoverSurfaces
    const patches = surfaces.patches.filter(p => ['cultivated_soil', 'field_headland', 'farm_track'].includes(p.material))
    const expected = new Map(patches.map(p => [p.id, p]))
    const seen = [], mismatches = []
    window.__nearScene.traverse(o => {
      if (!o.isMesh || !o.name.startsWith('landcover:')) return
      const id = o.name.slice('landcover:'.length), patch = expected.get(id)
      if (!patch) return
      seen.push(id)
      const pos = o.geometry.getAttribute('position'), indices = o.geometry.index.array
      if (o.userData.landcoverMaterial !== patch.material || pos.count !== patch.vertices.length || indices.length !== patch.triangles.length * 3) {
        mismatches.push(id + ':layout'); return
      }
      for (let i = 0; i < pos.count; i++) for (let axis = 0; axis < 3; axis++) if (Math.abs(pos.array[i * 3 + axis] - patch.vertices[i][axis]) > 1e-4) { mismatches.push(id + ':position'); return }
      for (let i = 0; i < indices.length; i++) if (indices[i] !== patch.triangles[Math.floor(i / 3)][i % 3]) { mismatches.push(id + ':triangles'); return }
    })
    return { expected: [...expected.keys()].sort(), seen: seen.sort(), mismatches,
      fields: surfaces.agriculture.fields.length,
      detours: surfaces.agriculture.fields.filter(f => f.accessMethod === 'bounded_grid_detour').map(f => ({ id: f.id, roadId: f.roadId, lengthM: f.accessLengthM, expanded: f.accessSearchExpanded, path: f.accessPath })),
      areas: Object.fromEntries(['cultivated_soil', 'field_headland', 'farm_track'].map(m => [m, patches.filter(p => p.material === m).reduce((s,p) => s + p.areaM2, 0)])) }
  })
  assert(proof.detours.length > 0, 'must show actual detour fields')
  assert.deepEqual(proof.seen, proof.expected)
  assert.deepEqual(proof.mismatches, [])
  assert.deepEqual(errors, [])
  await p.screenshot({ path: output })
  await writeFile(output + '.json', JSON.stringify({ status: 'pass', proof, errors }, null, 2))
  console.log(JSON.stringify({ status: 'pass', fields: proof.fields, detours: proof.detours.length, patches: proof.seen.length, areas: proof.areas, errors, screenshot: output }, null, 2))
} finally { await b.close() }
