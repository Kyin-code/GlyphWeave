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
const roadGraph = new Map()
const buildingPoints = []
const freshFoodPoints = []
let roadLengthM = 0
let drivableLengthM = 0
let buildingCount = 0
let buildingFootprintM2 = 0
let buildingLevelsObservedCount = 0
let estimatedObservedFloorAreaM2 = 0
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
    if (tags.area !== 'yes' && !['construction', 'proposed'].includes(tags.highway)) {
      addGraphPath(roadGraph, way.refs, parsedOsm.nodes)
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
    buildingPoints.push({ lat: centroid.lat, lon: centroid.lon })
    const footprintM2 = projectedPolygonArea(points, lat)
    buildingFootprintM2 += footprintM2
    const levels = parsedBuildingLevels(tags)
    if (levels !== null) {
      buildingLevelsObservedCount += 1
      estimatedObservedFloorAreaM2 += footprintM2 * levels
    }
  }

  const freshFoodKind = freshFoodKindFor(tags)
  if (freshFoodKind) {
    freshFoodKinds[freshFoodKind] = (freshFoodKinds[freshFoodKind] ?? 0) + 1
    const centroid = polygonCentroid(points)
    if (
      centroid !== null
      && distanceM(lat, lon, centroid.lat, centroid.lon) <= radiusM
    ) {
      freshFoodPoints.push({ ...centroid, kind: freshFoodKind })
    }
  }
}

for (const node of parsedOsm.nodesWithTags) {
  if (distanceM(lat, lon, node.lat, node.lon) > radiusM) {
    continue
  }
  const freshFoodKind = freshFoodKindFor(node.tags)
  if (freshFoodKind) {
    freshFoodKinds[freshFoodKind] = (freshFoodKinds[freshFoodKind] ?? 0) + 1
    freshFoodPoints.push({
      lat: node.lat,
      lon: node.lon,
      kind: freshFoodKind,
    })
  }
}

for (const relation of parsedOsm.relations) {
  const freshFoodKind = freshFoodKindFor(relation.tags)
  if (freshFoodKind) {
    freshFoodKinds[freshFoodKind] = (freshFoodKinds[freshFoodKind] ?? 0) + 1
  }
}

const areaKm2 = Math.PI * radiusM * radiusM / 1_000_000
const freshFoodAccess = measureFreshFoodAccess({
  graph: roadGraph,
  nodes: parsedOsm.nodes,
  buildingPoints,
  freshFoodPoints,
  centre: { lat, lon },
  radiusM,
})
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
    buildingLevelsObservedCount,
    buildingLevelsObservedRatio: round(buildingLevelsObservedCount / Math.max(1, buildingCount)),
    estimatedObservedFloorAreaM2: round(estimatedObservedFloorAreaM2),
    observedFloorAreaRatioProxy: round(
      estimatedObservedFloorAreaM2 / (areaKm2 * 1_000_000),
    ),
    freshFoodFacilityCounts: freshFoodKinds,
    freshFoodNetworkAccess: freshFoodAccess,
  },
  limitations: [
    'OSM is a current collaborative map snapshot, not a historical census.',
    'Building coverage uses centroid inclusion and projected polygon area; boundary clipping is approximate.',
    'Road density is centreline length, not lane capacity or right-of-way area.',
    'Facility counts do not prove capacity, catchment, load, quality or opening hours.',
    'Fresh-food access is a walking-network proxy using OSM connectivity; it does not model queues, capacity, opening hours, transit or competition.',
    'Observed FAR uses only buildings with explicit OSM level tags and is therefore a lower-bound proxy.',
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

function parsedBuildingLevels(tags) {
  const raw =
    tags['building:levels']
    ?? tags['building:levels:aboveground']
    ?? null
  if (raw === null) {
    return null
  }
  const match = String(raw).trim().match(/^(\d+(?:\.\d+)?)/)
  if (!match) {
    return null
  }
  const levels = Number(match[1])
  return Number.isFinite(levels) && levels > 0 && levels <= 200
    ? levels
    : null
}

function addGraphPath(graph, refs, nodes) {
  for (let index = 1; index < refs.length; index += 1) {
    const fromId = refs[index - 1]
    const toId = refs[index]
    const from = nodes.get(fromId)
    const to = nodes.get(toId)
    if (!from || !to || fromId === toId) {
      continue
    }
    const lengthM = distanceM(from.lat, from.lon, to.lat, to.lon)
    const forward = graph.get(fromId) ?? new Map()
    const backward = graph.get(toId) ?? new Map()
    const previousForward = forward.get(toId)
    const previousBackward = backward.get(fromId)
    if (previousForward === undefined || lengthM < previousForward) {
      forward.set(toId, lengthM)
      graph.set(fromId, forward)
    }
    if (previousBackward === undefined || lengthM < previousBackward) {
      backward.set(fromId, lengthM)
      graph.set(toId, backward)
    }
  }
}

function measureFreshFoodAccess({
  graph,
  nodes,
  buildingPoints,
  freshFoodPoints,
  centre,
  radiusM,
}) {
  const marketLikeKinds = new Set([
    'marketplace',
    'supermarket',
    'greengrocer',
    'butcher',
    'seafood',
  ])
  const walkSpeedMps = 1.3
  const snapRadiusM = 120
  const accessFor = (facilities) => {
    const sources = []
    for (const facility of facilities) {
      const snapped = nearestGraphNode(
        facility,
        graph,
        nodes,
        centre,
        snapRadiusM,
      )
      if (snapped) {
        sources.push(snapped)
      }
    }
    if (sources.length === 0) {
      return {
        routedFacilityCount: 0,
        demandBuildingCount: buildingPoints.length,
        buildingsWithTime: 0,
        unreachableBuildingCount: 0,
        medianMinutes: null,
        p90Minutes: null,
        within5MinuteRatio: null,
        within10MinuteRatio: null,
        within15MinuteRatio: null,
      }
    }
    const distances = multiSourceDijkstra(graph, sources)
    const times = []
    let unreachable = 0
    for (const building of buildingPoints) {
      const snapped = nearestGraphNode(
        building,
        graph,
        nodes,
        centre,
        snapRadiusM,
      )
      if (!snapped) {
        unreachable += 1
        continue
      }
      const networkDistance = distances.get(snapped.nodeId)
      if (networkDistance === undefined || !Number.isFinite(networkDistance)) {
        unreachable += 1
        continue
      }
      times.push(
        (networkDistance + snapped.distanceM) / walkSpeedMps / 60,
      )
    }
    const ratioWithin = (minutes) => times.length === 0
      ? null
      : round(
        times.filter((value) => value <= minutes).length
        / Math.max(1, buildingPoints.length),
      )
    return {
      routedFacilityCount: sources.length,
      demandBuildingCount: buildingPoints.length,
      buildingsWithTime: times.length,
      unreachableBuildingCount: unreachable,
      medianMinutes: roundOptional(percentile(times, 0.5)),
      p90Minutes: roundOptional(percentile(times, 0.9)),
      within5MinuteRatio: ratioWithin(5),
      within10MinuteRatio: ratioWithin(10),
      within15MinuteRatio: ratioWithin(15),
    }
  }

  return {
    model: 'walking-network-proxy',
    walkSpeedMps,
    snapRadiusM,
    sampleRadiusM: radiusM,
    routingGraphNodeCount: graph.size,
    allMappedFreshFood: accessFor(freshFoodPoints),
    marketLikeFreshFood: accessFor(
      freshFoodPoints.filter((facility) => marketLikeKinds.has(facility.kind)),
    ),
    limitations: [
      'Equal building weight substitutes for household or occupancy weight.',
      'OSM network connectivity and facility completeness affect the result.',
      'Capacity, competition, opening hours, transit and quality are not modelled.',
    ],
  }
}

function nearestGraphNode(point, graph, nodes, centre, maxDistanceM) {
  let best = null
  for (const nodeId of graph.keys()) {
    const node = nodes.get(nodeId)
    if (!node) {
      continue
    }
    const distance = approximateDistanceM(point, node, centre.lat)
    if (distance > maxDistanceM) {
      continue
    }
    if (best === null || distance < best.distanceM) {
      best = { nodeId, distanceM: distance }
    }
  }
  return best
}

function multiSourceDijkstra(graph, sources) {
  const distances = new Map()
  const heap = []
  for (const source of sources) {
    const previous = distances.get(source.nodeId)
    if (previous === undefined || source.distanceM < previous) {
      distances.set(source.nodeId, source.distanceM)
      heapPush(heap, [source.distanceM, source.nodeId])
    }
  }
  while (heap.length > 0) {
    const [distance, nodeId] = heapPop(heap)
    if (distance !== distances.get(nodeId)) {
      continue
    }
    for (const [nextId, lengthM] of graph.get(nodeId) ?? []) {
      const candidate = distance + lengthM
      const previous = distances.get(nextId)
      if (previous === undefined || candidate < previous) {
        distances.set(nextId, candidate)
        heapPush(heap, [candidate, nextId])
      }
    }
  }
  return distances
}

function heapPush(heap, entry) {
  heap.push(entry)
  let index = heap.length - 1
  while (index > 0) {
    const parent = Math.floor((index - 1) / 2)
    if (heap[parent][0] <= heap[index][0]) {
      break
    }
    const parentEntry = heap[parent]
    heap[parent] = heap[index]
    heap[index] = parentEntry
    index = parent
  }
}

function heapPop(heap) {
  const first = heap[0]
  const last = heap.pop()
  if (heap.length === 0) {
    return first
  }
  heap[0] = last
  let index = 0
  while (true) {
    const left = index * 2 + 1
    const right = left + 1
    let smallest = index
    if (left < heap.length && heap[left][0] < heap[smallest][0]) {
      smallest = left
    }
    if (right < heap.length && heap[right][0] < heap[smallest][0]) {
      smallest = right
    }
    if (smallest === index) {
      break
    }
    const smallestEntry = heap[smallest]
    heap[smallest] = heap[index]
    heap[index] = smallestEntry
    index = smallest
  }
  return first
}

function approximateDistanceM(from, to, referenceLat) {
  const metresPerDegreeLat = 111_320
  const metresPerDegreeLon =
    metresPerDegreeLat * Math.cos(referenceLat * Math.PI / 180)
  const dy = (to.lat - from.lat) * metresPerDegreeLat
  const dx = (to.lon - from.lon) * metresPerDegreeLon
  return Math.hypot(dx, dy)
}

function percentile(values, percentileValue) {
  if (values.length === 0) {
    return null
  }
  const sorted = [...values].sort((a, b) => a - b)
  const index = (sorted.length - 1) * percentileValue
  const lower = Math.floor(index)
  const upper = Math.ceil(index)
  if (lower === upper) {
    return sorted[lower]
  }
  return sorted[lower] + (sorted[upper] - sorted[lower]) * (index - lower)
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

function roundOptional(value) {
  return value === null ? null : round(value)
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
