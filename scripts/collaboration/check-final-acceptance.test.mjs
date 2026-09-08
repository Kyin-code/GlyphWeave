import {test} from 'node:test'
import assert from 'node:assert/strict'
import {createHash} from 'node:crypto'
import {mkdtempSync, mkdirSync, readFileSync, writeFileSync} from 'node:fs'
import {join, resolve} from 'node:path'
import {checkAcceptance, FORMAT, VERSION} from './check-final-acceptance.mjs'

const dir = mkdtempSync(join(resolve('work'), 'final-acceptance-r3-unit-'))
mkdirSync(dir, {recursive: true})
const sha = bytes => createHash('sha256').update(bytes).digest('hex')
const write = (name, value) => {
  const bytes = Buffer.from(typeof value === 'string' ? value : JSON.stringify(value))
  const path = join(dir, name)
  writeFileSync(path, bytes)
  return {path, bytes: bytes.length, sha256: sha(bytes)}
}
const rebind = (candidate, field) => {
  const bytes = readFileSync(candidate[field].path)
  candidate[field].bytes = bytes.length
  candidate[field].sha256 = sha(bytes)
}
const rewriteJson = (binding, value) => {
  const bytes = Buffer.from(JSON.stringify(value))
  writeFileSync(binding.path, bytes)
  binding.bytes = bytes.length
  binding.sha256 = sha(bytes)
}
const rewriteSamples = (candidate, label, mutate) => {
  const measurement = candidate.measurements[label]
  const rows = readFileSync(measurement.samples.path, 'utf8').trim().split(/\r?\n/).map(JSON.parse)
  mutate(rows)
  const bytes = Buffer.from(rows.map(JSON.stringify).join('\n') + '\n')
  writeFileSync(measurement.samples.path, bytes)
  measurement.samples.bytes = bytes.length
  measurement.samples.sha256 = sha(bytes)
  const process = JSON.parse(readFileSync(measurement.process.path, 'utf8'))
  process.samplesJsonlSha256 = sha(bytes)
  rewriteJson(measurement.process, process)
}
const samplesText = (pid, {maxPrivate = 1000, maxWorking = 900, maxElapsed = 2.0, maxCpu = 0.3} = {}) => [
  {utc: '2026-09-09T00:00:00.000Z', elapsedSeconds: 0.5, pid, privateBytes: 500, workingSetBytes: 700, cpuSeconds: 0.1},
  {utc: '2026-09-09T00:00:01.000Z', elapsedSeconds: 1.0, pid, privateBytes: maxPrivate, workingSetBytes: maxWorking, cpuSeconds: maxCpu},
  {utc: '2026-09-09T00:00:02.000Z', elapsedSeconds: maxElapsed, pid, privateBytes: 800, workingSetBytes: 800, cpuSeconds: maxCpu + 0.1},
].map(row => JSON.stringify(row)).join('\n') + '\n'

function fixture({gatePass = false} = {}) {
  const revision = 'rev-final'
  const runtime = write('glyphweave.exe', 'runtime')
  const auditRuntime = write('glyphweave-audit.exe', 'audit-runtime')
  const manifest = write('manifest.json', {format: 'glyphweave-world', version: 1})
  const world = write('world.json', {format: 'glyphweave-world', version: 1, revision})
  const sceneDir = join(dir, 'world', 'scenes', 's')
  mkdirSync(sceneDir, {recursive: true})
  const source = write('world/scenes/s/scene.json', {sceneId: 's', worldRevision: revision})
  const header = write('world/scenes/s/preview-scene.json', {sceneId: 's', worldRevision: revision})
  const auditInput = write('audit-source-s.json', {sceneId: 's', worldRevision: revision})
  const parity = write('parity.json', {
    format: 'glyphweave.render-parity', version: 1, status: 'pass', worldRevision: revision,
    worldFile: {path: world.path, sha256: world.sha256},
    scenes: [{sceneId: 's', source, header, auditInput: {...auditInput, path: 'audit-source-s.json'}}],
  })
  const observation = write('observation.json', {
    format: 'glyphweave.preview-traversal', version: 1, status: 'complete',
    worldRevision: revision, worldSha256: world.sha256,
  })
  const evidence = write('evidence.json', {ok: true})
  const log = write('run.log', 'ok\n')

  const generationSamples = write('generation.samples.jsonl', samplesText(1234))
  const gateSamples = write('gate.samples.jsonl', samplesText(5678, {maxPrivate: 2000, maxWorking: 1800, maxElapsed: 3.0, maxCpu: 0.5}))
  const generationProcess = write('generation.process.json', {
    format: 'glyphweave.process-benchmark', version: 1, status: 'baked', operation: 'generate-world', pid: 1234,
    executable: runtime.path, executableSha256: runtime.sha256,
    manifest: manifest.path, manifestSha256: manifest.sha256,
    output: dir, samplesJsonl: generationSamples.path, samplesJsonlSha256: generationSamples.sha256,
    elapsedSeconds: 10.5, sampledPeakPrivateBytes: 1000, peakWorkingSetBytes: 1200,
    sampleCount: 3, timeoutSeconds: 600, maxPrivateMiB: 8192, stopReason: null, exitCode: 0,
  })
  const gateProcess = write('gate.process.json', {
    format: 'glyphweave.process-benchmark', version: 1, status: gatePass ? 'completed' : 'failed',
    operation: 'world-quality-gate', pid: 5678,
    executable: runtime.path, executableSha256: runtime.sha256,
    manifest: manifest.path, manifestSha256: manifest.sha256,
    output: dir, samplesJsonl: gateSamples.path, samplesJsonlSha256: gateSamples.sha256,
    elapsedSeconds: 5.5, sampledPeakPrivateBytes: 2000, peakWorkingSetBytes: 2500,
    sampleCount: 3, timeoutSeconds: 600, maxPrivateMiB: 8192, stopReason: null,
    exitCode: gatePass ? 0 : 1,
  })
  const quality = write('quality.json', {status: 'fail', failures: ['approved content gap']})

  const candidate = {
    format: FORMAT, version: VERSION, taskId: 'A-FINAL-R3', status: 'ready_for_review',
    authority: 'ds-v4.1f-candidate', runtime, auditRuntime, manifest,
    world: {...world, worldRevision: revision},
    parity: {...parity, worldRevision: revision},
    browserObservation: observation,
    evidence: [{requirement: 'A-01', ...evidence}, {requirement: 'A-04.gate-report', ...quality}],
    logs: [log],
    measurements: {
      generation: {
        status: 'measured', operation: 'generate-world', process: generationProcess,
        samples: generationSamples, executable: runtime, manifest, output: dir,
        exitCode: 0, elapsedSeconds: 10.5, timeoutSeconds: 600, maxPrivateMiB: 8192,
        sampledPeakPrivateBytes: 1000, peakWorkingSetBytes: 1200, sampleCount: 3,
      },
      gate: {
        status: 'measured', operation: 'world-quality-gate', process: gateProcess,
        samples: gateSamples, executable: runtime, manifest, output: dir,
        exitCode: gatePass ? 0 : 1, elapsedSeconds: 5.5, timeoutSeconds: 600, maxPrivateMiB: 8192,
        sampledPeakPrivateBytes: 2000, peakWorkingSetBytes: 2500, sampleCount: 3,
        ...(gatePass ? {} : {qualityReport: quality}),
      },
    },
    unverified: [],
  }
  return candidate
}
async function rejected(mutate, id) {
  const candidate = fixture()
  await mutate(candidate)
  const result = await checkAcceptance(candidate)
  assert.equal(result.closureStatus, 'incomplete', JSON.stringify(result))
  assert.ok(result.checks.some(check => check.id === id), JSON.stringify(result))
}

test('measured control passes closure and readiness', async () => {
  const result = await checkAcceptance(fixture())
  assert.equal(result.closureStatus, 'pass')
  assert.equal(result.readiness, 'ready_for_review')
  assert.deepEqual(result.measurements, {generation: 'measured', gate: 'measured'})
})
test('normal gate pass is allowed without a quality-fail report', async () => {
  const result = await checkAcceptance(fixture({gatePass: true}))
  assert.equal(result.closureStatus, 'pass')
})
test('pending measurement keeps readiness incomplete', async () => {
  const candidate = fixture()
  candidate.measurements.generation = {status: 'measurement_pending', missing: ['sample JSONL']}
  const result = await checkAcceptance(candidate)
  assert.equal(result.closureStatus, 'pass')
  assert.equal(result.readiness, 'incomplete')
})
test('candidate summary elapsed must equal process', () => rejected(c => { c.measurements.generation.elapsedSeconds = 1 }, 'generation.elapsedSeconds-mismatch'))
test('candidate summary private peak must equal process', () => rejected(c => { c.measurements.generation.sampledPeakPrivateBytes = 1 }, 'generation.sampledPeakPrivateBytes-mismatch'))
test('candidate summary working-set peak must equal process', () => rejected(c => { c.measurements.generation.peakWorkingSetBytes = 1 }, 'generation.peakWorkingSetBytes-mismatch'))
test('candidate summary sample count must equal process', () => rejected(c => { c.measurements.generation.sampleCount = 2 }, 'generation.sampleCount-mismatch'))
test('sample PID must equal process PID and survive hash refresh', () => rejected(c => {
  const rows = readFileSync(c.measurements.generation.samples.path, 'utf8').trim().split(/\r?\n/).map(JSON.parse)
  rows[0].pid += 1
  const bytes = Buffer.from(rows.map(JSON.stringify).join('\n') + '\n')
  writeFileSync(c.measurements.generation.samples.path, bytes)
  c.measurements.generation.samples.bytes = bytes.length
  c.measurements.generation.samples.sha256 = sha(bytes)
  const process = JSON.parse(readFileSync(c.measurements.generation.process.path, 'utf8'))
  process.samplesJsonlSha256 = sha(bytes)
  rewriteJson(c.measurements.generation.process, process)
}, 'generation.sample-pid'))
test('sample peak private must match process and summary even if both are changed', () => rejected(c => {
  c.measurements.generation.sampledPeakPrivateBytes = 9999
  const process = JSON.parse(readFileSync(c.measurements.generation.process.path, 'utf8'))
  process.sampledPeakPrivateBytes = 9999
  rewriteJson(c.measurements.generation.process, process)
}, 'generation.sample-private'))
test('process samples path/hash must bind the candidate samples file', () => rejected(c => {
  const process = JSON.parse(readFileSync(c.measurements.generation.process.path, 'utf8'))
  process.samplesJsonl = join(dir, 'other.samples.jsonl')
  rewriteJson(c.measurements.generation.process, process)
}, 'generation.samples-binding'))
test('generation and gate roles cannot be swapped', () => rejected(c => {
  c.measurements.generation.operation = 'world-quality-gate'
  const process = JSON.parse(readFileSync(c.measurements.generation.process.path, 'utf8'))
  process.operation = 'world-quality-gate'
  rewriteJson(c.measurements.generation.process, process)
}, 'generation.role'))
test('measurement runtime must match candidate runtime', () => rejected(c => {
  c.measurements.generation.executable = write('other-runtime.exe', 'other')
}, 'generation.runtime-binding'))
test('measurement manifest must match candidate manifest', () => rejected(c => {
  c.measurements.generation.manifest = write('other-manifest.json', {other: true})
}, 'generation.manifest-binding'))
test('measurement output must match candidate world root', () => rejected(c => {
  c.measurements.generation.output = join(dir, 'other-world')
}, 'generation.output-mismatch'))
test('running process is rejected', () => rejected(c => {
  const process = JSON.parse(readFileSync(c.measurements.generation.process.path, 'utf8'))
  process.status = 'running'
  rewriteJson(c.measurements.generation.process, process)
}, 'generation.terminal'))
test('timeout process is rejected', () => rejected(c => {
  const process = JSON.parse(readFileSync(c.measurements.generation.process.path, 'utf8'))
  process.status = 'budget-stopped'
  rewriteJson(c.measurements.generation.process, process)
}, 'generation.terminal'))
test('non-null stopReason is rejected', () => rejected(c => {
  const process = JSON.parse(readFileSync(c.measurements.generation.process.path, 'utf8'))
  process.stopReason = 'interrupted'
  rewriteJson(c.measurements.generation.process, process)
}, 'generation.terminal'))
test('gate exit1 without a quality fail report is rejected', () => rejected(c => {
  delete c.measurements.gate.qualityReport
  c.evidence = c.evidence.filter(entry => entry.requirement !== 'A-04.gate-report')
}, 'gate.quality-report'))
test('gate exit1 with a passing quality report is rejected', () => rejected(c => {
  const bytes = Buffer.from(JSON.stringify({status: 'pass', failures: []}))
  writeFileSync(c.measurements.gate.qualityReport.path, bytes)
  for (const binding of [c.measurements.gate.qualityReport, c.evidence.find(entry => entry.requirement === 'A-04.gate-report')]) {
    binding.bytes = bytes.length
    binding.sha256 = sha(bytes)
  }
}, 'gate.quality-status'))
test('negative sample values are rejected', () => rejected(c => {
  const rows = readFileSync(c.measurements.generation.samples.path, 'utf8').trim().split(/\r?\n/).map(JSON.parse)
  rows[0].privateBytes = -1
  const bytes = Buffer.from(rows.map(JSON.stringify).join('\n') + '\n')
  writeFileSync(c.measurements.generation.samples.path, bytes)
  c.measurements.generation.samples.bytes = bytes.length
  c.measurements.generation.samples.sha256 = sha(bytes)
  const process = JSON.parse(readFileSync(c.measurements.generation.process.path, 'utf8'))
  process.samplesJsonlSha256 = sha(bytes)
  rewriteJson(c.measurements.generation.process, process)
}, 'generation.sample-fields'))
test('invalid sample UTC is rejected', () => rejected(c => {
  const rows = readFileSync(c.measurements.generation.samples.path, 'utf8').trim().split(/\r?\n/).map(JSON.parse)
  rows[0].utc = 'not-a-date'
  const bytes = Buffer.from(rows.map(JSON.stringify).join('\n') + '\n')
  writeFileSync(c.measurements.generation.samples.path, bytes)
  c.measurements.generation.samples.bytes = bytes.length
  c.measurements.generation.samples.sha256 = sha(bytes)
  const process = JSON.parse(readFileSync(c.measurements.generation.process.path, 'utf8'))
  process.samplesJsonlSha256 = sha(bytes)
  rewriteJson(c.measurements.generation.process, process)
}, 'generation.sample-utc'))
for (const label of ['generation', 'gate']) {
  test(`${label} sample UTC cannot move backwards`, () => rejected(c => {
    rewriteSamples(c, label, rows => { rows[1].utc = '2026-09-08T23:59:59.999Z' })
  }, `${label}.sample-utc-order`))
  test(`${label} equal sample UTC is allowed`, async () => {
    const candidate = fixture()
    rewriteSamples(candidate, label, rows => { rows[1].utc = rows[0].utc })
    const result = await checkAcceptance(candidate)
    assert.equal(result.closureStatus, 'pass', JSON.stringify(result))
  })
}
test('sample elapsed cannot move backwards', () => rejected(c => {
  const rows = readFileSync(c.measurements.generation.samples.path, 'utf8').trim().split(/\r?\n/).map(JSON.parse)
  rows[1].elapsedSeconds = 0.1
  const bytes = Buffer.from(rows.map(JSON.stringify).join('\n') + '\n')
  writeFileSync(c.measurements.generation.samples.path, bytes)
  c.measurements.generation.samples.bytes = bytes.length
  c.measurements.generation.samples.sha256 = sha(bytes)
  const process = JSON.parse(readFileSync(c.measurements.generation.process.path, 'utf8'))
  process.samplesJsonlSha256 = sha(bytes)
  rewriteJson(c.measurements.generation.process, process)
}, 'generation.sample-time'))
test('sample CPU cannot move backwards', () => rejected(c => {
  const rows = readFileSync(c.measurements.generation.samples.path, 'utf8').trim().split(/\r?\n/).map(JSON.parse)
  rows[1].cpuSeconds = 0.05
  const bytes = Buffer.from(rows.map(JSON.stringify).join('\n') + '\n')
  writeFileSync(c.measurements.generation.samples.path, bytes)
  c.measurements.generation.samples.bytes = bytes.length
  c.measurements.generation.samples.sha256 = sha(bytes)
  const process = JSON.parse(readFileSync(c.measurements.generation.process.path, 'utf8'))
  process.samplesJsonlSha256 = sha(bytes)
  rewriteJson(c.measurements.generation.process, process)
}, 'generation.sample-cpu'))
test('empty samples are rejected', () => rejected(c => {
  const bytes = Buffer.from('')
  writeFileSync(c.measurements.generation.samples.path, bytes)
  c.measurements.generation.samples.bytes = bytes.length
  c.measurements.generation.samples.sha256 = sha(bytes)
  const process = JSON.parse(readFileSync(c.measurements.generation.process.path, 'utf8'))
  process.samplesJsonlSha256 = sha(bytes)
  rewriteJson(c.measurements.generation.process, process)
}, 'generation.sample-count'))
test('budget cannot be raised', () => rejected(c => {
  c.measurements.generation.timeoutSeconds = 601
  const process = JSON.parse(readFileSync(c.measurements.generation.process.path, 'utf8'))
  process.timeoutSeconds = 601
  rewriteJson(c.measurements.generation.process, process)
}, 'generation.budget'))
test('missing evidence file is rejected', () => rejected(c => { c.evidence[0].path = join(dir, 'missing.json') }, 'evidence.A-01'))
test('wrong evidence hash is rejected', () => rejected(c => { c.evidence[0].sha256 = '0'.repeat(64) }, 'evidence.A-01'))
test('parity world path must identify candidate.world', () => rejected(c => {
  const parity = JSON.parse(readFileSync(c.parity.path, 'utf8'))
  parity.worldFile.path = join(dir, 'other-world.json')
  rewriteJson(c.parity, parity)
}, 'parity.world-path'))
test('parity world hash must equal candidate.world bytes', () => rejected(c => {
  const parity = JSON.parse(readFileSync(c.parity.path, 'utf8'))
  parity.worldFile.sha256 = '0'.repeat(64)
  rewriteJson(c.parity, parity)
}, 'parity.world-hash'))
test('same revision with different world bytes cannot substitute', async () => {
  const candidate = fixture()
  const other = write('other-world.json', {format: 'glyphweave-world', version: 1, revision: 'rev-final', different: true})
  candidate.world = {...other, worldRevision: 'rev-final'}
  const parity = JSON.parse(readFileSync(candidate.parity.path, 'utf8'))
  parity.worldFile = {path: other.path, sha256: other.sha256}
  rewriteJson(candidate.parity, parity)
  const result = await checkAcceptance(candidate)
  assert.equal(result.closureStatus, 'incomplete')
  assert.ok(result.checks.some(check => check.id === 'observation.binding'))
})
test('parity source/header/auditInput must be hash-bound', () => rejected(c => {
  const parity = JSON.parse(readFileSync(c.parity.path, 'utf8'))
  parity.scenes[0].auditInput.sha256 = '0'.repeat(64)
  rewriteJson(c.parity, parity)
}, 'parity.scene.0.auditInput'))
test('old runtime binding is rejected', () => rejected(c => { c.runtime.sha256 = '0'.repeat(64) }, 'runtime'))
test('old audit runtime binding is rejected', () => rejected(c => { c.auditRuntime.sha256 = '0'.repeat(64) }, 'auditRuntime'))
test('observation must bind final world bytes', () => rejected(c => {
  const observation = JSON.parse(readFileSync(c.browserObservation.path, 'utf8'))
  observation.worldSha256 = '0'.repeat(64)
  rewriteJson(c.browserObservation, observation)
}, 'observation.binding'))
test('external candidate cannot claim main-agent acceptance', () => rejected(c => { c.authority = 'main-agent-acceptance' }, 'candidate.authority'))
test('missing log closure is rejected', () => rejected(c => { c.logs = [] }, 'logs.present'))
