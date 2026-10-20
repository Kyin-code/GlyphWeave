import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'

const evidencePath = resolve(
  process.argv[2] ?? 'assets/data/urban-case-evidence-12.json',
)
const priorsPath = resolve(
  process.argv[3] ?? 'assets/data/urban-case-priors-48.json',
)
const evidence = JSON.parse(await readFile(evidencePath, 'utf8'))
const priors = JSON.parse(await readFile(priorsPath, 'utf8'))
const failures = []

if (evidence.format !== 'glyphweave.urban-case-evidence') {
  failures.push('unsupported evidence format')
}
if (evidence.version !== 1) {
  failures.push('unsupported evidence version')
}
if (!/^\d{4}-\d{2}-\d{2}$/.test(evidence.retrievedAt ?? '')) {
  failures.push('retrievedAt must be an ISO date')
}

const sourceById = new Map()
const publishersById = new Map()
for (const [index, source] of (evidence.sources ?? []).entries()) {
  if (typeof source.id !== 'string' || source.id.trim() === '') {
    failures.push(`source ${index}: missing id`)
    continue
  }
  if (sourceById.has(source.id)) {
    failures.push(`duplicate source id ${source.id}`)
  }
  sourceById.set(source.id, source)
  publishersById.set(source.id, source.publisher)
  if (
    typeof source.publisher !== 'string'
    || source.publisher.trim() === ''
    || typeof source.title !== 'string'
    || source.title.trim() === ''
  ) {
    failures.push(`source ${source.id}: missing publisher or title`)
  }
  if (!String(source.url ?? '').startsWith('https://')) {
    failures.push(`source ${source.id}: url must use https`)
  }
  if (!/^\d{4}-\d{2}-\d{2}$/.test(source.retrievedAt ?? '')) {
    failures.push(`source ${source.id}: invalid retrievedAt`)
  }
  if (!['direct-200', 'reader-verified'].includes(source.accessStatus)) {
    failures.push(`source ${source.id}: unsupported accessStatus`)
  }
  if (source.accessStatus === 'reader-verified') {
    if (!String(source.readerUrl ?? '').startsWith('https://')) {
      failures.push(`source ${source.id}: missing readerUrl`)
    }
    if (
      !Number.isInteger(source.directHttpStatus)
      || source.directHttpStatus < 400
    ) {
      failures.push(`source ${source.id}: reader-verified source needs a 4xx/5xx direct status`)
    }
  }
  if (
    !Array.isArray(source.supportedClaims)
    || source.supportedClaims.length === 0
    || source.supportedClaims.some(
      (claim) => typeof claim !== 'string' || claim.trim() === '',
    )
  ) {
    failures.push(`source ${source.id}: supportedClaims must be non-empty text`)
  }
}

const priorIds = new Set((priors.cases ?? []).map((item) => item.id))
const coverageIds = new Set()
const auditedIds = []
for (const [index, item] of (evidence.coverage ?? []).entries()) {
  if (typeof item.caseId !== 'string' || item.caseId === '') {
    failures.push(`coverage ${index}: missing caseId`)
    continue
  }
  if (coverageIds.has(item.caseId)) {
    failures.push(`coverage ${index}: duplicate caseId ${item.caseId}`)
  }
  coverageIds.add(item.caseId)
  if (!priorIds.has(item.caseId)) {
    failures.push(`coverage ${index}: unknown prior case ${item.caseId}`)
  }
  if (!Array.isArray(item.measurementGaps) || item.measurementGaps.length === 0) {
    failures.push(`coverage ${index}: measurementGaps must not be empty`)
  }
  if (item.level === 'source-audited-core-cut') {
    auditedIds.push(item.caseId)
    const sourceIds = Array.isArray(item.sourceIds) ? item.sourceIds : []
    const minimum = evidence.policy?.minimumSourceCountForAuditedCase ?? 2
    if (sourceIds.length < minimum) {
      failures.push(`${item.caseId}: needs at least ${minimum} sources`)
    }
    for (const sourceId of sourceIds) {
      if (!sourceById.has(sourceId)) {
        failures.push(`${item.caseId}: unknown source ${sourceId}`)
      }
    }
    const publisherCount = new Set(
      sourceIds
        .filter((sourceId) => sourceById.has(sourceId))
        .map((sourceId) => publishersById.get(sourceId)),
    ).size
    if (
      publisherCount
      < (evidence.policy?.minimumPublisherDiversity ?? 2)
    ) {
      failures.push(`${item.caseId}: insufficient publisher diversity`)
    }
    if (
      !Array.isArray(item.supportedMechanisms)
      || item.supportedMechanisms.length === 0
    ) {
      failures.push(`${item.caseId}: supportedMechanisms must not be empty`)
    }
  } else if (item.level !== 'screened-heuristic') {
    failures.push(`${item.caseId}: unsupported coverage level`)
  }
}

for (const priorId of priorIds) {
  if (!coverageIds.has(priorId)) {
    failures.push(`missing coverage for ${priorId}`)
  }
}
if (coverageIds.size !== priorIds.size) {
  failures.push(
    `coverage count mismatch: evidence ${coverageIds.size}, priors ${priorIds.size}`,
  )
}
if (auditedIds.length !== 12) {
  failures.push(`expected 12 source-audited cases, got ${auditedIds.length}`)
}

if (failures.length) {
  console.error(JSON.stringify({ status: 'fail', failures }, null, 2))
  process.exitCode = 1
} else {
  console.log(JSON.stringify({
    status: 'pass',
    evidenceVersion: evidence.version,
    cases: coverageIds.size,
    sourceAuditedCases: auditedIds.length,
    screenedCases: coverageIds.size - auditedIds.length,
    sources: sourceById.size,
    evidencePath,
    priorsPath,
  }))
}
