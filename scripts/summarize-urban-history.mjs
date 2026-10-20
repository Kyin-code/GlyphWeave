import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'

const input = resolve(
  process.argv[2] ?? 'assets/data/urban-case-priors-48.json',
)
const output = resolve(
  process.argv[3] ?? 'assets/data/urban-history-patterns.json',
)
const data = JSON.parse(await readFile(input, 'utf8'))
if (!Array.isArray(data.cases) || data.cases.length !== 48) {
  throw new Error(`expected 48 cases, got ${data.cases?.length ?? 0}`)
}

const categories = {
  waterPort: {
    pattern: /port|river|canal|water|harbou?r|pier|fishing|lake|delta|sea|shore|tidal/i,
    description: 'river, lake, coast, port or water-management origin',
  },
  railTransit: {
    pattern: /rail|station|metro|tram|streetcar|transit|subway/i,
    description: 'rail, station, tram or metro-driven growth',
  },
  industry: {
    pattern: /industrial|factory|warehouse|dock|manufactur|mining/i,
    description: 'industrial, warehouse, dock or mining layer',
  },
  reclamationEngineering: {
    pattern: /reclaim|land.?fill|drain|levee|canaliz|water management|avulsion|flood/i,
    description: 'reclamation, drainage, embankment or hydrological engineering',
  },
  renewalProtection: {
    pattern: /renew|redevelop|preserv|conserv|heritage|urban village|demolition|infill|post-industrial|post-earthquake|reconstruction|tech-era/i,
    description: 'renewal, preservation, reconstruction or urban-village treatment',
  },
  plannedExpansion: {
    pattern: /plan|expansion|growth|new town|new city|suburban|sprawl|ring road|polycentric/i,
    description: 'planned expansion, new town, suburban growth or polycentric formation',
  },
  capitalCivic: {
    pattern: /imperial|capital|roman|prussian|treaty|colonial|government|axis/i,
    description: 'imperial, capital, colonial or civic-institutional origin',
  },
  tourism: {
    pattern: /tourism|recreation|casino|entertainment|luxury waterfront/i,
    description: 'tourism, recreation, casino or entertainment economy',
  },
  agriculturePasture: {
    pattern: /farm|irrigat|ranch|garden-city|market village|agricultural/i,
    description: 'agriculture, irrigation, pasture or market-village origin',
  },
}

const regions = [...new Set(data.cases.map((item) => item.region))].sort()
const cases = data.cases.map((item) => {
  const stages = item.history.map((stage) => {
    const text = `${stage.era} ${stage.change}`
    return {
      order: stage.order,
      era: stage.era,
      change: stage.change,
      categories: Object.entries(categories)
        .filter(([, definition]) => definition.pattern.test(text))
        .map(([id]) => id),
    }
  })
  return {
    caseId: item.id,
    region: item.region,
    focusFeature: item.focusFeature,
    stages,
    allCategories: [...new Set(
      stages.flatMap((stage) => stage.categories),
    )].sort(),
  }
})

const categorySummary = Object.fromEntries(
  Object.entries(categories).map(([id, definition]) => {
    const matched = cases.filter((item) => item.allCategories.includes(id))
    return [id, {
      description: definition.description,
      caseCount: matched.length,
      byStage: [0, 1, 2].map(
        (stageIndex) => cases.filter(
          (item) => item.stages[stageIndex]?.categories.includes(id),
        ).length,
      ),
      byRegion: Object.fromEntries(
        regions.map((region) => [
          region,
          matched.filter((item) => item.region === region).length,
        ]),
      ),
    }]
  }),
)

const result = {
  format: 'glyphweave.urban-history-patterns',
  version: 1,
  generatedAt: new Date().toISOString().slice(0, 10),
  input,
  method: 'keyword-assisted-classification-over-prior-history',
  warning: 'These counts summarize existing prior text. They are not yet a replacement for case-specific historical source verification.',
  categorySummary,
  cases,
}

await mkdir(dirname(output), { recursive: true })
await writeFile(output, `${JSON.stringify(result, null, 2)}\n`, 'utf8')
console.log(JSON.stringify({
  status: 'pass',
  output,
  cases: cases.length,
  categorySummary,
}, null, 2))
