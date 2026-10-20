import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'

const file = resolve(process.argv[2] ?? 'assets/data/urban-case-priors-48.json')
const data = JSON.parse(await readFile(file, 'utf8'))
const failures = []
if (
  data.format !== 'glyphweave.urban-case-priors' ||
  ![1, 2].includes(data.version)
) {
  failures.push('unsupported format/version')
}
if (!Array.isArray(data.cases) || data.cases.length !== 48) {
  failures.push(`expected 48 cases, got ${data.cases?.length ?? 0}`)
}
const expectedPrefixCounts = { cn: 12, us: 12, eu: 12, jp: 6, hk: 6 }
const ids = new Set()
const prefixCounts = { cn: 0, us: 0, eu: 0, jp: 0, hk: 0 }
const regions = new Set([
  'china',
  'united-states',
  'europe',
  'japan',
  'hong-kong',
])
const processes = new Set([
  'tectonic-uplift',
  'tectonic-subsidence',
  'faulting',
  'fluvial-incision',
  'fluvial-deposition',
  'alluvial-fan-deposition',
  'river-avulsion',
  'delta-deposition',
  'tidal-processes',
  'coastal-wave-erosion',
  'coastal-deposition',
  'reclamation',
  'land-reclamation',
  'aeolian-erosion',
  'aeolian-deposition',
  'glacial-scour',
  'glacial-deposition',
  'karst-dissolution',
  'mass-wasting',
  'volcanism',
  'lake-sedimentation',
  'wetland-accumulation',
  'peat-subsidence',
  'polder-drainage',
  'drainage-canalization',
  'monsoon-flooding',
  'flash-flood',
  'irrigation-sedimentation',
  'irrigation-canalization',
  'wildfire',
  'post-glacial-uplift',
  'coastal-plain-sedimentation',
])
const profiles = new Set([
  'china-gated',
  'china-urban-village',
  'us-suburban',
  'us-mixed-income',
  'europe-perimeter',
  'japan-machiya',
  'hong-kong-estate',
])
const scales = new Set([
  'village',
  'town',
  'county-city',
  'city',
  'metropolis',
  'megacity',
])
const regionForPrefix = {
  cn: 'china',
  us: 'united-states',
  eu: 'europe',
  jp: 'japan',
  hk: 'hong-kong',
}

function requireText(value, label, index) {
  if (typeof value !== 'string' || value.trim() === '') {
    failures.push(`case ${index}: empty ${label}`)
  }
}

for (const [index, item] of (data.cases ?? []).entries()) {
  requireText(item.id, 'id', index)
  if (ids.has(item.id)) failures.push(`duplicate id ${item.id}`)
  ids.add(item.id)
  const prefix = item.id?.split('-')[0]
  if (!(prefix in prefixCounts)) {
    failures.push(`case ${index}: unsupported id prefix ${prefix}`)
  } else {
    prefixCounts[prefix] += 1
  }

  if (data.version === 1) {
    for (const field of ['naturalGeography', 'historicalLayers', 'focusCuts']) {
      requireText(item[field], field, index)
    }
    continue
  }

  if (item.region !== regionForPrefix[prefix]) {
    failures.push(`case ${index}: region ${item.region} mismatches id prefix ${prefix}`)
  }
  if (!regions.has(item.region)) {
    failures.push(`case ${index}: unsupported region ${item.region}`)
  }
  for (const field of [
    'coreCut',
    'focusFeature',
    'communityProfile',
  ]) {
    requireText(item[field], field, index)
  }
  if (!profiles.has(item.communityProfile)) {
    failures.push(`case ${index}: unsupported communityProfile ${item.communityProfile}`)
  }
  const natural = item.naturalGeography
  if (!natural || typeof natural !== 'object') {
    failures.push(`case ${index}: missing naturalGeography object`)
  } else {
    requireText(natural.setting, 'naturalGeography.setting', index)
    requireText(natural.climateBand, 'naturalGeography.climateBand', index)
    requireText(natural.hydrography, 'naturalGeography.hydrography', index)
    if (
      !Number.isFinite(natural.regionalReliefM) ||
      natural.regionalReliefM < 0 ||
      natural.regionalReliefM > 5000
    ) {
      failures.push(`case ${index}: invalid regionalReliefM`)
    }
    if (
      !Array.isArray(natural.geomorphicProcesses) ||
      natural.geomorphicProcesses.length === 0
    ) {
      failures.push(`case ${index}: no geomorphicProcesses`)
    } else {
      for (const process of natural.geomorphicProcesses) {
        if (!processes.has(process)) {
          failures.push(`case ${index}: unsupported process ${process}`)
        }
      }
    }
    if (!Array.isArray(natural.hazards) || natural.hazards.length === 0) {
      failures.push(`case ${index}: no hazards`)
    }
  }
  if (!Array.isArray(item.history) || item.history.length < 3) {
    failures.push(`case ${index}: history must have at least three stages`)
  } else {
    item.history.forEach((stage, stageIndex) => {
      if (stage.order !== stageIndex + 1) {
        failures.push(`case ${index}: history order mismatch at ${stageIndex}`)
      }
      requireText(stage.era, `history[${stageIndex}].era`, index)
      requireText(stage.change, `history[${stageIndex}].change`, index)
    })
  }
  const structure = item.urbanStructure
  if (!structure || typeof structure !== 'object') {
    failures.push(`case ${index}: missing urbanStructure`)
  } else {
    for (const field of ['oldCore', 'renewal', 'newCity', 'edge']) {
      requireText(structure[field], `urbanStructure.${field}`, index)
    }
    if (!Array.isArray(structure.centres) || structure.centres.length < 2) {
      failures.push(`case ${index}: urbanStructure.centres needs at least two entries`)
    }
  }
  const pressure = item.landPressurePrior
  if (!pressure || typeof pressure !== 'object') {
    failures.push(`case ${index}: missing landPressurePrior`)
  } else {
    if (!scales.has(pressure.settlementScale)) {
      failures.push(`case ${index}: unsupported settlementScale`)
    }
    for (const field of [
      'terrainScarcity',
      'protectionResistance',
      'specialRegime',
    ]) {
      if (
        !Number.isFinite(pressure[field]) ||
        pressure[field] < 0.2 ||
        pressure[field] > 2.5
      ) {
        failures.push(`case ${index}: invalid ${field}`)
      }
    }
  }
  if (!Array.isArray(item.specialFactors) || item.specialFactors.length === 0) {
    failures.push(`case ${index}: specialFactors must not be empty`)
  }
}
for (const [prefix, expected] of Object.entries(expectedPrefixCounts)) {
  if (prefixCounts[prefix] !== expected) {
    failures.push(`${prefix}: expected ${expected}, got ${prefixCounts[prefix]}`)
  }
}
if (failures.length) {
  console.error(JSON.stringify({ status: 'fail', failures }, null, 2))
  process.exitCode = 1
} else {
  console.log(JSON.stringify({
    status: 'pass',
    version: data.version,
    cases: data.cases.length,
    prefixCounts,
    file,
  }))
}
