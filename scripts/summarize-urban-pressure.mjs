import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'

const priorsPath = resolve(
  process.argv[2] ?? 'assets/data/urban-case-priors-48.json',
)
const sourcePath = resolve(
  process.argv[3] ?? 'bevy/crates/core/src/worldgen/land_pressure.rs',
)
const output = resolve(
  process.argv[4] ?? 'assets/data/urban-pressure-patterns.json',
)

const data = JSON.parse(await readFile(priorsPath, 'utf8'))
const source = await readFile(sourcePath, 'utf8')
if (!Array.isArray(data.cases) || data.cases.length !== 48) {
  throw new Error(`expected 48 cases, got ${data.cases?.length ?? 0}`)
}

const factorIds = new Set(
  data.cases.flatMap((item) => item.specialFactors ?? []),
)
const implementationWeights = Object.fromEntries(
  [...factorIds].map((factor) => {
    const escaped = factor.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    const match = source.match(
      new RegExp(`"${escaped}"\\s*=>\\s*([0-9.]+)`),
    )
    return [factor, match ? Number(match[1]) : null]
  }),
)
const missingWeights = Object.entries(implementationWeights)
  .filter(([, weight]) => weight === null)
  .map(([factor]) => factor)
if (missingWeights.length > 0) {
  throw new Error(
    `missing implementation weights for: ${missingWeights.join(', ')}`,
  )
}

const cases = data.cases.map((item) => ({
  caseId: item.id,
  region: item.region,
  scale: item.landPressurePrior.settlementScale,
  factors: item.specialFactors,
  terrainScarcity: item.landPressurePrior.terrainScarcity,
  protectionResistance: item.landPressurePrior.protectionResistance,
  specialRegime: item.landPressurePrior.specialRegime,
}))
for (const item of cases) {
  for (const field of [
    'terrainScarcity',
    'protectionResistance',
    'specialRegime',
  ]) {
    if (!Number.isFinite(item[field]) || item[field] <= 0) {
      throw new Error(`${item.caseId}: invalid ${field}`)
    }
  }
}

const scaleGroups = groupBy(cases, (item) => item.scale)
const factorGroups = Object.fromEntries(
  [...factorIds].sort().map((factor) => {
    const matching = cases.filter((item) => item.factors.includes(factor))
    const other = cases.filter((item) => !item.factors.includes(factor))
    return [factor, {
      caseIds: matching.map((item) => item.caseId),
      grouping: summarize(matching),
      otherCases: summarize(other),
      implementationWeight: implementationWeights[factor],
      descriptiveRegimeRatio:
        ratio(
          median(matching.map((item) => item.specialRegime)),
          median(other.map((item) => item.specialRegime)),
        ),
    }]
  }),
)

const result = {
  format: 'glyphweave.urban-pressure-patterns',
  version: 1,
  generatedAt: new Date().toISOString().slice(0, 10),
  priorsPath,
  implementationSource: sourcePath,
  method: 'descriptive-grouping-of-heuristic-priors',
  warning: 'Factors overlap heavily. Group medians and ratios are descriptive, not causal coefficients. The landPressurePrior values are not yet measured from official statistics.',
  scaleGroups: Object.fromEntries(
    Object.entries(scaleGroups).map(([scale, items]) => [
      scale,
      {
        caseIds: items.map((item) => item.caseId),
        ...summarize(items),
      },
    ]),
  ),
  factorGroups,
  implementationWeights,
  cases,
}

await mkdir(dirname(output), { recursive: true })
await writeFile(output, `${JSON.stringify(result, null, 2)}\n`, 'utf8')
console.log(JSON.stringify({
  status: 'pass',
  output,
  cases: cases.length,
  scaleGroups: result.scaleGroups,
  factorGroups,
  implementationWeights,
}, null, 2))

function groupBy(items, keyFn) {
  const groups = {}
  for (const item of items) {
    const key = keyFn(item)
    groups[key] ??= []
    groups[key].push(item)
  }
  return groups
}

function summarize(items) {
  return {
    terrainScarcity: stats(items.map((item) => item.terrainScarcity)),
    protectionResistance: stats(
      items.map((item) => item.protectionResistance),
    ),
    specialRegime: stats(items.map((item) => item.specialRegime)),
  }
}

function stats(values) {
  const sorted = [...values].sort((a, b) => a - b)
  return {
    count: sorted.length,
    min: sorted[0],
    median: median(sorted),
    max: sorted.at(-1),
  }
}

function median(sortedOrUnsorted) {
  const sorted = [...sortedOrUnsorted].sort((a, b) => a - b)
  const middle = Math.floor(sorted.length / 2)
  return sorted.length % 2
    ? sorted[middle]
    : (sorted[middle - 1] + sorted[middle]) / 2
}

function ratio(numerator, denominator) {
  return Math.round((numerator / denominator) * 1000) / 1000
}
