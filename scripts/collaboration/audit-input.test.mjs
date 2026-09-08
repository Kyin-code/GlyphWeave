import {after, test} from 'node:test'
import assert from 'node:assert/strict'
import {createHash} from 'node:crypto'
import {mkdtempSync, mkdirSync, rmSync, writeFileSync} from 'node:fs'
import {join, resolve} from 'node:path'
import {spawnSync} from 'node:child_process'
import {loadAuditInput, verifyAuditInputBinding} from './audit-input.mjs'

const sha = bytes => createHash('sha256').update(bytes).digest('hex')
const dir = mkdtempSync(join(resolve('work'), 'audit-input-unit-'))
mkdirSync(dir, {recursive: true})

function fixture() {
  const source = {format: 'glyphweave.render-audit-source', version: 1, sceneId: 'scene-0', worldRevision: 'rev'}
  const bytes = Buffer.from(JSON.stringify(source))
  const path = join(dir, 'audit-source-scene-0.json')
  writeFileSync(path, bytes)
  const parity = {
    format: 'glyphweave.render-parity', version: 1, status: 'pass', worldRevision: 'rev',
    scenes: [{sceneId: 'scene-0', auditInput: {path: 'audit-source-scene-0.json', bytes: bytes.length, sha256: sha(bytes)}}],
  }
  return {bytes, path, parity}
}
async function rejected(mutate, code) {
  const f = fixture()
  mutate(f)
  await assert.rejects(() => verifyAuditInputBinding(dir, f.parity, 0, {parse: true}),
    error => error.code === code)
}

test('control audit input is hash-bound and revision-checked', async () => {
  const f = fixture()
  const binding = await verifyAuditInputBinding(dir, f.parity, 0)
  assert.equal(binding.bytes, f.bytes.length)
  assert.equal((await loadAuditInput(dir, f.parity, 0)).sceneId, 'scene-0')
})
test('missing audit input is rejected', () => rejected(f => { f.parity.scenes[0].auditInput.path = 'missing.json' }, 'audit.missing'))
test('wrong audit input hash is rejected', () => rejected(f => { f.parity.scenes[0].auditInput.sha256 = '0'.repeat(64) }, 'audit.hash'))
test('audit input byte count is rejected', () => rejected(f => { f.parity.scenes[0].auditInput.bytes++ }, 'audit.bytes'))
test('path escape is rejected', () => rejected(f => {
  writeFileSync(join(dir, '..', 'outside.json'), '{}')
  f.parity.scenes[0].auditInput.path = '../outside.json'
}, 'audit.escape'))
test('replaced source scene is rejected by sceneId', () => rejected(f => {
  const source = {format: 'glyphweave.render-audit-source', version: 1, sceneId: 'other', worldRevision: 'rev'}
  const bytes = Buffer.from(JSON.stringify(source))
  writeFileSync(f.path, bytes)
  f.parity.scenes[0].auditInput = {path: 'audit-source-scene-0.json', bytes: bytes.length, sha256: sha(bytes)}
}, 'audit.scene-id'))
test('revision mismatch is rejected', () => rejected(f => {
  const source = {format: 'glyphweave.render-audit-source', version: 1, sceneId: 'scene-0', worldRevision: 'old'}
  const bytes = Buffer.from(JSON.stringify(source))
  writeFileSync(f.path, bytes)
  f.parity.scenes[0].auditInput = {path: 'audit-source-scene-0.json', bytes: bytes.length, sha256: sha(bytes)}
}, 'audit.revision'))

for (const [script, args] of [
  ['scripts/check-render-stream.mjs', ['WORLD', 'REPORT']],
  ['scripts/check-maximum-world.mjs', ['WORLD', 'REPORT']],
  ['scripts/check-frontage.mjs', ['WORLD', 'REPORT']],
]) {
  test(`${script} refuses missing auditInput before consuming it`, () => {
    const world = join(dir, 'consumer-world')
    const audit = join(dir, 'consumer-audit')
    mkdirSync(world, {recursive: true})
    mkdirSync(audit, {recursive: true})
    writeFileSync(join(world, 'world.json'), JSON.stringify({format: 'glyphweave-world', version: 1, revision: 'rev', scenes: ['scenes/scene-0/scene.json']}))
    writeFileSync(join(world, 'glyphweave.manifest.json'), JSON.stringify({format: 'glyphweave-world', version: 1}))
    writeFileSync(join(audit, 'parity.json'), JSON.stringify({format: 'glyphweave.render-parity', version: 1, status: 'pass', worldRevision: 'rev', scenes: [{sceneId: 'scene-0'}]}))
    const commandArgs = args.map(value => value === 'WORLD' ? world : value === 'REPORT' ? join(dir, `${script.replaceAll('/', '_')}.json`) : value)
    const result = spawnSync(process.execPath, [resolve(script), ...commandArgs, '--audit-dir', audit], {encoding: 'utf8'})
    assert.notEqual(result.status, 0, `${script} unexpectedly accepted missing auditInput`)
    assert.match(result.stderr + result.stdout, /auditInput/, `${script} did not name auditInput as the rejection reason`)
  })
}

after(() => rmSync(dir, {recursive: true, force: true}))
