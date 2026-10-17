// Pure display transforms for the engineering preview.
//
// No THREE import, no DOM: the browser viewer and the Node tests import exactly the same
// functions, so the numbers the browser uploads are the numbers the tests check.
//
// The one declared convention: display metres are world metres on the contract frame
// (X east, Y up, Z north). Nothing is flattened, dropped to the ground or raised, and the
// route and the solids are never scaled apart. Uploading an f64 world coordinate to an f32
// GPU attribute is the only difference, and its bound is declared rather than hidden.
//
// The render model is the production bundle re-keyed, and its JSON keys are camelCase
// because the Rust side serialises with `rename_all = "camelCase"`. Every read below uses
// that spelling; a missing key is a rejection, never a silent `undefined`.

export const DISPLAY_CONVENTION = Object.freeze({
  worldToDisplay: 'identity',
  f32ToleranceM: 1e-5,
})

export const TERRAIN_MODES = Object.freeze(['engineered', 'natural', 'baseline'])

export const ROLE_MATERIAL = Object.freeze({
  tread: 'tread',
  riser: 'riser',
  landing: 'landing',
  support_block: 'support',
  support_column: 'support',
})

export class PreviewRejection extends Error {
  constructor(message) {
    super(message)
    this.name = 'PreviewRejection'
  }
}

function finite(value, what) {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new PreviewRejection(`${what} is not a finite number: ${String(value)}`)
  }
  return value
}

function required(object, key, what) {
  if (!object || object[key] === undefined || object[key] === null) {
    throw new PreviewRejection(`${what} is missing its ${key}`)
  }
  return object[key]
}

function numberArray(values, what) {
  if (!Array.isArray(values) || values.length < 3) {
    throw new PreviewRejection(`${what} is not a three-number coordinate: ${JSON.stringify(values)}`)
  }
  return [finite(values[0], `${what}.x`), finite(values[1], `${what}.y`), finite(values[2], `${what}.z`)]
}

/** World metres -> display metres. Throws on a non-finite or malformed coordinate. */
export function worldToDisplay(point) {
  return numberArray(point, 'world coordinate')
}

/** The f32 attribute the viewer uploads, with the declared error bound enforced. */
export function cornersToFloat32(cornersM, partId) {
  if (!Array.isArray(cornersM) || cornersM.length !== 8) {
    throw new PreviewRejection(`part ${partId}: ${cornersM?.length} corners, expected 8`)
  }
  const positions = new Float32Array(cornersM.length * 3)
  let maxError = 0
  cornersM.forEach((corner, index) => {
    const display = worldToDisplay(corner)
    for (let axis = 0; axis < 3; axis += 1) {
      positions[index * 3 + axis] = display[axis]
      maxError = Math.max(maxError, Math.abs(positions[index * 3 + axis] - display[axis]))
    }
  })
  if (maxError > DISPLAY_CONVENTION.f32ToleranceM) {
    throw new PreviewRejection(
      `part ${partId}: the f32 upload error ${maxError} exceeds the declared ${DISPLAY_CONVENTION.f32ToleranceM} m`,
    )
  }
  return positions
}

export function trianglesToIndices(triangles) {
  if (!Array.isArray(triangles) || triangles.length !== 12) {
    throw new PreviewRejection(`${triangles?.length} triangles, expected 12 for a box`)
  }
  const indices = new Uint16Array(triangles.length * 3)
  triangles.forEach((triangle, index) => {
    if (!Array.isArray(triangle) || triangle.length !== 3) {
      throw new PreviewRejection(`a triangle needs three corners: ${JSON.stringify(triangle)}`)
    }
    indices[index * 3 + 0] = triangle[0]
    indices[index * 3 + 1] = triangle[1]
    indices[index * 3 + 2] = triangle[2]
  })
  return indices
}

/** Display samples in metres for one saved surface, as a flat row-major array. */
export function surfaceHeightsM(surface, mode) {
  const heights = required(surface, 'heightsM', `terrain ${mode}`)
  if (!Array.isArray(heights)) {
    throw new PreviewRejection(`terrain ${mode}: heightsM is not an array`)
  }
  const width = finite(required(surface, 'width', `terrain ${mode}`), `terrain ${mode}.width`)
  const depth = finite(required(surface, 'depth', `terrain ${mode}`), `terrain ${mode}.depth`)
  const expected = width * depth
  if (heights.length !== expected) {
    throw new PreviewRejection(
      `terrain ${mode}: ${width}x${depth} declares ${expected} samples but carries ${heights.length}`,
    )
  }
  return heights.map((value, index) => finite(value, `terrain ${mode} sample ${index}`))
}

/**
 * The display mesh for one saved surface: vertices at
 * (originX + ix, heightMetres, originZ + iz) and two triangles per cell. This is a
 * rendering of the saved samples, not a terrain synthesis and not a carve.
 */
export function terrainMesh(surface, mode) {
  const heights = surfaceHeightsM(surface, mode)
  const width = surface.width
  const depth = surface.depth
  const originX = finite(surface.originX, `terrain ${mode}.originX`)
  const originZ = finite(surface.originZ, `terrain ${mode}.originZ`)
  const positions = new Float32Array(width * depth * 3)
  for (let iz = 0; iz < depth; iz += 1) {
    for (let ix = 0; ix < width; ix += 1) {
      const at = (iz * width + ix) * 3
      positions[at + 0] = originX + ix
      positions[at + 1] = heights[iz * width + ix]
      positions[at + 2] = originZ + iz
    }
  }
  const quads = Math.max(0, (width - 1) * (depth - 1))
  const indices = new Uint32Array(quads * 6)
  let cursor = 0
  for (let iz = 0; iz < depth - 1; iz += 1) {
    for (let ix = 0; ix < width - 1; ix += 1) {
      const a = iz * width + ix
      const b = a + 1
      const c = a + width
      const d = c + 1
      indices[cursor++] = a
      indices[cursor++] = c
      indices[cursor++] = b
      indices[cursor++] = b
      indices[cursor++] = c
      indices[cursor++] = d
    }
  }
  return { positions, indices, rows: depth, columns: width }
}

/** The whole derived display model: what the viewer actually uploads. */
export function buildDisplayModel(renderModel) {
  required(renderModel, 'parts', 'the render model')
  const convention = required(renderModel, 'displayConvention', 'the render model')
  if (convention.worldToDisplay !== DISPLAY_CONVENTION.worldToDisplay) {
    throw new PreviewRejection(
      `the render model declares display convention ${convention.worldToDisplay}, not identity`,
    )
  }
  const parts = []
  const seenPartIds = new Set()
  for (const part of renderModel.parts) {
    const partId = required(part, 'partId', 'a part')
    if (seenPartIds.has(partId)) throw new PreviewRejection(`duplicate part id ${partId}`)
    seenPartIds.add(partId)
    parts.push({
      partId,
      structureId: required(part, 'structureId', `part ${partId}`),
      role: required(part, 'role', `part ${partId}`),
      levelId: required(part, 'levelId', `part ${partId}`),
      runIndex: part.runIndex ?? 0,
      index: part.index ?? 0,
      topMm: part.topMm ?? null,
      bottomMm: part.bottomMm ?? null,
      material: ROLE_MATERIAL[part.role] ?? 'other',
      positions: cornersToFloat32(required(part, 'cornersM', `part ${partId}`), partId),
      indices: trianglesToIndices(required(part, 'triangles', `part ${partId}`)),
      cornersM: part.cornersM,
      triangleCount: part.triangles.length,
    })
  }
  const nodes = (renderModel.nodes ?? []).map((node) => ({
    nodeId: required(node, 'nodeId', 'a port'),
    kind: node.kind ?? 'port',
    positionM: numberArray(required(node, 'positionM', `port ${node.nodeId}`), `port ${node.nodeId}`),
    surfaceId: node.surfaceId ?? '',
    levelId: node.levelId ?? '',
    roadId: node.roadId ?? null,
    capabilities: [...(node.capabilities ?? [])],
  }))
  const edges = (renderModel.edges ?? []).map((edge) => {
    const polyline = required(edge, 'polylineM', `edge ${edge.edgeId}`)
    return {
      edgeId: required(edge, 'edgeId', 'an edge'),
      fromNode: edge.fromNode ?? '',
      toNode: edge.toNode ?? '',
      kind: edge.kind ?? '',
      bidirectional: !!edge.bidirectional,
      capabilities: [...(edge.capabilities ?? [])],
      structureId: edge.structureId ?? null,
      positions: Float32Array.from(polyline.flatMap((point, index) => worldToDisplay(point, index))),
      pointCount: polyline.length,
    }
  })
  const routeSource = required(renderModel, 'route', 'the render model')
  const routePolyline = routeSource.polylineM ?? []
  const route = {
    status: routeSource.status,
    capability: routeSource.capability,
    fromPortId: routeSource.fromPortId,
    toRoadId: routeSource.toRoadId,
    pathNodeIds: [...(routeSource.pathNodeIds ?? [])],
    pathEdgeIds: [...(routeSource.pathEdgeIds ?? [])],
    positions: Float32Array.from(routePolyline.flatMap((point, index) => worldToDisplay(point, index))),
    pointCount: routePolyline.length,
    horizontalLengthM: routeSource.horizontalLengthM,
    pathLength3dM: routeSource.pathLength3dM,
    cumulativeRiseM: routeSource.cumulativeRiseM,
    cumulativeDescentM: routeSource.cumulativeDescentM,
    limits: { ...routeSource.limits },
    query: { ...routeSource.query },
  }
  const terrainSource = required(renderModel, 'terrain', 'the render model')
  const terrain = {}
  for (const mode of TERRAIN_MODES) {
    const surface = required(terrainSource, mode, 'the render model terrain')
    const mesh = terrainMesh(surface, mode)
    terrain[mode] = {
      mode,
      contentHash: surface.contentHash,
      rows: mesh.rows,
      columns: mesh.columns,
      cells: surface.width * surface.depth,
      positions: mesh.positions,
      indices: mesh.indices,
      heightsM: surfaceHeightsM(surface, mode),
    }
  }
  const boxes = (list, what) =>
    (list ?? []).map((entry) => ({
      id: required(entry, 'id', what),
      corners: [
        [entry.x0, entry.bottomM ?? 0, entry.z0],
        [entry.x1, entry.bottomM ?? 0, entry.z0],
        [entry.x1, entry.topM ?? 0, entry.z1],
        [entry.x0, entry.topM ?? 0, entry.z1],
      ].map((point) => worldToDisplay(point)),
      bottomM: entry.bottomM ?? null,
      topM: entry.topM ?? null,
    }))
  return {
    sceneId: renderModel.sceneId,
    units: renderModel.units,
    parts,
    nodes,
    edges,
    route,
    terrain,
    obstacles: boxes(renderModel.obstacles, 'an obstacle'),
    water: boxes(renderModel.water, 'a water rect'),
    declaredObstacles: renderModel.declaredObstacles === true,
    declaredWater: renderModel.declaredWater === true,
    counts: {
      parts: parts.length,
      nodes: nodes.length,
      edges: edges.length,
      routePoints: route.pointCount,
      triangles: parts.reduce((total, part) => total + part.triangleCount, 0),
      terrainCells: terrain.engineered.cells,
      obstacles: (renderModel.obstacles ?? []).length,
      water: (renderModel.water ?? []).length,
      supports: parts.filter((part) => part.material === 'support').length,
      routeStatus: route.status,
    },
  }
}

/** Visibility is a display filter over saved capability declarations only. */
export function edgeVisibleForCapability(edge, capability) {
  if (!capability) return true
  return edge.capabilities.includes(capability)
}

/**
 * The parity assertion: the derived display model must reproduce the saved projection's
 * geometry, identities and counts. A wrong coordinate, a missing part, a swapped role or a
 * shortened route is a finding, and the caller refuses to draw rather than falling back to
 * an empty scene.
 */
export function assertParity(renderModel, display) {
  const findings = []
  const tolerance = DISPLAY_CONVENTION.f32ToleranceM
  if (!display) return { ok: false, findings: ['the display model was not built'] }
  const savedParts = renderModel.parts ?? []
  if (display.parts.length !== savedParts.length) {
    findings.push(`parts: ${display.parts.length} displayed against ${savedParts.length} saved`)
  }
  for (const part of display.parts) {
    const saved = savedParts.find((candidate) => candidate.partId === part.partId)
    if (!saved) {
      findings.push(`part ${part.partId} is not in the saved projection`)
      continue
    }
    if (part.role !== saved.role) findings.push(`part ${part.partId}: role ${part.role} against ${saved.role}`)
    if (part.levelId !== saved.levelId) findings.push(`part ${part.partId}: level ${part.levelId} against ${saved.levelId}`)
    if (part.structureId !== saved.structureId) {
      findings.push(`part ${part.partId}: structure ${part.structureId} against ${saved.structureId}`)
    }
    if (part.topMm !== saved.topMm || part.bottomMm !== saved.bottomMm) {
      findings.push(`part ${part.partId}: the saved millimetre levels are not the displayed ones`)
    }
    saved.cornersM.forEach((corner, index) => {
      for (let axis = 0; axis < 3; axis += 1) {
        const uploaded = part.positions[index * 3 + axis]
        if (Math.abs(uploaded - corner[axis]) > tolerance) {
          findings.push(
            `part ${part.partId} corner ${index} axis ${axis}: displayed ${uploaded} against saved ${corner[axis]}`,
          )
        }
      }
    })
  }
  for (const saved of savedParts) {
    if (!display.parts.some((part) => part.partId === saved.partId)) {
      findings.push(`saved part ${saved.partId} is missing from the display model`)
    }
  }
  const savedNodes = renderModel.nodes ?? []
  if (display.nodes.length !== savedNodes.length) {
    findings.push(`ports: ${display.nodes.length} displayed against ${savedNodes.length} saved`)
  }
  savedNodes.forEach((saved, index) => {
    const shown = display.nodes[index]
    if (!shown || shown.nodeId !== saved.nodeId) {
      findings.push(`port ${index}: ${shown?.nodeId} against ${saved.nodeId}`)
      return
    }
    saved.positionM.forEach((value, axis) => {
      if (Math.abs(shown.positionM[axis] - value) > tolerance) {
        findings.push(`port ${saved.nodeId} axis ${axis}: displayed ${shown.positionM[axis]} against saved ${value}`)
      }
    })
  })
  const savedRoute = renderModel.route?.polylineM ?? []
  if (display.route.pointCount !== savedRoute.length) {
    findings.push(`route: ${display.route.pointCount} displayed against ${savedRoute.length} saved`)
  }
  savedRoute.forEach((point, index) => {
    for (let axis = 0; axis < 3; axis += 1) {
      const uploaded = display.route.positions[index * 3 + axis]
      if (Math.abs(uploaded - point[axis]) > tolerance) {
        findings.push(`route point ${index} axis ${axis}: displayed ${uploaded} against saved ${point[axis]}`)
      }
    }
  })
  for (const mode of TERRAIN_MODES) {
    const saved = renderModel.terrain?.[mode]
    const shown = display.terrain[mode]
    if (!saved) {
      findings.push(`terrain ${mode} is not in the saved projection`)
      continue
    }
    if (!shown) {
      findings.push(`terrain ${mode} is not in the display model`)
      continue
    }
    if (shown.cells !== saved.width * saved.depth) {
      findings.push(`terrain ${mode}: ${shown.cells} cells against ${saved.width * saved.depth}`)
    }
    if (shown.contentHash !== saved.contentHash) {
      findings.push(`terrain ${mode}: displayed hash ${shown.contentHash} against saved ${saved.contentHash}`)
    }
    const samples = saved.heightsM ?? []
    const probe = Math.min(8, samples.length)
    for (let index = 0; index < probe; index += 1) {
      const at = Math.floor((index * (samples.length - 1)) / Math.max(1, probe - 1))
      if (Math.abs(shown.heightsM[at] - samples[at]) > tolerance) {
        findings.push(`terrain ${mode} sample ${at}: displayed ${shown.heightsM[at]} against saved ${samples[at]}`)
      }
    }
  }
  if (display.counts.triangles !== (renderModel.counts?.triangles ?? -1)) {
    findings.push(`triangles: ${display.counts.triangles} against ${renderModel.counts?.triangles}`)
  }
  return { ok: findings.length === 0, findings }
}

function boundsOf(arrays) {
  const min = [Infinity, Infinity, Infinity]
  const max = [-Infinity, -Infinity, -Infinity]
  for (const values of arrays) {
    for (let index = 0; index + 2 < values.length; index += 3) {
      for (let axis = 0; axis < 3; axis += 1) {
        min[axis] = Math.min(min[axis], values[index + axis])
        max[axis] = Math.max(max[axis], values[index + axis])
      }
    }
  }
  if (!Number.isFinite(min[0])) return null
  const size = [max[0] - min[0], max[1] - min[1], max[2] - min[2]]
  return {
    min,
    max,
    size,
    center: [(min[0] + max[0]) / 2, (min[1] + max[1]) / 2, (min[2] + max[2]) / 2],
  }
}

/** Physical extent of everything displayed, terrain included. Used for the ground grid. */
export function displayBounds(display) {
  return (
    boundsOf([...display.parts.map((part) => part.positions), display.terrain.engineered.positions, display.route.positions]) ?? {
      min: [0, 0, 0],
      max: [1, 1, 1],
      size: [1, 1, 1],
      center: [0, 0, 0],
    }
  )
}

/**
 * Extent of the engineering geometry alone: the saved solids, the saved route and the ports.
 *
 * The camera frames this, not the terrain. A 4 m flight inside a 48 m field must fill the
 * view, otherwise the individual treads the packet requires an inspector to see are a few
 * pixels wide and the screenshot proves nothing.
 */
export function structureBounds(display) {
  const arrays = [...display.parts.map((part) => part.positions), display.route.positions]
  for (const node of display.nodes) arrays.push(Float32Array.from(node.positionM))
  return boundsOf(arrays) ?? displayBounds(display)
}

/** Deterministic camera placements for the acceptance views. */
export function cameraPose(view, bounds) {
  const center = bounds.center
  const radius = Math.max(2, Math.hypot(bounds.size[0], bounds.size[1], bounds.size[2]))
  const distance = radius * 1.35
  switch (view) {
    case 'plan':
      return { position: [center[0], center[1] + distance, center[2] + 0.001], target: center }
    case 'side':
      return { position: [center[0] + distance, center[1] + radius * 0.15, center[2]], target: center }
    case 'perspective':
    default:
      return {
        position: [center[0] + distance * 0.75, center[1] + distance * 0.6, center[2] + distance * 0.75],
        target: center,
      }
  }
}
