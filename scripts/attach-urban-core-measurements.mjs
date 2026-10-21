import { readFile, readdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'

const evidencePath = resolve(
  process.argv[2] ?? 'assets/data/urban-case-evidence.json',
)
const measurementDir = resolve(
  process.argv[3] ?? 'assets/data/urban-core-measurements',
)
const evidence = JSON.parse(await readFile(evidencePath, 'utf8'))
const caseIds = evidence.coverage.map((item) => item.caseId)
const aliases = new Map([
  ['jp-tokyo-asakusa', 'jp-tokyo-shitamachi'],
])
const measurementFiles = (await readdir(measurementDir))
  .filter((name) => name.endsWith('.json'))
  .sort()
const byCase = new Map()
const multipolygonLimitation =
  'Building counts are way-based; multipolygon building relations are not expanded.'

for (const fileName of measurementFiles) {
  const filePath = resolve(measurementDir, fileName)
  const measurement = JSON.parse(await readFile(filePath, 'utf8'))
  if (!measurement.limitations.includes(multipolygonLimitation)) {
    measurement.limitations.push(multipolygonLimitation)
    await writeFile(
      filePath,
      `${JSON.stringify(measurement, null, 2)}\n`,
      'utf8',
    )
  }
  const measurementCaseId = measurement.caseId
  const caseId = aliases.get(measurementCaseId)
    ?? caseIds
      .filter(
        (candidate) => measurementCaseId === candidate
          || measurementCaseId.startsWith(`${candidate}-`),
      )
      .sort((a, b) => b.length - a.length)[0]
  if (!caseId) {
    throw new Error(`cannot map measurement ${measurementCaseId}`)
  }
  if (byCase.has(caseId)) {
    throw new Error(`duplicate measurement for ${caseId}`)
  }
  byCase.set(caseId, {
    file: `assets/data/urban-core-measurements/${fileName}`,
    measurementCaseId,
  })
}

if (byCase.size !== caseIds.length) {
  const missing = caseIds.filter((caseId) => !byCase.has(caseId))
  throw new Error(
    `expected ${caseIds.length} measurements, got ${byCase.size}; missing ${missing.join(', ')}`,
  )
}

for (const item of evidence.coverage) {
  const measurement = byCase.get(item.caseId)
  item.measurementFile = measurement.file
  item.measurementCaseId = measurement.measurementCaseId
  item.measurementStatus = 'partial-osm'
  item.observedFields = [
    'road-density',
    'building-coverage',
    'building-level-coverage',
    'fresh-food-points',
    'fresh-food-network-access',
  ]
}
evidence.policy.expectedPartialOsmWindows = byCase.size

await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`, 'utf8')
console.log(JSON.stringify({
  status: 'pass',
  evidencePath,
  measurements: byCase.size,
  cases: caseIds.length,
}, null, 2))
