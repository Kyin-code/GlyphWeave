import { access, readFile } from 'node:fs/promises'
import { resolve } from 'node:path'

const priorsPath = resolve(
  process.argv[2] ?? 'assets/data/urban-case-priors-48.json',
)
const evidencePath = resolve(
  process.argv[3] ?? 'assets/data/urban-case-evidence.json',
)
const historyPath = resolve(
  process.argv[4] ?? 'assets/data/urban-history-patterns.json',
)

const [priors, evidence, history] = await Promise.all(
  [priorsPath, evidencePath, historyPath].map(async (path) =>
    JSON.parse(await readFile(path, 'utf8'))),
)
const failures = []
const sourceIds = new Set((evidence.sources ?? []).map((source) => source.id))
const coverageByCase = new Map(
  (evidence.coverage ?? []).map((item) => [item.caseId, item]),
)
const historyByCase = new Map(
  (history.cases ?? []).map((item) => [item.caseId, item]),
)
const caseIds = new Set()
const processCounts = new Map()
const hazardCounts = new Map()
const scaleCounts = new Map()
let sourceAudited = 0
let heuristicGeography = 0

const text = (value) => typeof value === 'string' && value.trim() !== ''
const requireText = (caseId, value, field) => {
  if (!text(value)) failures.push(`${caseId}: missing ${field}`)
}
const count = (map, value) => map.set(value, (map.get(value) ?? 0) + 1)

for (const item of priors.cases ?? []) {
  if (!text(item.id)) {
    failures.push('case with missing id')
    continue
  }
  const { id } = item
  if (caseIds.has(id)) failures.push(`${id}: duplicate case id`)
  caseIds.add(id)
  for (const field of ['coreCut', 'focusFeature', 'communityProfile']) {
    requireText(id, item[field], field)
  }

  const natural = item.naturalGeography
  if (!natural || typeof natural !== 'object') {
    failures.push(`${id}: missing naturalGeography`)
  } else {
    for (const field of ['setting', 'climateBand', 'hydrography', 'evidenceTier']) {
      requireText(id, natural[field], `naturalGeography.${field}`)
    }
    if (!Number.isFinite(natural.regionalReliefM) || natural.regionalReliefM < 0) {
      failures.push(`${id}: invalid regionalReliefM`)
    }
    if (!Array.isArray(natural.geomorphicProcesses) || natural.geomorphicProcesses.length === 0) {
      failures.push(`${id}: no geomorphicProcesses`)
    } else {
      for (const process of natural.geomorphicProcesses) count(processCounts, process)
    }
    if (!Array.isArray(natural.hazards) || natural.hazards.length === 0) {
      failures.push(`${id}: no hazards`)
    } else {
      for (const hazard of natural.hazards) count(hazardCounts, hazard)
    }
    if (natural.evidenceTier !== 'source-verified') heuristicGeography += 1
  }

  if (!Array.isArray(item.history) || item.history.length < 3) {
    failures.push(`${id}: history needs at least three stages`)
  } else {
    item.history.forEach((stage, index) => {
      if (stage.order !== index + 1) failures.push(`${id}: history order ${index} is invalid`)
      requireText(id, stage.era, `history[${index}].era`)
      requireText(id, stage.change, `history[${index}].change`)
    })
  }

  const structure = item.urbanStructure
  if (!structure || typeof structure !== 'object') {
    failures.push(`${id}: missing urbanStructure`)
  } else {
    for (const field of ['oldCore', 'renewal', 'newCity', 'edge']) {
      requireText(id, structure[field], `urbanStructure.${field}`)
    }
    if (!Array.isArray(structure.centres) || structure.centres.length < 2) {
      failures.push(`${id}: urbanStructure.centres needs at least two entries`)
    }
  }

  const pressure = item.landPressurePrior
  if (!pressure || !text(pressure.settlementScale)) {
    failures.push(`${id}: missing landPressurePrior.settlementScale`)
  } else {
    count(scaleCounts, pressure.settlementScale)
  }
  if (!Array.isArray(item.specialFactors) || item.specialFactors.length === 0) {
    failures.push(`${id}: specialFactors must not be empty`)
  }

  const coverage = coverageByCase.get(id)
  if (!coverage) {
    failures.push(`${id}: missing evidence coverage`)
    continue
  }
  if (coverage.level === 'source-audited-core-cut') sourceAudited += 1
  if (!Array.isArray(coverage.sourceIds) || coverage.sourceIds.length < 2) {
    failures.push(`${id}: needs at least two evidence sources`)
  } else {
    for (const sourceId of coverage.sourceIds) {
      if (!sourceIds.has(sourceId)) failures.push(`${id}: unknown source ${sourceId}`)
    }
  }
  if (!text(coverage.measurementFile)) {
    failures.push(`${id}: missing measurementFile`)
  } else {
    try {
      await access(resolve(coverage.measurementFile))
    } catch {
      failures.push(`${id}: measurement file not found`)
    }
  }

  const historyCategories = historyByCase.get(id)
  if (!historyCategories) {
    failures.push(`${id}: missing history-category audit`)
  } else if (!Array.isArray(historyCategories.stages) || historyCategories.stages.length < 3) {
    failures.push(`${id}: history-category audit needs three stages`)
  }
}

if (caseIds.size !== 48) failures.push(`expected 48 cases, got ${caseIds.size}`)
if (sourceAudited !== 48) failures.push(`expected 48 source-audited cases, got ${sourceAudited}`)
if (coverageByCase.size !== 48) failures.push(`expected 48 evidence rows, got ${coverageByCase.size}`)
if (historyByCase.size !== 48) failures.push(`expected 48 history rows, got ${historyByCase.size}`)

const sortedObject = (map) => Object.fromEntries(
  [...map.entries()].sort(([left], [right]) => left.localeCompare(right)),
)
const result = {
  format: 'glyphweave.urban-case-completeness-audit',
  version: 1,
  status: failures.length === 0 ? 'pass' : 'fail',
  cases: caseIds.size,
  sourceAuditedCases: sourceAudited,
  heuristicGeographyCases: heuristicGeography,
  measurementWindows: coverageByCase.size,
  historyAudits: historyByCase.size,
  scaleCounts: sortedObject(scaleCounts),
  topProcesses: Object.entries(sortedObject(processCounts))
    .sort((left, right) => right[1] - left[1])
    .slice(0, 12),
  hazards: sortedObject(hazardCounts),
  failures,
}

if (failures.length) process.exitCode = 1
console.log(JSON.stringify(result, null, 2))
