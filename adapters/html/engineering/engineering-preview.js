// Engineering preview viewer.
//
// This page consumes one exported engineering package and nothing else:
//
//   ../engineering-preview.json   the versioned description (the completion marker)
//   ../source/input.json          the saved human input, original bytes
//   ../source/bundle.json         the committed production bundle, original bytes
//   ../render-model.json          the derived display projection
//   ./vendor/three/...            the vendored renderer (no CDN, no network)
//
// Every declared file is fetched and its SHA-256 is compared with the description before
// anything is drawn. A mismatch refuses the geometry and the route outright: there is no
// catch-to-empty-array path, and the success label never claims that the browser ran the
// Rust engineering audit.

import * as THREE from 'three'
import { OrbitControls } from 'three/addons/controls/OrbitControls.js'
import {
  DISPLAY_CONVENTION,
  PreviewRejection,
  TERRAIN_MODES,
  assertParity,
  buildDisplayModel,
  cameraPose,
  displayBounds,
  edgeVisibleForCapability,
  structureBounds,
} from './preview-model.js'

const ROOT = new URL('..', import.meta.url)
const DESCRIPTION = 'engineering-preview.json'

const ROLE_COLORS = {
  tread: 0x8fb3d9,
  riser: 0x5d6b7a,
  landing: 0xa8c6a0,
  support: 0x9a8b74,
  other: 0xb0796f,
}

const SURFACE_COLORS = {
  engineered: 0x40564a,
  natural: 0x2f4250,
  baseline: 0x53483a,
}

const state = {
  phase: 'loading',
  rejection: null,
  parity: null,
  description: null,
  renderModel: null,
  display: null,
  bounds: null,
  terrainMode: 'engineered',
  capabilityFilter: null,
  visibility: {
    structures: true,
    supports: true,
    terrain: true,
    route: true,
    ports: true,
    obstacles: false,
    water: false,
    helpers: true,
  },
  counts: {},
  ids: { parts: [], nodes: [], edges: [] },
  lastPick: null,
  reloads: 0,
  duplicateChecks: [],
  verifiedFiles: [],
  webgl: null,
  errors: [],
}

const elements = {}
let renderer = null
let scene = null
let camera = null
let controls = null
let raycaster = null
let groups = null
const disposables = []

function hashHex(buffer) {
  return crypto.subtle
    .digest('SHA-256', buffer)
    .then((digest) =>
      Array.from(new Uint8Array(digest))
        .map((byte) => byte.toString(16).padStart(2, '0'))
        .join(''),
    )
}

function recordError(message) {
  state.errors.push(message)
  const box = elements.errors
  if (box) {
    box.hidden = false
    box.textContent = state.errors.join('\n')
  }
}

function reject(message) {
  state.phase = 'rejected'
  state.rejection = message
  disposeScene()
  recordError(message)
  const banner = elements.banner
  if (banner) {
    banner.hidden = false
    banner.className = 'banner reject'
    banner.textContent = `拒绝加载：${message}`
  }
  updateStatus()
}

async function loadVerifiedDescription() {
  const response = await fetch(new URL(DESCRIPTION, ROOT), { cache: 'no-store' })
  if (!response.ok) {
    throw new PreviewRejection(`${DESCRIPTION} is missing: HTTP ${response.status}`)
  }
  const buffer = await response.arrayBuffer()
  const text = new TextDecoder().decode(buffer)
  let description
  try {
    description = JSON.parse(text)
  } catch (error) {
    throw new PreviewRejection(`${DESCRIPTION} is not readable JSON: ${error}`)
  }
  return description
}

/** Fetch one declared file and refuse it unless bytes and SHA-256 both match. */
async function loadVerifiedFile(fact) {
  const response = await fetch(new URL(fact.path, ROOT), { cache: 'no-store' })
  if (!response.ok) {
    throw new PreviewRejection(`${fact.path}: HTTP ${response.status}`)
  }
  const buffer = await response.arrayBuffer()
  if (buffer.byteLength !== fact.bytes) {
    throw new PreviewRejection(
      `${fact.path}: ${buffer.byteLength} bytes on the wire, ${fact.bytes} declared`,
    )
  }
  const digest = await hashHex(buffer)
  if (digest !== fact.sha256) {
    throw new PreviewRejection(`${fact.path}: sha256 ${digest} does not match the declared ${fact.sha256}`)
  }
  return buffer
}

function makeRenderer() {
  renderer = new THREE.WebGLRenderer({ canvas: elements.canvas, antialias: true })
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
  renderer.setSize(elements.viewport.clientWidth, elements.viewport.clientHeight, false)
  renderer.setClearColor(0x0d1117, 1)
  const gl = renderer.getContext()
  const debug = gl.getExtension('WEBGL_debug_renderer_info')
  state.webgl = {
    version: gl.getParameter(gl.VERSION),
    renderer: debug ? gl.getParameter(debug.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER),
    vendor: debug ? gl.getParameter(debug.UNMASKED_VENDOR_WEBGL) : gl.getParameter(gl.VENDOR),
  }
  scene = new THREE.Scene()
  scene.background = new THREE.Color(0x0d1117)
  camera = new THREE.PerspectiveCamera(
    45,
    elements.viewport.clientWidth / Math.max(1, elements.viewport.clientHeight),
    0.05,
    400,
  )
  controls = new OrbitControls(camera, elements.canvas)
  controls.enableDamping = true
  controls.dampingFactor = 0.12
  raycaster = new THREE.Raycaster()
  scene.add(new THREE.HemisphereLight(0xdff0ff, 0x2a2b26, 1.35))
  const sun = new THREE.DirectionalLight(0xffffff, 1.5)
  sun.position.set(24, 40, 18)
  scene.add(sun)
  groups = {
    terrain: new THREE.Group(),
    structures: new THREE.Group(),
    supports: new THREE.Group(),
    route: new THREE.Group(),
    edges: new THREE.Group(),
    ports: new THREE.Group(),
    obstacles: new THREE.Group(),
    water: new THREE.Group(),
    helpers: new THREE.Group(),
  }
  for (const group of Object.values(groups)) scene.add(group)
}

function track(resource) {
  disposables.push(resource)
  return resource
}

function disposeScene() {
  if (!scene) return
  for (const group of Object.values(groups ?? {})) {
    for (const child of [...group.children]) {
      group.remove(child)
    }
  }
  for (const resource of disposables) {
    if (resource && typeof resource.dispose === 'function') resource.dispose()
  }
  disposables.length = 0
  groups.route.clear()
}

function buildTerrain(mode) {
  const surface = state.display.terrain[mode]
  const geometry = track(new THREE.BufferGeometry())
  geometry.setAttribute('position', new THREE.BufferAttribute(surface.positions.slice(), 3))
  geometry.setIndex(new THREE.BufferAttribute(surface.indices.slice(), 1))
  geometry.computeVertexNormals()
  const material = track(
    new THREE.MeshStandardMaterial({
      color: SURFACE_COLORS[mode],
      roughness: 0.95,
      metalness: 0,
      side: THREE.DoubleSide,
      flatShading: false,
    }),
  )
  const mesh = new THREE.Mesh(geometry, material)
  mesh.name = `terrain:${mode}`
  mesh.userData.kind = 'terrain'
  mesh.userData.surface = mode
  return mesh
}

function buildStructures() {
  for (const part of state.display.parts) {
    const geometry = track(new THREE.BufferGeometry())
    geometry.setAttribute('position', new THREE.BufferAttribute(part.positions.slice(), 3))
    geometry.setIndex(new THREE.BufferAttribute(part.indices.slice(), 1))
    geometry.computeVertexNormals()
    const material = track(
      new THREE.MeshStandardMaterial({
        color: ROLE_COLORS[part.material] ?? ROLE_COLORS.other,
        roughness: part.material === 'support' ? 0.9 : 0.55,
        metalness: 0.05,
        side: THREE.DoubleSide,
      }),
    )
    const mesh = new THREE.Mesh(geometry, material)
    mesh.name = part.partId
    mesh.userData = {
      kind: 'part',
      partId: part.partId,
      structureId: part.structureId,
      role: part.role,
      levelId: part.levelId,
      material: part.material,
      topMm: part.topMm,
      bottomMm: part.bottomMm,
      runIndex: part.runIndex,
      index: part.index,
    }
    groups[part.material === 'support' ? 'supports' : 'structures'].add(mesh)
  }
}

function buildEdges() {
  for (const edge of state.display.edges) {
    const geometry = track(new THREE.BufferGeometry())
    geometry.setAttribute('position', new THREE.BufferAttribute(edge.positions.slice(), 3))
    const material = track(
      new THREE.LineBasicMaterial({
        color: edge.kind === 'ground' ? 0xc9a227 : edge.kind === 'stair' ? 0x6fa8dc : 0x93c47d,
        transparent: true,
        opacity: 0.85,
      }),
    )
    const line = new THREE.Line(geometry, material)
    line.name = edge.edgeId
    line.userData = { kind: 'edge', edgeId: edge.edgeId, capabilities: edge.capabilities }
    groups.edges.add(line)
  }
}

function buildRoute() {
  const route = state.display.route
  // The saved route is drawn only from the saved polyline of a found route. A route that
  // was not found is never drawn as if it were, and no path is invented here.
  if (route.status !== 'found' || route.pointCount < 2) return
  const geometry = track(new THREE.BufferGeometry())
  geometry.setAttribute('position', new THREE.BufferAttribute(route.positions.slice(), 3))
  const material = track(new THREE.LineBasicMaterial({ color: 0x39d353 }))
  const line = new THREE.Line(geometry, material)
  line.name = 'route'
  line.renderOrder = 10
  line.userData = { kind: 'route' }
  groups.route.add(line)
  const markerGeometry = track(new THREE.SphereGeometry(Math.max(0.05, state.focus.size[1] * 0.08), 10, 8))
  const markerMaterial = track(new THREE.MeshBasicMaterial({ color: 0x39d353 }))
  for (let index = 0; index < route.pointCount; index += 1) {
    const marker = new THREE.Mesh(markerGeometry, markerMaterial)
    marker.position.set(
      route.positions[index * 3],
      route.positions[index * 3 + 1],
      route.positions[index * 3 + 2],
    )
    marker.userData = { kind: 'route-point', index }
    groups.route.add(marker)
  }
}

function buildPorts() {
  const radius = Math.max(0.05, state.focus.size[1] * 0.1)
  for (const node of state.display.nodes) {
    const geometry = track(new THREE.SphereGeometry(radius, 12, 10))
    const material = track(
      new THREE.MeshStandardMaterial({
        color: node.kind === 'road' ? 0xf6c453 : 0xf0796b,
        emissive: 0x2a1a12,
        roughness: 0.4,
      }),
    )
    const marker = new THREE.Mesh(geometry, material)
    marker.position.set(node.positionM[0], node.positionM[1], node.positionM[2])
    marker.name = node.nodeId
    marker.userData = {
      kind: 'port',
      nodeId: node.nodeId,
      nodeKind: node.kind,
      surfaceId: node.surfaceId,
      levelId: node.levelId,
      roadId: node.roadId,
      capabilities: node.capabilities,
      positionM: node.positionM,
    }
    groups.ports.add(marker)
  }
}

function buildCheckBoxes(list, target, color) {
  const material = track(new THREE.LineBasicMaterial({ color }))
  for (const box of list) {
    const [a, b, c, d] = box.corners
    const lowY = box.bottomM ?? Math.min(a[1], c[1]) - 1
    const highY = box.topM ?? Math.max(a[1], c[1]) + 1
    const corners = [
      [a[0], lowY, a[2]],
      [b[0], lowY, b[2]],
      [b[0], highY, b[2]],
      [a[0], highY, a[2]],
      [c[0], lowY, c[2]],
      [d[0], lowY, d[2]],
      [d[0], highY, d[2]],
      [c[0], highY, c[2]],
    ]
    const positions = new Float32Array(corners.flat())
    const edges = [
      [0, 1], [1, 2], [2, 3], [3, 0],
      [4, 5], [5, 6], [6, 7], [7, 4],
      [0, 4], [1, 5], [2, 6], [3, 7],
    ]
    const indices = new Uint16Array(edges.flat())
    const geometry = track(new THREE.BufferGeometry())
    geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3))
    geometry.setIndex(new THREE.BufferAttribute(indices, 1))
    const segments = new THREE.LineSegments(geometry, material)
    segments.name = box.id
    segments.userData = { kind: target === groups.obstacles ? 'obstacle' : 'water', id: box.id }
    target.add(segments)
  }
}

function buildHelpers() {
  const size = Math.max(...state.bounds.size, 2)
  const grid = track(new THREE.GridHelper(Math.ceil(size * 1.5), Math.ceil(size * 1.5), 0x2b3a33, 0x1d2723))
  grid.position.set(state.bounds.center[0], state.bounds.min[1], state.bounds.center[2])
  grid.userData = { kind: 'helper' }
  groups.helpers.add(grid)
  const axes = track(new THREE.AxesHelper(Math.max(1.5, size * 0.25)))
  axes.position.set(state.bounds.min[0], state.bounds.min[1], state.bounds.min[2])
  axes.userData = { kind: 'helper' }
  groups.helpers.add(axes)
}

function applyVisibility() {
  groups.structures.visible = state.visibility.structures
  groups.supports.visible = state.visibility.supports
  groups.terrain.visible = state.visibility.terrain
  groups.route.visible = state.visibility.route
  groups.ports.visible = state.visibility.ports
  groups.obstacles.visible = state.visibility.obstacles
  groups.water.visible = state.visibility.water
  groups.helpers.visible = state.visibility.helpers
  // The capability filter is a display filter over saved declarations. It never re-routes
  // and it never changes the saved route's status.
  for (const line of groups.edges.children) {
    const declared = line.userData.capabilities ?? []
    line.visible = state.capabilityFilter === null || declared.includes(state.capabilityFilter)
  }
  groups.edges.visible = state.visibility.structures
}

function duplicateAudit() {
  const partIds = []
  const nodeIds = []
  for (const part of groups.structures.children) partIds.push(part.name)
  for (const part of groups.supports.children) partIds.push(part.name)
  for (const node of groups.ports.children) nodeIds.push(node.name)
  const duplicates = {
    parts: partIds.filter((id, index) => partIds.indexOf(id) !== index),
    ports: nodeIds.filter((id, index) => nodeIds.indexOf(id) !== index),
    routeLines: groups.route.children.filter((child) => child.userData.kind === 'route').length,
    structureObjects: groups.structures.children.length + groups.supports.children.length,
    routeObjects: groups.route.children.length,
    portObjects: groups.ports.children.length,
  }
  state.duplicateChecks.push({ ...duplicates, at: state.reloads })
  return duplicates
}

function rebuild() {
  disposeScene()
  state.display = buildDisplayModel(state.renderModel)
  state.parity = assertParity(state.renderModel, state.display)
  if (!state.parity.ok) {
    // No green line, no partial scene: the projection is refused as a whole.
    reject(`渲染投影与保存几何不一致：${state.parity.findings.slice(0, 3).join('; ')}`)
    return false
  }
  state.bounds = displayBounds(state.display)
  // The camera and the marker sizes follow the engineering geometry, not the terrain: an
  // inspector has to see the individual treads, not a speck in a 48 m field.
  state.focus = structureBounds(state.display)
  groups.terrain.add(buildTerrain(state.terrainMode))
  buildStructures()
  buildEdges()
  buildRoute()
  buildPorts()
  buildCheckBoxes(state.display.obstacles, groups.obstacles, 0xff8c42)
  buildCheckBoxes(state.display.water, groups.water, 0x4aa3ff)
  buildHelpers()
  applyVisibility()
  duplicateAudit()
  state.counts = {
    ...state.display.counts,
    sceneObjects: scene.children.length,
    structureObjects: groups.structures.children.length,
    supportObjects: groups.supports.children.length,
    routeObjects: groups.route.children.length,
    portObjects: groups.ports.children.length,
    edgeObjects: groups.edges.children.length,
  }
  state.ids = {
    parts: [...groups.structures.children, ...groups.supports.children].map((child) => child.name).sort(),
    nodes: groups.ports.children.map((child) => child.name).sort(),
    edges: groups.edges.children.map((child) => child.name).sort(),
  }
  setView('perspective')
  updateStatus()
  updatePanels()
  return true
}

function switchTerrain(mode) {
  if (!TERRAIN_MODES.includes(mode)) return
  state.terrainMode = mode
  if (!state.display) return
  for (const child of [...groups.terrain.children]) {
    groups.terrain.remove(child)
    if (child.geometry) child.geometry.dispose()
    if (child.material) child.material.dispose()
  }
  groups.terrain.add(buildTerrain(mode))
  applyVisibility()
  updatePanels()
}

function setView(view) {
  if (!state.focus) return
  const pose = cameraPose(view, state.focus)
  camera.position.set(pose.position[0], pose.position[1], pose.position[2])
  controls.target.set(pose.target[0], pose.target[1], pose.target[2])
  controls.update()
  state.view = view
}

/**
 * How much of the frame the saved structure geometry actually occupies, in viewport
 * fractions. A view that shrinks the flight into a few pixels cannot support a claim that
 * every tread was visible, so the acceptance asserts this rather than trusting the camera.
 */
function projectedExtent() {
  if (!state.display || !camera) return null
  let minX = Infinity
  let maxX = -Infinity
  let minY = Infinity
  let maxY = -Infinity
  const vector = new THREE.Vector3()
  for (const part of state.display.parts) {
    for (let index = 0; index < 8; index += 1) {
      vector.set(part.positions[index * 3], part.positions[index * 3 + 1], part.positions[index * 3 + 2])
      vector.project(camera)
      if (!Number.isFinite(vector.x) || !Number.isFinite(vector.y)) continue
      minX = Math.min(minX, vector.x)
      maxX = Math.max(maxX, vector.x)
      minY = Math.min(minY, vector.y)
      maxY = Math.max(maxY, vector.y)
    }
  }
  if (!Number.isFinite(minX)) return null
  return {
    fractionX: (maxX - minX) / 2,
    fractionY: (maxY - minY) / 2,
    centerX: (minX + maxX) / 2,
    centerY: (minY + maxY) / 2,
    onScreen: maxX > -1 && minX < 1 && maxY > -1 && minY < 1,
  }
}

function pick(clientX, clientY) {
  const rect = elements.canvas.getBoundingClientRect()
  const pointer = new THREE.Vector2(
    ((clientX - rect.left) / rect.width) * 2 - 1,
    -((clientY - rect.top) / rect.height) * 2 + 1,
  )
  raycaster.setFromCamera(pointer, camera)
  const targets = []
  if (groups.structures.visible) targets.push(...groups.structures.children)
  if (groups.supports.visible) targets.push(...groups.supports.children)
  if (groups.ports.visible) targets.push(...groups.ports.children)
  const hits = raycaster.intersectObjects(targets, false)
  if (hits.length === 0) {
    state.lastPick = null
    updatePanels()
    return null
  }
  const hit = hits[0].object
  const data = hit.userData
  const point = hits[0].point
  state.lastPick = {
    kind: data.kind,
    partId: data.partId ?? null,
    structureId: data.structureId ?? null,
    role: data.role ?? null,
    levelId: data.levelId ?? null,
    nodeId: data.nodeId ?? null,
    nodeKind: data.nodeKind ?? null,
    surfaceId: data.surfaceId ?? null,
    roadId: data.roadId ?? null,
    capabilities: data.capabilities ?? null,
    topMm: data.topMm ?? null,
    bottomMm: data.bottomMm ?? null,
    pointM: [point.x, point.y, point.z],
    distanceM: hits[0].distance,
  }
  updatePanels()
  return state.lastPick
}

function selectById(id) {
  const object = [...groups.structures.children, ...groups.supports.children, ...groups.ports.children].find(
    (child) => child.name === id,
  )
  if (!object) return null
  const data = object.userData
  state.lastPick = {
    kind: data.kind,
    partId: data.partId ?? null,
    structureId: data.structureId ?? null,
    role: data.role ?? null,
    levelId: data.levelId ?? null,
    nodeId: data.nodeId ?? null,
    surfaceId: data.surfaceId ?? null,
    roadId: data.roadId ?? null,
    capabilities: data.capabilities ?? null,
    topMm: data.topMm ?? null,
    bottomMm: data.bottomMm ?? null,
    pointM: [object.position.x, object.position.y, object.position.z],
    distanceM: null,
  }
  updatePanels()
  return state.lastPick
}

function updateStatus() {
  const banner = elements.banner
  if (!banner) return
  if (state.phase === 'rejected') return
  banner.hidden = false
  if (state.phase === 'verified') {
    banner.className = 'banner ok'
    const verified = state.description?.verification ?? {}
    banner.textContent = `导出时工程验证通过（verify_bundle: ${verified.verifyStatus}，findings ${verified.verifyFindingCount}）；当前 ${state.verifiedFiles.length} 个文件完整性一致（sha256）。浏览器未运行 Rust 工程审核。`
  } else {
    banner.className = 'banner'
    banner.textContent = '正在校验导出文件…'
  }
  elements.banner.dataset.phase = state.phase
}

function updatePanels() {
  if (!elements.panels) return
  const description = state.description
  if (description) {
    elements.identity.textContent = [
      `scene ${description.sceneId}`,
      `domain ${description.domain}`,
      `engine ${description.verification.engine} v${description.verification.engineModuleVersion} (contract ${description.verification.engineContractVersion})`,
      `input sha256 ${description.source.inputSha256.slice(0, 16)}… (${description.source.inputBytes} B)`,
      `bundle sha256 ${description.source.bundleSha256.slice(0, 16)}… (${description.source.bundleBytes} B)`,
      `bundle payloadHash ${description.source.bundlePayloadHash.slice(0, 16)}… (different hash unit from the file hash)`,
      `display ${DISPLAY_CONVENTION.worldToDisplay} (f32 ≤ ${description.displayConvention.f32ToleranceM} m)`,
    ].join('\n')
  }
  const route = state.display?.route
  if (route && elements.route) {
    elements.route.textContent = [
      `saved query: ${route.query.fromPortId} → road ${route.query.toRoadId} (${route.query.capability})`,
      `limits: ≤ ${route.limits.maxLength3dM} m 3D, ≤ ${route.limits.maxRiseM} m rise`,
      `route status: ${route.status}`,
      `3D length ${route.pathLength3dM.toFixed(3)} m, horizontal ${route.horizontalLengthM.toFixed(3)} m, rise ${route.cumulativeRiseM.toFixed(3)} m, descent ${route.cumulativeDescentM.toFixed(3)} m`,
      `capability filter: ${state.capabilityFilter ?? 'none'} (saved declarations only; the preview never re-routes and never changes the saved status)`,
    ].join('\n')
  }
  const terrain = state.display?.terrain?.[state.terrainMode]
  if (terrain && elements.terrain) {
    elements.terrain.textContent = [
      `surface: ${state.terrainMode}`,
      `hash ${terrain.contentHash}`,
      `${terrain.columns} x ${terrain.rows} samples = ${terrain.cells} cells`,
    ].join('\n')
  }
  if (elements.pick) {
    const pick = state.lastPick
    elements.pick.textContent = pick
      ? Object.entries(pick)
          .map(([key, value]) => `${key}: ${Array.isArray(value) ? value.join(', ') : value}`)
          .join('\n')
      : '（未选中）点击一个踏步、平台或端口'
  }
  if (elements.counts) {
    elements.counts.textContent = Object.entries(state.counts)
      .map(([key, value]) => `${key}: ${value}`)
      .join('\n')
  }
}

function wireUi() {
  for (const input of elements.layerInputs) {
    input.addEventListener('change', () => {
      state.visibility[input.dataset.layer] = input.checked
      applyVisibility()
    })
  }
  for (const input of elements.terrainInputs) {
    input.addEventListener('change', () => {
      if (input.checked) switchTerrain(input.value)
    })
  }
  elements.capability.addEventListener('change', () => {
    state.capabilityFilter = elements.capability.value === '' ? null : elements.capability.value
    applyVisibility()
    updatePanels()
  })
  elements.reload.addEventListener('click', () => {
    state.reloads += 1
    rebuild()
  })
  elements.canvas.addEventListener('pointerdown', (event) => {
    if (!state.display) return
    pick(event.clientX, event.clientY)
  })
  window.addEventListener('resize', () => {
    if (!renderer) return
    const width = elements.viewport.clientWidth
    const height = Math.max(1, elements.viewport.clientHeight)
    renderer.setSize(width, height, false)
    camera.aspect = width / height
    camera.updateProjectionMatrix()
  })
}

function animate() {
  requestAnimationFrame(animate)
  if (controls) controls.update()
  if (renderer && scene && camera) renderer.render(scene, camera)
}

async function main() {
  elements.canvas = document.getElementById('canvas')
  elements.viewport = document.getElementById('viewport')
  elements.banner = document.getElementById('banner')
  elements.errors = document.getElementById('errors')
  elements.identity = document.getElementById('identity')
  elements.route = document.getElementById('route')
  elements.terrain = document.getElementById('terrain')
  elements.pick = document.getElementById('pick')
  elements.counts = document.getElementById('counts')
  elements.panels = document.getElementById('panels')
  elements.reload = document.getElementById('reload')
  elements.capability = document.getElementById('capability')
  elements.layerInputs = [...document.querySelectorAll('input[data-layer]')]
  elements.terrainInputs = [...document.querySelectorAll('input[name="terrain"]')]
  wireUi()

  try {
    const description = await loadVerifiedDescription()
    state.description = description
    if (description.schema !== 'glyphweave.engineering-preview/1') {
      throw new PreviewRejection(`unsupported preview schema ${description.schema}`)
    }
    if (description.exportStatus !== 'complete') {
      throw new PreviewRejection(`export status ${description.exportStatus} is not complete`)
    }
    for (const fact of description.files) {
      await loadVerifiedFile(fact)
      state.verifiedFiles.push(fact.path)
    }
    const modelFact = description.files.find((fact) => fact.path === 'render-model.json')
    if (!modelFact) throw new PreviewRejection('the description declares no render model')
    const modelBuffer = await loadVerifiedFile(modelFact)
    state.renderModel = JSON.parse(new TextDecoder().decode(modelBuffer))

    makeRenderer()
    if (!rebuild()) return
    state.phase = 'verified'
    updateStatus()
    animate()
  } catch (error) {
    reject(error instanceof PreviewRejection ? error.message : String(error))
  }
}

window.__engineeringPreview = {
  getState: () => ({
    phase: state.phase,
    rejection: state.rejection,
    parity: state.parity,
    counts: state.counts,
    ids: state.ids,
    duplicateChecks: state.duplicateChecks,
    verifiedFiles: state.verifiedFiles,
    webgl: state.webgl,
    errors: state.errors,
    terrainMode: state.terrainMode,
    capabilityFilter: state.capabilityFilter,
    visibility: { ...state.visibility },
    view: state.view,
    focus: state.focus,
    projectedExtent: projectedExtent(),
    lastPick: state.lastPick,
    reloads: state.reloads,
    sceneId: state.description?.sceneId ?? null,
    routeStatus: state.display?.route?.status ?? null,
    routeQueryCapability: state.display?.route?.query?.capability ?? null,
    terrainHash: state.display?.terrain?.[state.terrainMode]?.contentHash ?? null,
    displayConvention: DISPLAY_CONVENTION,
  }),
  setView,
  switchTerrain,
  setLayer: (layer, visible) => {
    state.visibility[layer] = visible
    applyVisibility()
    return state.visibility
  },
  setCapabilityFilter: (capability) => {
    state.capabilityFilter = capability || null
    applyVisibility()
    updatePanels()
    return state.capabilityFilter
  },
  selectById,
  pickAt: pick,
  reload: () => {
    state.reloads += 1
    return rebuild()
  },
  bounds: () => state.bounds,
  focusBounds: () => state.focus,
  projectedExtent,
}

main()
