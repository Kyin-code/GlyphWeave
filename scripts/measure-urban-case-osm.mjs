import { mkdir, writeFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'

const args = parseArgs(process.argv.slice(2))
const anchor = args.anchor
if (!anchor) {
  fail('usage: node scripts/measure-urban-case-osm.mjs --case-id <id> --anchor <name> [--output <file>]')
}

const caseId = args['case-id'] ?? slug(anchor)
const radiusM = Number(args.radius ?? Math.sqrt(1_000_000 / Math.PI))
const output = resolve(
  args.output
  ?? `assets/data/urban-core-measurements/${caseId}-1km2.json`,
)
if (!Number.isFinite(radiusM) || radiusM <= 0) {
  fail('radius must be a positive metre value')
}

const userAgent = 'GlyphWeave-research/1.0 (local urban evidence audit)'
const geocodeUrl = new URL('https://nominatim.openstreetmap.org/search')
geocodeUrl.searchParams.set('format', 'jsonv2')
geocodeUrl.searchParams.set('limit', '1')
geocodeUrl.searchParams.set('q', anchor)

const geocodeResponse = await fetch(geocodeUrl, {
  headers: { 'User-Agent': userAgent },
})
if (!geocodeResponse.ok) {
  fail(`Nominatim returned ${geocodeResponse.status}`)
}
const geocode = await geocodeResponse.json()
if (!Array.isArray(geocode) || geocode.length === 0) {
  fail(`no OSM anchor found for ${anchor}`)
}
const lat = Number(geocode[0].lat)
const lon = Number(geocode[0].lon)
if (!Number.isFinite(lat) || !Number.isFinite(lon)) {
  fail('Nominatim returned invalid coordinates')
}

const metresPerDegreeLat = 111_320
const latDelta = radiusM / metresPerDegreeLat
const lonDelta =
  radiusM / (metresPerDegreeLat * Math.cos(lat * Math.PI / 180))
const mapUrl = new URL('https://api.openstreetmap.org/api/0.6/map')
mapUrl.searchParams.set(
  'bbox',
  [lon - lonDelta, lat - latDelta, lon + lonDelta, lat + latDelta]
    .map((value) => value.toFixed(7))
    .join(','),
)
const mapResponse = await fetch(mapUrl, {
  headers: { 'User-Agent': userAgent },
  signal: AbortSignal.timeout(120_000),
})
if (!mapResponse.ok) {
  fail(`OSM map API returned ${mapResponse.status}: ${await mapResponse.text()}`)
}
const osmXml = await mapResponse.text()
const parsedOsm = parseOsmXml(osmXml)

const drivable = new Set([
  'motorway',
  'trunk',
  'primary',
  'secondary',
  'tertiary',
  'unclassified',
  'residential',
  'living_street',
  'service',
  'road',
])
const roadByClass = {}
const highwayLengthByClass = {}
let roadLengthM = 0
let drivableLengthM = 0
let buildingCount = 0
let buildingFootprintM2 = 0
const freshFoodKinds = {}

for (const way of parsedOsm.ways) {
  const tags = way.tags
  const points = way.refs
    .map((reference) => parsedOsm.nodes.get(reference))
    .filter((point) => point !== undefined)
  if (tags.highway) {
    const lengthM = geometryLengthInsideCircle(
      points,
      lat,
      lon,
      radiusM,
    )
    roadLengthM += lengthM
    highwayLengthByClass[tags.highway] =
      (highwayLengthByClass[tags.highway] ?? 0) + lengthM
    if (drivable.has(tags.highway)) {
      drivableLengthM += lengthM
      roadByClass[tags.highway] = (roadByClass[tags.highway] ?? 0) + lengthM
    }
  }

  if (tags.building && points.length >= 3) {
    const centroid = polygonCentroid(points)
    if (
      centroid === null
      || distanceM(lat, lon, centroid.lat, centroid.lon) > radiusM
    ) {
      continue
    }
    buildingCount += 1
    buildingFootprintM2 += projectedPolygonArea(points, lat)
  }

  const freshFoodKind = freshFoodKindFor(tags)
  if (freshFoodKind) {
    freshFoodKinds[freshFoodKind] = (freshFoodKinds[freshFoodKind] ?? 0) + 1
  }
}

for (const node of parsedOsm.nodesWithTags) {
  if (distanceM(lat, lon, node.lat, node.lon) > radiusM) {
    continue
  }
  const freshFoodKind = freshFoodKindFor(node.tags)
  if (freshFoodKind) {
    freshFoodKinds[freshFoodKind] = (freshFoodKinds[freshFoodKind] ?? 0) + 1
  }
}

for (const relation of parsedOsm.relations) {
  const freshFoodKind = freshFoodKindFor(relation.tags)
  if (freshFoodKind) {
    freshFoodKinds[freshFoodKind] = (freshFoodKinds[freshFoodKind] ?? 0) + 1
  }
}

const areaKm2 = Math.PI * radiusM * radiusM / 1_000_000
const result = {
  format: 'glyphweave.urban-core-osm-measurement',
  version: 1,
  caseId,
  retrievedAt: new Date().toISOString().slice(0, 10),
  osmSnapshotTimestamp: parsedOsm.timestamp,
  anchor: {
    query: anchor,
    displayName: geocode[0].display_name,
    osmType: geocode[0].osm_type,
    osmId: geocode[0].osm_id,
    lat,
    lon,
  },
  sample: {
    shape: 'circle',
    radiusM,
    areaKm2,
    rationale: 'One-square-kilometre circular sample centred on the named core anchor; not a whole-city average.',
  },
  observed: {
    roadCenterlineLengthM: round(roadLengthM),
    roadDensityKmPerKm2: round(roadLengthM / 1000 / areaKm2),
    drivableCenterlineLengthM: round(drivableLengthM),
    drivableDensityKmPerKm2: round(drivableLengthM / 1000 / areaKm2),
    roadLengthByClassM: roundRecord(highwayLengthByClass),
    buildingCountInsideCircleCentroid: buildingCount,
    estimatedBuildingFootprintM2: round(buildingFootprintM2),
    estimatedBuildingCoverageRatio: round(buildingFootprintM2 / (areaKm2 * 1_000_000)),
    freshFoodFacilityCounts: freshFoodKinds,
  },
  limitations: [
    'OSM is a current collaborative map snapshot, not a historical census.',
    'Building coverage uses centroid inclusion and projected polygon area; boundary clipping is approximate.',
    'Road density is centreline length, not lane capacity or right-of-way area.',
    'Facility counts do not prove capacity, catchment, load, quality or opening hours.',
    'The circular window is a measurement sample and must not be described as the whole city.',
  ],
  attribution: {
    source: 'OpenStreetMap contributors',
    license: 'Open Database License 1.0',
    copyright: 'https://www.openstreetmap.org/copyright',
  },
}

await mkdir(dirname(output), { recursive: true })
await writeFile(output, `${JSON.stringify(result, null, 2)}\n`, 'utf8')
console.log(JSON.stringify({ status: 'pass', output, ...result.observed }, null, 2))

function parseArgs(values) {
  const parsed = {}
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index]
    if (!value.startsWith('--')) {
      continue
    }
    const key = value.slice(2)
    const next = values[index + 1]
    if (next === undefined || next.startsWith('--')) {
      parsed[key] = true
    } else {
      parsed[key] = next
      index += 1
    }
  }
  return parsed
}

function slug(value) {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '')
}

function geometryLengthInsideCircle(points, centreLat, centreLon, radiusM) {
  let total = 0
  for (let index = 1; index < points.length; index += 1) {
    const from = points[index - 1]
    const to = points[index]
    const segmentM = distanceM(from.lat, from.lon, to.lat, to.lon)
    const steps = Math.max(1, Math.ceil(segmentM / 5))
    for (let step = 0; step < steps; step += 1) {
      const t = (step + 0.5) / steps
      const sampleLat = from.lat + (to.lat - from.lat) * t
      const sampleLon = from.lon + (to.lon - from.lon) * t
      if (distanceM(centreLat, centreLon, sampleLat, sampleLon) <= radiusM) {
        total += segmentM / steps
      }
    }
  }
  return total
}

function polygonCentroid(points) {
  if (points.length === 0) {
    return null
  }
  const last = points.at(-1)
  const closed = last.lat === points[0].lat && last.lon === points[0].lon
  const polygon = closed ? points.slice(0, -1) : points
  if (polygon.length === 0) {
    return null
  }
  return {
    lat: polygon.reduce((sum, point) => sum + point.lat, 0) / polygon.length,
    lon: polygon.reduce((sum, point) => sum + point.lon, 0) / polygon.length,
  }
}

function projectedPolygonArea(points, referenceLat) {
  if (points.length < 3) {
    return 0
  }
  const metresPerDegreeLat = 111_320
  const metresPerDegreeLon = metresPerDegreeLat * Math.cos(referenceLat * Math.PI / 180)
  let twiceArea = 0
  for (let index = 0; index < points.length; index += 1) {
    const current = points[index]
    const next = points[(index + 1) % points.length]
    const x1 = current.lon * metresPerDegreeLon
    const y1 = current.lat * metresPerDegreeLat
    const x2 = next.lon * metresPerDegreeLon
    const y2 = next.lat * metresPerDegreeLat
    twiceArea += x1 * y2 - x2 * y1
  }
  return Math.abs(twiceArea) / 2
}

function distanceM(lat1, lon1, lat2, lon2) {
  const earthRadiusM = 6_371_008.8
  const phi1 = lat1 * Math.PI / 180
  const phi2 = lat2 * Math.PI / 180
  const deltaPhi = (lat2 - lat1) * Math.PI / 180
  const deltaLambda = (lon2 - lon1) * Math.PI / 180
  const a = Math.sin(deltaPhi / 2) ** 2
    + Math.cos(phi1) * Math.cos(phi2) * Math.sin(deltaLambda / 2) ** 2
  return 2 * earthRadiusM * Math.asin(Math.sqrt(a))
}

function parseOsmXml(xml) {
  const root = regexAttributes(xml.match(/<osm\b([^>]*)>/)?.[1] ?? '')
  const nodes = new Map()
  const nodesWithTags = []
  for (const match of xml.matchAll(
    /<node\b([^>]*?)(?:\/>|>(.*?)<\/node>)/gs,
  )) {
    const attributes = regexAttributes(match[1])
    const id = attributes.id
    const latValue = Number(attributes.lat)
    const lonValue = Number(attributes.lon)
    if (
      !id
      || !Number.isFinite(latValue)
      || !Number.isFinite(lonValue)
    ) {
      continue
    }
    const point = { lat: latValue, lon: lonValue }
    nodes.set(id, point)
    const tags = parseTagBlock(match[2] ?? '')
    if (Object.keys(tags).length > 0) {
      nodesWithTags.push({ id, ...point, tags })
    }
  }

  const ways = []
  for (const match of xml.matchAll(/<way\b([^>]*)>(.*?)<\/way>/gs)) {
    const attributes = regexAttributes(match[1])
    const refs = [...match[2].matchAll(/<nd\b[^>]*\bref="([^"]+)"/g)]
      .map((reference) => reference[1])
    ways.push({
      id: attributes.id,
      refs,
      tags: parseTagBlock(match[2]),
    })
  }

  const relations = []
  for (const match of xml.matchAll(/<relation\b([^>]*)>(.*?)<\/relation>/gs)) {
    const attributes = regexAttributes(match[1])
    relations.push({
      id: attributes.id,
      tags: parseTagBlock(match[2]),
    })
  }

  return {
    timestamp: root.timestamp ?? null,
    nodes,
    nodesWithTags,
    ways,
    relations,
  }
}

function parseTagBlock(block) {
  const tags = {}
  for (const match of block.matchAll(/<tag\b([^>]*)\/>/g)) {
    const attributes = regexAttributes(match[1])
    if (attributes.k && attributes.v !== undefined) {
      tags[attributes.k] = attributes.v
    }
  }
  return tags
}

function regexAttributes(value) {
  const attributes = {}
  for (const match of value.matchAll(/([A-Za-z0-9:_-]+)="([^"]*)"/g)) {
    attributes[match[1]] = decodeXml(match[2])
  }
  return attributes
}

function decodeXml(value) {
  return value
    .replaceAll('&quot;', '"')
    .replaceAll('&apos;', "'")
    .replaceAll('&lt;', '<')
    .replaceAll('&gt;', '>')
    .replaceAll('&amp;', '&')
}

function freshFoodKindFor(tags) {
  if (tags.amenity === 'marketplace') {
    return 'marketplace'
  }
  if (tags.shop === 'supermarket') {
    return 'supermarket'
  }
  if (tags.shop === 'convenience') {
    return 'convenience'
  }
  if (tags.shop === 'greengrocer') {
    return 'greengrocer'
  }
  if (tags.shop === 'butcher') {
    return 'butcher'
  }
  if (tags.shop === 'seafood') {
    return 'seafood'
  }
  if (tags.shop === 'bakery') {
    return 'bakery'
  }
  return null
}

function round(value) {
  return Math.round(value * 1000) / 1000
}

function roundRecord(record) {
  return Object.fromEntries(
    Object.entries(record).map(([key, value]) => [key, round(value)]),
  )
}

function fail(message) {
  console.error(JSON.stringify({ status: 'fail', message }))
  process.exit(1)
}
