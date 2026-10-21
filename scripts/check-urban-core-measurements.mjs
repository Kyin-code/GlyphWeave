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
  if (
    !Number.isFinite(observed.buildingLevelsObservedRatio)
    || observed.buildingLevelsObservedRatio < 0
    || observed.buildingLevelsObservedRatio > 1
  ) {
    failures.push(`${name}: invalid buildingLevelsObservedRatio`)
  }
  if (
    !Number.isFinite(observed.observedFloorAreaRatioProxy)
    || observed.observedFloorAreaRatioProxy < 0
  ) {
    failures.push(`${name}: invalid observedFloorAreaRatioProxy`)
  }
  const access = observed.freshFoodNetworkAccess
  if (!access || access.model !== 'walking-network-proxy') {
    failures.push(`${name}: missing walking-network access model`)
  } else {
    validateAccessMetric(
      name,
      'allMappedFreshFood',
      access.allMappedFreshFood,
      failures,
    )
    validateAccessMetric(
      name,
      'marketLikeFreshFood',
      access.marketLikeFreshFood,
      failures,
    )
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
    buildingLevelsObservedRatio: observed.buildingLevelsObservedRatio,
    observedFloorAreaRatioProxy: observed.observedFloorAreaRatioProxy,
    marketLikeMedianMinutes:
      access?.marketLikeFreshFood?.medianMinutes ?? null,
    marketLikeWithin10MinuteRatio:
      access?.marketLikeFreshFood?.within10MinuteRatio ?? null,
    file: name,
  })
}

if (rows.length !== 48) {
  failures.push(`expected 48 measured core windows, got ${rows.length}`)
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
    'buildingLevelsObservedRatio',
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
  const marketRows = rows.filter(
    (row) => row.marketLikeMedianMinutes !== null,
  )
  summary.mappedMarketAccessWindows = marketRows.length
  summary.marketLikeMedianMinutes = summarizeNumbers(
    marketRows.map((row) => row.marketLikeMedianMinutes),
  )
  summary.marketLikeWithin10MinuteRatio = summarizeNumbers(
    marketRows.map((row) => row.marketLikeWithin10MinuteRatio),
  )
  console.log(JSON.stringify({
    status: 'pass',
    windows: rows.length,
    summary,
    rows: rows.sort((a, b) => a.caseId.localeCompare(b.caseId)),
  }, null, 2))
}

function validateAccessMetric(name, label, metric, failures) {
  if (!metric) {
    failures.push(`${name}: missing access.${label}`)
    return
  }
  if (
    !Number.isInteger(metric.routedFacilityCount)
    || metric.routedFacilityCount < 0
    || !Number.isInteger(metric.demandBuildingCount)
    || metric.demandBuildingCount < 0
    || !Number.isInteger(metric.buildingsWithTime)
    || metric.buildingsWithTime < 0
    || !Number.isInteger(metric.unreachableBuildingCount)
    || metric.unreachableBuildingCount < 0
  ) {
    failures.push(`${name}: invalid access counts for ${label}`)
  }
  if (metric.routedFacilityCount === 0) {
    if (
      metric.medianMinutes !== null
      || metric.p90Minutes !== null
      || metric.within10MinuteRatio !== null
    ) {
      failures.push(`${name}: zero routed facilities must not claim access metrics`)
    }
  } else {
    for (const field of ['medianMinutes', 'p90Minutes']) {
      if (!Number.isFinite(metric[field]) || metric[field] < 0) {
        failures.push(`${name}: invalid ${label}.${field}`)
      }
    }
    for (const field of [
      'within5MinuteRatio',
      'within10MinuteRatio',
      'within15MinuteRatio',
    ]) {
      if (
        !Number.isFinite(metric[field])
        || metric[field] < 0
        || metric[field] > 1
      ) {
        failures.push(`${name}: invalid ${label}.${field}`)
      }
    }
  }
}

function summarizeNumbers(values) {
  if (values.length === 0) {
    return null
  }
  const sorted = [...values].sort((a, b) => a - b)
  const middle = Math.floor(sorted.length / 2)
  return {
    min: sorted[0],
    median: sorted.length % 2
      ? sorted[middle]
      : (sorted[middle - 1] + sorted[middle]) / 2,
    max: sorted[sorted.length - 1],
  }
}
