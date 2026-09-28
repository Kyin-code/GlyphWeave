import {test} from 'node:test'
import assert from 'node:assert/strict'
import {execFileSync} from 'node:child_process'
import {createHash} from 'node:crypto'
import {mkdtempSync, mkdirSync, writeFileSync, rmSync, readFileSync, symlinkSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join, dirname, resolve} from 'node:path'
import {fileURLToPath} from 'node:url'
import {parseCliArgs, runReceipt, DEFAULT_MAX_JSON_BYTES} from './check-evidence.mjs'

// A-D1-R2-L: independent regression contract for the frozen processReceipt API.
// The fixtures use glyphweave-world/version=1 and the real process-benchmark shape.
// Every negative mutates exactly one control fact and asserts the rejection reason.

const SCRIPT = join(dirname(fileURLToPath(import.meta.url)), 'check-evidence.mjs')
const REV = 'r2-fixture-world-revision'.padEnd(64, '0').slice(0, 64)
const GEN = 'r2-fixture-generator-revision'
const SCENE = {
  sceneId: 'scene-0',
  widthM: 6000,
  depthM: 10000,
  originX: 0,
  originZ: 0,
  seedOffset: 0,
}
const tmpRoot = mkdtempSync(join(tmpdir(), 'a-d1-r2-luna-'))

function clone(value) {
  return structuredClone(value)
}

function write(dir, rel, content) {
  const path = join(dir, rel)
  mkdirSync(dirname(path), {recursive: true})
  writeFileSync(path, typeof content === 'string' ? content : `${JSON.stringify(content, null, 2)}\n`, 'utf8')
  return path
}

function sha256(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex')
}

function validManifest(scene = SCENE) {
  return {
    format: 'glyphweave-world',
    version: 1,
    world: {
      name: 'R2 synthetic control world',
      seed: 7,
      renderMode: '2.5d',
    },
    scenes: [clone(scene)],
    style: {family: 'procedural-natural-settlement'},
    landmarks: [],
    sceneGraph: {transitions: []},
    patchChainHash: '',
  }
}

function processRecord(w, kind, overrides = {}) {
  const isGeneration = kind === 'generation'
  return {
    format: 'glyphweave.process-benchmark',
    version: 1,
    status: isGeneration ? 'baked' : 'success',
    operation: isGeneration ? 'generate-world' : 'world-quality-gate',
    timestampUtc: '2026-09-08T00:00:00.000Z',
    pid: 1000,
    executable: w.runtimePath,
    executableSha256: sha256(w.runtimePath),
    manifest: w.manifestPath,
    manifestSha256: sha256(w.manifestPath),
    output: w.dir,
    scenes: [clone(SCENE)],
    timeoutSeconds: 600,
    maxPrivateMiB: 8192,
    elapsedSeconds: 0.01,
    sampledPeakPrivateBytes: 1,
    peakWorkingSetBytes: 1,
    cpuSeconds: 0.01,
    sampleCount: 1,
    lastTraceStage: isGeneration ? 'world-publication' : 'quality-gate',
    stopReason: null,
    exitCode: 0,
    ...overrides,
  }
}

function buildWorld(dir, options = {}) {
  mkdirSync(dir, {recursive: true})
  const scenePath = write(dir, 'scenes/scene-0/scene.json', 'not parsed: stat-only scene payload\n')
  const manifestPath = write(dir, 'input.manifest.json', options.manifest === undefined ? validManifest() : options.manifest)
  const worldPath = write(dir, 'world.json', {
    format: 'glyphweave-world',
    version: 1,
    name: 'R2 synthetic control world',
    seed: 7,
    renderMode: '2.5d',
    revision: REV,
    generatorRevision: GEN,
    patchChainHash: '',
    scenes: ['scenes/scene-0/scene.json'],
    sceneGraph: {transitions: []},
    sidecar: {
      format: 'glyphweave-sidecar',
      version: 1,
      authoritativeForTerrain: true,
      sceneIndexRoot: 'scenes/',
    },
    waterLevelM: 0,
  })
  const genPath = write(dir, 'generation-report.json', {
    format: 'glyphweave.generation-report',
    version: 1,
    status: 'baked',
    generatorRevision: GEN,
    worldRevision: REV,
    sceneCount: 1,
    chunkCount: 12,
    entityCount: 5,
    nextGate: 'world-quality-gate',
    output: dir,
  })
  const gatePath = write(dir, 'world-quality-gate.json', {
    format: 'glyphweave.world-quality-gate',
    version: 1,
    status: options.gateStatus || 'pass',
    worldRevision: REV,
    failures: options.failures === undefined ? [] : options.failures,
    inputs: ['validate-world'],
  })
  const qualityReportPath = options.withFrontage === true
    ? write(dir, 'quality-report.json', {
        format: 'glyphweave.quality-report',
        version: 2,
        status: 'warn',
        worldRevision: REV,
        scenes: [{sceneId: 'scene-0', frontageAudit: {
          requested: 5,
          planned: 4,
          realizedWithAccess: 3,
          unfilled: 1,
        }}],
      })
    : null
  const runtimePath = write(dir, 'runtime/glyphweave.exe', 'synthetic runtime bytes\n')
  const genProcessPath = write(dir, 'process/generation.json', processRecord({dir, manifestPath, runtimePath}, 'generation'))
  const gateProcessPath = write(dir, 'process/gate.json', processRecord({dir, manifestPath, runtimePath}, 'gate'))
  return {dir, scenePath, worldPath, genPath, gatePath, manifestPath, qualityReportPath, runtimePath, genProcessPath, gateProcessPath}
}

function bound(path) {
  return {path, sha256: sha256(path)}
}

function controlReceipt(w, overrides = {}) {
  const receipt = {
    receiptFormat: 'glyphweave.generation-receipt',
    receiptVersion: 1,
    taskId: 'A-D1-R2-L-control',
    authority: 'main-agent-acceptance',
    sourceSnapshot: {
      commit: 'synthetic-control-commit',
      sha256: 'informational-only',
      dirtyFiles: [],
      files: [bound(w.runtimePath), bound(w.worldPath)],
    },
    runtime: bound(w.runtimePath),
    manifest: bound(w.manifestPath),
    generation: {
      operation: 'generate-world',
      command: 'generate-world --manifest input.manifest.json',
      exitCode: 0,
      status: 'baked',
      stillRunning: false,
      log: null,
      processReceipt: bound(w.genProcessPath),
    },
    gateRun: {
      operation: 'world-quality-gate',
      command: 'world-quality-gate --world world',
      exitCode: 0,
      status: 'success',
      stillRunning: false,
      log: null,
      processReceipt: bound(w.gateProcessPath),
    },
    world: {path: w.dir},
    reports: [bound(w.worldPath), bound(w.genPath), bound(w.gatePath)],
    allowedGateFailures: [],
    claimedScope: {
      worldRevision: REV,
      generatorRevision: GEN,
      sceneCount: 1,
      chunkCount: 12,
      entityCount: 5,
      frontage: null,
    },
  }
  if (w.qualityReportPath !== null) receipt.reports.push(bound(w.qualityReportPath))
  return applyReceiptOverrides(receipt, overrides)
}

function applyReceiptOverrides(receipt, overrides) {
  const out = clone(receipt)
  for (const [key, value] of Object.entries(overrides)) {
    if (value !== null && typeof value === 'object' && !Array.isArray(value) &&
        out[key] !== null && typeof out[key] === 'object' && !Array.isArray(out[key])) {
      out[key] = {...out[key], ...clone(value)}
    } else {
      out[key] = clone(value)
    }
  }
  return out
}

function writeReceipt(w, receipt, name = 'receipt.json') {
  return write(join(tmpRoot, 'receipts'), name, receipt)
}

async function runCase(w, receipt, extra = {}) {
  const receiptPath = writeReceipt(w, receipt, `${Date.now()}-${Math.random().toString(36).slice(2)}.json`)
  return runReceipt({world: w.dir, receipt: receiptPath, out: join(tmpRoot, 'unused.json'), ...extra})
}

function checkText(result) {
  return result.result.checks.map((check) =>
    `${check.id} ${check.status} ${check.reason || ''}`).join('\n')
}

function assertRejected(result, reason) {
  assert.notEqual(result.result.status, 'pass', checkText(result))
  assert.match(checkText(result), reason, `rejection reason was not specific enough:\n${checkText(result)}`)
  assert.notEqual(result.exitCode, 0)
}

function rewriteJson(path, value) {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, 'utf8')
}

function refreshProcessBinding(w, kind, changes = {}) {
  const path = kind === 'generation' ? w.genProcessPath : w.gateProcessPath
  const record = JSON.parse(readFileSync(path, 'utf8'))
  rewriteJson(path, {...record, ...changes})
  return bound(path)
}

function runCli(args) {
  try {
    const stdout = execFileSync(process.execPath, [SCRIPT, ...args], {encoding: 'utf8'})
    return {code: 0, stdout, stderr: ''}
  } catch (error) {
    return {
      code: error.status === undefined ? -1 : error.status,
      stdout: String(error.stdout || ''),
      stderr: String(error.stderr || ''),
    }
  }
}

// ---------------------------------------------------------------------------
// Positive controls: these prove the fixture is not a universal rejection.
// ---------------------------------------------------------------------------

test('R2 control: complete processReceipt evidence passes', async () => {
  const w = buildWorld(join(tmpRoot, 'p-control'))
  const result = await runCase(w, controlReceipt(w))
  assert.equal(result.result.status, 'pass', checkText(result))
  assert.equal(result.exitCode, 0)
  assert.equal(result.result.worldQualityStatus.value, 'pass')
})

test('R2 control: authorized normal gate failure passes evidence but preserves quality fail', async () => {
  const w = buildWorld(join(tmpRoot, 'p-authorized-fail'), {
    gateStatus: 'fail',
    failures: ['frontage.unfilled'],
  })
  const gateProcess = refreshProcessBinding(w, 'gate', {status: 'failed', exitCode: 1, stopReason: null})
  const receipt = controlReceipt(w, {
    gateRun: {status: 'failed', exitCode: 1, processReceipt: gateProcess},
    allowedGateFailures: ['frontage.unfilled'],
  })
  const result = await runCase(w, receipt)
  assert.equal(result.result.status, 'pass', checkText(result))
  assert.equal(result.exitCode, 0)
  assert.equal(result.result.worldQualityStatus.value, 'fail')
})

test('R2 control: frontage ledger and stat-only scene remain accepted', async () => {
  const w = buildWorld(join(tmpRoot, 'p-frontage'), {withFrontage: true})
  const receipt = controlReceipt(w, {
    claimedScope: {frontage: {
      sceneId: 'scene-0',
      requested: 5,
      planned: 4,
      realizedWithAccess: 3,
      unfilled: 1,
      auditPath: w.qualityReportPath,
    }},
  })
  const result = await runCase(w, receipt)
  assert.equal(result.result.status, 'pass', checkText(result))
})

test('R2 control: equivalent dot path for world root is accepted', async () => {
  const w = buildWorld(join(tmpRoot, 'p-world-alias'))
  const result = await runCase(w, controlReceipt(w, {world: {path: join(w.dir, '.')}}))
  assert.equal(result.result.status, 'pass', checkText(result))
})

// ---------------------------------------------------------------------------
// F1/F2: structured operation identity and terminal-state semantics.
// ---------------------------------------------------------------------------

test('rejects generation outer operation mismatch, regardless of command text', async () => {
  const w = buildWorld(join(tmpRoot, 'n-operation-generation'))
  const result = await runCase(w, controlReceipt(w, {generation: {operation: 'preview-world'}}))
  assertRejected(result, /generation.*operation|generate-world/i)
})

test('rejects gate process operation mismatch, regardless of command text', async () => {
  const w = buildWorld(join(tmpRoot, 'n-operation-gate-process'))
  const processReceipt = refreshProcessBinding(w, 'gate', {operation: 'generate-world'})
  const result = await runCase(w, controlReceipt(w, {gateRun: {processReceipt}}))
  assertRejected(result, /gateRun.*operation|world-quality-gate/i)
})

test('log-only generation and gate receipts are incomplete', async () => {
  const w = buildWorld(join(tmpRoot, 'n-log-only'))
  const receipt = controlReceipt(w, {
    generation: {log: bound(write(w.dir, 'logs/generation.log', 'synthetic log')),
      processReceipt: null},
    gateRun: {log: bound(write(w.dir, 'logs/gate.log', 'synthetic log')),
      processReceipt: null},
  })
  const result = await runCase(w, receipt)
  assertRejected(result, /processReceipt|structured|log-only|process evidence/i)
})

test('rejects generation process timeout even with exitCode zero', async () => {
  const w = buildWorld(join(tmpRoot, 'n-timeout-generation'))
  const processReceipt = refreshProcessBinding(w, 'generation', {stopReason: 'timeout'})
  const result = await runCase(w, controlReceipt(w, {generation: {processReceipt}}))
  assertRejected(result, /timeout|stopReason|interrupted|terminal/i)
})

test('rejects gate budget stop even when failure is authorized', async () => {
  const w = buildWorld(join(tmpRoot, 'n-budget-gate'), {
    gateStatus: 'fail',
    failures: ['frontage.unfilled'],
  })
  const processReceipt = refreshProcessBinding(w, 'gate', {
    status: 'failed', exitCode: 1, stopReason: 'budget',
  })
  const receipt = controlReceipt(w, {
    gateRun: {status: 'failed', exitCode: 1, processReceipt},
    allowedGateFailures: ['frontage.unfilled'],
  })
  const result = await runCase(w, receipt)
  assertRejected(result, /budget|stopReason|interrupted|complete/i)
})

test('rejects stillRunning and outer stopReason contradictions', async () => {
  const w = buildWorld(join(tmpRoot, 'n-running-outer'))
  const result = await runCase(w, controlReceipt(w, {
    generation: {stillRunning: true, stopReason: 'timeout'},
  }))
  assertRejected(result, /still.?running|timeout|stopReason|running/i)
})

// ---------------------------------------------------------------------------
// Process binding: each case changes one structured field and refreshes the
// processReceipt hash. This prevents a stale-hash failure from masking the
// intended runtime/manifest/output/exitCode assertion.
// ---------------------------------------------------------------------------

test('rejects process runtime identity conflict', async () => {
  const w = buildWorld(join(tmpRoot, 'n-process-runtime'))
  const otherRuntime = write(w.dir, 'runtime/other.exe', 'different runtime\n')
  const processReceipt = refreshProcessBinding(w, 'generation', {
    executable: otherRuntime,
    executableSha256: sha256(otherRuntime),
  })
  const result = await runCase(w, controlReceipt(w, {generation: {processReceipt}}))
  assertRejected(result, /runtime|executable|source|identity/i)
})

test('rejects process manifest identity conflict', async () => {
  const w = buildWorld(join(tmpRoot, 'n-process-manifest'))
  const otherManifest = write(w.dir, 'other.manifest.json', validManifest({...SCENE, seedOffset: 1}))
  const processReceipt = refreshProcessBinding(w, 'generation', {
    manifest: otherManifest,
    manifestSha256: sha256(otherManifest),
  })
  const result = await runCase(w, controlReceipt(w, {generation: {processReceipt}}))
  assertRejected(result, /manifest|identity|process/i)
})

test('rejects process output directory conflict', async () => {
  const w = buildWorld(join(tmpRoot, 'n-process-output'))
  const otherOutput = join(w.dir, 'other-output')
  mkdirSync(otherOutput)
  const processReceipt = refreshProcessBinding(w, 'generation', {output: otherOutput})
  const result = await runCase(w, controlReceipt(w, {generation: {processReceipt}}))
  assertRejected(result, /output|world|directory/i)
})

test('rejects process exitCode conflict while outer receipt remains zero', async () => {
  const w = buildWorld(join(tmpRoot, 'n-process-exit'))
  const processReceipt = refreshProcessBinding(w, 'generation', {exitCode: 1, status: 'baked'})
  const result = await runCase(w, controlReceipt(w, {generation: {processReceipt}}))
  assertRejected(result, /exitCode|exit|process/i)
})

test('rejects process scene geometry conflict with the real manifest', async () => {
  const w = buildWorld(join(tmpRoot, 'n-process-scene'))
  const processReceipt = refreshProcessBinding(w, 'generation', {
    scenes: [{...SCENE, widthM: 6001}],
  })
  const result = await runCase(w, controlReceipt(w, {generation: {processReceipt}}))
  assertRejected(result, /scene|width|dimension|manifest/i)
})

// ---------------------------------------------------------------------------
// F3/F4: manifest shape, scene matching, and exact world-root identity.
// ---------------------------------------------------------------------------

test('rejects non-JSON manifest with a manifest-specific reason', async () => {
  const w = buildWorld(join(tmpRoot, 'n-manifest-json'), {manifest: '{broken'})
  const result = await runCase(w, controlReceipt(w))
  assertRejected(result, /manifest|json|parse|valid/i)
})

test('rejects unknown manifest version after updating all relevant hashes', async () => {
  const w = buildWorld(join(tmpRoot, 'n-manifest-version'), {manifest: {...validManifest(), version: 99}})
  const processReceipt = refreshProcessBinding(w, 'generation', {
    manifestSha256: sha256(w.manifestPath),
  })
  const gateProcess = refreshProcessBinding(w, 'gate', {
    manifestSha256: sha256(w.manifestPath),
  })
  const result = await runCase(w, controlReceipt(w, {
    manifest: bound(w.manifestPath),
    generation: {processReceipt},
    gateRun: {processReceipt: gateProcess},
  }))
  assertRejected(result, /manifest|version|unsupported/i)
})

test('rejects manifest/process scene dimension conflict', async () => {
  const w = buildWorld(join(tmpRoot, 'n-manifest-scene-dimension'))
  const manifest = JSON.parse(readFileSync(w.manifestPath, 'utf8'))
  manifest.scenes[0].depthM = 9999
  rewriteJson(w.manifestPath, manifest)
  // Refresh identity hashes so the only changed fact is the scene geometry.
  const generationProcess = refreshProcessBinding(w, 'generation', {
    manifestSha256: sha256(w.manifestPath),
  })
  const gateProcess = refreshProcessBinding(w, 'gate', {
    manifestSha256: sha256(w.manifestPath),
  })
  const receipt = controlReceipt(w, {
    manifest: bound(w.manifestPath),
    generation: {processReceipt: generationProcess},
    gateRun: {processReceipt: gateProcess},
  })
  const result = await runCase(w, receipt)
  assertRejected(result, /manifest|scene|dimension|depth/i)
})

test('rejects a receipt world path that names a subdirectory, not the world root', async () => {
  const w = buildWorld(join(tmpRoot, 'n-world-subdirectory'))
  const subdir = join(w.dir, 'scenes')
  const result = await runCase(w, controlReceipt(w, {world: {path: subdir}}))
  assertRejected(result, /world.*root|world.*path|exact|identity|subdirector/i)
})

// ---------------------------------------------------------------------------
// F5: failures is always a string array; pass and fail semantics are separate.
// ---------------------------------------------------------------------------

for (const [label, failures, pattern] of [
  ['missing', undefined, /failures|array|required/i],
  ['null', null, /failures|array|null/i],
  ['object', {reason: 'not-an-array'}, /failures|array|object/i],
  ['scalar', 'not-an-array', /failures|array|string/i],
  ['non-string item', ['ok', 7], /failures|string|entry/i],
]) {
  test(`rejects pass gate failures shape: ${label}`, async () => {
    const w = buildWorld(join(tmpRoot, `n-pass-failures-${label.replace(/ /g, '-')}`))
    const gate = JSON.parse(readFileSync(w.gatePath, 'utf8'))
    if (failures === undefined) delete gate.failures
    else gate.failures = failures
    rewriteJson(w.gatePath, gate)
    const result = await runCase(w, controlReceipt(w, {
      reports: [bound(w.worldPath), bound(w.genPath), bound(w.gatePath)],
    }))
    assertRejected(result, pattern)
  })
}

test('rejects failed gate with empty failures even when exitCode is one', async () => {
  const w = buildWorld(join(tmpRoot, 'n-fail-empty'), {gateStatus: 'fail', failures: []})
  const processReceipt = refreshProcessBinding(w, 'gate', {status: 'failed', exitCode: 1})
  const result = await runCase(w, controlReceipt(w, {
    gateRun: {status: 'failed', exitCode: 1, processReceipt},
  }))
  assertRejected(result, /failures|empty|failure/i)
})

test('rejects failed gate when a failure is not in the authorized baseline', async () => {
  const w = buildWorld(join(tmpRoot, 'n-fail-unauthorized'), {
    gateStatus: 'fail', failures: ['unexpected.failure'],
  })
  const processReceipt = refreshProcessBinding(w, 'gate', {status: 'failed', exitCode: 1})
  const result = await runCase(w, controlReceipt(w, {
    gateRun: {status: 'failed', exitCode: 1, processReceipt},
    allowedGateFailures: ['different.failure'],
  }))
  assertRejected(result, /authori[sz]|baseline|unexpected|failure/i)
})

test('rejects pass gate that reports any failure string', async () => {
  const w = buildWorld(join(tmpRoot, 'n-pass-with-failure'), {failures: ['silent.failure']})
  const result = await runCase(w, controlReceipt(w))
  assertRejected(result, /pass|failure|failures|contradict/i)
})

test('demo authority does not pass complete evidence', async () => {
  const w = buildWorld(join(tmpRoot, 'n-demo'))
  const result = await runCase(w, controlReceipt(w, {authority: 'demo'}))
  assertRejected(result, /authority|main-agent|acceptance|demo/i)
})

// ---------------------------------------------------------------------------
// Preserved R1 regressions: read-only output, byte budget, CLI and hash paths.
// ---------------------------------------------------------------------------

test('scene payload is stat-only and need not be valid JSON', async () => {
  const w = buildWorld(join(tmpRoot, 'p-stat-only'))
  writeFileSync(w.scenePath, 'this is deliberately not JSON', 'utf8')
  const result = await runCase(w, controlReceipt(w))
  assert.equal(result.result.status, 'pass', checkText(result))
})

test('byte budget rejects zero and safe defaults remain positive', async () => {
  assert.ok(parseCliArgs(['--max-json-bytes', '0']).errs.length > 0)
  const w = buildWorld(join(tmpRoot, 'p-byte-default'))
  const result = await runCase(w, controlReceipt(w))
  assert.equal(result.result.status, 'pass', checkText(result))
  assert.ok(DEFAULT_MAX_JSON_BYTES > 0)
})

test('CLI full control writes output outside the read-only world and exits zero', async () => {
  const w = buildWorld(join(tmpRoot, 'p-cli'))
  const receiptPath = writeReceipt(w, controlReceipt(w), 'cli-control.json')
  const out = join(tmpRoot, 'cli-result.json')
  const result = runCli(['--world', w.dir, '--receipt', receiptPath, '--out', out])
  assert.equal(result.code, 0, `${result.stdout}\n${result.stderr}`)
  assert.equal(JSON.parse(readFileSync(out, 'utf8')).status, 'pass')
})

test('CLI refuses an output path inside the read-only world tree with usage exit two', () => {
  const w = buildWorld(join(tmpRoot, 'n-cli-output-inside'))
  const receiptPath = writeReceipt(w, controlReceipt(w), 'cli-inside.json')
  const result = runCli(['--world', w.dir, '--receipt', receiptPath, '--out', join(w.dir, 'result.json')])
  assert.equal(result.code, 2, `${result.stdout}\n${result.stderr}`)
})

test('CLI missing required arguments remains usage exit two', () => {
  assert.equal(runCli(['--world', 'missing']).code, 2)
  assert.equal(runCli(['--bogus']).code, 2)
})

test('missing required artifact is rejected with its file name', async () => {
  const w = buildWorld(join(tmpRoot, 'n-missing-artifact'))
  const receipt = controlReceipt(w)
  rmSync(w.gatePath)
  const result = await runCase(w, receipt)
  assertRejected(result, /world-quality-gate|required|missing/i)
})
// ---------------------------------------------------------------------------
// Restored R1 security regressions. These retain the R2 fixture and process
// receipts; each subcase mutates exactly one control fact and refreshes all
// affected binding hashes.
// ---------------------------------------------------------------------------

function mutateReport(w, key, mutate) {
  const path = w[key]
  const data = JSON.parse(readFileSync(path, 'utf8'))
  mutate(data)
  rewriteJson(path, data)
}

test('restored sourceSnapshot binding regressions', async (t) => {
  const cases = [
    ['empty sourceSnapshot.files', (w, receipt) => {
      receipt.sourceSnapshot = {commit: 'synthetic-control-commit', files: []}
      return receipt
    }],
    ['wrong sourceSnapshot hash', (w, receipt) => {
      receipt.sourceSnapshot.files = [{path: w.runtimePath, sha256: '0'.repeat(64)}]
      return receipt
    }],
    ['missing sourceSnapshot file', (w, receipt) => {
      receipt.sourceSnapshot.files = [{path: join(w.dir, 'missing-source.bin'), sha256: '0'.repeat(64)}]
      return receipt
    }],
  ]
  for (const [label, makeReceipt] of cases) {
    await t.test(label, async () => {
      const w = buildWorld(join(tmpRoot, `r1-source-${label.replace(/[^a-z]+/gi, '-')}`))
      const result = await runCase(w, makeReceipt(w, controlReceipt(w)))
      assertRejected(result, /sourceSnapshot|source snapshot|hash|exist|file/i)
    })
  }
})

test('restored required report hash coverage', async (t) => {
  for (const [label, reportName] of [
    ['world report', 'world.json'],
    ['generation report', 'generation-report.json'],
    ['gate report', 'world-quality-gate.json'],
  ]) {
    await t.test(`receipt.reports binds ${label}`, async () => {
      const w = buildWorld(join(tmpRoot, `r1-report-coverage-${label.split(' ')[0]}`))
      const receipt = controlReceipt(w)
      receipt.reports = receipt.reports.filter((entry) => !entry.path.endsWith(reportName))
      const result = await runCase(w, receipt)
      assertRejected(result, /reports|hash-bind|binding|not.*bound|receipt/i)
    })
  }
})

test('restored required report hash mismatch coverage', async (t) => {
  for (const [label, reportName] of [
    ['world report', 'world.json'],
    ['generation report', 'generation-report.json'],
    ['gate report', 'world-quality-gate.json'],
  ]) {
    await t.test(`detects wrong hash for ${label}`, async () => {
      const w = buildWorld(join(tmpRoot, `r1-report-hash-${label.split(' ')[0]}`))
      const receipt = controlReceipt(w)
      const entry = receipt.reports.find((candidate) => candidate.path.endsWith(reportName))
      assert.ok(entry, `missing fixture report binding for ${reportName}`)
      entry.sha256 = '0'.repeat(64)
      const result = await runCase(w, receipt)
      assertRejected(result, /reports|sha256|mismatch|hash/i)
    })
  }
})

test('restored runtime hash mismatch regression', async () => {
  const w = buildWorld(join(tmpRoot, 'r1-runtime-hash'))
  const result = await runCase(w, controlReceipt(w, {
    runtime: {path: w.runtimePath, sha256: '0'.repeat(64)},
  }))
  assertRejected(result, /runtime|sha256|mismatch|hash/i)
})

test('restored report format/version/revision regressions', async (t) => {
  const reports = [
    ['world', 'worldPath'],
    ['generation', 'genPath'],
    ['gate', 'gatePath'],
  ]
  for (const [label, key] of reports) {
    for (const [kind, mutate, reason] of [
      ['format', (data) => { data.format = 'wrong-format' }, /format|schema/i],
      ['version', (data) => { data.version = 99 }, /version|unsupported/i],
      ['revision', (data) => {
        if (label === 'world') data.revision = 'wrong-revision'
        else data.worldRevision = 'wrong-revision'
      }, /revision|consistent|match/i],
    ]) {
      await t.test(`${label} report ${kind}`, async () => {
        const w = buildWorld(join(tmpRoot, `r1-report-${label}-${kind}`))
        mutateReport(w, key, mutate)
        const result = await runCase(w, controlReceipt(w))
        assertRejected(result, reason)
      })
    }
  }
})

test('restored unknown gate status regression', async () => {
  const w = buildWorld(join(tmpRoot, 'r1-unknown-gate'))
  mutateReport(w, 'gatePath', (gate) => { gate.status = 'banana' })
  const result = await runCase(w, controlReceipt(w))
  assertRejected(result, /gate|status|recognised|unknown/i)
})

test('restored lexical scene escape regression', async () => {
  const w = buildWorld(join(tmpRoot, 'r1-lexical-scene-escape'))
  write(tmpRoot, 'lexical-outside/scene.json', 'outside scene payload')
  mutateReport(w, 'worldPath', (world) => {
    world.scenes = ['../lexical-outside/scene.json']
  })
  const result = await runCase(w, controlReceipt(w))
  assertRejected(result, /scene|contain|outside|escape|world/i)
})

test('restored junction scene escape regression', async (t) => {
  const w = buildWorld(join(tmpRoot, 'r1-junction-scene-escape'))
  const target = join(tmpRoot, 'junction-target')
  const link = join(w.dir, 'escape-link')
  mkdirSync(target, {recursive: true})
  writeFileSync(join(target, 'scene.json'), 'outside junction scene payload', 'utf8')
  try {
    rmSync(link, {recursive: true, force: true})
    if (process.platform === 'win32') {
      execFileSync('cmd', ['/c', 'mklink', '/J', link, target], {stdio: 'ignore'})
    } else {
      symlinkSync(target, link, 'junction')
    }
  } catch {
    t.skip('cannot create junction/symlink in this environment')
    return
  }
  try {
    mutateReport(w, 'worldPath', (world) => { world.scenes = ['escape-link/scene.json'] })
    const result = await runCase(w, controlReceipt(w))
    assertRejected(result, /scene|contain|outside|escape|junction|world/i)
  } finally {
    rmSync(link, {recursive: true, force: true})
  }
})

test('restored CLI refuses to overwrite an existing --out file', () => {
  const w = buildWorld(join(tmpRoot, 'r1-out-overwrite'))
  const receiptPath = writeReceipt(w, controlReceipt(w), 'r1-out-overwrite-receipt.json')
  const out = write(tmpRoot, 'already-existing-result.json', 'occupied')
  const result = runCli(['--world', w.dir, '--receipt', receiptPath, '--out', out])
  assert.equal(result.code, 2, `${result.stdout}\n${result.stderr}`)
  assert.match(`${result.stdout} ${result.stderr}`, /exist|overwrite|exclusive|occupied/i)
})

test('restored directory masquerade regressions for required reports', async (t) => {
  for (const [label, key] of [
    ['world', 'worldPath'],
    ['generation', 'genPath'],
    ['gate', 'gatePath'],
  ]) {
    await t.test(`${label} report directory is not accepted as JSON`, async () => {
      const w = buildWorld(join(tmpRoot, `r1-directory-report-${label}`))
      const receipt = controlReceipt(w)
      rmSync(w[key], {recursive: true, force: true})
      mkdirSync(w[key], {recursive: true})
      const result = await runCase(w, receipt)
      assertRejected(result, /directory|regular|report|JSON|read/i)
    })
  }
})

test('restored directory masquerade regression for receipt', async () => {
  const w = buildWorld(join(tmpRoot, 'r1-directory-receipt'))
  const receiptDir = join(tmpRoot, 'receipt-as-directory-r1')
  mkdirSync(receiptDir, {recursive: true})
  const result = await runReceipt({
    world: w.dir,
    receipt: receiptDir,
    out: join(tmpRoot, 'r1-directory-receipt-out.json'),
  })
  assertRejected(result, /receipt|JSON|read|directory/i)
})

test('restored oversize JSON regressions for required reports', async (t) => {
  for (const [label, key] of [
    ['world', 'worldPath'],
    ['generation', 'genPath'],
    ['gate', 'gatePath'],
  ]) {
    await t.test(`${label} JSON exceeds the configured read budget`, async () => {
      const w = buildWorld(join(tmpRoot, `r1-oversize-${label}`))
      mutateReport(w, key, (data) => { data.syntheticPadding = 'x'.repeat(5000) })
      const result = await runCase(w, controlReceipt(w), {maxJsonBytes: 4096})
      assertRejected(result, /too_large|over|json|max-json|budget/i)
    })
  }
})

// ---------------------------------------------------------------------------
// Five independent R2 regressions supplied by the coordinator. Each case
// changes one field only and refreshes the affected process/receipt hashes.
// ---------------------------------------------------------------------------

test('independent regression: missing process.stopReason is rejected', async () => {
  const w = buildWorld(join(tmpRoot, 'independent-missing-stop-reason'))
  const process = JSON.parse(readFileSync(w.gateProcessPath, 'utf8'))
  delete process.stopReason
  rewriteJson(w.gateProcessPath, process)
  const receipt = controlReceipt(w)
  receipt.gateRun.processReceipt = bound(w.gateProcessPath)
  const result = await runCase(w, receipt)
  assertRejected(result, /stopReason|missing|process/i)
})

test('independent regression: canonical world sceneId must match manifest and both process scenes', async () => {
  const w = buildWorld(join(tmpRoot, 'independent-scene-id'))
  const manifest = JSON.parse(readFileSync(w.manifestPath, 'utf8'))
  manifest.scenes[0].sceneId = 'another-scene'
  rewriteJson(w.manifestPath, manifest)
  const receipt = controlReceipt(w)
  receipt.manifest = bound(w.manifestPath)
  for (const [key, processPath] of [['generation', w.genProcessPath], ['gateRun', w.gateProcessPath]]) {
    const process = JSON.parse(readFileSync(processPath, 'utf8'))
    process.manifestSha256 = receipt.manifest.sha256
    process.scenes = clone(manifest.scenes)
    rewriteJson(processPath, process)
    receipt[key].processReceipt = bound(processPath)
  }
  const result = await runCase(w, receipt)
  assertRejected(result, /sceneId|scene|canonical|world|path|identity/i)
})

test('independent regression: manifest/world seed mismatch is rejected', async () => {
  const w = buildWorld(join(tmpRoot, 'independent-seed'))
  const manifest = JSON.parse(readFileSync(w.manifestPath, 'utf8'))
  manifest.world.seed = 999
  rewriteJson(w.manifestPath, manifest)
  const receipt = controlReceipt(w)
  receipt.manifest = bound(w.manifestPath)
  for (const [key, processPath] of [['generation', w.genProcessPath], ['gateRun', w.gateProcessPath]]) {
    const process = JSON.parse(readFileSync(processPath, 'utf8'))
    process.manifestSha256 = receipt.manifest.sha256
    rewriteJson(processPath, process)
    receipt[key].processReceipt = bound(processPath)
  }
  const result = await runCase(w, receipt)
  assertRejected(result, /seed|manifest|world|match|identity/i)
})

test('independent regression: generation-report.output must be the world root', async () => {
  const w = buildWorld(join(tmpRoot, 'independent-generation-output'))
  mutateReport(w, 'genPath', (report) => { report.output = join(w.dir, 'scenes') })
  const result = await runCase(w, controlReceipt(w))
  assertRejected(result, /output|world|root|directory|generation/i)
})

test('independent regression: Rust SceneSpec widthM=0.5 is rejected', async () => {
  const w = buildWorld(join(tmpRoot, 'independent-fractional-scene'))
  const manifest = JSON.parse(readFileSync(w.manifestPath, 'utf8'))
  manifest.scenes[0].widthM = 0.5
  rewriteJson(w.manifestPath, manifest)
  const receipt = controlReceipt(w)
  receipt.manifest = bound(w.manifestPath)
  for (const [key, processPath] of [['generation', w.genProcessPath], ['gateRun', w.gateProcessPath]]) {
    const process = JSON.parse(readFileSync(processPath, 'utf8'))
    process.manifestSha256 = receipt.manifest.sha256
    process.scenes = clone(manifest.scenes)
    rewriteJson(processPath, process)
    receipt[key].processReceipt = bound(processPath)
  }
  const result = await runCase(w, receipt)
  assertRejected(result, /widthM|width|scene|integer|fraction|Rust|schema/i)
})
