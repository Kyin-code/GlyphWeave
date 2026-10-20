import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'

const file = resolve(process.argv[2] ?? 'assets/data/urban-case-priors-48.json')
const data = JSON.parse(await readFile(file, 'utf8'))
const failures = []
if (data.format !== 'glyphweave.urban-case-priors' || data.version !== 1) {
  failures.push('unsupported format/version')
}
if (!Array.isArray(data.cases) || data.cases.length !== 48) {
  failures.push(`expected 48 cases, got ${data.cases?.length ?? 0}`)
}
const expectedPrefixCounts = { cn: 12, us: 12, eu: 12, jp: 6, hk: 6 }
const ids = new Set()
const prefixCounts = { cn: 0, us: 0, eu: 0, jp: 0, hk: 0 }
for (const [index, item] of (data.cases ?? []).entries()) {
  const fields = ['id', 'naturalGeography', 'historicalLayers', 'focusCuts']
  for (const field of fields) {
    if (typeof item[field] !== 'string' || item[field].trim() === '') {
      failures.push(`case ${index}: empty ${field}`)
    }
  }
  if (ids.has(item.id)) failures.push(`duplicate id ${item.id}`)
  ids.add(item.id)
  const prefix = item.id?.split('-')[0]
  if (!(prefix in prefixCounts)) failures.push(`case ${index}: unsupported region ${prefix}`)
  else prefixCounts[prefix] += 1
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
    cases: data.cases.length,
    prefixCounts,
    file,
  }))
}
