import { readFile, readdir } from 'node:fs/promises'
import { resolve } from 'node:path'

const directory = resolve(
  process.argv[2] ?? 'assets/data/urban-core-measurements',
)
const names = (await readdir(directory))
  .filter((name) => name.endsWith('.json'))
  .sort()
const failures = []
const rows = []
const ids = new Set()

for (const name of names) {
  const file = resolve(directory, name)
  let item
  try {
    item = JSON.parse(await readFile(file, 'utf8'))
  } catch (error) {
    failures.push(`${name}: invalid JSON (${error.message})`)
    continue
  }
  if (item.format !== 'glyphweave.urban-core-osm-measurement') {
    failures.push(`${name}: unsupported format`)
  }
  if (item.version !== 1) {
    failures.push(`${name}: unsupported version`)
  }
  if (typeof item.caseId !== 'string' || item.caseId === '') {
    failures.push(`${name}: missing caseId`)
    continue
  }
  if (ids.has(item.caseId)) {
    failures.push(`${name}: duplicate caseId ${item.caseId}`)
  }
  ids.add(item.caseId)
  const area = item.sample?.areaKm2
  if (!Number.isFinite(area) || Math.abs(area - 1) > 0.01) {
    failures.push(`${name}: sample area must be approximately 1 km2`)
  }
  const observed = item.observed ?? {}
  for (const field of [
    'roadCenterlineLengthM',
    'roadDensityKmPerKm2',
    'drivableCenterlineLengthM',
    'drivableDensityKmPerKm2',
    'buildingCountInsideCircleCentroid',
    'estimatedBuildingFootprintM2',
    'estimatedBuildingCoverageRatio',
  ]) {
    if (!Number.isFinite(observed[field]) || observed[field] < 0) {
      failures.push(`${name}: invalid observed.${field}`)
    }
  }
  if (
    Number.isFinite(observed.estimatedBuildingCoverageRatio)
    && observed.estimatedBuildingCoverageRatio > 1
  ) {
    failures.push(`${name}: building coverage exceeds 1`)
  }
  if (
    !observed.freshFoodFacilityCounts
    || Object.values(observed.freshFoodFacilityCounts).some(
      (value) => !Number.isInteger(value) || value < 0,
    )
  ) {
    failures.push(`${name}: invalid freshFoodFacilityCounts`)
  }
  if (!Array.isArray(item.limitations) || item.limitations.length < 4) {
    failures.push(`${name}: limitations are incomplete`)
  }
  if (
    item.attribution?.source !== 'OpenStreetMap contributors'
    || item.attribution?.license !== 'Open Database License 1.0'
  ) {
    failures.push(`${name}: missing OSM attribution`)
  }
  const freshFoodPoints = Object.values(
    observed.freshFoodFacilityCounts ?? {},
  ).reduce((sum, value) => sum + value, 0)
  rows.push({
    caseId: item.caseId,
    drivableDensityKmPerKm2: observed.drivableDensityKmPerKm2,
    buildingCount: observed.buildingCountInsideCircleCentroid,
    buildingCoverageRatio: observed.estimatedBuildingCoverageRatio,
    freshFoodPoints,
    file: name,
  })
}

if (rows.length !== 12) {
  failures.push(`expected 12 measured core windows, got ${rows.length}`)
}

if (failures.length) {
  console.error(JSON.stringify({ status: 'fail', failures }, null, 2))
  process.exitCode = 1
} else {
  const summary = {}
  for (const key of [
    'drivableDensityKmPerKm2',
    'buildingCount',
    'buildingCoverageRatio',
    'freshFoodPoints',
  ]) {
    const values = rows.map((row) => row[key]).sort((a, b) => a - b)
    const middle = Math.floor(values.length / 2)
    summary[key] = {
      min: values[0],
      median: values.length % 2
        ? values[middle]
        : (values[middle - 1] + values[middle]) / 2,
      max: values[values.length - 1],
    }
  }
  console.log(JSON.stringify({
    status: 'pass',
    windows: rows.length,
    summary,
    rows: rows.sort((a, b) => a.caseId.localeCompare(b.caseId)),
  }, null, 2))
}
