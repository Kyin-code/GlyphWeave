#!/usr/bin/env node
// Validate a final-acceptance candidate closure before any reviewer accepts it.
// This verifies content identity only; it does not authenticate authorship or
// prove that a binary was compiled from the listed source.
import {createHash} from 'node:crypto'
import {createReadStream, existsSync} from 'node:fs'
import {open, readFile, realpath, stat} from 'node:fs/promises'
import {isAbsolute, relative, resolve, sep} from 'node:path'

export const FORMAT = 'glyphweave.final-acceptance-candidate'
export const VERSION = 1
export const MAX_JSON_BYTES = 64 * 1024 * 1024

class AcceptanceError extends Error {
  constructor(id, reason, status = 'incomplete') {
    super(reason)
    this.id = id
    this.status = status
  }
}

const need = (ok, id, reason, status = 'incomplete') => {
  if (!ok) throw new AcceptanceError(id, reason, status)
}
const hex = value => typeof value === 'string' && /^[0-9a-f]{64}$/i.test(value)
const digest = bytes => createHash('sha256').update(bytes).digest('hex')
const inside = (child, parent) => {
  const rel = relative(parent, child)
  return rel === '' || (!isAbsolute(rel) && rel !== '..' && !rel.startsWith('..' + sep))
}
const samePath = async (a, b) => {
  try { return await realpath(resolve(a)) === await realpath(resolve(b)) } catch { return false }
}

async function readBound(binding, id, {json = false, raw = false} = {}) {
  need(binding && typeof binding.path === 'string' && binding.path.length > 0
    && Number.isSafeInteger(binding.bytes) && binding.bytes >= 0 && hex(binding.sha256),
    id, `${id} needs path, bytes, sha256`)
  const path = resolve(binding.path)
  let info
  try { info = await stat(path) } catch { throw new AcceptanceError(id, `${id} file is missing: ${binding.path}`) }
  need(info.isFile(), id, `${id} is not a regular file: ${binding.path}`)
  need(info.size === binding.bytes, id, `${id} bytes mismatch: expected ${binding.bytes}, got ${info.size}`)
  need(info.size <= MAX_JSON_BYTES, id, `${id} exceeds ${MAX_JSON_BYTES} bytes`)
  const handle = await open(path, 'r')
  let bytes
  try {
    bytes = Buffer.alloc(info.size)
    let offset = 0
    while (offset < bytes.length) {
      const read = await handle.read(bytes, offset, bytes.length - offset, offset)
      need(read.bytesRead > 0, id, `${id} truncated while reading`)
      offset += read.bytesRead
    }
  } finally { await handle.close() }
  const sha256 = digest(bytes)
  need(sha256 === binding.sha256.toLowerCase(), id, `${id} sha256 mismatch: expected ${binding.sha256}, got ${sha256}`)
  if (!json) return {path, bytes: bytes.length, sha256, ...(raw ? {rawBytes: bytes} : {})}
  try { return {path, bytes: bytes.length, sha256, data: JSON.parse(bytes.toString('utf8')), ...(raw ? {rawBytes: bytes} : {})} }
  catch (error) { throw new AcceptanceError(id, `${id} is not valid JSON: ${String(error.message || error)}`) }
}

async function hashLargeBound(binding, id) {
  need(binding && typeof binding.path === 'string' && binding.path.length > 0
    && Number.isSafeInteger(binding.bytes) && binding.bytes >= 0 && hex(binding.sha256),
    id, `${id} needs path, bytes, sha256`)
  const path = resolve(binding.path)
  let info
  try { info = await stat(path) } catch { throw new AcceptanceError(id, `${id} file is missing: ${binding.path}`) }
  need(info.isFile(), id, `${id} is not a regular file: ${binding.path}`)
  need(info.size === binding.bytes, id, `${id} bytes mismatch: expected ${binding.bytes}, got ${info.size}`)
  const hash = createHash('sha256')
  let bytes = 0
  for await (const chunk of createReadStream(path)) { bytes += chunk.length; hash.update(chunk) }
  const sha256 = hash.digest('hex')
  need(bytes === binding.bytes, id, `${id} streamed bytes mismatch`)
  need(sha256 === binding.sha256.toLowerCase(), id, `${id} sha256 mismatch: expected ${binding.sha256}, got ${sha256}`)
  return {path, bytes, sha256}
}

async function checkParityBindings(parity, parityPath, worldBinding, worldData) {
  const worldRoot = await realpath(resolve(worldBinding.path, '..'))
  const parityRoot = await realpath(resolve(parityPath, '..'))
  need(parity.worldFile && typeof parity.worldFile.path === 'string' && hex(parity.worldFile.sha256),
    'parity.worldFile', 'parity.worldFile path/sha256 is missing', 'fail')
  need(await samePath(parity.worldFile.path, worldBinding.path),
    'parity.world-path', 'parity.worldFile.path does not identify candidate.world', 'fail')
  need(parity.worldFile.sha256.toLowerCase() === worldBinding.sha256.toLowerCase(),
    'parity.world-hash', 'parity.worldFile.sha256 does not match candidate.world bytes', 'fail')
  need(Array.isArray(parity.scenes) && parity.scenes.length > 0,
    'parity.scenes', 'parity scenes are missing', 'fail')
  for (const [index, scene] of parity.scenes.entries()) {
    for (const field of ['source', 'header', 'auditInput']) {
      const binding = scene[field]
      need(binding && typeof binding.path === 'string' && Number.isSafeInteger(binding.bytes) && hex(binding.sha256),
        `parity.scene.${index}.${field}`, `parity scene ${index} ${field} binding is incomplete`, 'fail')
      const base = field === 'auditInput' ? parityRoot : worldRoot
      const absoluteBinding = {...binding, path: isAbsolute(binding.path) ? binding.path : resolve(base, binding.path)}
      const resolved = await hashLargeBound(absoluteBinding, `parity.scene.${index}.${field}`)
      if (field === 'auditInput') {
        need(inside(await realpath(resolved.path), parityRoot),
          `parity.scene.${index}.auditInput.containment`, 'auditInput escapes parity directory', 'fail')
      } else {
        need(inside(await realpath(resolved.path), worldRoot),
          `parity.scene.${index}.${field}.containment`, `${field} escapes world directory`, 'fail')
      }
    }
  }
  need(parity.worldRevision === worldData.revision,
    'parity.worldRevision', 'parity worldRevision does not match world.json', 'fail')
}

const safeNonNegativeInteger = value => Number.isSafeInteger(value) && value >= 0
const finiteNonNegative = value => typeof value === 'number' && Number.isFinite(value) && value >= 0

async function checkMeasurement(measurement, label, missing, context) {
  if (measurement?.status === 'measurement_pending') {
    need(Array.isArray(measurement.missing) && measurement.missing.length > 0,
      `${label}.missing`, `${label} pending state must list concrete missing evidence`, 'fail')
    missing.push(...measurement.missing.map(item => `${label}: ${item}`))
    return {status: 'pending'}
  }
  need(measurement?.status === 'measured', `${label}.status`,
    `${label} must be measured or measurement_pending`, 'fail')
  const expectedOperation = label === 'generation' ? 'generate-world' : 'world-quality-gate'
  need(measurement.operation === expectedOperation, `${label}.role`,
    `${label} must record operation ${expectedOperation}`, 'fail')
  const process = await readBound(measurement.process, `${label}.process`, {json: true})
  const samples = await readBound(measurement.samples, `${label}.samples`, {raw: true})
  const executable = await readBound(measurement.executable, `${label}.executable`)
  const manifest = await readBound(measurement.manifest, `${label}.manifest`)
  const processData = process.data
  need(await samePath(measurement.executable.path, context.runtime.path)
    && measurement.executable.bytes === context.runtime.bytes
    && measurement.executable.sha256.toLowerCase() === context.runtime.sha256.toLowerCase(),
    `${label}.runtime-binding`, `${label} executable binding does not match candidate.runtime`, 'fail')
  need(await samePath(measurement.manifest.path, context.manifest.path)
    && measurement.manifest.bytes === context.manifest.bytes
    && measurement.manifest.sha256.toLowerCase() === context.manifest.sha256.toLowerCase(),
    `${label}.manifest-binding`, `${label} manifest binding does not match candidate.manifest`, 'fail')
  need(processData.format === 'glyphweave.process-benchmark' && processData.version === 1,
    `${label}.process.schema`, `${label} process report is not glyphweave.process-benchmark/v1`, 'fail')
  need(processData.operation === expectedOperation, `${label}.process-operation`,
    `${label} process operation mismatch`, 'fail')
  need(safeNonNegativeInteger(processData.pid) && processData.pid > 0, `${label}.pid`,
    `${label} process pid is missing or invalid`, 'fail')
  need(await samePath(processData.executable, context.runtime.path)
    && processData.executableSha256?.toLowerCase() === context.runtime.sha256.toLowerCase(),
    `${label}.runtime`, `${label} process executable/hash mismatch`, 'fail')
  need(await samePath(processData.manifest, context.manifest.path)
    && processData.manifestSha256?.toLowerCase() === context.manifest.sha256.toLowerCase(),
    `${label}.manifest`, `${label} process manifest/hash mismatch`, 'fail')
  need(await samePath(processData.output, context.worldRoot), `${label}.output`,
    `${label} process output does not identify candidate world root`, 'fail')
  need(await samePath(measurement.output, processData.output), `${label}.output-mismatch`,
    `${label} summary output differs from process output`, 'fail')
  need(await samePath(processData.samplesJsonl, measurement.samples.path)
    && processData.samplesJsonlSha256?.toLowerCase() === samples.sha256.toLowerCase(),
    `${label}.samples-binding`, `${label} process samplesJsonl path/hash mismatch`, 'fail')
  if (label === 'generation') {
    need(processData.status === 'baked' && processData.exitCode === 0 && processData.stopReason === null,
      `${label}.terminal`, `${label} must be baked/exit0/null stopReason`, 'fail')
  } else {
    if (processData.exitCode === 0) {
      need(processData.status === 'completed' && processData.stopReason === null,
        `${label}.terminal`, `${label} pass must be completed/exit0/null stopReason`, 'fail')
    } else {
      need(processData.status === 'failed' && processData.exitCode === 1 && processData.stopReason === null,
        `${label}.terminal`, `${label} must be failed/exit1/null stopReason`, 'fail')
      need(context.gateReportPath && existsSync(context.gateReportPath), `${label}.quality-report`,
        `${label} exit1 requires an existing bound quality report`, 'fail')
      const qualityBinding = measurement.qualityReport ?? {
        path: context.gateReportPath,
        bytes: (await stat(context.gateReportPath)).size,
        sha256: digest(await readFile(context.gateReportPath)),
      }
      const quality = await readBound(qualityBinding, `${label}.quality-report`, {json: true})
      need(context.evidencePaths.has(await realpath(quality.path)), `${label}.quality-binding`,
        `${label} quality report is not bound in candidate evidence`, 'fail')
      need(quality.data.status === 'fail' && Array.isArray(quality.data.failures) && quality.data.failures.length > 0,
        `${label}.quality-status`, `${label} exit1 is not backed by a quality fail report`, 'fail')
    }
  }
  for (const field of ['elapsedSeconds', 'sampledPeakPrivateBytes', 'peakWorkingSetBytes',
    'sampleCount', 'timeoutSeconds', 'maxPrivateMiB', 'exitCode']) {
    need(Object.is(measurement[field], processData[field]), `${label}.${field}-mismatch`,
      `${label} summary ${field} differs from process report`, 'fail')
  }
  need(finiteNonNegative(measurement.elapsedSeconds)
    && measurement.elapsedSeconds <= measurement.timeoutSeconds,
    `${label}.time`, `${label} elapsed exceeds budget`, 'fail')
  need(measurement.timeoutSeconds === 600 && measurement.maxPrivateMiB === 8192,
    `${label}.budget`, `${label} budget was changed`, 'fail')
  need(safeNonNegativeInteger(measurement.sampledPeakPrivateBytes)
    && measurement.sampledPeakPrivateBytes <= measurement.maxPrivateMiB * 1024 * 1024,
    `${label}.private`, `${label} sampled private bytes exceed budget`, 'fail')
  need(safeNonNegativeInteger(measurement.peakWorkingSetBytes) && measurement.peakWorkingSetBytes > 0,
    `${label}.working-set`, `${label} peak working set is missing`, 'fail')
  const sampleRows = []
  const sampleLines = samples.rawBytes.toString('utf8').trim().split(/\r?\n/).filter(Boolean)
  for (const [index, line] of sampleLines.entries()) {
    try { sampleRows.push(JSON.parse(line)) }
    catch { throw new AcceptanceError(`${label}.sample-json`, `${label} sample row ${index} is not valid JSON`, 'fail') }
  }
  need(sampleRows.length === processData.sampleCount && sampleRows.length === measurement.sampleCount
    && sampleRows.length > 0, `${label}.sample-count`,
  `${label} sample count differs from process/summary`, 'fail')
  let maxPrivate = 0, maxWorking = 0, lastElapsed = -Infinity, lastCpu = -Infinity, lastUtc = -Infinity
  for (const [index, row] of sampleRows.entries()) {
    need(row && row.pid === processData.pid, `${label}.sample-pid`,
      `${label} sample row ${index} PID differs from process pid`, 'fail')
    const utc = Date.parse(row.utc)
    need(Number.isFinite(utc), `${label}.sample-utc`, `${label} sample row ${index} UTC is invalid`, 'fail')
    need(utc >= lastUtc, `${label}.sample-utc-order`,
      `${label} sample UTC moves backwards at row ${index}`, 'fail')
    need(finiteNonNegative(row.elapsedSeconds) && finiteNonNegative(row.cpuSeconds)
      && safeNonNegativeInteger(row.privateBytes) && safeNonNegativeInteger(row.workingSetBytes),
      `${label}.sample-fields`, `${label} sample row ${index} has invalid values`, 'fail')
    need(row.elapsedSeconds >= lastElapsed, `${label}.sample-time`,
      `${label} sample elapsed moves backwards at row ${index}`, 'fail')
    need(row.cpuSeconds >= lastCpu, `${label}.sample-cpu`,
      `${label} sample CPU moves backwards at row ${index}`, 'fail')
    need(row.elapsedSeconds <= processData.elapsedSeconds + 1e-6, `${label}.sample-bounds`,
      `${label} sample row ${index} exceeds process elapsed`, 'fail')
    maxPrivate = Math.max(maxPrivate, row.privateBytes)
    maxWorking = Math.max(maxWorking, row.workingSetBytes)
    lastElapsed = row.elapsedSeconds
    lastCpu = row.cpuSeconds
    lastUtc = utc
  }
  need(maxPrivate === processData.sampledPeakPrivateBytes
    && maxPrivate === measurement.sampledPeakPrivateBytes,
    `${label}.sample-private`, `${label} sample private peak differs from reports`, 'fail')
  need(processData.peakWorkingSetBytes >= maxWorking
    && measurement.peakWorkingSetBytes >= maxWorking,
    `${label}.sample-working-set`, `${label} OS working-set peak is below sampled working set`, 'fail')
  return {status: 'measured', process, samples, executable, manifest}
}

export async function checkAcceptance(candidate) {
  const checks = [], missing = []
  let closureStatus = 'incomplete'
  try {
    need(candidate && candidate.format === FORMAT && candidate.version === VERSION,
      'candidate.schema', `candidate must be ${FORMAT}/v${VERSION}`, 'fail')
    need(typeof candidate.taskId === 'string' && candidate.taskId.length > 0,
      'candidate.task', 'candidate taskId is missing', 'fail')
    need(candidate.status === 'ready_for_review', 'candidate.status',
      'candidate must remain ready_for_review', 'fail')
    need(candidate.authority !== 'main-agent-acceptance', 'candidate.authority',
      'external DS candidate must not claim main-agent-acceptance', 'fail')
    const runtime = await readBound(candidate.runtime, 'runtime')
    const auditRuntime = await readBound(candidate.auditRuntime, 'auditRuntime')
    const manifest = await readBound(candidate.manifest, 'manifest')
    const world = await readBound(candidate.world, 'world', {json: true})
    need(typeof candidate.world.worldRevision === 'string' && candidate.world.worldRevision === world.data.revision,
      'world.revision', 'worldRevision does not match world.json', 'fail')
    const parity = await readBound(candidate.parity, 'parity', {json: true})
    need(parity.data.format === 'glyphweave.render-parity' && parity.data.version === 1 && parity.data.status === 'pass',
      'parity.schema', 'parity is not a passing glyphweave.render-parity/v1', 'fail')
    await checkParityBindings(parity.data, parity.path, candidate.world, world.data)
    const observation = await readBound(candidate.browserObservation, 'browserObservation', {json: true})
    need(observation.data.format === 'glyphweave.preview-traversal' && observation.data.version === 1
      && observation.data.status === 'complete',
      'observation.schema', 'browser observation is not complete glyphweave.preview-traversal/v1', 'fail')
    need(observation.data.worldRevision === world.data.revision
      && observation.data.worldSha256 === candidate.world.sha256.toLowerCase(),
      'observation.binding', 'browser observation is not bound to the final world bytes', 'fail')
    if (candidate.browserCheck) {
      const report = await readBound(candidate.browserCheck, 'browserCheck', {json: true})
      need(report.data.format === 'glyphweave.preview-world-check'
        && report.data.parity?.sha256 === parity.sha256
        && report.data.bindings?.status === 'pass'
        && report.data.bindings?.worldRevision === world.data.revision,
        'browserCheck.binding', 'browser check report is not bound to candidate parity/world', 'fail')
    }
    need(Array.isArray(candidate.evidence) && candidate.evidence.length > 0,
      'evidence.present', 'evidence array must be non-empty', 'fail')
    const seenRequirements = new Set()
    const evidencePaths = new Set()
    const evidenceByRequirement = new Map()
    for (const [index, entry] of candidate.evidence.entries()) {
      need(entry && typeof entry.requirement === 'string' && entry.requirement.length > 0
        && !seenRequirements.has(entry.requirement),
        `evidence.${index}.requirement`, 'evidence requirement is missing or duplicated', 'fail')
      seenRequirements.add(entry.requirement)
      await readBound(entry, `evidence.${entry.requirement}`)
      evidencePaths.add(await realpath(resolve(entry.path)))
      evidenceByRequirement.set(entry.requirement, entry)
    }
    need(Array.isArray(candidate.logs) && candidate.logs.length > 0,
      'logs.present', 'log bindings must be non-empty', 'fail')
    for (const [index, log] of candidate.logs.entries()) await readBound(log, `log.${index}`)
    const measurementContext = {
      runtime,
      manifest,
      worldRoot: await realpath(resolve(candidate.world.path, '..')),
      gateReportPath: evidenceByRequirement.get('A-04.gate-report')?.path
        ?? resolve(candidate.world.path, '..', 'world-quality-gate.json'),
      evidencePaths,
    }
    const generation = await checkMeasurement(candidate.measurements?.generation, 'generation', missing, measurementContext)
    const gate = await checkMeasurement(candidate.measurements?.gate, 'gate', missing, measurementContext)
    if (Array.isArray(candidate.unverified)) missing.push(...candidate.unverified)
    closureStatus = 'pass'
    checks.push({id: 'closure', status: 'pass',
      reason: `verified generation runtime, audit runtime, manifest, world/parity cross-binding, observation, ${candidate.evidence.length} evidence files and ${candidate.logs.length} logs`})
    return {
      closureStatus,
      readiness: missing.length === 0 ? 'ready_for_review' : 'incomplete',
      missing,
      candidateStatus: candidate.status,
      checks,
      measurements: {generation: generation.status, gate: gate.status},
      identity: {runtime, auditRuntime, manifest, worldRevision: world.data.revision, parity: parity.sha256},
    }
  } catch (error) {
    checks.push({id: error.id ?? 'unexpected', status: error.status ?? 'fail', reason: String(error.message || error)})
    return {closureStatus: 'incomplete', readiness: 'incomplete', missing, candidateStatus: candidate?.status ?? null, checks}
  }
}

export function parseArgs(argv) {
  const out = {}
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index]
    need(key === '--acceptance' || key === '--out', 'cli.args', `unknown argument ${key}`, 'fail')
    need(argv[index + 1] && !argv[index + 1].startsWith('--') && !Object.hasOwn(out, key),
      'cli.args', `missing/duplicate argument ${key}`, 'fail')
    out[key] = argv[index + 1]
  }
  need(out['--acceptance'] && out['--out'], 'cli.usage', 'usage: --acceptance PATH --out PATH', 'fail')
  return {acceptancePath: out['--acceptance'], outPath: out['--out']}
}

if (process.argv[1]?.endsWith('check-final-acceptance.mjs')) {
  try {
    const {acceptancePath, outPath} = parseArgs(process.argv.slice(2))
    const candidateBytes = await readFile(resolve(acceptancePath))
    const candidate = await readBound({path: acceptancePath, bytes: candidateBytes.length,
      sha256: digest(candidateBytes)}, 'candidate', {json: true})
    const result = await checkAcceptance(candidate.data)
    const handle = await open(resolve(outPath), 'wx')
    try { await handle.writeFile(JSON.stringify(result, null, 2) + '\n') }
    finally { await handle.close() }
    console.log(JSON.stringify({closureStatus: result.closureStatus, readiness: result.readiness,
      missing: result.missing, checks: result.checks}, null, 2))
    process.exitCode = result.closureStatus === 'pass' ? 0 : 1
  } catch (error) {
    console.error(String(error.message || error))
    process.exitCode = 2
  }
}
