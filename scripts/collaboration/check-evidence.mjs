// Read-only evidence validator for GlyphWeave generation runs (Milestone A-D1).
//
// A-D1-R2 frozen processReceipt contract: generation and gateRun each require
// an independently hash-bound glyphweave.process-benchmark/v1 receipt. Logs
// are supplementary only; log-only run evidence is incomplete. The validator
// checks internal bindings, not provenance or source-to-binary derivation.
//
// CLI:
//   node scripts/collaboration/check-evidence.mjs \
//     --world ABS_WORLD_DIR --receipt ABS_RECEIPT_PATH --out ABS_NEW_OUTPUT_PATH
//
// It never writes into --world or any input, never edits reports/gates, and
// refuses to overwrite an existing --out (exclusive create). Large scene
// payloads declared by world.json are only stat()-checked and never read; every
// JSON it parses is size-guarded by --max-json-bytes.
//
// Trust boundary: this tool performs strict evidence completeness/integrity
// checks ONLY. A receipt's provenance (that it was really authored by the main
// agent for this world) is confirmed OUTSIDE this tool. A pass here means "the
// evidence is complete, internally consistent, bound by hashes, and the receipt
// claims main-agent acceptance" - never "the world quality gate passed".
//
// Receipt schema v1 (authored by the coordinating agent; this tool verifies):
// {
//   "receiptFormat": "glyphweave.generation-receipt",   // must match
//   "receiptVersion": 1,                                // only 1 supported
//   "taskId": string,
//   "authority": "main-agent-acceptance" | ...          // only main-agent-acceptance may pass; others => incomplete
//   "sourceSnapshot": {                                 // required to pass
//       "commit": string|null,
//       "sha256": string|null,                          // informational; not independently derivable here
//       "files": [{ "path": string, "sha256": string }] // required non-empty; each hash-verified on disk
//   } | null,
//   "runtime":    { "path": string, "sha256": string } | null,   // required to pass (hash-verified)
//   "manifest":   { "path": string, "sha256": string } | null,   // required to pass (hash-verified)
//   "generation": {                                       // independent generation run receipt
//       "operation": string|null, "command": string|null,
//       "exitCode": number, "status": string|null, "stillRunning": bool|null,
//       "log": {path,sha256}|null, "processReceipt": {path,sha256}|null
//   } | null,
//   "gateRun":    {                                       // independent full world-quality-gate run receipt
//       same shape as generation
//   } | null,
//   "world": { "path": string },
//   "reports": [ { "path": string, "sha256": string } ], // MUST bind world.json, generation-report.json,
//                                                        //   and world-quality-gate.json
//   "allowedGateFailures": string[] | null,              // exact strings, no wildcards
//   "claimedScope": {
//       "worldRevision": string|null, "generatorRevision": string|null,
//       "sceneCount": number|null, "chunkCount": number|null, "entityCount": number|null,
//       "frontage": {
//           "sceneId": string,
//           "requested": number, "planned": number, "realizedWithAccess": number, "unfilled": number,
//           "auditPath": string|null,                     // compact audit (quality-report-like); default <world>/quality-report.json
//           "geometricVerificationReceiptPath": string|null
//       } | null
//   } | null
// }
//
// Result JSON:
//   status             = evidence complete AND internally consistent (pass|incomplete|fail|unsupported)
//   worldQualityStatus = REAL gate outcome read from world-quality-gate.json (pass|fail|unknown|not_found)
// Exit code: 0 == status pass; 1 == status incomplete|fail|unsupported; 2 == usage error.

import {createHash} from 'node:crypto'
import {createReadStream} from 'node:fs'
import {readFile, stat, writeFile, mkdir, realpath} from 'node:fs/promises'
import {dirname, resolve, isAbsolute, relative, sep} from 'node:path'
import {fileURLToPath, pathToFileURL} from 'node:url'

export const FORMAT = 'glyphweave.evidence-check'
export const VERSION = 1
export const DEFAULT_MAX_JSON_BYTES = 64 * 1024 * 1024
export const DEFAULT_MAX_HASH_BYTES = 1024 * 1024 * 1024
export const SUPPORTED_REPORT_VERSION = 1
export const SUPPORTED_RECEIPT_VERSION = 1
export const ACCEPTED_AUTHORITY = 'main-agent-acceptance'
export const RECEIPT_FORMAT = 'glyphweave.generation-receipt'

const REQUIRED_WORLD_FILES = ['world.json', 'generation-report.json', 'world-quality-gate.json']
const KNOWN_FORMATS = {
  'world.json': 'glyphweave-world',
  'generation-report.json': 'glyphweave.generation-report',
  'world-quality-gate.json': 'glyphweave.world-quality-gate',
  'scale-audit.json': 'glyphweave.scale-audit',
  'quality-report.json': 'glyphweave.quality-report',
  'sidecar.json': 'glyphweave-sidecar',
  'glyphweave.manifest.json': 'glyphweave.manifest',
}

const TERMINAL_SUCCESS = new Set(['baked', 'complete', 'completed', 'success', 'succeeded', 'ok'])
const RUNNING_TOKEN = /running|in[\s-]?progress/i
const FAILURE_TOKEN = /interrupt|timeout|kill(ed)?|budget|abort|crashed|failed|error|panic/i
const GATE_PASS_TOKENS = new Set(['pass', 'passed', 'ok', 'success'])
const GATE_FAIL_TOKENS = new Set(['fail', 'failed'])

export function isFinitePositiveInt(v) {
  return Number.isInteger(v) && v > 0
}

export function parseCliArgs(argv) {
  const out = {world: null, receipt: null, out: null, maxJsonBytes: DEFAULT_MAX_JSON_BYTES, maxHashBytes: DEFAULT_MAX_HASH_BYTES, help: false}
  const errs = []
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]
    const need = (name) => {
      if (i + 1 >= argv.length) { errs.push(`${name} requires a value`); return null }
      return argv[++i]
    }
    if (a === '--help' || a === '-h') out.help = true
    else if (a === '--world') out.world = need(a)
    else if (a === '--receipt') out.receipt = need(a)
    else if (a === '--out') out.out = need(a)
    else if (a === '--max-json-bytes') {
      const v = Number(need(a))
      if (!isFinitePositiveInt(v)) errs.push('--max-json-bytes must be a finite positive integer')
      else out.maxJsonBytes = v
    }
    else if (a === '--max-hash-bytes') {
      const v = Number(need(a))
      if (!isFinitePositiveInt(v)) errs.push('--max-hash-bytes must be a finite positive integer')
      else out.maxHashBytes = v
    }
    else errs.push(`unknown argument: ${a}`)
  }
  return {out, errs}
}

export const USAGE = `usage: node check-evidence.mjs --world ABS_WORLD_DIR --receipt ABS_RECEIPT --out ABS_NEW_PATH [--max-json-bytes N] [--max-hash-bytes N]

--world   absolute path to a generated world directory (read-only input)
--receipt absolute path to the JSON receipt authored by the coordinating agent
--out     absolute path for the result JSON; must be a NEW path outside --world, and must not exist
exit code: 0 = status pass, 1 = status incomplete/fail/unsupported, 2 = usage error`

async function tryStat(p) {
  try { return await stat(p) } catch { return null }
}

export async function tryRealpath(p) {
  try { return await realpath(p) } catch { return null }
}

// Lexical + realpath containment. Never relies on lowercasing for identity.
export function isInside(childResolved, parentResolved) {
  const rel = relative(parentResolved, childResolved)
  return rel === '' || (rel !== '..' && rel !== '.' && !rel.startsWith('..' + sep) && !isAbsolute(rel))
}

export async function isRealInside(childPath, parentPath) {
  const childReal = await tryRealpath(childPath)
  const parentReal = await tryRealpath(parentPath)
  if (childReal === null || parentReal === null) return {contained: null, childReal, parentReal}
  return {contained: isInside(childReal, parentReal), childReal, parentReal}
}

export async function hashFileSha256(p, maxBytes) {
  const s = await tryStat(p)
  if (s === null) return {status: 'missing'}
  if (!s.isFile()) return {status: 'not_a_file'}
  if (s.size > maxBytes) return {status: 'too_large', size: s.size}
  const h = createHash('sha256')
  try {
    await new Promise((resolvePromise, rejectPromise) => {
      const rs = createReadStream(p)
      rs.on('data', (c) => h.update(c))
      rs.on('end', () => resolvePromise())
      rs.on('error', rejectPromise)
    })
  } catch (e) {
    return {status: 'read_error', error: String(e && e.message || e)}
  }
  return {status: 'ok', sha256: h.digest('hex'), size: s.size}
}

export async function readJsonSmall(p, maxBytes) {
  const s = await tryStat(p)
  if (s === null) return {status: 'missing', path: p}
  if (!s.isFile()) return {status: 'not_a_file', path: p}
  if (s.size > maxBytes) return {status: 'too_large', size: s.size, path: p}
  let raw
  try { raw = await readFile(p, 'utf8') } catch (e) { return {status: 'read_error', error: String(e && e.message || e), path: p} }
  if (raw.length === 0) return {status: 'empty', path: p}
  try { return {status: 'ok', data: JSON.parse(raw.replace(/^\uFEFF/, '')), size: s.size, path: p} }
  catch (e) { return {status: 'invalid', error: String(e && e.message || e), path: p} }
}

export function resolveAbs(p, base) {
  if (p === null || p === undefined) return null
  return isAbsolute(p) ? p : resolve(base, p)
}

function firstDefined(...xs) {
  for (const x of xs) if (x !== null && x !== undefined) return x
  return null
}

function asNonNegInt(v) {
  return Number.isInteger(v) && v >= 0 ? v : null
}

function check(checks, id, status, source, reason, extra) {
  const c = {id, status, source, reason}
  if (extra) Object.assign(c, extra)
  checks.push(c)
}

function isPlainObject(v) {
  return v !== null && typeof v === 'object' && !Array.isArray(v)
}

// ---------------------------------------------------------------------------
// Small helpers reused across checks
// ---------------------------------------------------------------------------

// Verify one bound file object {path, sha256}. Returns a check status string
// (pass|fail|incomplete) and pushes nothing itself.
export async function verifyBoundFile(b, maxHash, label) {
  if (!isPlainObject(b) || typeof b.path !== 'string' || b.path.length === 0) {
    return {status: 'incomplete', reason: `${label}: bound file entry must be {path, sha256} with a non-empty path`}
  }
  if (typeof b.sha256 !== 'string' || !/^[0-9a-fA-F]{64}$/.test(b.sha256)) {
    return {status: 'incomplete', reason: `${label}: sha256 must be a 64-hex string`}
  }
  const h = await hashFileSha256(b.path, maxHash)
  if (h.status === 'missing') return {status: 'fail', reason: `${label}: declared file does not exist: ${b.path}`}
  if (h.status === 'not_a_file') return {status: 'fail', reason: `${label}: declared file is not a regular file: ${b.path}`}
  if (h.status === 'too_large') return {status: 'incomplete', reason: `${label}: file ${b.path} is ${h.size} bytes, over --max-hash-bytes`}
  if (h.status === 'read_error') return {status: 'incomplete', reason: `${label}: read error on ${b.path}: ${h.error}`}
  if (h.sha256 !== b.sha256.toLowerCase()) {
    return {status: 'fail', reason: `${label}: sha256 mismatch for ${b.path}: declared ${b.sha256.toLowerCase()} vs actual ${h.sha256}`}
  }
  return {status: 'pass', reason: `${label}: hash matches on disk (${b.path})`}
}

export async function verifyBoundFileArray(entries, maxHash, label, checks) {
  if (!Array.isArray(entries) || entries.length === 0) {
    check(checks, label, 'incomplete', 'receipt', `${label}: expected a non-empty array of {path, sha256}`)
    return false
  }
  let ok = true
  let hasNonPass = false
  for (const e of entries) {
    const r = await verifyBoundFile(e, maxHash, label)
    check(checks, label, r.status, typeof e === 'object' && e.path ? e.path : 'receipt', r.reason)
    if (r.status !== 'pass') { ok = false; hasNonPass = true }
  }
  if (!hasNonPass) {
    check(checks, label, 'pass', 'receipt', `${label}: all ${entries.length} file(s) match declared hashes`)
  }
  return ok
}

// status token classification
export function classifyToken(status) {
  if (status === null || status === undefined) return {kind: 'absent'}
  const s = String(status).toLowerCase()
  if (TERMINAL_SUCCESS.has(s)) return {kind: 'success'}
  if (RUNNING_TOKEN.test(s)) return {kind: 'running'}
  if (FAILURE_TOKEN.test(s)) return {kind: 'failed'}
  return {kind: 'unknown'}
}

const PROCESS_FORMAT = 'glyphweave.process-benchmark'
const PROCESS_VERSION = 1
const NORMAL_GATE_FAILURE = new Set(['fail', 'failed'])
const INTERRUPTION_TOKEN = /interrupt|timeout|kill(?:ed)?|budget|abort|crash(?:ed)?|panic|error/i

function hasSha256(v) {
  return typeof v === 'string' && /^[0-9a-fA-F]{64}$/.test(v)
}

function sameSha256(a, b) {
  return hasSha256(a) && hasSha256(b) && a.toLowerCase() === b.toLowerCase()
}

async function sameRealPath(a, b) {
  const [ar, br] = await Promise.all([tryRealpath(a), tryRealpath(b)])
  return ar !== null && br !== null && ar === br
}

const U32_MAX = 0xffff_ffff
const I32_MIN = -0x8000_0000
const I32_MAX = 0x7fff_ffff

function exactU64(value, label) {
  if (typeof value !== 'number' || !Number.isFinite(value) || !Number.isInteger(value) || value < 0) {
    return {ok: false, status: 'fail', reason: `${label} must be a non-negative integer`}
  }
  if (!Number.isSafeInteger(value)) {
    return {ok: false, status: 'unsupported', reason: `${label} exceeds JavaScript exact-integer precision; cannot verify Rust u64 identity safely`}
  }
  return {ok: true, value}
}

function sceneRecord(value, label) {
  if (!isPlainObject(value)) return {ok: false, status: 'fail', reason: `${label} must be an object`}
  const sceneId = typeof value.sceneId === 'string' && value.sceneId.length > 0 ? value.sceneId : null
  if (sceneId === null) return {ok: false, status: 'fail', reason: `${label}.sceneId must be a non-empty string`}
  if (!Number.isInteger(value.widthM) || value.widthM <= 0 || value.widthM > U32_MAX) {
    return {ok: false, status: 'fail', reason: `${label}.widthM must fit Rust u32 and be positive`}
  }
  if (!Number.isInteger(value.depthM) || value.depthM <= 0 || value.depthM > U32_MAX) {
    return {ok: false, status: 'fail', reason: `${label}.depthM must fit Rust u32 and be positive`}
  }
  // Rust SceneSpec defaults originX/originZ/seedOffset when omitted.
  const originX = value.originX === undefined ? 0 : value.originX
  const originZ = value.originZ === undefined ? 0 : value.originZ
  const seedOffset = value.seedOffset === undefined ? 0 : value.seedOffset
  if (!Number.isInteger(originX) || originX < I32_MIN || originX > I32_MAX) {
    return {ok: false, status: 'fail', reason: `${label}.originX must fit Rust i32`}
  }
  if (!Number.isInteger(originZ) || originZ < I32_MIN || originZ > I32_MAX) {
    return {ok: false, status: 'fail', reason: `${label}.originZ must fit Rust i32`}
  }
  const parsedSeedOffset = exactU64(seedOffset, `${label}.seedOffset`)
  if (!parsedSeedOffset.ok) return parsedSeedOffset
  return {ok: true, value: {sceneId, widthM: value.widthM, depthM: value.depthM, originX, originZ, seedOffset: parsedSeedOffset.value}}
}

function sameScene(a, b) {
  return a.sceneId === b.sceneId && a.widthM === b.widthM && a.depthM === b.depthM &&
    a.originX === b.originX && a.originZ === b.originZ && a.seedOffset === b.seedOffset
}

function canonicalPublishedSceneIds(worldData) {
  if (!worldData || !Array.isArray(worldData.scenes) || worldData.scenes.length === 0) {
    return {ok: false, reason: 'world.json has no published scene paths from which canonical sceneIds can be derived'}
  }
  const ids = new Set()
  for (const scenePath of worldData.scenes) {
    if (typeof scenePath !== 'string') return {ok: false, reason: 'world.json scene path is not a string'}
    const match = /^scenes\/([^\/]+)\/scene\.json$/.exec(scenePath.replace(/\\/g, '/'))
    if (!match || match[1].trim().length === 0 || ids.has(match[1])) {
      return {ok: false, reason: 'world.json scene path is not a unique canonical scenes/<sceneId>/scene.json path'}
    }
    ids.add(match[1])
  }
  return {ok: true, ids}
}

function worldCompactIdentity(worldData) {
  if (!worldData || !isPlainObject(worldData)) return {ok: false, status: 'incomplete', reason: 'world.json is unavailable for compact identity comparison'}
  const seed = exactU64(worldData.seed, 'world.json.seed')
  if (!seed.ok) return seed
  if (worldData.renderMode !== '2d' && worldData.renderMode !== '2.5d') {
    return {ok: false, status: 'fail', reason: 'world.json.renderMode must be "2d" or "2.5d"'}
  }
  return {ok: true, value: {seed: seed.value, renderMode: worldData.renderMode}}
}

async function validateManifestInput(manifestPath, maxJson, checks, missingEvidence) {
  const res = await readJsonSmall(manifestPath, maxJson)
  if (res.status !== 'ok') {
    check(checks, 'manifest.input', 'incomplete', manifestPath,
      `manifest must be readable JSON: ${res.status}${res.error ? ' — ' + res.error : ''}`)
    missingEvidence.push('valid manifest JSON')
    return null
  }
  const data = res.data
  if (!isPlainObject(data)) {
    check(checks, 'manifest.input', 'fail', manifestPath, 'manifest root must be a JSON object')
    return null
  }
  if (data.format !== 'glyphweave-world') {
    check(checks, 'manifest.input', 'fail', manifestPath,
      `manifest format must be "glyphweave-world", got ${JSON.stringify(data.format)}`)
    return null
  }
  if (data.version !== 1) {
    const status = data.version === undefined || data.version === null ? 'incomplete' : 'unsupported'
    check(checks, 'manifest.input', status, manifestPath,
      `manifest version must be 1, got ${data.version === undefined ? 'missing' : JSON.stringify(data.version)}`)
    if (status === 'incomplete') missingEvidence.push('manifest.version')
    return null
  }
  if (!isPlainObject(data.world)) {
    check(checks, 'manifest.input', 'incomplete', manifestPath, 'manifest.world must be an object')
    missingEvidence.push('manifest.world')
    return null
  }
  const manifestSeed = exactU64(data.world.seed, 'manifest.world.seed')
  if (!manifestSeed.ok) {
    check(checks, 'manifest.input', manifestSeed.status, manifestPath, manifestSeed.reason)
    return null
  }
  const renderMode = data.world.renderMode === undefined ? '2.5d' : data.world.renderMode
  if (renderMode !== '2d' && renderMode !== '2.5d') {
    check(checks, 'manifest.input', 'fail', manifestPath,
      'manifest.world.renderMode must be "2d" or "2.5d" (omitted defaults to "2.5d")')
    return null
  }
  if (!Array.isArray(data.scenes) || data.scenes.length === 0) {
    check(checks, 'manifest.input', 'incomplete', manifestPath, 'manifest.scenes must be a non-empty array')
    missingEvidence.push('manifest.scenes')
    return null
  }
  const scenes = new Map()
  for (let index = 0; index < data.scenes.length; index += 1) {
    const parsed = sceneRecord(data.scenes[index], `manifest.scenes[${index}]`)
    if (!parsed.ok) {
      check(checks, 'manifest.input', 'fail', manifestPath, parsed.reason)
      return null
    }
    if (scenes.has(parsed.value.sceneId)) {
      check(checks, 'manifest.input', 'fail', manifestPath,
        `manifest.scenes has duplicate sceneId "${parsed.value.sceneId}"`)
      return null
    }
    scenes.set(parsed.value.sceneId, parsed.value)
  }
  check(checks, 'manifest.input', 'pass', manifestPath,
    `glyphweave-world/v1 manifest has ${scenes.size} uniquely identified scene(s)`)
  return {path: manifestPath, data, scenes, identity: {seed: manifestSeed.value, renderMode}}
}

function terminalOutcome(status, exitCode, stopReason, stillRunning, expectedOperation) {
  const lower = typeof status === 'string' ? status.toLowerCase() : null
  if (stillRunning === true || (lower !== null && RUNNING_TOKEN.test(lower))) {
    return {ok: false, reason: 'run is still running'}
  }
  if (stopReason !== undefined && stopReason !== null) {
    return {ok: false, reason: `stopReason ${JSON.stringify(stopReason)} records an interrupted run`}
  }
  if (lower === null) return {ok: false, reason: 'run status is missing'}
  if (!Number.isInteger(exitCode)) return {ok: false, reason: 'run exitCode is missing or not an integer'}
  if (expectedOperation === 'generate-world') {
    if (!TERMINAL_SUCCESS.has(lower) || exitCode !== 0) {
      return {ok: false, reason: `generation requires terminal success and exitCode 0 (status=${JSON.stringify(status)}, exitCode=${exitCode})`}
    }
    return {ok: true, outcome: 'success'}
  }
  if (TERMINAL_SUCCESS.has(lower) && exitCode === 0) return {ok: true, outcome: 'success'}
  if (NORMAL_GATE_FAILURE.has(lower) && exitCode === 1) return {ok: true, outcome: 'failed'}
  const interruption = typeof status === 'string' && INTERRUPTION_TOKEN.test(status)
  return {ok: false, reason: interruption
    ? `gate status ${JSON.stringify(status)} records an interrupted run, not a normal quality failure`
    : `gate requires success/0 or normal failed/1 with no stopReason (status=${JSON.stringify(status)}, exitCode=${exitCode})`}
}

function validateProcessScenes(processScenes, manifestInfo) {
  if (!Array.isArray(processScenes) || processScenes.length === 0) {
    return {ok: false, reason: 'processReceipt.scenes must be a non-empty array'}
  }
  if (processScenes.length !== manifestInfo.scenes.size) {
    return {ok: false, reason: `processReceipt.scenes count ${processScenes.length} does not match manifest scene count ${manifestInfo.scenes.size}`}
  }
  const seen = new Set()
  for (let index = 0; index < processScenes.length; index += 1) {
    const parsed = sceneRecord(processScenes[index], `processReceipt.scenes[${index}]`)
    if (!parsed.ok) return parsed
    if (seen.has(parsed.value.sceneId)) return {ok: false, reason: `processReceipt.scenes has duplicate sceneId "${parsed.value.sceneId}"`}
    seen.add(parsed.value.sceneId)
    const manifestScene = manifestInfo.scenes.get(parsed.value.sceneId)
    if (!manifestScene || !sameScene(parsed.value, manifestScene)) {
      return {ok: false, reason: `processReceipt scene "${parsed.value.sceneId}" does not match the manifest scene geometry or seedOffset`}
    }
  }
  return {ok: true}
}

// Validate one independently bound glyphweave.process-benchmark/v1 run receipt.
async function validateRunReceipt(run, idPrefix, checks, missingEvidence, maxJson, maxHash, context) {
  const id = (n) => `${idPrefix}.${n}`
  const expectedOperation = context.expectedOperation
  if (!isPlainObject(run)) {
    check(checks, id('present'), 'incomplete', context.source, `${idPrefix} run receipt is missing; cannot prove the operation ran`)
    missingEvidence.push(`${idPrefix}`)
    return {ok: false, outcome: null}
  }
  if (run.operation !== expectedOperation) {
    check(checks, id('operation'), 'fail', context.source,
      `${idPrefix}.operation must be "${expectedOperation}"; command is display-only and cannot establish run identity`)
  } else {
    check(checks, id('operation'), 'pass', context.source, `${idPrefix}.operation is "${expectedOperation}"`)
  }
  const outerTerminal = terminalOutcome(run.status, run.exitCode, run.stopReason, run.stillRunning, expectedOperation)
  check(checks, id('terminal'), outerTerminal.ok ? 'pass' : 'fail', context.source,
    outerTerminal.ok ? `${idPrefix} outer receipt records terminal ${outerTerminal.outcome}` : `${idPrefix} ${outerTerminal.reason}`)

  const proc = run.processReceipt
  if (!isPlainObject(proc)) {
    check(checks, id('process-receipt'), 'incomplete', context.source,
      `${idPrefix}.processReceipt is required as a hash-bound structured process record; log-only evidence is incomplete`)
    missingEvidence.push(`${idPrefix}.processReceipt`)
    return {ok: false, outcome: null}
  }
  const bound = {path: resolveAbs(proc.path, context.worldDir), sha256: proc.sha256}
  const binding = await verifyBoundFile(bound, maxHash, `${idPrefix}.processReceipt`)
  check(checks, id('process-receipt'), binding.status, bound.path || context.source, binding.reason)
  if (binding.status !== 'pass') return {ok: false, outcome: null}

  // Logs remain supplementary and are hash-checked when supplied, but cannot
  // replace a process receipt or weaken its required structural bindings.
  if (run.log !== null && run.log !== undefined) {
    const log = {path: resolveAbs(run.log && run.log.path, context.worldDir), sha256: run.log && run.log.sha256}
    const logResult = await verifyBoundFile(log, maxHash, `${idPrefix}.log`)
    check(checks, id('log'), logResult.status, log.path || context.source, logResult.reason)
  }

  const parsed = await readJsonSmall(bound.path, maxJson)
  if (parsed.status !== 'ok' || !isPlainObject(parsed.data)) {
    check(checks, id('process-schema'), 'incomplete', bound.path,
      `${idPrefix}.processReceipt must be a readable JSON object: ${parsed.status}${parsed.error ? ' — ' + parsed.error : ''}`)
    missingEvidence.push(`${idPrefix}.processReceipt JSON`)
    return {ok: false, outcome: null}
  }
  const processData = parsed.data
  if (processData.format !== PROCESS_FORMAT || processData.version !== PROCESS_VERSION) {
    const status = processData.version !== PROCESS_VERSION && processData.version !== undefined ? 'unsupported' : 'fail'
    check(checks, id('process-schema'), status, bound.path,
      `${idPrefix}.processReceipt must be ${PROCESS_FORMAT}/v${PROCESS_VERSION}`)
  } else {
    check(checks, id('process-schema'), 'pass', bound.path, `${idPrefix}.processReceipt schema is supported`)
  }
  if (processData.operation !== expectedOperation) {
    check(checks, id('process-operation'), 'fail', bound.path,
      `${idPrefix}.processReceipt.operation must be "${expectedOperation}"`)
  } else {
    check(checks, id('process-operation'), 'pass', bound.path, 'structured process operation matches outer run type')
  }
  const hasProcessStopReason = Object.prototype.hasOwnProperty.call(processData, 'stopReason')
  const processTerminal = hasProcessStopReason
    ? terminalOutcome(processData.status, processData.exitCode, processData.stopReason, processData.stillRunning, expectedOperation)
    : {ok: false, outcome: null, reason: 'stopReason must be explicitly present and null for a completed process receipt'}
  if (!processTerminal.ok) {
    check(checks, id('process-terminal'), hasProcessStopReason ? 'fail' : 'incomplete', bound.path,
      `${idPrefix}.processReceipt ${processTerminal.reason}`)
  } else if (!outerTerminal.ok || processTerminal.outcome !== outerTerminal.outcome || processData.exitCode !== run.exitCode) {
    check(checks, id('process-terminal'), 'fail', bound.path,
      `${idPrefix} outer and process receipt terminal outcomes/exitCode must agree`)
  } else {
    check(checks, id('process-terminal'), 'pass', bound.path, 'outer and process terminal semantics agree')
  }

  const runtimeSamePath = typeof processData.executable === 'string' && context.runtimePath !== null &&
    await sameRealPath(resolveAbs(processData.executable, context.worldDir), context.runtimePath)
  if (!runtimeSamePath || !sameSha256(processData.executableSha256, context.runtimeSha256)) {
    check(checks, id('process-runtime'), 'fail', bound.path,
      `${idPrefix}.processReceipt executable/executableSha256 must match receipt.runtime`)
  } else {
    check(checks, id('process-runtime'), 'pass', bound.path, 'process runtime path and hash match receipt.runtime')
  }

  const manifestSamePath = typeof processData.manifest === 'string' && context.manifestInfo !== null &&
    await sameRealPath(resolveAbs(processData.manifest, context.worldDir), context.manifestInfo.path)
  if (!manifestSamePath || !sameSha256(processData.manifestSha256, context.manifestSha256)) {
    check(checks, id('process-manifest'), 'fail', bound.path,
      `${idPrefix}.processReceipt manifest/manifestSha256 must match receipt.manifest`)
  } else {
    check(checks, id('process-manifest'), 'pass', bound.path, 'process manifest path and hash match receipt.manifest')
  }

  const outputSameWorld = typeof processData.output === 'string' &&
    await sameRealPath(resolveAbs(processData.output, context.worldDir), context.worldDir)
  if (!outputSameWorld) {
    check(checks, id('process-output'), 'fail', bound.path,
      `${idPrefix}.processReceipt.output must identify the inspected world root exactly`)
  } else {
    check(checks, id('process-output'), 'pass', bound.path, 'process output identifies the inspected world root')
  }

  if (context.manifestInfo === null) {
    check(checks, id('process-scenes'), 'incomplete', bound.path,
      'process scenes cannot be matched because receipt.manifest is not a valid input manifest')
    missingEvidence.push('valid manifest for process scene comparison')
  } else {
    const scenes = validateProcessScenes(processData.scenes, context.manifestInfo)
    check(checks, id('process-scenes'), scenes.ok ? 'pass' : 'fail', bound.path,
      scenes.ok ? 'process scenes match manifest scene identities and geometry' : scenes.reason)
  }

  const bad = checks.some((c) => c.id.startsWith(idPrefix + '.') && c.status !== 'pass')
  return {ok: !bad, outcome: processTerminal.ok ? processTerminal.outcome : null}
}

// ---------------------------------------------------------------------------
// Main collection
// ---------------------------------------------------------------------------

export async function collectChecks(worldDir, receiptPath, cfg = {}) {
  const maxJson = isFinitePositiveInt(cfg.maxJsonBytes) ? cfg.maxJsonBytes : DEFAULT_MAX_JSON_BYTES
  const maxHash = isFinitePositiveInt(cfg.maxHashBytes) ? cfg.maxHashBytes : DEFAULT_MAX_HASH_BYTES
  const checks = []
  const missingEvidence = []
  const unverified = []

  const world = await tryStat(worldDir)
  if (world === null || !world.isDirectory()) {
    check(checks, 'cli.world-dir', 'fail', 'cli', `world directory not found: ${worldDir}`)
    return {checks, missingEvidence, worldQualityStatus: {value: 'not_found', source: null, note: 'world directory missing'}, receipt: null, gateInfo: null, summary: null, unverified}
  }
  const worldReal = await tryRealpath(worldDir)

  // ---- receipt -------------------------------------------------------------
  const receiptRes = await readJsonSmall(receiptPath, maxJson)
  if (receiptRes.status !== 'ok') {
    check(checks, 'receipt.readable', 'incomplete', receiptPath,
      `receipt could not be parsed as JSON: ${receiptRes.status}${receiptRes.error ? ' — ' + receiptRes.error : ''}`)
    return {checks, missingEvidence: [...missingEvidence, `receipt not parseable (${receiptRes.status})`], worldQualityStatus: {value: 'unknown', source: receiptPath, note: 'receipt unreadable'}, receipt: null, gateInfo: null, summary: null, unverified}
  }
  const receipt = receiptRes.data
  if (!isPlainObject(receipt)) {
    check(checks, 'receipt.shape', 'incomplete', receiptPath, 'receipt root must be a JSON object')
    return {checks, missingEvidence: ['receipt root not an object'], worldQualityStatus: {value: 'unknown', source: receiptPath, note: 'receipt malformed'}, receipt: null, gateInfo: null, summary: null, unverified}
  }
  const taskId = typeof receipt.taskId === 'string' && receipt.taskId.length > 0 ? receipt.taskId : null
  if (taskId === null) {
    check(checks, 'receipt.taskId', 'incomplete', receiptPath, 'receipt is missing a non-empty taskId')
    missingEvidence.push('receipt.taskId')
  }

  const authority = typeof receipt.authority === 'string' && receipt.authority.length > 0 ? receipt.authority : null
  if (authority === null) {
    check(checks, 'authority.trusted', 'incomplete', receiptPath, 'receipt.authority is missing')
    missingEvidence.push('receipt.authority')
  } else if (authority === ACCEPTED_AUTHORITY) {
    check(checks, 'authority.trusted', 'pass', receiptPath,
      'receipt claims main-agent acceptance; provenance itself is confirmed outside this tool (trust boundary is external)')
  } else {
    check(checks, 'authority.trusted', 'incomplete', receiptPath,
      `authority "${authority}" is not main-agent acceptance; demo/worker-draft/none/unknown receipts cannot produce a formal pass`)
    missingEvidence.push(`main-agent acceptance for authority="${authority}"`)
  }

  if (receipt.receiptFormat !== RECEIPT_FORMAT) {
    check(checks, 'receipt.format', 'fail', receiptPath,
      `receipt.receiptFormat "${receipt.receiptFormat}" is not "${RECEIPT_FORMAT}"`)
  }
  if (receipt.receiptVersion !== SUPPORTED_RECEIPT_VERSION) {
    const v = receipt.receiptVersion
    if (v === undefined || v === null) {
      check(checks, 'receipt.version', 'incomplete', receiptPath, 'receipt.receiptVersion is missing')
      missingEvidence.push('receipt.receiptVersion')
    } else {
      check(checks, 'receipt.version', 'unsupported', receiptPath,
        `receipt.receiptVersion ${v} is unsupported (supported: ${SUPPORTED_RECEIPT_VERSION})`)
    }
  }

  // ---- required reports: read + schema ------------------------------------
  const parsed = {}
  for (const f of REQUIRED_WORLD_FILES) {
    const p = resolve(worldDir, f)
    const res = await readJsonSmall(p, maxJson)
    parsed[f] = res
    if (res.status === 'missing') {
      if (f === 'world-quality-gate.json') {
        const audit = await readJsonSmall(resolve(worldDir, 'scale-audit.json'), maxJson)
        const scalePass = audit.status === 'ok' && isPlainObject(audit.data) && GATE_PASS_TOKENS.has(String(audit.data.status || '').toLowerCase())
        if (scalePass) {
          check(checks, 'artifacts.required', 'fail', p,
            'world-quality-gate.json is missing while scale-audit.json reports pass; a scale audit is NOT the full world quality gate')
        }
      }
      check(checks, 'artifacts.required', 'fail', p, `required world artifact missing: ${f}`)
      continue
    }
    const reasonFor = {
      invalid: 'not valid JSON (truncated or corrupted)',
      empty: 'is empty (zero bytes)',
      not_a_file: 'is not a regular file (directory or special entry)',
      read_error: 'could not be read',
      too_large: `is ${res.size} bytes, over --max-json-bytes=${maxJson}; refusing to whole-read`,
    }
    const failKind = res.status === 'too_large' ? 'incomplete' : 'fail'
    if (res.status === 'too_large') missingEvidence.push(`${f} too large to parse safely (${res.size} bytes)`)
    if (res.status === 'read_error') missingEvidence.push(`${f} read error: ${res.error}`)
    if (res.status !== 'ok') {
      check(checks, 'reports.json-valid', failKind, p, `${f} ${reasonFor[res.status]}${res.error ? ' — ' + res.error : ''}`)
      continue
    }
    // schema per report
    const fname = f.replace(/\.json$/, '').replace(/^world-quality-gate$/, 'world-quality-gate')
    const shortId = f === 'world.json' ? 'world' : f === 'generation-report.json' ? 'generation-report' : 'gate'
    const data = res.data
    if (!isPlainObject(data)) {
      check(checks, `schema.${shortId}`, 'fail', p, `${f} root must be a JSON object`)
      continue
    }
    const expFmt = KNOWN_FORMATS[f]
    const fmtOk = typeof data.format === 'string' && data.format === expFmt
    if (!fmtOk) {
      check(checks, `schema.${shortId}`, 'fail', p,
        `${f} format is ${data.format === undefined ? 'missing' : JSON.stringify(data.format)}, expected "${expFmt}"`)
    } else {
      check(checks, `schema.${shortId}`, 'pass', p, `${f} format matches "${expFmt}"`)
    }
    if (data.version !== SUPPORTED_REPORT_VERSION) {
      if (data.version === undefined || data.version === null) {
        check(checks, `schema.${shortId}`, 'incomplete', p, `${f} version is missing`)
        missingEvidence.push(`${f}.version`)
      } else {
        check(checks, `schema.${shortId}`, 'unsupported', p,
          `${f} version ${data.version} is unsupported (supported: ${SUPPORTED_REPORT_VERSION})`)
      }
    }
  }
  if (!checks.some((c) => c.id === 'artifacts.required' && c.status !== 'pass')) {
    check(checks, 'artifacts.required', 'pass', worldDir, 'world.json, generation-report.json and world-quality-gate.json all present')
  }

  const worldData = parsed['world.json'] && parsed['world.json'].status === 'ok' ? parsed['world.json'].data : null
  const genData = parsed['generation-report.json'] && parsed['generation-report.json'].status === 'ok' ? parsed['generation-report.json'].data : null
  const gateData = parsed['world-quality-gate.json'] && parsed['world-quality-gate.json'].status === 'ok' ? parsed['world-quality-gate.json'].data : null

  // The generation report is a compact published binding: it must name the
  // exact world root, not merely a directory contained by that root.
  if (genData) {
    if (typeof genData.output !== 'string' || genData.output.length === 0) {
      check(checks, 'identity.generation-report-output', 'incomplete', resolve(worldDir, 'generation-report.json'),
        'generation-report.output is missing; cannot bind the report to the inspected world root')
      missingEvidence.push('generation-report.output')
    } else if (await sameRealPath(resolveAbs(genData.output, worldDir), worldDir)) {
      check(checks, 'identity.generation-report-output', 'pass', resolve(worldDir, 'generation-report.json'),
        'generation-report.output identifies the exact inspected world root')
    } else {
      check(checks, 'identity.generation-report-output', 'fail', resolve(worldDir, 'generation-report.json'),
        'generation-report.output does not identify the exact inspected world root')
    }
  }

  // ---- scenes: non-empty, inside world, present ----------------------------
  const sceneFiles = []
  if (worldData && Array.isArray(worldData.scenes)) {
    if (worldData.scenes.length === 0) {
      check(checks, 'artifacts.scenes-nonempty', 'fail', resolve(worldDir, 'world.json'),
        'world.json declares an empty scenes list; an empty world cannot be a completed run')
    } else {
      check(checks, 'artifacts.scenes-nonempty', 'pass', worldDir, `world.json declares ${worldData.scenes.length} scene(s)`)
      let containedOk = true
      let presentOk = true
      for (const rel of worldData.scenes) {
        if (typeof rel !== 'string' || rel.length === 0) {
          check(checks, 'artifacts.scenes-contained', 'fail', worldDir, 'scene path entry is not a non-empty string')
          containedOk = false
          continue
        }
        const abs = resolveAbs(rel, worldDir)
        if (!isInside(abs, resolve(worldDir))) {
          check(checks, 'artifacts.scenes-contained', 'fail', abs,
            `scene path escapes the world directory: ${rel}`)
          containedOk = false
          continue
        }
        const containment = await isRealInside(abs, worldDir)
        if (containment.contained === false) {
          check(checks, 'artifacts.scenes-contained', 'fail', abs,
            `scene path resolves (symlink/junction) outside the world directory: ${rel}`)
          containedOk = false
          continue
        }
        const s = await tryStat(abs)
        if (s === null || !s.isFile()) {
          check(checks, 'artifacts.scenes-present', 'fail', abs, `declared scene file missing: ${rel}`)
          presentOk = false
        } else {
          sceneFiles.push(abs)
        }
      }
      if (containedOk) check(checks, 'artifacts.scenes-contained', 'pass', worldDir, 'all scene paths stay inside the world directory')
      if (presentOk) check(checks, 'artifacts.scenes-present', 'pass', worldDir,
        `all ${worldData.scenes.length} declared scene file(s) exist on disk (existence only, payload not read)`)
    }
    if (genData && Number.isInteger(genData.sceneCount) && genData.sceneCount !== worldData.scenes.length) {
      check(checks, 'identity.scene-count', 'fail', worldDir,
        `world.json declares ${worldData.scenes.length} scene(s) but generation-report.sceneCount=${genData.sceneCount}`)
    } else if (genData) {
      check(checks, 'identity.scene-count', 'pass', worldDir, 'world.json scene list length matches generation-report.sceneCount')
    }
  } else if (worldData) {
    check(checks, 'artifacts.scenes-nonempty', 'incomplete', resolve(worldDir, 'world.json'),
      'world.json does not declare a scenes array')
    missingEvidence.push('world.json.scenes')
  }

  // ---- worldQualityStatus + gate contradictions ----------------------------
  let worldQualityStatus = {value: 'not_found', source: null, note: 'world-quality-gate.json not present'}
  const gateInfo = {
    rawStatus: null, gateResult: null, failures: [], allowedGateFailures: null,
    unexpectedFailures: [], resolvedFailures: [],
  }
  if (gateData) {
    const raw = typeof gateData.status === 'string' ? gateData.status : null
    const low = raw === null ? null : raw.toLowerCase()
    const gateResult = low !== null && GATE_PASS_TOKENS.has(low) ? 'pass'
      : low !== null && GATE_FAIL_TOKENS.has(low) ? 'fail'
        : null
    gateInfo.rawStatus = raw
    gateInfo.gateResult = gateResult
    worldQualityStatus = {
      value: gateResult === null ? 'unknown' : gateResult,
      original: raw,
      source: resolve(worldDir, 'world-quality-gate.json'),
      note: 'quality gate result read verbatim; evidence checks never rewrite a gate fail',
    }
    if (gateResult === null) {
      check(checks, 'gate.status-recognised', 'incomplete', resolve(worldDir, 'world-quality-gate.json'),
        `gate.status "${raw}" is not a recognised outcome (expected pass or fail)`)
      missingEvidence.push('recognised gate.status')
    } else {
      check(checks, 'gate.status-recognised', 'pass', resolve(worldDir, 'world-quality-gate.json'),
        `gate.status recognised as "${gateResult}"`)
    }
    const failures = gateData.failures
    const validFailures = Array.isArray(failures) && failures.every((f) => typeof f === 'string')
    if (!Array.isArray(failures)) {
      check(checks, 'gate.failures-type', 'fail', resolve(worldDir, 'world-quality-gate.json'),
        'gate.failures is required and must be an array of strings')
    } else if (!validFailures) {
      check(checks, 'gate.failures-type', 'fail', resolve(worldDir, 'world-quality-gate.json'),
        'gate.failures must be an array of strings (non-string entries are not silently dropped)')
    } else {
      check(checks, 'gate.failures-type', 'pass', resolve(worldDir, 'world-quality-gate.json'),
        'gate.failures is a string array')
    }
    const strFailures = validFailures ? failures : []
    gateInfo.failures = strFailures

    if (!validFailures) {
      check(checks, 'gate.contradiction', 'fail', resolve(worldDir, 'world-quality-gate.json'),
        `gate.status ${JSON.stringify(raw)} cannot be interpreted because gate.failures is not a string array`)
    } else if (gateResult === 'pass' && strFailures.length > 0) {
      check(checks, 'gate.contradiction', 'fail', resolve(worldDir, 'world-quality-gate.json'),
        `gate.status is "pass" but gate.failures lists ${strFailures.length} failure(s); this is contradictory and cannot be authorised away`)
    } else if (gateResult === 'fail' && strFailures.length === 0) {
      check(checks, 'gate.contradiction', 'incomplete', resolve(worldDir, 'world-quality-gate.json'),
        'gate.status is "fail" but gate.failures is empty; cannot assess which failures are real')
      missingEvidence.push('gate failure detail')
    } else {
      check(checks, 'gate.contradiction', 'pass', resolve(worldDir, 'world-quality-gate.json'),
        'gate.status and gate.failures content are not contradictory')
    }

    if (gateResult === 'fail') {
      const allowed = receipt.allowedGateFailures
      let allowedArr = []
      if (allowed === null || allowed === undefined) {
        check(checks, 'gate.authorization', 'fail', receiptPath,
          'gate has failures but receipt.allowedGateFailures is absent; no authorised baseline exists')
      } else if (Array.isArray(allowed) && allowed.every((f) => typeof f === 'string')) {
        allowedArr = allowed
        check(checks, 'gate.authorization', 'pass', receiptPath, 'allowedGateFailures is an exact-string baseline')
      } else {
        check(checks, 'gate.authorization', 'fail', receiptPath,
          'receipt.allowedGateFailures must be an array of exact strings (no substring wildcards)')
      }
      gateInfo.allowedGateFailures = allowedArr
      const allowedSet = new Set(allowedArr)
      const failSet = new Set(strFailures)
      gateInfo.unexpectedFailures = strFailures.filter((f) => !allowedSet.has(f))
      gateInfo.resolvedFailures = allowedArr.filter((f) => !failSet.has(f))
      if (strFailures.length > 0) {
        if (gateInfo.unexpectedFailures.length === 0 && allowedArr.length > 0) {
          check(checks, 'gate.failures-vs-baseline', 'pass', worldDir,
            `all ${strFailures.length} gate failure(s) exactly match the authorised baseline`,
            {authorizedFailuresStillPresent: strFailures})
        } else {
          check(checks, 'gate.failures-vs-baseline', 'fail', worldDir,
            `gate failures not authorised by the receipt baseline: ${gateInfo.unexpectedFailures.join(' | ')}`,
            {unexpectedFailures: gateInfo.unexpectedFailures})
        }
      }
    } else if (gateResult === 'pass') {
      gateInfo.allowedGateFailures = Array.isArray(receipt.allowedGateFailures) ? receipt.allowedGateFailures : []
      check(checks, 'gate.failures-vs-baseline', 'pass', worldDir, 'quality gate reports pass with no failures')
    }
  }

  // ---- full gate evidence cannot be substituted ----------------------------
  if (!gateData) {
    const audit = await readJsonSmall(resolve(worldDir, 'scale-audit.json'), maxJson)
    if (audit.status === 'ok' && isPlainObject(audit.data) && GATE_PASS_TOKENS.has(String(audit.data.status || '').toLowerCase())) {
      check(checks, 'gate.full-gate-substitute', 'fail', worldDir,
        'scale-audit.json reports pass but world-quality-gate.json is absent; a scale audit cannot substitute for the full quality gate')
    } else {
      check(checks, 'gate.full-gate-substitute', 'incomplete', worldDir,
        'world-quality-gate.json absent; no full quality gate outcome is available')
    }
  } else {
    check(checks, 'gate.full-gate-substitute', 'pass', worldDir,
      'world-quality-gate.json present; the gate outcome is read from it rather than from any partial audit')
  }

  // ---- identity across reports ---------------------------------------------
  const revs = {}
  if (worldData && typeof worldData.revision === 'string' && worldData.revision.length > 0) revs['world.json.revision'] = worldData.revision
  if (genData && typeof genData.worldRevision === 'string' && genData.worldRevision.length > 0) revs['generation-report.worldRevision'] = genData.worldRevision
  if (gateData && typeof gateData.worldRevision === 'string' && gateData.worldRevision.length > 0) revs['world-quality-gate.worldRevision'] = gateData.worldRevision
  for (const extra of ['world.json', 'generation-report.json', 'world-quality-gate.json']) {
    if (!parsed[extra]) continue
    const d = extra === 'world.json' ? worldData : extra === 'generation-report.json' ? genData : gateData
    if (!d) continue
    const field = extra === 'world.json' ? d.revision : d.worldRevision
    if ((typeof field !== 'string' || field.length === 0) && (extra === 'generation-report.json' || extra === 'world-quality-gate.json' || extra === 'world.json')) {
      missingEvidence.push(`${extra} revision field`)
    }
  }
  const revSet = new Set(Object.values(revs))
  const revKeys = Object.keys(revs)
  if (revKeys.length < 3) {
    check(checks, 'identity.revision-consistent', 'incomplete', worldDir,
      `each required report must carry its revision; only found on ${revKeys.join(', ') || '(none)'}`)
    missingEvidence.push('revision on every required report')
  } else if (revSet.size === 1) {
    check(checks, 'identity.revision-consistent', 'pass', worldDir,
      `worldRevision identical across ${revKeys.join(', ')}`)
  } else {
    check(checks, 'identity.revision-consistent', 'fail', worldDir,
      `worldRevision mismatch: ${revKeys.map((k) => `${k}=${revs[k]}`).join('; ')}`)
  }

  const gens = {}
  if (worldData && typeof worldData.generatorRevision === 'string') gens['world.json.generatorRevision'] = worldData.generatorRevision
  if (genData && typeof genData.generatorRevision === 'string') gens['generation-report.generatorRevision'] = genData.generatorRevision
  const genSet = new Set(Object.values(gens))
  if (genSet.size === 0) {
    check(checks, 'identity.generator-consistent', 'incomplete', worldDir,
      'no generatorRevision found in world.json/generation-report.json')
    missingEvidence.push('generatorRevision')
  } else if (genSet.size === 1) {
    check(checks, 'identity.generator-consistent', 'pass', worldDir,
      `generatorRevision identical across ${Object.keys(gens).join(', ')}`)
  } else {
    check(checks, 'identity.generator-consistent', 'fail', worldDir,
      `generatorRevision mismatch: ${Object.keys(gens).map((k) => `${k}=${gens[k]}`).join('; ')}`)
  }

  // ---- declared scope vs reports -------------------------------------------
  const scope = isPlainObject(receipt.claimedScope) ? receipt.claimedScope : {}
  const declaredWorldRev = typeof scope.worldRevision === 'string' && scope.worldRevision.length > 0 ? scope.worldRevision : null
  if (declaredWorldRev === null) {
    check(checks, 'identity.claim-world-revision', 'incomplete', receiptPath, 'claimedScope.worldRevision is missing')
    missingEvidence.push('claimedScope.worldRevision')
  } else if (revSet.size === 1 && declaredWorldRev !== [...revSet][0]) {
    check(checks, 'identity.claim-world-revision', 'fail', receiptPath,
      `receipt claims worldRevision "${declaredWorldRev}" but reports say "${[...revSet][0]}"`)
  } else {
    check(checks, 'identity.claim-world-revision', 'pass', receiptPath, 'claimedScope.worldRevision matches the reports')
  }

  const claimGenRev = typeof scope.generatorRevision === 'string' && scope.generatorRevision.length > 0 ? scope.generatorRevision : null
  if (claimGenRev === null) {
    check(checks, 'identity.claim-generator', 'incomplete', receiptPath, 'claimedScope.generatorRevision is missing')
    missingEvidence.push('claimedScope.generatorRevision')
  } else if (genSet.size === 1 && claimGenRev !== [...genSet][0]) {
    check(checks, 'identity.claim-generator', 'fail', worldDir,
      `receipt claims generatorRevision "${claimGenRev}" but reports say "${[...genSet][0]}"`)
  } else {
    check(checks, 'identity.claim-generator', 'pass', worldDir, 'receipt generatorRevision matches the reported generation label')
  }

  const countFields = ['sceneCount', 'chunkCount', 'entityCount']
  let countIssues = 0
  for (const f of countFields) {
    const declared = asNonNegInt(scope[f])
    const actual = genData ? asNonNegInt(genData[f]) : null
    if (declared === null && actual === null) continue
    if (declared === null) {
      check(checks, 'identity.counts', 'incomplete', receiptPath,
        `claimedScope.${f} missing while generation-report declares ${actual}`)
      missingEvidence.push(`claimedScope.${f}`)
      countIssues++
      continue
    }
    if (actual === null) {
      check(checks, 'identity.counts', 'incomplete', worldDir, `claimedScope.${f}=${declared} but generation-report has no ${f}`)
      missingEvidence.push(`generation-report.${f}`)
      countIssues++
      continue
    }
    if (declared !== actual) {
      check(checks, 'identity.counts', 'fail', worldDir,
        `claimedScope.${f}=${declared} does not match generation-report.${f}=${actual}`)
      countIssues++
    }
  }
  if (worldData && Array.isArray(worldData.scenes) && scope.sceneCount !== null && scope.sceneCount !== undefined
    && asNonNegInt(scope.sceneCount) !== null && asNonNegInt(scope.sceneCount) !== worldData.scenes.length) {
    check(checks, 'identity.counts', 'fail', worldDir,
      `claimedScope.sceneCount=${scope.sceneCount} does not match the ${worldData.scenes.length} scene(s) in world.json`)
    countIssues++
  }
  if (countIssues === 0 && countFields.every((f) => scope[f] !== null && scope[f] !== undefined)) {
    check(checks, 'identity.counts', 'pass', worldDir,
      `declared counts (${countFields.map((f) => `${f}=${scope[f]}`).join(', ')}) match generation-report`)
  }

  // ---- report hash binding must cover the files actually read --------------
  const boundReports = new Set()
  if (!Array.isArray(receipt.reports) || receipt.reports.length === 0) {
    check(checks, 'reports.binding', 'incomplete', receiptPath,
      'receipt.reports is missing/empty; the required reports are not hash-bound')
    missingEvidence.push('receipt.reports covering world.json, generation-report.json, world-quality-gate.json')
  } else {
    let anyIssue = false
    for (const e of receipt.reports) {
      if (!isPlainObject(e) || typeof e.path !== 'string') {
        check(checks, 'reports.binding', 'incomplete', receiptPath, 'receipt.reports entry missing path')
        anyIssue = true
        continue
      }
      const abs = resolveAbs(e.path, worldDir)
      const r = await verifyBoundFile({path: abs, sha256: e.sha256}, maxHash, 'reports[]')
      check(checks, 'reports.hash', r.status, abs, r.reason)
      if (r.status !== 'pass') { anyIssue = true; continue }
      const rr = await tryRealpath(abs)
      boundReports.add(rr === null ? resolve(abs) : rr)
    }
    for (const f of REQUIRED_WORLD_FILES) {
      const abs = resolve(worldDir, f)
      const rr = await tryRealpath(abs)
      const key = rr === null ? resolve(abs) : rr
      if (!boundReports.has(key)) {
        check(checks, 'reports.binding', 'incomplete', abs,
          `${f} is read during validation but is NOT hash-bound by receipt.reports`)
        missingEvidence.push(`reports[] binding for ${f}`)
        anyIssue = true
      }
    }
    if (!anyIssue) check(checks, 'reports.binding', 'pass', worldDir,
      'receipt.reports hash-binds world.json, generation-report.json and world-quality-gate.json')
  }

  // ---- runtime -------------------------------------------------------------
  const runtime = receipt.runtime
  let runtimePath = null
  let runtimeSha256 = null
  if (runtime === null || runtime === undefined) {
    check(checks, 'identity.runtime', 'incomplete', receiptPath,
      'receipt.runtime is missing; a completed run must bind the runtime used (no mtime guessing)')
    missingEvidence.push('receipt.runtime')
  } else if (!isPlainObject(runtime) || typeof runtime.path !== 'string') {
    check(checks, 'identity.runtime', 'incomplete', receiptPath, 'receipt.runtime must be {path, sha256}')
    missingEvidence.push('receipt.runtime.{path,sha256}')
  } else {
    runtimePath = resolveAbs(runtime.path, worldDir)
    runtimeSha256 = runtime.sha256
    const r = await verifyBoundFile({path: runtimePath, sha256: runtimeSha256}, maxHash, 'runtime')
    check(checks, 'identity.runtime', r.status, r.path || receiptPath, r.reason + ' (a matching runtime hash alone does not prove this run used it; see generation/gateRun receipts)')
    if (r.status !== 'pass') { runtimePath = null; runtimeSha256 = null }
  }

  // ---- manifest ------------------------------------------------------------
  const manifest = receipt.manifest
  let manifestPath = null
  let manifestSha256 = null
  let manifestInfo = null
  if (manifest === null || manifest === undefined) {
    check(checks, 'identity.manifest', 'incomplete', receiptPath,
      'receipt.manifest is missing; the published manifest identity is not bound')
    missingEvidence.push('receipt.manifest')
  } else if (!isPlainObject(manifest) || typeof manifest.path !== 'string') {
    check(checks, 'identity.manifest', 'incomplete', receiptPath, 'receipt.manifest must be {path, sha256}')
    missingEvidence.push('receipt.manifest.{path,sha256}')
  } else {
    manifestPath = resolveAbs(manifest.path, worldDir)
    manifestSha256 = manifest.sha256
    const r = await verifyBoundFile({path: manifestPath, sha256: manifestSha256}, maxHash, 'manifest')
    check(checks, 'identity.manifest', r.status, r.path || receiptPath, r.reason)
    if (r.status === 'pass') {
      manifestInfo = await validateManifestInput(manifestPath, maxJson, checks, missingEvidence)
    } else {
      manifestPath = null
      manifestSha256 = null
    }
  }
  if (manifestInfo !== null) {
    const published = canonicalPublishedSceneIds(worldData)
    if (!published.ok) {
      check(checks, 'identity.manifest-world-scenes', 'incomplete', resolve(worldDir, 'world.json'),
        `${published.reason}; scene payloads are stat-only and are not read to recover identity`)
      missingEvidence.push('canonical published sceneId paths in world.json')
    } else if (published.ids.size !== manifestInfo.scenes.size ||
      [...manifestInfo.scenes.keys()].some((sceneId) => !published.ids.has(sceneId))) {
      check(checks, 'identity.manifest-world-scenes', 'fail', manifestPath,
        'manifest sceneIds do not match the canonical published world.json scene paths')
    } else {
      check(checks, 'identity.manifest-world-scenes', 'pass', manifestPath,
        'manifest sceneIds match canonical published world.json scene paths')
    }

    const publishedIdentity = worldCompactIdentity(worldData)
    if (!publishedIdentity.ok) {
      check(checks, 'identity.manifest-world', publishedIdentity.status, resolve(worldDir, 'world.json'),
        `${publishedIdentity.reason}; cannot verify manifest/world compact identity`)
      if (publishedIdentity.status === 'incomplete') missingEvidence.push('world.json seed/renderMode')
    } else if (manifestInfo.identity.seed !== publishedIdentity.value.seed ||
      manifestInfo.identity.renderMode !== publishedIdentity.value.renderMode) {
      check(checks, 'identity.manifest-world', 'fail', manifestPath,
        'manifest.world seed/renderMode do not match world.json')
    } else {
      check(checks, 'identity.manifest-world', 'pass', manifestPath,
        'manifest.world seed/renderMode match world.json')
    }
  }

  // ---- source snapshot (file-level binding) --------------------------------
  const snap = receipt.sourceSnapshot
  if (snap === null || snap === undefined || !isPlainObject(snap)) {
    check(checks, 'identity.source-snapshot', 'incomplete', receiptPath,
      'receipt.sourceSnapshot is missing; the source tree identity is not bound')
    missingEvidence.push('receipt.sourceSnapshot')
  } else {
    const files = snap.files
    if (!Array.isArray(files) || files.length === 0) {
      check(checks, 'identity.source-snapshot', 'incomplete', receiptPath,
        'receipt.sourceSnapshot.files must be a non-empty [{path, sha256}] list to bind the source snapshot on disk')
      missingEvidence.push('receipt.sourceSnapshot.files')
    } else {
      let allOk = true
      for (const e of files) {
        if (!isPlainObject(e) || typeof e.path !== 'string') {
          check(checks, 'identity.source-snapshot', 'incomplete', receiptPath, 'sourceSnapshot.files entry missing path')
          allOk = false
          continue
        }
        const r = await verifyBoundFile({path: resolveAbs(e.path, worldDir), sha256: e.sha256}, maxHash, 'sourceSnapshot.files[]')
        check(checks, 'identity.source-snapshot', r.status, r.path || receiptPath, r.reason)
        if (r.status !== 'pass') allOk = false
      }
      if (allOk) check(checks, 'identity.source-snapshot', 'pass', receiptPath,
        'sourceSnapshot.files are hash-bound on disk (commit provenance is confirmed outside this tool)')
    }
  }

  // ---- world.target --------------------------------------------------------
  const claimedWorld = receipt.world && isPlainObject(receipt.world) && typeof receipt.world.path === 'string'
    ? resolveAbs(receipt.world.path, worldDir) : null
  if (claimedWorld === null) {
    check(checks, 'identity.world-target', 'incomplete', receiptPath,
      'receipt.world.path is missing; cannot confirm this receipt was written for the inspected directory')
    missingEvidence.push('receipt.world.path')
  } else {
    const same = await sameRealPath(claimedWorld, worldDir)
    if (same) {
      check(checks, 'identity.world-target', 'pass', worldDir,
        'receipt.world.path identifies the exact inspected world root (realpath aliases accepted)')
    } else {
      check(checks, 'identity.world-target', 'fail', receiptPath,
        `receipt.world.path "${claimedWorld}" does not identify the exact --world root "${worldDir}"`)
    }
  }

  const runContext = {
    source: receiptPath,
    worldDir,
    runtimePath,
    runtimeSha256,
    manifestInfo,
    manifestSha256,
  }

  // ---- generation run receipt ----------------------------------------------
  const genRun = await validateRunReceipt(receipt.generation, 'generation', checks, missingEvidence, maxJson, maxHash, {
    ...runContext, expectedOperation: 'generate-world',
  })
  const genRunOk = genRun.ok
  // Independent terminal success: report status may back the exit code only if
  // the run receipt itself carries a success/absent status token. Unknown tokens
  // (e.g. "banana") on either side make completion unprovable.
  const genRunStatus = receipt.generation && isPlainObject(receipt.generation) && typeof receipt.generation.status === 'string'
    ? receipt.generation.status : null
  const genReportStatus = genData && typeof genData.status === 'string' ? genData.status : null
  if (genRunOk && genRunStatus !== null && classifyToken(genRunStatus).kind === 'unknown') {
    check(checks, 'generation.terminal', 'incomplete', receiptPath,
      `generation.status "${genRunStatus}" is not a recognised terminal token`)
    missingEvidence.push('generation.status terminal token')
  } else if (genRunOk && genReportStatus !== null && classifyToken(genReportStatus).kind === 'failed') {
    check(checks, 'generation.terminal', 'fail', worldDir,
      `generation-report.status is "${genReportStatus}"; the generation did not reach terminal success`)
  } else if (genRunOk && genReportStatus !== null && classifyToken(genReportStatus).kind === 'running') {
    check(checks, 'generation.terminal', 'fail', worldDir,
      `generation-report.status is "${genReportStatus}" (still running)`)
  } else if (genRunOk && genReportStatus !== null && classifyToken(genReportStatus).kind === 'unknown') {
    check(checks, 'generation.terminal', 'incomplete', worldDir,
      `generation-report.status "${genReportStatus}" is not recognised`)
    missingEvidence.push('generation-report.status terminal token')
  }

  // ---- independent full quality gate run receipt ---------------------------
  if (gateData && (gateInfo.gateResult === 'pass' || gateInfo.gateResult === 'fail')) {
    const gateRun = await validateRunReceipt(receipt.gateRun, 'gateRun', checks, missingEvidence, maxJson, maxHash, {
      ...runContext, expectedOperation: 'world-quality-gate',
    })
    if (gateRun.ok) {
      const expectedOutcome = gateInfo.gateResult === 'pass' ? 'success' : 'failed'
      if (gateRun.outcome !== expectedOutcome) {
        check(checks, 'gateRun.contradiction', 'fail', receiptPath,
          `gateRun terminal outcome ${gateRun.outcome} conflicts with world-quality-gate.json status ${gateInfo.gateResult}`)
      } else {
        check(checks, 'gateRun.contradiction', 'pass', receiptPath,
          'gateRun outcome is consistent with world-quality-gate.json')
      }
    }
  } else if (gateData && gateInfo.gateResult === null) {
    // unknown gate status already flagged
  } else if (!gateData) {
    // gate missing already flagged
  } else {
    check(checks, 'gateRun.present', 'incomplete', receiptPath,
      'gate result needs an independent gateRun receipt proving the full gate actually executed')
    missingEvidence.push('receipt.gateRun')
  }

  // ---- frontage ------------------------------------------------------------
  const frontage = scope.frontage && isPlainObject(scope.frontage) ? scope.frontage : null
  if (frontage !== null) {
    const sceneId = typeof frontage.sceneId === 'string' && frontage.sceneId.length > 0 ? frontage.sceneId : null
    if (sceneId === null) {
      check(checks, 'frontage.locate', 'incomplete', receiptPath,
        'frontage claim needs a sceneId to be located against the audit')
      missingEvidence.push('claimedScope.frontage.sceneId')
    }
    const auditPath = typeof frontage.auditPath === 'string' ? resolveAbs(frontage.auditPath, worldDir) : null
    const auditSrcPath = auditPath !== null ? auditPath : resolve(worldDir, 'quality-report.json')
    const ar = await readJsonSmall(auditSrcPath, maxJson)
    let fa = null
    let auditWorldRev = null
    if (ar.status !== 'ok') {
      check(checks, 'frontage.audit', 'incomplete', auditSrcPath,
        `frontage audit source unreadable (${ar.status})${ar.error ? ' — ' + ar.error : ''}`)
      missingEvidence.push(`frontage audit ${auditSrcPath}`)
    } else {
      const d = ar.data
      auditWorldRev = isPlainObject(d) && typeof d.worldRevision === 'string' ? d.worldRevision : null
      const scenes = isPlainObject(d) && Array.isArray(d.scenes) ? d.scenes
        : (Array.isArray(d) ? d : null)
      if (isPlainObject(d) && isPlainObject(d.frontageAudit)) {
        if (sceneId === null || d.sceneId === sceneId) fa = d.frontageAudit
      } else if (Array.isArray(scenes)) {
        if (sceneId !== null) {
          const hit = scenes.find((s) => isPlainObject(s) && s.sceneId === sceneId && isPlainObject(s.frontageAudit))
          if (hit) fa = hit.frontageAudit
        } else if (scenes.length === 1) {
          fa = scenes[0].frontageAudit
        }
      }
      if (fa === null) {
        check(checks, 'frontage.locate', 'fail', auditSrcPath,
          `frontageAudit for sceneId=${sceneId || '(any, multiple scenes present)'} not found in ${auditSrcPath}`)
      } else {
        check(checks, 'frontage.locate', 'pass', auditSrcPath,
          `frontageAudit located for scene ${sceneId === null ? '(single scene)' : sceneId}`)
      }
    }
    if (fa !== null) {
      const aReq = asNonNegInt(firstDefined(fa.requested))
      const aPlan = asNonNegInt(firstDefined(fa.planned))
      const aReal = asNonNegInt(firstDefined(fa.realizedWithAccess))
      const aUnf = asNonNegInt(firstDefined(fa.unfilled))
      if (aReq === null || aPlan === null || aReal === null || aUnf === null) {
        check(checks, 'frontage.audit-shape', 'incomplete', auditSrcPath,
          'frontageAudit must expose requested/planned/realizedWithAccess/unfilled integers')
        missingEvidence.push('frontageAudit.{requested,planned,realizedWithAccess,unfilled}')
      } else {
        const planningOk = aReq === aPlan + aUnf
        const realizeOk = aReal >= 0 && aReal <= aPlan
        if (!planningOk) {
          check(checks, 'frontage.audit-shape', 'fail', auditSrcPath,
            `frontage planning shortfall broken: requested(${aReq}) != planned(${aPlan}) + unfilled(${aUnf})`)
        } else if (!realizeOk) {
          check(checks, 'frontage.audit-shape', 'fail', auditSrcPath,
            `frontage realisedWithAccess(${aReal}) must satisfy 0 <= realizedWithAccess <= planned(${aPlan})`)
        } else {
          check(checks, 'frontage.audit-shape', 'pass', auditSrcPath,
            `frontage audit consistent: requested=${aReq}, planned=${aPlan}, plannedShortfall(unfilled)=${aUnf}, ` +
            `realizedWithAccess=${aReal}, implementationShortfall(planned-realized)=${aPlan - aReal}`)
        }
        if (planningOk && realizeOk) {
          const cReq = asNonNegInt(firstDefined(frontage.requested))
          const cPlan = asNonNegInt(firstDefined(frontage.planned))
          const cReal = asNonNegInt(firstDefined(frontage.realizedWithAccess, frontage.realized))
          const cUnf = asNonNegInt(firstDefined(frontage.unfilled))
          const claimMatch = cReq === aReq && cPlan === aPlan && cReal === aReal && cUnf === aUnf
          if (claimMatch) {
            check(checks, 'frontage.claim-match', 'pass', auditSrcPath,
              'frontage claim equals the audit ledger numbers')
          } else {
            check(checks, 'frontage.claim-match', 'fail', auditSrcPath,
              `frontage claim (requested=${cReq}, planned=${cPlan}, realizedWithAccess=${cReal}, unfilled=${cUnf}) ` +
              `does not match audit (${aReq}/${aPlan}/${aReal}/${aUnf})`)
          }
        }
      }
      const expectedRev = worldData && typeof worldData.revision === 'string' && worldData.revision.length > 0
        ? worldData.revision
        : (revSet.size === 1 ? [...revSet][0] : null)
      if (auditWorldRev !== null && expectedRev !== null && auditWorldRev !== expectedRev) {
        check(checks, 'frontage.revision-bind', 'fail', auditSrcPath,
          `frontage audit worldRevision ${auditWorldRev} does not match reports revision ${expectedRev}`)
      } else if (auditWorldRev !== null && expectedRev !== null) {
        check(checks, 'frontage.revision-bind', 'pass', auditSrcPath, 'frontage audit worldRevision matches the reports')
      } else {
        check(checks, 'frontage.revision-bind', 'incomplete', auditSrcPath,
          'frontage audit source carries no worldRevision to bind it to this world')
        missingEvidence.push('frontage audit worldRevision')
      }
    }
    const geomReceipt = typeof frontage.geometricVerificationReceiptPath === 'string'
      ? resolveAbs(frontage.geometricVerificationReceiptPath, worldDir) : null
    if (geomReceipt !== null && (await tryStat(geomReceipt)) !== null) {
      unverified.push('frontage geometry: independent geometric verification receipt referenced at ' +
        `${geomReceipt}; this tool only checks the numeric ledger and does not re-verify geometry`)
    } else {
      unverified.push('frontage geometry: only the numeric ledger was checked; counts (realizedWithAccess/planned) are NOT proof of geometric ' +
        'realisation (per-entity support/access needs an independent tool like scripts/check-frontage.mjs)')
    }
  }

  const summary = {
    worldRevision: revSet.size === 1 ? [...revSet][0] : null,
    generatorRevision: genSet.size === 1 ? [...genSet][0] : null,
    generationReportStatus: genData ? (typeof genData.status === 'string' ? genData.status : null) : null,
    gateRawStatus: gateInfo.rawStatus,
    sceneCount: genData && Number.isInteger(genData.sceneCount) ? genData.sceneCount : null,
    chunkCount: genData && Number.isInteger(genData.chunkCount) ? genData.chunkCount : null,
    entityCount: genData && Number.isInteger(genData.entityCount) ? genData.entityCount : null,
    sceneFiles: sceneFiles.length,
  }
  return {checks, missingEvidence, unverified, worldQualityStatus, receipt: {taskId, authority}, gateInfo, summary}
}

export function resolveStatus(checks) {
  let worst = 'pass'
  for (const c of checks) {
    if (c.status === 'fail') return 'fail'
    if (c.status === 'unsupported' && worst !== 'fail') worst = 'unsupported'
    if (c.status === 'incomplete' && worst === 'pass') worst = 'incomplete'
  }
  return worst
}

export async function runReceipt({world, receipt, out, maxJsonBytes, maxHashBytes} = {}) {
  const cfg = {
    maxJsonBytes: isFinitePositiveInt(maxJsonBytes) ? maxJsonBytes : DEFAULT_MAX_JSON_BYTES,
    maxHashBytes: isFinitePositiveInt(maxHashBytes) ? maxHashBytes : DEFAULT_MAX_HASH_BYTES,
  }
  const worldDir = resolve(world)
  const r = await collectChecks(worldDir, receipt, cfg)
  const status = resolveStatus(r.checks)
  const exitCode = status === 'pass' ? 0 : 1
  const result = {
    format: FORMAT,
    version: VERSION,
    taskId: r.receipt && r.receipt.taskId ? r.receipt.taskId : null,
    authority: r.receipt && r.receipt.authority ? r.receipt.authority : 'unknown',
    world: worldDir,
    receipt,
    out,
    generatedAt: new Date().toISOString(),
    status,
    worldQualityStatus: r.worldQualityStatus,
    checks: r.checks,
    missingEvidence: r.missingEvidence,
    unexpectedFailures: r.gateInfo ? r.gateInfo.unexpectedFailures : [],
    resolvedFailures: r.gateInfo ? r.gateInfo.resolvedFailures : [],
    unverified: r.unverified,
    summary: r.summary,
    exitCode,
  }
  return {result, exitCode}
}

export async function writeResult(out, result, opts = {}) {
  const {worldDir = null} = opts
  const outAbs = resolve(out)
  if (worldDir !== null) {
    // Lexical containment first (covers a not-yet-created out path), then realpath
    // containment to catch symlink/junction redirection of an existing parent.
    const worldAbs = resolve(worldDir)
    const lexInside = isInside(outAbs, worldAbs)
    const real = await isRealInside(outAbs, worldDir)
    const realInside = real.contained === true
    if (lexInside || realInside) {
      return {status: 'exists', reason: `refusing to write result inside the read-only world tree: ${out}`}
    }
  }
  await mkdir(dirname(outAbs), {recursive: true})
  try {
    await writeFile(outAbs, JSON.stringify(result, null, 2) + '\n', {encoding: 'utf8', flag: 'wx'})
  } catch (e) {
    if (e && e.code === 'EEXIST') {
      return {status: 'exists', reason: `refusing to overwrite existing path: ${out}`}
    }
    return {status: 'write_error', reason: `could not write ${out}: ${e && e.message || e}`}
  }
  return {status: 'ok'}
}

export function isMain(importMetaUrl) {
  const argv1 = process.argv[1]
  if (!argv1) return false
  try { return pathToFileURL(resolve(argv1)).href === importMetaUrl } catch { return false }
}

export const __self = pathToFileURL(resolve(fileURLToPath(import.meta.url))).href

async function main(argv) {
  const {out: opts, errs} = parseCliArgs(argv)
  if (errs.length > 0 || opts.help) {
    if (errs.length > 0) process.stderr.write(`error: ${errs.join('; ')}\n`)
    process.stdout.write(USAGE + '\n')
    return {exitCode: errs.length > 0 ? 2 : 0}
  }
  if (opts.world === null || opts.receipt === null || opts.out === null) {
    process.stderr.write('error: --world, --receipt and --out are all required\n')
    process.stdout.write(USAGE + '\n')
    return {exitCode: 2}
  }
  const worldStat = await tryStat(resolve(opts.world))
  if (worldStat === null || !worldStat.isDirectory()) {
    process.stderr.write(`error: --world is not an existing directory: ${opts.world}\n`)
    process.stdout.write(USAGE + '\n')
    return {exitCode: 2}
  }
  if (isInside(resolve(opts.out), resolve(opts.world))) {
    process.stderr.write(`error: --out must not be inside the read-only world tree: ${opts.out}\n`)
    process.stdout.write(USAGE + '\n')
    return {exitCode: 2}
  }
  const {result, exitCode} = await runReceipt({
    world: opts.world,
    receipt: opts.receipt,
    out: opts.out,
    maxJsonBytes: opts.maxJsonBytes,
    maxHashBytes: opts.maxHashBytes,
  })
  const wr = await writeResult(opts.out, result, {worldDir: resolve(opts.world)})
  if (wr.status !== 'ok') {
    process.stderr.write(`error: ${wr.reason}\n`)
    return {exitCode: 2}
  }
  process.stdout.write(`status=${result.status} worldQualityStatus=${result.worldQualityStatus.value} exitCode=${exitCode}\n`)
  return {exitCode}
}

if (isMain(__self)) {
  const {exitCode} = await main(process.argv.slice(2))
  process.exitCode = exitCode
}
