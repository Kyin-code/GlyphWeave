// Node-side tests for the engineering preview's render data conversion.
//
// These import the exact module the browser imports, so the numbers checked here are the
// numbers the viewer uploads. Three groups:
//   1. the transforms on a synthetic projection;
//   2. the injected faults the packet requires: a wrong coordinate, a missing part, a
//      swapped binding, a forged display convention, a duplicated part id;
//   3. when real exported render models are named on the command line, the same parity
//      assertion runs against the production export.
//
// Run: node preview-model.test.mjs [--model path/to/render-model.json ...]
import fs from 'node:fs'
import {
  DISPLAY_CONVENTION,
  PreviewRejection,
  assertParity,
  buildDisplayModel,
  cameraPose,
  cornersToFloat32,
  displayBounds,
  edgeVisibleForCapability,
  terrainMesh,
  trianglesToIndices,
  worldToDisplay,
} from './preview-model.js'

const failures = []
const checks = []
function check(name, condition, detail = '') {
  checks.push(name)
  if (!condition) failures.push(`${name}${detail ? `: ${detail}` : ''}`)
}
function throws(name, fn, expected) {
  checks.push(name)
  try {
    fn()
    failures.push(`${name}: expected a rejection, nothing was thrown`)
  } catch (error) {
    if (!(error instanceof PreviewRejection)) failures.push(`${name}: wrong error type ${error}`)
    else if (expected && !error.message.includes(expected)) {
      failures.push(`${name}: message ${JSON.stringify(error.message)} lacks ${JSON.stringify(expected)}`)
    }
  }
}

const triangles = [
  [4, 5, 6], [4, 6, 7], [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5],
  [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7], [0, 3, 2], [0, 2, 1],
]

function syntheticModel() {
  const corners = [
    [0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1],
    [0, 0.2, 0], [1, 0.2, 0], [1, 0.2, 1], [0, 0.2, 1],
  ]
  return {
    schema: 'glyphweave.engineering-render-model/1',
    version: 1,
    sceneId: 'synthetic',
    units: 'lengths_m;levels_mm',
    coordinateFrame: 'world metres',
    displayConvention: {
      worldToDisplay: 'identity',
      appliesTo: 'both',
      f32ToleranceM: DISPLAY_CONVENTION.f32ToleranceM,
      note: '',
    },
    parts: [
      {
        partId: 'deck::tread0',
        structureId: 'deck',
        role: 'tread',
        levelId: 'deck:deck',
        runIndex: 0,
        index: 0,
        topMm: 200,
        bottomMm: 0,
        cornersM: corners,
        triangles,
      },
      {
        partId: 'deck::riser0',
        structureId: 'deck',
        role: 'riser',
        levelId: 'deck:deck',
        runIndex: 0,
        index: 1,
        topMm: 200,
        bottomMm: 60,
        cornersM: corners,
        triangles,
      },
    ],
    nodes: [
      { nodeId: 'a', kind: 'port', positionM: [0, 0, 0], surfaceId: 'ground', levelId: 'ground', roadId: null, capabilities: ['pedestrian'] },
      { nodeId: 'b', kind: 'road', positionM: [3, 0.2, 0], surfaceId: 'ground', levelId: 'ground', roadId: 'BC', capabilities: ['pedestrian', 'wheelchair'] },
    ],
    edges: [
      { edgeId: 'walk', fromNode: 'a', toNode: 'b', kind: 'ground', bidirectional: true, capabilities: ['pedestrian'], structureId: null, polylineM: [[0, 0, 0], [3, 0.2, 0]], horizontalLengthM: 3, pathLength3dM: 3.006, riseM: 0.2 },
    ],
    route: {
      status: 'found',
      capability: 'pedestrian',
      fromPortId: 'a',
      toRoadId: 'BC',
      pathNodeIds: ['a', 'b'],
      pathEdgeIds: ['walk'],
      polylineM: [[0, 0, 0], [3, 0.2, 0]],
      horizontalLengthM: 3,
      pathLength3dM: 3.006,
      cumulativeRiseM: 0.2,
      cumulativeDescentM: 0,
      limits: { maxLength3dM: 10, maxRiseM: 1 },
      query: { fromPortId: 'a', toRoadId: 'BC', capability: 'pedestrian', maxLength3dM: 10, maxRiseM: 1, roadIdBinding: 'declared' },
    },
    terrain: {
      engineered: { width: 2, depth: 2, originX: 0, originZ: 0, heightsM: [0, 0, 0, 0], contentHash: 'aa' },
      natural: { width: 2, depth: 2, originX: 0, originZ: 0, heightsM: [0, 0.1, 0.2, 0.3], contentHash: 'bb' },
      baseline: { width: 2, depth: 2, originX: 0, originZ: 0, heightsM: [0, 0, 0, 0], contentHash: 'cc' },
    },
    obstacles: [{ id: 'wall', x0: 1, z0: 0, x1: 2, z1: 1, bottomM: 0, topM: 2 }],
    water: [{ id: 'pond', x0: 0, z0: 0, x1: 1, z1: 1, bottomM: null, topM: null }],
    declaredObstacles: true,
    declaredWater: true,
    counts: { parts: 2, structures: 1, nodes: 2, edges: 1, routePoints: 2, terrainCells: 4, obstacles: 1, water: 1, triangles: 24 },
    productionMeshPoints: 16,
    productionMeshTriangles: 24,
  }
}

// --- 1. transforms ---------------------------------------------------------
check('identity display', JSON.stringify(worldToDisplay([1.5, -2.25, 3])) === JSON.stringify([1.5, -2.25, 3]))
throws('non-finite coordinate is refused', () => worldToDisplay([1, Number.NaN, 3]), 'not a finite number')
throws('short coordinate is refused', () => worldToDisplay([1, 2]), 'three-number')

const positions = cornersToFloat32([[0, 0, 0], [1, 2, 3], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0]], 'p')
check('corner upload is exact for representable metres', positions[0] === 0 && positions[4] === 2 && positions[5] === 3)
check('corner count', positions.length === 24)
throws(
  'the declared f32 bound is enforced',
  () => cornersToFloat32(Array.from({ length: 8 }, () => [12345678.9, 0, 0]), 'wide'),
  'f32 upload error',
)
const indices = trianglesToIndices(triangles)
check('index buffer', indices.length === 36 && indices[0] === 4 && indices[35] === 1)
throws('a triangle table of the wrong length is refused', () => trianglesToIndices(triangles.slice(0, 11)), 'expected 12')

const mesh = terrainMesh({ width: 2, depth: 2, originX: 5, originZ: 7, heightsM: [0, 1, 2, 3], contentHash: 'x' }, 'engineered')
check(
  'terrain vertices',
  mesh.positions.length === 12 &&
    mesh.positions[3] === 6 && mesh.positions[4] === 1 && mesh.positions[5] === 7 &&
    mesh.positions[6] === 5 && mesh.positions[7] === 2 && mesh.positions[8] === 8,
  JSON.stringify([...mesh.positions]),
)
check('terrain indices', mesh.indices.length === 6 && mesh.rows === 2 && mesh.columns === 2)
throws(
  'a surface whose sample count disagrees with its size is refused',
  () => terrainMesh({ width: 2, depth: 2, originX: 0, originZ: 0, heightsM: [0, 1, 2], contentHash: 'x' }, 'engineered'),
  'declares 4 samples',
)

// --- 2. the derived display model and its parity ---------------------------
const model = syntheticModel()
const display = buildDisplayModel(model)
check('display parts', display.parts.length === 2)
check('display triangles', display.counts.triangles === 24)
check('display ports', display.nodes.length === 2 && display.nodes[1].roadId === 'BC')
check('display route', display.route.pointCount === 2 && display.route.status === 'found')
check('display terrain modes', Object.keys(display.terrain).length === 3)
check('display terrain hash is carried', display.terrain.natural.contentHash === 'bb')
check('display obstacle box', display.obstacles.length === 1 && display.water.length === 1)
check('declared-empty flags survive', display.declaredObstacles === true && display.declaredWater === true)
check('parity on a faithful model', assertParity(model, display).ok, JSON.stringify(assertParity(model, display).findings.slice(0, 2)))

// injected wrong coordinate
const wrongCoordinate = { ...display, parts: display.parts.map((part, index) => (index === 0 ? { ...part, positions: Float32Array.from([...part.positions].map((value, at) => (at === 4 ? value + 0.5 : value))) } : part)) }
const wrongReport = assertParity(model, wrongCoordinate)
check('a wrong coordinate fails parity', !wrongReport.ok && wrongReport.findings.some((f) => f.includes('axis 1')), JSON.stringify(wrongReport.findings.slice(0, 2)))

// injected missing part
const missingPart = { ...display, parts: display.parts.slice(0, 1), counts: { ...display.counts, parts: 1 } }
const missingReport = assertParity(model, missingPart)
check('a missing part fails parity', !missingReport.ok && missingReport.findings.some((f) => f.includes('missing from the display model')), JSON.stringify(missingReport.findings.slice(0, 2)))

// injected swapped binding
const swapped = { ...display, parts: display.parts.map((part, index) => (index === 0 ? { ...part, levelId: 'forged:level', role: 'landing' } : part)) }
const swappedReport = assertParity(model, swapped)
check('a swapped level/role fails parity', !swappedReport.ok && swappedReport.findings.some((f) => f.includes('level')) && swappedReport.findings.some((f) => f.includes('role')))

// injected shortened route
const shortened = { ...display, route: { ...display.route, pointCount: 1, positions: display.route.positions.slice(0, 3) } }
check('a shortened route fails parity', !assertParity(model, shortened).ok)

// injected forged terrain hash
const forgedTerrain = { ...display, terrain: { ...display.terrain, engineered: { ...display.terrain.engineered, contentHash: 'forged' } } }
check('a forged surface hash fails parity', !assertParity(model, forgedTerrain).ok)

// forged display convention and duplicate ids are refused at construction
const forgedConvention = { ...model, displayConvention: { ...model.displayConvention, worldToDisplay: 'z-up' } }
throws('a forged display convention is refused', () => buildDisplayModel(forgedConvention), 'not identity')
const duplicated = { ...model, parts: [model.parts[0], { ...model.parts[1], partId: model.parts[0].partId }] }
throws('a duplicated part id is refused', () => buildDisplayModel(duplicated), 'duplicate part id')

// --- capability display must not over-reach --------------------------------
check('capability filter reads saved declarations only', edgeVisibleForCapability(model.edges[0], 'pedestrian') === true && edgeVisibleForCapability(model.edges[0], 'vehicle') === false)
check('no filter shows every edge', edgeVisibleForCapability(model.edges[0], null) === true)
check('the saved route status is untouched by filtering', display.route.status === model.route.status && display.route.query.capability === 'pedestrian')

// camera and bounds are deterministic and do not scale the model
const bounds = displayBounds(display)
check('bounds cover the saved solids', bounds.size[0] >= 1 && bounds.size[1] >= 0.2)
check('plan camera looks down', cameraPose('plan', bounds).position[1] > bounds.center[1])
check('side camera is level with the model', Math.abs(cameraPose('side', bounds).position[1] - bounds.center[1]) < bounds.size[1] + 1)

// --- 3. the real exported projections -------------------------------------
const modelArgs = []
for (let index = 2; index < process.argv.length; index += 1) {
  if (process.argv[index] === '--model') modelArgs.push(process.argv[index + 1])
}
for (const file of modelArgs) {
  const real = JSON.parse(fs.readFileSync(file, 'utf8'))
  const realDisplay = buildDisplayModel(real)
  const report = assertParity(real, realDisplay)
  check(`real export ${file}`, report.ok, JSON.stringify(report.findings.slice(0, 3)))
  check(`real export ${file} carries saved geometry`, realDisplay.counts.parts === (real.counts?.parts ?? -1), `${realDisplay.counts.parts} against ${real.counts?.parts}`)
  check(
    `real export ${file} keeps the route line point for point`,
    realDisplay.route.pointCount === (real.route?.polylineM?.length ?? 0),
  )
  realDisplay.parts.forEach((part) => {
    if (part.positions.length !== 24) failures.push(`real export ${file}: part ${part.partId} has ${part.positions.length} position values`)
  })
}

const verdict = {
  status: failures.length === 0 ? 'PASS' : 'FAIL',
  checks: checks.length,
  failures,
  realModels: modelArgs,
}
process.stdout.write(JSON.stringify(verdict) + '\n')
process.exit(failures.length === 0 ? 0 : 1)
