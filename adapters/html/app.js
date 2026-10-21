import { makeGradedRoad } from './render/road-profile.js'
import { verticesOverlapChunks, createPreviewResourceTracker } from './core/render-stream.js'
import { loadPreviewScene, createRenderStream } from './core/render-stream.js'
import { createEcologyRenderer } from './render/ecology.js'
import { loadLandUseLedger } from './core/world.js'
import { decodeHeightPayload } from './core/height.js'
import { color, colorRgb, hashNoise, smoothNoise, fractalNoise, deterministicRand } from './core/shared.js'
import { makeSurfaceTexture, makeLodTexture, makeStripGeometry, makePolylineRibbonGeometry, makeArtisticGroundTexture, makeSteppeGrass, makeMCTerrain } from './render/terrain.js'
import { makeGrassBlades } from './render/grass.js'
import { createWorldWind } from './render/wind.js'
import { createPostFX } from './render/postfx.js'
import { makeWaterTexture, makeRiverGeometry, makeWaterMaterial } from './render/water.js'
import { makeSkyTexture } from './render/sky.js'
import { makeVoxelTree, buildTreeInstances, makeMangrove, makeForestPatch, buildRockInstances } from './render/vegetation.js'
import { makeWaterWell, makeRoadSign, makeStreetLamp, makePedestrian, makeFoodStall } from './render/props.js'
import { fitAssetToEntity, addBridgeBeam, makeBridgeStructure, makeHongKongTower, applySlopeFoundation, makeResidentialBlock, makeResidentialTower, makeResidentialHome, makePlannedResidentialHome, makePlannedServiceBuilding, makeResortLodge, makeSchool, makeCommercialCenter, makeEntertainmentCenter, makeParkingLot, makeTemple, makeChurch, makeFarmland, makePasture, makeCanal, makeTownHall, makeMarket, makeIndustrial, makeStorefront, makeCoastalFeature } from './render/buildings.js'

const canvas = document.querySelector('#map')
const context = canvas.getContext('2d')
const title = document.querySelector('#title')
const info = document.querySelector('#info')
const sceneSelect = document.querySelector('#scene')
const loadingOverlay = document.querySelector('#loading')
function setNearLoading(active) { if (loadingOverlay) loadingOverlay.hidden = !active }
let world
let scene
let mode = 'strategic'
let nearPreset = 'harbour'
// URL presets: ?view=near|skyline|street|explore opens straight into a 3D view.
const urlView = new URLSearchParams(location.search).get('view')
if (urlView && urlView !== 'ownership') { mode = 'near'; nearPreset = ['skyline', 'street', 'explore', 'waterfront', 'bridgehead', 'yard', 'farm', 'services', 'graded-road', 'frontage'].includes(urlView) ? urlView : 'harbour' }
let webglCanvas
let nearRenderer
let nearScene
let nearCamera
let nearControls
let exploreKeys = new Set()
const STATIC_PREVIEW = true
let worldWind
let exploreInputCleanup
const chunkDataCache = new Map()
const entityMeshCache = new Map()
const entityGroupCache = new Map()
let nearGroupRoot = null
let ecologyRenderer = null
let activeChunkKeys = new Set()
let nearBuildToken = 0
let renderNearFrame = null
const previewResources = createPreviewResourceTracker()
let frameAssets = [], nearDrawing = false
let previewState = {status: 'idle', requestedGeneration: 0, completedGeneration: 0, error: null}
const previewPageId = crypto.randomUUID()
function frameRoots() { return [nearGroupRoot, ...entityMeshCache.values(), ...entityGroupCache.values(), ...frameAssets] }
function releasePreviewFrame() {
  renderNearFrame = null
  ecologyRenderer?.dispose()
  previewResources.release(frameRoots())
  nearGroupRoot?.clear()
  entityMeshCache.clear(); entityGroupCache.clear(); chunkDataCache.clear(); frameAssets = []
  const fx = window.__nearFx
  if (fx) { for (const pass of fx.composer?.passes ?? []) pass.dispose?.(); fx.composer?.dispose(); window.__nearFx = null }
  window.__steppeGrassLayers = []
}
// Detached observation: callers can mutate their copy, never renderer state.
Object.defineProperty(window, '__previewStats', {value: () => {
  const plantIds = [], renderedPatchIds = [], accessSurfaceIds = [], renderedEntityInstanceIds = [], renderedEntityInstanceSources = []
  const drawableGeometry = object => {
    let meshCount = 0, geometryCount = 0, instanceCount = 0
    object.traverse(child => {
      if (!(child.isMesh || child.isInstancedMesh) || !child.geometry) return
      const count = child.isInstancedMesh ? Number(child.count) : 1
      if (!Number.isFinite(count) || count <= 0) return
      meshCount += 1; geometryCount += 1
      if (child.isInstancedMesh) instanceCount += count
    })
    return {meshCount, geometryCount, instanceCount}
  }
  nearScene?.traverse(object => {
    if (object.name?.startsWith('ecology-batch:')) plantIds.push(...object.userData.ecologyInstances.map(p => p.id))
    if (object.isMesh && object.name?.startsWith('access:')) accessSurfaceIds.push(object.name)
    if (object.isMesh && object.name?.startsWith('landcover:')) renderedPatchIds.push(object.name.slice('landcover:'.length))
    if (object.userData?.sourceEntityId) {
      const geometry = drawableGeometry(object)
      const valid = geometry.meshCount > 0 && geometry.geometryCount > 0
      renderedEntityInstanceIds.push(object.userData.sourceEntityId)
      renderedEntityInstanceSources.push({id: object.userData.sourceEntityId, source: 'object',
        objectUuid: object.uuid, type: object.type, name: object.name || '', ...geometry,
        valid, reason: valid ? null : 'no-drawable-geometry'})
    }
    if (Array.isArray(object.userData?.entityInstanceBindings)) {
      const meshes = []
      object.traverse(child => { if (child.isInstancedMesh && child.geometry) meshes.push(child) })
      const meshByUuid = new Map(meshes.map(mesh => [mesh.uuid, mesh]))
      for (const binding of object.userData.entityInstanceBindings) {
        const parts = Array.isArray(binding.parts) ? binding.parts : []
        let valid = Number.isSafeInteger(binding.instanceIndex) && binding.instanceIndex >= 0
          && Number.isSafeInteger(binding.count) && binding.instanceIndex < binding.count
          && parts.length > 0
        const checkedParts = parts.map(part => {
          const mesh = meshByUuid.get(part.batchUuid)
          const partValid = Boolean(mesh) && Number.isSafeInteger(part.instanceIndex) && part.instanceIndex >= 0
            && part.instanceIndex < mesh.count && part.geometryUuid === mesh.geometry?.uuid
            && Number.isSafeInteger(part.instanceCount) && part.instanceCount === mesh.count
          if (!partValid) valid = false
          return {...part, instanceCount: mesh?.count ?? part.instanceCount, partValid}
        })
        renderedEntityInstanceIds.push(binding.id)
        renderedEntityInstanceSources.push({id: binding.id, source: 'instances',
          instanceIndex: binding.instanceIndex, count: binding.count, parts: checkedParts,
          valid, reason: valid ? null : 'invalid-instance-binding'})
      }
    }
  })
  previewResources.observe(frameRoots())
  const streamPayload = window.glyphweaveFeedback?.visualChecks?.renderStream
  const patchIds = Array.isArray(streamPayload?.patchIds) ? streamPayload.patchIds : []
  const nonMeshPatchIds = Array.isArray(streamPayload?.nonMeshPatchIds) ? streamPayload.nonMeshPatchIds : []
  return structuredClone({format: 'glyphweave.preview-observation', version: 1, pageId: previewPageId,
    worldRevision: world?.revision, sceneId: scene?.sceneId, ...previewState,
    highDetailChunk: window.glyphweaveFeedback?.visualChecks?.highDetailChunk,
    loadedChunks: window.glyphweaveFeedback?.visualChecks?.loadedChunks ?? [],
    stream: renderStream?.stats() ?? null, payload: streamPayload,
    plantIds, accessSurfaceIds, patchIds, renderedPatchIds, nonMeshPatchIds,
    renderedEntityInstanceIds,
    renderedEntityInstanceSources,
    renderedEntityIds: [...new Set(renderedEntityInstanceIds)].sort(),
    caches: {terrain: chunkDataCache.size, entityMeshes: entityMeshCache.size, entityGroups: entityGroupCache.size},
    resources: previewResources.stats(), ecology: ecologyRenderer?.stats() ?? null,
    renderer: nearRenderer ? {geometries: nearRenderer.info.memory.geometries, textures: nearRenderer.info.memory.textures,
      drawCalls: nearRenderer.info.render.calls} : null,
    heap: performance.memory ? {usedBytes: performance.memory.usedJSHeapSize, source: 'performance.memory', gc: false} : null,
    gpuBytes: null})
}})
function requestNearDraw() {
  const requested = ++nearBuildToken
  previewState = {...previewState, status: 'loading', requestedGeneration: requested, error: null}
  setNearLoading(true)
  if (!nearDrawing) void drainNearDraws()
  return requested
}
async function drainNearDraws() {
  nearDrawing = true
  try {
    let token
    do {
      token = nearBuildToken
      try {
        await drawNear(token)
      } catch (error) {
        if (token === nearBuildToken) {
          previewState = {...previewState, status: 'failed', error: String(error.message || error)}
          setNearLoading(false)
          info.textContent = '2.5D load failed; retry the same focus: ' + previewState.error
          console.error('2.5D render failed', error)
        }
      }
    } while (token !== nearBuildToken)
  } finally { nearDrawing = false }
}

window.glyphweaveSubmitFeedback = async (feedback) => {
  const payload = {
    format: 'glyphweave.visual-feedback',
    version: 1,
    createdAt: new Date().toISOString(),
    ...feedback,
    runtime: window.glyphweaveFeedback ?? null,
  }
  const response = await fetch('../api/feedback', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload) })
  if (!response.ok) throw new Error(`feedback request failed: ${response.status}`)
  return response.json()
}

canvas.addEventListener('click', (event) => {
  if (!scene) return
  const rect = canvas.getBoundingClientRect()
  const scale = Math.min(canvas.clientWidth / scene.widthM, canvas.clientHeight / scene.depthM)
  const offsetX = (canvas.clientWidth - scene.widthM * scale) / 2
  const offsetZ = (canvas.clientHeight - scene.depthM * scale) / 2
  const worldX = Math.floor((event.clientX - rect.left - offsetX) / scale) + scene.originX
  const worldZ = Math.floor((event.clientY - rect.top - offsetZ) / scale) + scene.originZ
  if (mode === 'strategic') {
    info.textContent = `${scene.sceneId}  X=${worldX} Z=${worldZ}  (click again to enter this area)`;
  }
})
canvas.addEventListener('dblclick', (event) => {
  if (!scene || mode !== 'strategic') return
  const rect = canvas.getBoundingClientRect()
  const scale = Math.min(canvas.clientWidth / scene.widthM, canvas.clientHeight / scene.depthM)
  const offsetX = (canvas.clientWidth - scene.widthM * scale) / 2
  const offsetZ = (canvas.clientHeight - scene.depthM * scale) / 2
  const worldX = Math.floor((event.clientX - rect.left - offsetX) / scale) + scene.originX
  const worldZ = Math.floor((event.clientY - rect.top - offsetZ) / scale) + scene.originZ
  const chunk = scene.chunks.find(c => worldX >= c.worldX && worldX < c.worldX + c.validWidthM && worldZ >= c.worldZ && worldZ < c.worldZ + c.validDepthM)
  if (chunk) {
    window.__nearFocusOverride = { x: chunk.worldX - scene.originX + chunk.validWidthM / 2, z: chunk.worldZ - scene.originZ + chunk.validDepthM / 2 }
    nearPreset = 'focus'; mode = 'near'; draw()
    info.textContent = `${scene.sceneId} enter chunk (${chunk.chunkX}, ${chunk.chunkZ}) world ${chunk.worldX},${chunk.worldZ}`
  }
})

function resize() {
  canvas.width = canvas.clientWidth * devicePixelRatio
  canvas.height = canvas.clientHeight * devicePixelRatio
  context.setTransform(devicePixelRatio, 0, 0, devicePixelRatio, 0, 0)
  const viewport = document.querySelector('#viewport')
  const viewportWidth = Math.max(1, viewport.clientWidth)
  const viewportHeight = Math.max(1, viewport.clientHeight)
  if (nearRenderer) {
    nearRenderer.setSize(viewportWidth, viewportHeight, false)
  }
  if (nearCamera?.isPerspectiveCamera) {
    nearCamera.aspect = viewportWidth / viewportHeight
    nearCamera.updateProjectionMatrix()
  }
  if (window.__nearFx) {
    window.__nearFx.setSize(viewportWidth, viewportHeight)
  }
  if (scene) draw()
}














// Build all trees of one chunk into a few InstancedMeshes (trunk + layered
// foliage canopy) so thousands of trees cost a handful of draw calls instead
// of one THREE.Group per tree. Each tree keeps its own scale / position /
// tint. The canopy is stacked from two offset Dodecahedra per tree so it reads
// as a dense, shading mass rather than a single lollipop sphere.




// Slope-aware plinth (Mountain-City style). A building with a large footprint
// on uneven ground either floats or sinks at its corners; this adds a concrete
// foundation that reaches from the building anchor down to the local terrain
// so the structure reads as built into the slope (like Chongqing / stilt
// houses) instead of clipping through the grass.


// Chinese-style gated compound: a cluster of high-rise towers around a shared
// court, with a wall line and internal greenery. Reads as a dense urban
// residential block rather than a handful of small cottages.

// American-style detached home on its own lot, with a front yard and a single
// family house 鈥?low-rise and spread out, distinct from the dense core.

// Waterfront holiday resort lodge in the Shenzhen Bay / OCT Happy Harbour
// idiom. Now declarative: the CGA rule set CGA_RESORT produces the white
// plaster body, flush louver cladding, glass strip and flat parapet roof.
















// ---------------------------------------------------------------------------
// Lightweight CGA-style building rules.
//
// A "style" is pure data: a list of blocks that the interpreter expands into
// BoxGeometry meshes. Each block is anchored to the building pad (0,0,0 = the
// ground centre of the lot) and sized with fractions of the entity, absolute
// metres, or a small expression language. Blocks may repeat along Y (floors),
// mirror across an axis, or gate on a condition, so a single rule set adapts
// to any lot size 鈥?the same idea as symbios' Split / Extrude / Comp grammar,
// without the runtime parser.
//
// Block fields:
//   kind:  "box" | "repeatY" | "repeatX" | "mirror"
//   w/d/h: width/depth/height. Number = metres; "~0.8w" = fraction of entity
//          width; "~0.5h" = fraction of entity height.
//   mat:   color hex (or "glass"/"white"/"trim"/"louver"/"accent"/"dark").
//   x/z/y: local offsets (same unit rules as w/d/h).
//   rotY, tiltZ: optional rotation (radians).
//   cond:  optional "floors>1" / "width>15" style gate.
//   part:  for repeatY 鈥?the block repeated each floor.
// ---------------------------------------------------------------------------


// Measure a size token against the entity: "~0.8w" -> fraction of width,
// "3" -> 3 metres, "~0.5h" -> fraction of height, "~0.3d" -> fraction depth.


// Expand a single block into one or more meshes added to `group`.

// Build a building from a CGA style rule set.

// ---------------------------------------------------------------------------
// Style rule sets.
// ---------------------------------------------------------------------------

// Shenzhen Bay / OCT resort-lodge idiom: white plaster, wood louver band,
// glass curtain strip, flat roof with parapet. Declarative equivalent of
// makeResortLodge.

// Commercial storefront: a real 3.2m ground floor (window, awning, sign,
// door) plus repeated upper-floor window bands. No element floats mid-air.





// Street-corner food stall (琛楄灏忓悆鎽?: a shaded cart with a stove, a small
// seating area, a tiled apron underfoot, planted greenery and a shade tree.
// This is a self-contained micro-scene that reads as a lived-in corner even
// though every part is primitive geometry.


let sceneLoadToken = 0
let renderStream = null
async function loadScene() {
  const token = ++sceneLoadToken
  const loaded = await loadPreviewScene(world.scenes[sceneSelect.selectedIndex], world.revision)
  loaded.urbanDistrictAudit = await fetch(`../scenes/${loaded.sceneId}/urban-district-audit.json`)
    .then(response => response.ok ? response.json() : null)
    .catch(() => null)
  loaded.routeEvolutionPlan = await fetch(`../scenes/${loaded.sceneId}/route-evolution-plan.json`)
    .then(response => response.ok ? response.json() : null)
    .catch(() => null)
  loaded.settlementEvolutionPlan = await fetch(`../scenes/${loaded.sceneId}/settlement-evolution-plan.json`)
    .then(response => response.ok ? response.json() : null)
    .catch(() => null)
  loaded.marketCatchmentPlan = await fetch(`../scenes/${loaded.sceneId}/market-catchment-plan.json`)
    .then(response => response.ok ? response.json() : null)
    .catch(() => null)
  loaded.epochEvolutionPlan = await fetch(`../scenes/${loaded.sceneId}/epoch-evolution-plan.json`)
    .then(response => response.ok ? response.json() : null)
    .catch(() => null)
  loaded.landUseLedger = await loadLandUseLedger(loaded)
  if (token !== sceneLoadToken) return
  nearBuildToken++
  renderStream?.dispose()
  scene = loaded
  renderStream = loaded.renderStream ? createRenderStream(loaded) : null
  draw()
}

function drawStrategicOverlays(ox, oz, scale) {
  const districtColors = ['#d8aa58', '#4f9d9a', '#6789c9']
  for (const route of scene.routeEvolutionPlan?.routes ?? []) {
    if (route.status !== 'pass' || !(route.polyline?.length >= 2)) continue
    context.save()
    context.lineCap = 'round'
    context.lineJoin = 'round'
    context.strokeStyle = '#5b5043'
    context.lineWidth = Math.max(2, route.roadWidthM * scale)
    context.beginPath()
    route.polyline.forEach((point, index) => {
      const x = ox + (point.x - scene.originX) * scale
      const z = oz + (point.z - scene.originZ) * scale
      if (index === 0) context.moveTo(x, z)
      else context.lineTo(x, z)
    })
    context.stroke()
    context.strokeStyle = '#c8b882'
    context.lineWidth = Math.max(1, scale * .7)
    context.stroke()
    for (const span of route.bridgeSpans ?? []) {
      if (span.kind !== 'bridge') continue
      const points = route.polyline.slice(span.startIndex, span.endIndex + 1)
      if (points.length < 2) continue
      context.strokeStyle = '#cdbb92'
      context.lineWidth = Math.max(3, route.roadWidthM * scale * 1.35)
      context.beginPath()
      points.forEach((point, index) => {
        const x = ox + (point.x - scene.originX) * scale
        const z = oz + (point.z - scene.originZ) * scale
        if (index === 0) context.moveTo(x, z)
        else context.lineTo(x, z)
      })
      context.stroke()
    }
    context.restore()
  }
  const epochByBuilding = new Map((scene.epochEvolutionPlan?.assignedBuildings ?? []).map(item => [item.buildingId, item]))
  for (const building of scene.settlementEvolutionPlan?.buildings ?? []) {
    const x = ox + (building.worldX - scene.originX - building.widthM / 2) * scale
    const z = oz + (building.worldZ - scene.originZ - building.depthM / 2) * scale
    context.fillStyle = building.kind === 'storefront' ? '#c65b43'
      : building.kind === 'office_tower' ? '#9aadb4'
      : '#d4b48a'
    context.fillRect(x, z, Math.max(2, building.widthM * scale), Math.max(2, building.depthM * scale))
    const epoch = epochByBuilding.get(building.id)
    context.strokeStyle = epoch?.operation === 'replace' ? '#e3b55f'
      : epoch?.operation === 'modify' ? '#7ab7a7'
      : epoch?.cohort === 'old-core' ? '#2f3430'
      : 'rgba(24,34,38,.55)'
    context.lineWidth = 1
    context.setLineDash(epoch?.operation === 'replace' ? [4, 3] : epoch?.cohort === 'old-core' ? [] : [2, 2])
    context.strokeRect(x, z, Math.max(2, building.widthM * scale), Math.max(2, building.depthM * scale))
    context.setLineDash([])
  }
  const marketDemand = new Map((scene.marketCatchmentPlan?.demandPoints ?? []).map(point => [point.id, point]))
  for (const access of scene.marketCatchmentPlan?.accessibility ?? []) {
    const point = marketDemand.get(access.demandId)
    if (!point) continue
    const x = ox + (point.worldX - scene.originX) * scale
    const z = oz + (point.worldZ - scene.originZ) * scale
    const deficitRatio = access.deficit / Math.max(1, point.demand)
    context.beginPath()
    context.arc(x, z, 5 + Math.min(5, point.demand / 40), 0, Math.PI * 2)
    context.fillStyle = deficitRatio > .25 ? 'rgba(190,66,48,.38)'
      : deficitRatio > .05 ? 'rgba(210,164,72,.32)'
      : 'rgba(74,151,104,.28)'
    context.fill()
  }
  for (const market of scene.marketCatchmentPlan?.markets ?? []) {
    const x = ox + (market.worldX - scene.originX) * scale
    const z = oz + (market.worldZ - scene.originZ) * scale
    context.fillStyle = '#e0a73f'
    context.strokeStyle = '#3a2d1d'
    context.lineWidth = 1.5
    context.beginPath()
    context.roundRect(x - 6, z - 6, 12, 12, 2)
    context.fill()
    context.stroke()
  }
  const districts = scene.urbanDistrictAudit?.iterations ?? []
  if (districts.length) {
    const center = scene.urbanDistrictAudit.centerWorld
    const px = ox + (center[0] - scene.originX) * scale
    const pz = oz + (center[1] - scene.originZ) * scale
    context.save()
    districts.forEach((district, index) => {
      const radius = district.centerDistanceM[1] * scale
      context.beginPath()
      context.arc(px, pz, radius, 0, Math.PI * 2)
      context.setLineDash([7, 6])
      context.lineWidth = index === 0 ? 2 : 1.5
      context.strokeStyle = districtColors[index % districtColors.length]
      context.globalAlpha = .78
      context.stroke()
      context.setLineDash([])
      context.fillStyle = districtColors[index % districtColors.length]
      context.font = '600 12px system-ui'
      context.fillText(district.id, px + radius * .72, pz - radius * .72)
    })
    context.globalAlpha = 1
    context.fillStyle = '#111c22'; context.fillRect(18, canvas.clientHeight - 102, 334, 84)
    context.font = '12px system-ui'; context.textBaseline = 'middle'
    districts.forEach((district, index) => {
      context.fillStyle = districtColors[index % districtColors.length]
      context.fillRect(28, canvas.clientHeight - 94 + index * 22, 12, 12)
      context.fillStyle = '#e2e9e6'
      context.fillText(`${district.constructionCohort} · ${district.centerDistanceM[0]}–${district.centerDistanceM[1]}m · ${district.planned}/${district.requested}`, 48, canvas.clientHeight - 88 + index * 22)
    })
    context.restore()
  }
  for (const entity of scene.entities ?? []) {
    if (entity.kind !== 'road' && entity.kind !== 'sidewalk' && entity.kind !== 'building' && entity.kind !== 'urban_building' && entity.kind !== 'building_tower' && entity.kind !== 'town_hall' && entity.kind !== 'market' && entity.kind !== 'industrial' && entity.kind !== 'commercial_center' && entity.kind !== 'school' && entity.kind !== 'residential_block') continue
    if (entity.kind === 'road') {
      const alongX = entity.widthM >= entity.depthM
      const halfW = Math.max(3, (alongX ? entity.widthM : entity.depthM) / 2)
      const halfD = Math.max(3, (alongX ? entity.depthM : entity.widthM) / 2)
      context.fillStyle = '#a08c6b'
      context.fillRect(ox + (entity.worldX - scene.originX - halfW) * scale, oz + (entity.worldZ - scene.originZ - halfD) * scale, halfW * 2 * scale, halfD * 2 * scale)
      continue
    }
    const px = ox + (entity.worldX - scene.originX) * scale
    const pz = oz + (entity.worldZ - scene.originZ) * scale
    const civic = entity.kind === 'town_hall' || entity.kind === 'market'
    const size = entity.kind === 'building_tower' ? 3 : civic ? 3 : entity.kind === 'urban_building' ? 2.5 : entity.kind === 'industrial' ? 3.2 : 2
    const height = entity.kind === 'building_tower' ? Math.min(42, entity.heightM * .12) : civic ? Math.min(18, entity.heightM * .1) : entity.kind === 'industrial' ? Math.min(14, entity.heightM * .09) : 0
    context.fillStyle = entity.kind === 'building_tower' ? '#aabcc1' : entity.kind === 'town_hall' ? '#d8ccb2' : entity.kind === 'market' ? '#c65b43' : entity.kind === 'industrial' ? '#7d8791' : entity.kind === 'commercial_center' ? '#b3a37e' : entity.kind === 'school' ? '#c9a86a' : entity.kind === 'residential_block' ? '#d4b48a' : '#8b8578'
    context.fillRect(px - size, pz - size - height, size * 2, size * 2 + height)
    if (height > 0) {
      context.fillStyle = 'rgba(18, 32, 38, .55)'
      context.fillRect(px + size, pz - size - height, Math.max(1, size * .7), height)
    }
  }
  for (const landmark of scene.landmarks) {
    if (landmark.type === 'bridge') {
      const x = ox + (landmark.worldX - scene.originX) * scale
      const z = oz + (landmark.worldZ - scene.originZ) * scale
      context.fillStyle = '#c4b28d'
      context.fillRect(x - landmark.widthM * scale / 2, z - landmark.depthM * scale / 2, landmark.widthM * scale, Math.max(4, landmark.depthM * scale))
      context.strokeStyle = '#e0d1a9'; context.lineWidth = Math.max(1, scale * 2); context.strokeRect(x - landmark.widthM * scale / 2, z - landmark.depthM * scale / 2, landmark.widthM * scale, Math.max(4, landmark.depthM * scale))
      continue
    }
    if (!['pagoda', 'station', 'ferris_wheel', 'park', 'boat'].includes(landmark.type)) continue
    const px = ox + (landmark.worldX - scene.originX) * scale
    const pz = oz + (landmark.worldZ - scene.originZ) * scale
    context.fillStyle = landmark.type === 'pagoda' ? '#d9b13a' : landmark.type === 'ferris_wheel' ? '#c03a2e' : landmark.type === 'boat' ? '#5a4a3e' : landmark.type === 'station' ? '#c8b99a' : '#5d8a4a'
    if (landmark.type === 'ferris_wheel') {
      context.beginPath(); context.arc(px, pz, 5, 0, Math.PI * 2); context.fill()
    } else {
      context.fillRect(px - 4, pz - 4, 8, 8)
    }
  }
}

// Debug view is a read-only projection of Rust's partition. No browser
// placement, clipping, nearest-parcel inference or realization decisions.
function drawOwnership(ox, oz, scale) {
  const plan = scene.developmentPlan
  if (!plan) { info.textContent = '当前场景没有 Rust 土地归属计划'; return }
  const colors = { road: '#71b9ce', building: '#edac67', yard: '#cab785', unplanned: '#263b42', managed_grass:'#719555', access_pavement:'#b2aaa0' }
  context.save()
  for (const cell of plan.cells) {
    const r = cell.boundary
    context.fillStyle = colors[cell.role] ?? '#bf4b52'
    context.fillRect(ox + (r.x - scene.originX) * scale, oz + (r.z - scene.originZ) * scale, r.widthM * scale, r.depthM * scale)
  }
  let surfaceTriangles=0
  for(const patch of scene.landcoverSurfaces?.patches ?? []) {
    context.fillStyle=({managed_grass:'#719555',access_pavement:'#b2aaa0',cultivated_soil:'#927046',field_headland:'#baaa70',farm_track:'#a59384'})[patch.material]??'#bf4b52'
    for(const triangle of patch.triangles) {
      context.beginPath()
      triangle.forEach((index,i)=>{const [x,,z]=patch.vertices[index];const px=ox+(x-scene.originX)*scale;const pz=oz+(z-scene.originZ)*scale;if(i===0)context.moveTo(px,pz);else context.lineTo(px,pz)})
      context.closePath();context.fill();surfaceTriangles+=1
    }
  }
  window.glyphweaveFeedback.visualChecks.ownershipSurfaceTriangles=surfaceTriangles
  context.strokeStyle = '#99b0aa'; context.lineWidth = .6
  for (const zone of scene.landUseZones) {
    context.strokeRect(ox + (zone.worldX - scene.originX) * scale, oz + (zone.worldZ - scene.originZ) * scale, zone.widthM * scale, zone.depthM * scale)
  }
  context.font = '13px system-ui'; context.textBaseline = 'middle'
  context.fillStyle = '#111c22'; context.fillRect(18, 16, 300, 194)
  const labels = [['road','道路：交叉区域只计一次'],['building','建筑：计划 footprint'],['yard','庭院：候选或待实现'],['unplanned','剩余：无开发／保护实施'],['managed_grass','庭院地表：实际 Rust 网格'],['access_pavement','步道：复用实际入口通路']]
  labels.forEach(([role,label],i) => {
    context.fillStyle = colors[role]; context.fillRect(28,30+i*28,14,14)
    context.fillStyle = '#e2e9e6'; context.fillText(label,52,37+i*28)
  })
  context.restore()
  const ledger = scene.landUseLedger
  if (ledger) {
    // Display Rust's numbers verbatim, never derive land use from visible props.
    context.save()
    context.fillStyle = '#111c22'; context.fillRect(18, 220, 620, 190)
    context.textBaseline = 'middle'; context.fillStyle = '#e2e9e6'; context.font = '13px system-ui'
    context.fillText(scene.renderStream ? '用地账本：全图；实际地表网格：仅当前已加载块' : '用地账本 · Rust bake 诊断（独立验收请运行 CLI）', 28, 238)
    const columns = [28, 126, 228, 360, 492]
    ;['用途', '目标比例', '规划 m²', '已实现 m²', '未实现 m²'].forEach((text, i) => context.fillText(text, columns[i], 267))
    ;[['urban', '城市'], ['rural', '农村'], ['nature', '自然'], ['unassigned', '未指定']].forEach(([kind, label], i) => {
      const row = ledger.byPurpose[kind]
      const values = [label, (row.targetRatio * 100).toFixed(2) + '%', row.plannedM2.toFixed(1), row.realizedM2.toFixed(1), row.unresolvedM2.toFixed(1)]
      values.forEach((text, j) => context.fillText(text, columns[j], 292 + i * 24))
    })
    context.fillStyle = '#c7b98b'; context.fillText('道路已计入所在用途；保护高度不变不等于生态实现。', 28, 391)
    if (scene.landcoverSurfaces?.ecology?.classes?.length) {
      context.fillStyle = '#a8c9b1'; context.fillText('自然地表分类：' + scene.landcoverSurfaces.ecology.classes.map(c => c.className + ' ' + c.areaM2.toFixed(0) + 'm²').join(' · '), 28, 410)
    }
    context.restore()
    window.glyphweaveFeedback.visualChecks.landUseLedgerDisplayed = true
    window.glyphweaveFeedback.landUseLedgerDiagnostic = ledger
  }
  const joint = scene.roadNetworkPlan?.jointLayout
  if (joint) {
    context.save(); context.fillStyle = '#111c22'; context.fillRect(18, 420, 620, 62)
    context.font = '13px system-ui'; context.textBaseline = 'middle'; context.fillStyle = '#e2e9e6'
    context.fillText('联合布局规划：住宅最低 ' + joint.residentialMinimum + '，原位保留 ' + joint.retainedResidentialIds.length + '，替代选址 ' + joint.replacementResidentialIds.length + '，未补足 ' + joint.unfilledResidential, 28, 440)
    context.fillStyle = '#c7b98b'; context.fillText('候选调整发生在生成前；实际住宅与入口仍需 CLI 独立验收。', 28, 465)
    context.restore()
    window.glyphweaveFeedback.visualChecks.jointLayoutDisplayed = true
  }
  info.textContent = 'Rust 土地归属诊断 · 规划不等于完成 · 双击进入近景'
  window.glyphweaveFeedback.visualChecks.ownershipRendered = true
  window.glyphweaveFeedback.visualChecks.ownershipCells = plan.cells.length
}

function drawStrategic() {
  const scale = Math.min(canvas.clientWidth / scene.widthM, canvas.clientHeight / scene.depthM)
  const ox = (canvas.clientWidth - scene.widthM * scale) / 2
  const oz = (canvas.clientHeight - scene.depthM * scale) / 2
  const background = context.createLinearGradient(0, 0, 0, canvas.clientHeight)
  background.addColorStop(0, '#152a38'); background.addColorStop(.72, '#23332f'); background.addColorStop(1, '#111816')
  context.fillStyle = background; context.fillRect(0, 0, canvas.clientWidth, canvas.clientHeight)
  if (urlView === 'ownership') { drawOwnership(ox, oz, scale); return }
  let loadedChunks = 0
  for (const chunk of scene.chunks) {
    fetch(`../scenes/${scene.sceneId}/${chunk.surfaceFile}`).then(response => response.arrayBuffer()).then(bytes => {
      const data = new Uint8Array(bytes)
      const step = scene.widthM * scene.depthM > 2_000_000 ? 16 : 8
      for (let z = 0; z < chunk.validDepthM; z += step) for (let x = 0; x < chunk.validWidthM; x += step) {
        const sampleX = Math.min(chunk.validWidthM - 1, x + Math.floor(step / 2))
        const sampleZ = Math.min(chunk.validDepthM - 1, z + Math.floor(step / 2))
        const surface = data[sampleZ * chunk.validWidthM + sampleX] ?? 0
        const cellWidth = Math.min(step, chunk.validWidthM - x)
        const cellDepth = Math.min(step, chunk.validDepthM - z)
        context.fillStyle = color(surface)
        context.fillRect(ox + (chunk.worldX - scene.originX + x) * scale, oz + (chunk.worldZ - scene.originZ + z) * scale, cellWidth * scale + .5, cellDepth * scale + .5)
      }
      loadedChunks += 1
      if (loadedChunks === scene.chunks.length) drawStrategicOverlays(ox, oz, scale)
    })
  }
  drawStrategicOverlays(ox, oz, scale)
}

async function drawNear(buildToken) {
  const hiddenRenderLayers = new Set((new URLSearchParams(location.search).get('hide') ?? '').split(',').filter(Boolean))
  const THREE = await import('three')
  const { GLTFLoader } = await import('three/addons/loaders/GLTFLoader.js')
  const { OrbitControls } = await import('three/addons/controls/OrbitControls.js')
  const { OBJLoader } = await import('three/addons/loaders/OBJLoader.js')
  // Post-processing is opt-in in the static preview. Loading six addon modules
  // plus extra render targets during the first 2.5D frame created an avoidable
  // memory spike on integrated GPUs. Use ?fx=1 for an explicit cinematic test.
  const query = new URLSearchParams(location.search)
  const enableFx = query.get('fx') === '1' && !query.has('nofx')
  const postAddons = enableFx ? await Promise.all([
    import('three/addons/postprocessing/EffectComposer.js'),
    import('three/addons/postprocessing/RenderPass.js'),
    import('three/addons/postprocessing/BokehPass.js'),
    import('three/addons/postprocessing/UnrealBloomPass.js'),
    import('three/addons/postprocessing/ShaderPass.js'),
    import('three/addons/postprocessing/OutputPass.js'),
  ]).then(modules => ({ EffectComposer: modules[0].EffectComposer, RenderPass: modules[1].RenderPass, BokehPass: modules[2].BokehPass, UnrealBloomPass: modules[3].UnrealBloomPass, ShaderPass: modules[4].ShaderPass, OutputPass: modules[5].OutputPass })) : null
  if (buildToken !== nearBuildToken) return
  releasePreviewFrame()
  const viewport = document.querySelector('#viewport')
  const viewportWidth = viewport.clientWidth
  const viewportHeight = viewport.clientHeight
  canvas.style.display = 'none'
  webglCanvas ??= document.createElement('canvas')
  webglCanvas.style.width = '100%'
  webglCanvas.style.height = '100%'
  webglCanvas.style.display = 'block'
  if (!webglCanvas.dataset.contextGuard) {
    webglCanvas.dataset.contextGuard = '1'
    webglCanvas.addEventListener('webglcontextlost', event => {
      event.preventDefault()
      info.textContent = '2.5D 渲染上下文被系统回收，请重新加载页面'
    })
  }
  document.querySelector('#viewport').append(webglCanvas)
  if (nearControls) { nearControls.dispose(); nearControls = null }
  if (exploreInputCleanup) { exploreInputCleanup(); exploreInputCleanup = null }
  if (!nearRenderer) {
    nearRenderer = new THREE.WebGLRenderer({ canvas: webglCanvas, antialias: !STATIC_PREVIEW })
    nearRenderer.setSize(viewportWidth, viewportHeight, false)
    nearRenderer.setPixelRatio(STATIC_PREVIEW ? 1 : Math.min(devicePixelRatio, 2))
    nearRenderer.shadowMap.enabled = !STATIC_PREVIEW
    nearRenderer.shadowMap.type = THREE.PCFSoftShadowMap
    nearRenderer.outputColorSpace = THREE.SRGBColorSpace
    nearRenderer.toneMapping = THREE.ACESFilmicToneMapping
    nearRenderer.toneMappingExposure = 1.15
    nearRenderer.setClearColor('#0c100e')
  } else {
    nearRenderer.setAnimationLoop(null)
    nearRenderer.setSize(viewportWidth, viewportHeight, false)
  }
  const focusLandmark = (scene.landmarks ?? []).filter(landmark => landmark.type === 'building_tower').sort((a, b) => (b.heightM || 0) - (a.heightM || 0))[0]
  const isExplore = nearPreset === 'explore'
  const isSkyline = nearPreset === 'skyline'
  const isStreet = nearPreset === 'street'
  const isWaterfront = nearPreset === 'waterfront'
  const isStreetView = isStreet || isExplore
  const frontageEntity=nearPreset==='frontage' ? scene.entities.find(e=>e.grounding?.frontageRoadPoint) : null
  const gradedRoad=nearPreset==='graded-road' ? scene.entities.find(e=>e.grounding?.roadProfile) : null
  const serviceId=(scene.roadNetworkPlan?.roadsideSites?.find(s=>s.program?.kind==='school') ?? scene.roadNetworkPlan?.roadsideSites?.find(s=>s.program))?.id
  const serviceEntity=nearPreset==='services' ? scene.entities.find(e=>e.entityId===serviceId) : null
  const yardBuildingId=scene.developmentPlan?.buildings?.[0]?.entity?.entityId
  const yardEntity=nearPreset==='yard' ? scene.entities.find(e=>e.entityId===yardBuildingId) : null
  const farmFields = scene.landcoverSurfaces?.agriculture?.fields ?? []
  const requestedFarm = query.get('field')
  const farmField = nearPreset === 'farm' ? (requestedFarm ? farmFields.find(f => f.id === requestedFarm) : farmFields[0]) : null
  if (nearPreset === 'farm' && requestedFarm && !farmField) throw new Error('Unknown Rust farm field: ' + requestedFarm)
  const farmPatch=farmField ? scene.landcoverSurfaces.patches.find(p=>p.parcelId===farmField.id&&p.material==='cultivated_soil') : null
  const focusOverride = window.__nearFocusOverride ?? (frontageEntity ? {x:frontageEntity.worldX-scene.originX,z:frontageEntity.worldZ-scene.originZ,y:frontageEntity.worldY+(frontageEntity.grounding.bottomOffsetM??0)} : gradedRoad ? {x:gradedRoad.worldX-scene.originX,z:gradedRoad.worldZ-scene.originZ,y:gradedRoad.worldY+(gradedRoad.grounding.bottomOffsetM??0)} : serviceEntity ? {x:serviceEntity.worldX-scene.originX,z:serviceEntity.worldZ-scene.originZ,y:serviceEntity.worldY} : yardEntity ? {x:yardEntity.worldX-scene.originX,z:yardEntity.worldZ-scene.originZ,y:yardEntity.worldY} : farmField ? {x:farmField.boundary.x+farmField.boundary.widthM/2-scene.originX,z:farmField.boundary.z+farmField.boundary.depthM/2-scene.originZ,y:farmPatch?.vertices[0]?.[1]??scene.landcoverSurfaces.agriculture.previewFocusHeights?.[farmField.id]??0} : null)
  const hasBridge = (scene.entities ?? []).some(entity => entity.kind === 'bridge') || (scene.landmarks ?? []).some(landmark => landmark.type === 'bridge')
  const hasUrbanFabric = (scene.entities ?? []).some(entity => ['storefront', 'residential_block', 'residential_tower', 'building_tower', 'urban_building', 'resort_lodge', 'road', 'bridge'].includes(entity.kind)) || (scene.landmarks ?? []).some(entity => ['bridge', 'shopping_mall', 'office_tower', 'convenience_store', 'boardwalk', 'scenic_belt', 'community_plaza', 'night_market'].includes(entity.type))
  const naturalFeature = !hasUrbanFabric
    ? (scene.landmarks ?? []).find(landmark => ['lake', 'river', 'sea'].includes(landmark.type))
    : null
  const primaryRoute = scene.routeEvolutionPlan?.routes
    ?.find(route => route.status === 'pass' && route.polyline?.length >= 2)
  const routeFocus = primaryRoute
    ? primaryRoute.polyline[Math.floor(primaryRoute.polyline.length / 2)]
    : null
  // Harbour's default 2.5D entry is the bridge inspection view. Only fall
  // back to a wide overview when a scene has no bridge landmark to focus.
  const isHarbourOverview = nearPreset === 'harbour' && !focusOverride
  // Focus / 2.5d near view uses a street-block scale (~120m) so doors,
  // windows, street lamps and lane markings are readable inside a single
  // 512m chunk. Skyline stays full-world; street/explore are perspective.
  const viewportAspect = viewportWidth / Math.max(1, viewportHeight)
  const wholeWorldHeight = Math.max(scene.widthM, scene.depthM, scene.widthM / Math.max(.2, viewportAspect) * 1.12)
  const orthoHeight = isSkyline ? wholeWorldHeight : isWaterfront ? 520 : isHarbourOverview ? Math.max(Math.min(820, Math.max(scene.widthM, scene.depthM) * .36), wholeWorldHeight * .82) : isStreet ? 220 : 120
  const orthoWidth = orthoHeight * viewportWidth / Math.max(1, viewportHeight)
  nearCamera = isExplore || isStreet
    ? new THREE.PerspectiveCamera(isStreet ? 54 : 68, viewportWidth / viewportHeight, .1, 12000)
    : new THREE.OrthographicCamera(-orthoWidth / 2, orthoWidth / 2, orthoHeight / 2, -orthoHeight / 2, -orthoHeight, 12000)
  const localCoordinate = entity => ({ x: entity.worldX - scene.originX, z: entity.worldZ - scene.originZ })
  const focusEntity = (scene.entities ?? []).find(entity => {
    const local = localCoordinate(entity)
    return entity.kind === 'commercial_center' && local.x > 64 && local.x < scene.widthM - 64 && local.z > 64 && local.z < scene.depthM - 64
  }) ?? (scene.entities ?? []).find(entity => {
    const local = localCoordinate(entity)
    return entity.kind === 'school' && local.x > 64 && local.x < scene.widthM - 64 && local.z > 64 && local.z < scene.depthM - 64
  }) ?? (scene.entities ?? []).find(entity => {
    const local = localCoordinate(entity)
    return entity.kind === 'residential_block' && local.x > 64 && local.x < scene.widthM - 64 && local.z > 64 && local.z < scene.depthM - 64
  }) ?? (scene.entities ?? []).find(entity => {
    const local = localCoordinate(entity)
    return entity.kind === 'building' && local.x > 64 && local.x < scene.widthM - 64 && local.z > 64 && local.z < scene.depthM - 64
  }) ?? (scene.entities ?? []).find(entity => entity.kind === 'bridge')
    ?? (scene.entities ?? []).find(entity => ['tree', 'bush', 'bench'].includes(entity.kind))
  const bridgeLength = focusEntity?.kind === 'bridge' ? (focusEntity.widthM || 4588) : 0
  const harbour = (scene.landmarks ?? []).find(landmark => landmark.type === 'river')
  const harbourBridge = (scene.landmarks ?? []).find(landmark => landmark.type === 'bridge') ?? (scene.entities ?? []).find(entity => entity.kind === 'bridge')
  const centreDist = e => Math.hypot(e.worldX - (scene.originX + scene.widthM / 2), e.worldZ - (scene.originZ + scene.depthM / 2))
  const streetFabricKinds = new Set(['residential_home', 'residential_block', 'residential_tower', 'storefront', 'market', 'urban_building', 'office_tower', 'community_plaza'])
  const streetFabric = (scene.entities ?? []).filter(entity => streetFabricKinds.has(entity.kind))
  const streetRoads = (scene.entities ?? []).filter(entity => entity.kind === 'road')
  const streetFabricCentre = streetFabric.length ? {
    x: streetFabric.reduce((sum, entity) => sum + entity.worldX, 0) / streetFabric.length,
    z: streetFabric.reduce((sum, entity) => sum + entity.worldZ, 0) / streetFabric.length,
    y: streetFabric.reduce((sum, entity) => sum + (entity.worldY || 0), 0) / streetFabric.length,
  } : null
  // A single global centroid of every storefront and home lands in the open
  // field between separated settlements. Street view instead chooses the
  // densest local commercial fabric and a real road through it.
  const streetWeight = entity => entity.kind === 'market' ? 6
    : entity.kind === 'storefront' ? 5
      : entity.kind === 'community_plaza' ? 4
        : entity.kind === 'residential_block' ? 2.5
          : entity.kind === 'residential_home' || entity.kind === 'residential_tower' ? 2
            : 1.5
  const nearestStreetRoad = candidate => streetRoads.reduce((best, road) => {
    const d = Math.hypot(road.worldX - candidate.worldX, road.worldZ - candidate.worldZ)
    return !best || d < best.distance ? { road, distance: d } : best
  }, null)
  const streetHotspot = streetFabric.map(candidate => {
    let score = streetWeight(candidate)
    for (const other of streetFabric) {
      if (other === candidate) continue
      const d = Math.hypot(other.worldX - candidate.worldX, other.worldZ - candidate.worldZ)
      if (d <= 90) score += streetWeight(other) * (1 - d / 110)
    }
    const road = nearestStreetRoad(candidate)
    if (road && road.distance <= 46) score += 8 * (1 - road.distance / 52)
    score -= centreDist(candidate) * .0008
    return { entity: candidate, road, score }
  }).sort((a, b) => b.score - a.score || a.entity.entityId.localeCompare(b.entity.entityId))[0] ?? null
  const streetAnchor = (scene.entities ?? []).filter(entity => entity.kind === 'storefront').sort((a, b) => centreDist(a) - centreDist(b))[0]
    ?? (scene.entities ?? []).filter(entity => entity.kind === 'street_lamp').sort((a, b) => centreDist(a) - centreDist(b))[0]
    ?? (scene.entities ?? []).filter(entity => entity.kind === 'road').sort((a, b) => centreDist(a) - centreDist(b))[0]
  const streetX = (streetHotspot?.entity.worldX ?? streetFabricCentre?.x ?? streetAnchor?.worldX ?? (harbour ? harbour.worldX + harbour.widthM / 2 + 150 : scene.widthM * .68)) - scene.originX
  const streetZ = (streetHotspot?.entity.worldZ ?? streetFabricCentre?.z ?? streetAnchor?.worldZ ?? scene.depthM * .5) - scene.originZ
  const streetYaw = ((streetHotspot?.road?.rotationYDeg ?? 0) * Math.PI) / 180
  const streetForward = { x: Math.cos(streetYaw), z: Math.sin(streetYaw) }
  const streetTargetX = streetX + streetForward.x * 58
  const streetTargetZ = streetZ + streetForward.z * 58
  const focusX = focusOverride ? focusOverride.x : routeFocus ? routeFocus.x - scene.originX : naturalFeature ? naturalFeature.worldX - scene.originX : isSkyline ? scene.widthM * .5 : isWaterfront && harbour ? harbour.worldX - harbour.widthM * .5 - 60 - scene.originX : isHarbourOverview && harbourBridge ? harbourBridge.worldX - scene.originX : isHarbourOverview ? scene.widthM * .5 : isStreetView ? streetX : focusLandmark ? focusLandmark.worldX - scene.originX : (focusEntity ? focusEntity.worldX - scene.originX + (nearPreset === 'bridgehead' ? bridgeLength / 2 - 70 : 0) : scene.widthM * .5)
  const focusZ = focusOverride ? focusOverride.z : routeFocus ? routeFocus.z - scene.originZ : naturalFeature ? naturalFeature.worldZ - scene.originZ : isSkyline ? scene.depthM * .5 : (isWaterfront || isHarbourOverview) && harbourBridge ? harbourBridge.worldZ - scene.originZ : isWaterfront && harbour ? harbour.worldZ - scene.originZ : isHarbourOverview ? scene.depthM * .48 : isStreetView ? streetZ : focusLandmark ? focusLandmark.worldZ - scene.originZ : (focusEntity ? focusEntity.worldZ - scene.originZ : scene.depthM * .5)
  const focusY = Number.isFinite(focusOverride?.y) ? focusOverride.y : isSkyline || isExplore ? 0 : isStreet ? (streetFabricCentre?.y ?? 0) : isHarbourOverview ? 4 : routeFocus ? routeFocus.designHeightM : naturalFeature ? naturalFeature.worldY : focusLandmark ? focusLandmark.worldY : focusEntity ? focusEntity.worldY : 0
  const isNaturalExplore = isExplore && Boolean(naturalFeature || routeFocus)
  const isBridgeFocus = nearPreset === 'bridgehead' && focusEntity?.kind === 'bridge'
  const isLandmarkFocus = Boolean(focusLandmark) && !isSkyline && !isStreetView
  const focusTargetY = isBridgeFocus ? focusY + 10 : isLandmarkFocus ? focusY + (focusLandmark.heightM || 100) * .45 : isHarbourOverview ? 10 : focusY + 6
  const focusTargetZ = focusZ
  const viewOffsetX = nearPreset === 'bridgehead' ? 100 : 0
  const viewOffsetZ = nearPreset === 'bridgehead' ? 105 : 72
  const landmarkViewDistance = Math.max(180, (focusLandmark?.heightM || 100) * 1.15)
  nearCamera.position.set(
    focusX + viewOffsetX + (isNaturalExplore ? 190 : isExplore ? 130 : isStreet ? 105 : isLandmarkFocus ? orthoHeight * .72 : isSkyline ? orthoHeight * .3 : isHarbourOverview ? orthoHeight * .46 : orthoHeight * .5),
    focusY + (isNaturalExplore ? 42 : isExplore ? 16 : isStreet ? 44 : isBridgeFocus ? 34 : isLandmarkFocus ? (focusLandmark.heightM || 100) * 1.08 : isSkyline ? orthoHeight * .64 : isHarbourOverview ? orthoHeight * .56 : orthoHeight * .95) + (isLandmarkFocus && !isExplore ? orthoHeight * .2 : 0),
    focusZ + (isNaturalExplore ? 210 : isExplore ? 125 : isStreet ? 150 : isBridgeFocus ? viewOffsetZ : isLandmarkFocus ? landmarkViewDistance : isSkyline ? orthoHeight * .40 : isHarbourOverview ? -orthoHeight * .18 : orthoHeight * .42),
  )
  // Harbour overview is an orientation shot, not a bridge close-up. Look
  // obliquely across the bay so the bridge, water and both shores can read in
  // one frame; bridgehead remains the detailed inspection preset.
  if (isHarbourOverview) {
    nearCamera.position.set(focusX + Math.max(420, orthoHeight * .72), focusY + Math.max(260, orthoHeight * .62), focusZ + Math.max(300, orthoHeight * .48))
    nearCamera.lookAt(focusX, focusY + 4, focusZ)
  }
  // The waterfront mechanism view looks from the inland city toward the sea,
  // so the public waterfront and the urban fabric are visible in one frame.
  if (isWaterfront) {
    nearCamera.position.set(focusX - Math.max(420, orthoHeight * .72), focusY + Math.max(260, orthoHeight * .62), focusZ + Math.max(300, orthoHeight * .48))
    nearCamera.lookAt(focusX, focusY + 4, focusZ)
  }
  // Street view is a road-axis shot. Looking along the selected frontage road
  // puts pavement, storefronts and street furniture in the same frame instead
  // of aiming at a building centroid across an empty parcel.
  if (isStreet) {
    nearCamera.position.set(
      focusX - streetForward.x * 108 - streetForward.z * 12,
      focusY + 34,
      focusZ - streetForward.z * 108 + streetForward.x * 12,
    )
    nearCamera.lookAt(streetTargetX, focusY + 8, streetTargetZ)
  }
  if (nearScene) {
    // Persistent scene: remove the old chunk root so we can rebuild it, but
    // keep the scene, lights and sky so navigation does not recreate them.
    if (nearGroupRoot) nearScene.remove(nearGroupRoot)
  } else {
    nearScene = new THREE.Scene()
    window.__nearScene = nearScene
    // Natural / artistic scenes (pure steppe, no urban fabric) get a warm,
    // painterly look inspired by Journey / Flower: a soft warm key light, a
    // gentle distance haze, and no hard shadows. Urban scenes keep the cool
    // clinical look.
    const steppe = !hasUrbanFabric
    window.__steppe = steppe
    nearScene.background = makeSkyTexture(THREE)
    if (steppe) {
      // Natural daylight: cool-ish white sun + pale sky, so the grass-green
      // vertex colour reads true instead of being tinted yellow by warm fog.
      // The fog is a soft grass-green (not straw-yellow) so distant ground
      // stays green and blends toward the sky rather than turning tan.
      nearScene.fog = new THREE.Fog(0xa8c48a, 3500, 8000)
      nearScene.add(new THREE.HemisphereLight(0xeaf4ff, 0x5a8a4a, .85))
      const sun = new THREE.DirectionalLight(0xffffff, .95)
      sun.position.set(-300, 600, 200)
      sun.castShadow = false
      nearScene.add(sun)
      nearScene.add(new THREE.AmbientLight(0xffffff, .28))
    } else {
      nearScene.fog = new THREE.Fog('#294658', 900, 3600)
      // A 2.8 directional key stacked on a 2.0 hemisphere light drove the
      // already-bright ground palette several stops over white. The resulting
      // clip looked like irregular holes in the terrain. Keep urban scenes
      // cool and legible, but leave headroom for the baked vertex colours.
      nearScene.add(new THREE.HemisphereLight(0xdce8df, 0x202820, .35))
      const sun = new THREE.DirectionalLight(0xffe6b0, .72)
      sun.position.set(-500, 900, 420)
      sun.castShadow = false
      nearScene.add(sun)
      nearScene.add(new THREE.AmbientLight(0xffffff, .12))
    }
  }
  nearControls = new OrbitControls(nearCamera, webglCanvas)
  nearControls.target.set(isStreet ? streetTargetX : focusX, isNaturalExplore ? focusY + 2 : isExplore ? focusY + 26 : isStreet ? focusY + 8 : focusTargetY, isStreet ? streetTargetZ : focusTargetZ)
  nearControls.enableDamping = true
  nearControls.enablePan = true
  nearControls.enableRotate = true
  nearControls.mouseButtons.LEFT = isExplore || isStreet ? THREE.MOUSE.ROTATE : THREE.MOUSE.PAN
  nearControls.mouseButtons.MIDDLE = THREE.MOUSE.PAN
  nearControls.mouseButtons.RIGHT = isExplore ? THREE.MOUSE.PAN : THREE.MOUSE.ROTATE
  nearControls.minDistance = 10
  nearControls.maxDistance = 2400
  nearControls.maxPolarAngle = isExplore ? Math.PI * .8 : Math.PI * .48
  // The constructor looked at its initial (0,0,0) target. Static previews
  // have no animation loop to apply our new target: sync once BEFORE the
  // change listener, never recursively inside renderNearFrame.
  nearControls.update()
  nearControls.addEventListener('change', () => {
    updateNearCoordinateInfo()
    if (STATIC_PREVIEW) renderNearFrame?.()
  })
  window.__nearControls = nearControls
  window.__nearCamera = nearCamera
  window.__focusAt = (x, z) => { window.__nearFocusOverride = { x, z }; nearPreset = 'focus'; mode = 'near'; draw(); return nearBuildToken }
  window.__chunkNav = (dx, dz) => {
    const focusChunk = scene.chunks.find(chunk => focusX >= chunk.worldX - scene.originX && focusX < chunk.worldX - scene.originX + chunk.validWidthM && focusZ >= chunk.worldZ - scene.originZ && focusZ < chunk.worldZ - scene.originZ + chunk.validDepthM)
    if (!focusChunk) return
    const nextX = focusChunk.chunkX + dx
    const nextZ = focusChunk.chunkZ + dz
    const target = scene.chunks.find(chunk => chunk.chunkX === nextX && chunk.chunkZ === nextZ)
    if (!target) return
    window.__nearFocusOverride = { x: target.worldX - scene.originX + target.validWidthM / 2, z: target.worldZ - scene.originZ + target.validDepthM / 2 }
    nearPreset = 'focus'; mode = 'near'; draw()
  }
  webglCanvas.addEventListener('pointerdown', event => {
    if (event.button === 0 && event.altKey) nearControls.mouseButtons.LEFT = THREE.MOUSE.ROTATE
  }, true)
  webglCanvas.addEventListener('pointerup', event => {
    if (event.button === 0) nearControls.mouseButtons.LEFT = THREE.MOUSE.PAN
  }, true)
  webglCanvas.addEventListener('pointercancel', () => { nearControls.mouseButtons.LEFT = THREE.MOUSE.PAN }, true)
  let edgePointer = null
  webglCanvas.addEventListener('pointermove', event => {
    const rect = webglCanvas.getBoundingClientRect()
    edgePointer = { x: event.clientX - rect.left, y: event.clientY - rect.top, width: rect.width, height: rect.height }
  })
  webglCanvas.addEventListener('pointerleave', () => { edgePointer = null })
  const onKeyDown = event => {
    if (isExplore && ['KeyW', 'KeyA', 'KeyS', 'KeyD', 'ShiftLeft'].includes(event.code)) { exploreKeys.add(event.code); event.preventDefault() }
    if (!isExplore && ['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(event.code) && window.__chunkNav) {
      const map = { ArrowUp: [0, 1], ArrowDown: [0, -1], ArrowLeft: [-1, 0], ArrowRight: [1, 0] }
      window.__chunkNav(...map[event.code]); event.preventDefault()
    }
  }
  const onKeyUp = event => { exploreKeys.delete(event.code) }
  window.addEventListener('keydown', onKeyDown)
  window.addEventListener('keyup', onKeyUp)
  exploreInputCleanup = () => {
    window.removeEventListener('keydown', onKeyDown)
    window.removeEventListener('keyup', onKeyUp)
    exploreKeys.clear()
  }
  const group = new THREE.Group()
  if (!nearGroupRoot) { nearGroupRoot = new THREE.Group() }
  // Terrain meshes are emitted in scene-local coordinates while entities keep
  // authoritative world coordinates. Translate the shared root once so custom
  // scene origins do not shift buildings and roads away from the terrain.
  nearGroupRoot.position.set(-scene.originX, 0, -scene.originZ)
  if (!nearGroupRoot.parent) nearScene.add(nearGroupRoot)
  // Old transient groups must not accumulate across camera/scene rebuilds.
  nearGroupRoot.clear()
  nearGroupRoot.add(group)
  const focusChunk = scene.chunks.find(chunk => focusX >= chunk.worldX - scene.originX && focusX < chunk.worldX - scene.originX + chunk.validWidthM && focusZ >= chunk.worldZ - scene.originZ && focusZ < chunk.worldZ - scene.originZ + chunk.validDepthM)
  // Static 2.5D renders the authoritative focus chunk only. A 3x3 ring
  // multiplies 1m terrain geometry, PBR maps, vegetation instances and entity
  // groups before the first frame, which can make Chromium lose WebGL context.
  // Opt into ring streaming explicitly with ?ring=1.
  const streamRing = query.get('ring') === '1'
  const activeChunks = focusChunk
        ? (streamRing || isSkyline || isHarbourOverview || isWaterfront
        ? scene.chunks.filter(chunk => Math.abs(chunk.chunkX - focusChunk.chunkX) <= 1 && Math.abs(chunk.chunkZ - focusChunk.chunkZ) <= 1)
        : [focusChunk])
    : scene.chunks.slice(0, 1)
  if (renderStream) {
    const loader = renderStream
    const loaded = await loader.load(activeChunks)
    if (buildToken !== nearBuildToken || loader !== renderStream) return
    scene.landcoverSurfaces = loaded.surfaces
    window.glyphweaveFeedback.visualChecks.renderStream = loaded.stats
    // Release only retired landcover meshes here. Other legacy terrain/entity
    // caches are separate and are not claimed to be bounded by this change.
    const activeIds = new Set((loaded.surfaces?.patches ?? []).map(p => 'landcover:' + p.id))
    for (const [key, mesh] of entityMeshCache) if (key.startsWith('landcover:') && !activeIds.has(key)) {
      mesh.removeFromParent(); mesh.geometry.dispose(); mesh.material.dispose(); entityMeshCache.delete(key)
    }
  }
  // Cache each active chunk's terrain mesh so navigating back is instant.
  const chunkGroupMap = new Map()
  for (const chunk of activeChunks) {
    if (buildToken !== nearBuildToken) return
    const cacheKey = `${scene.sceneId}/${chunk.chunkX}/${chunk.chunkZ}`
    let chunkGroup = entityMeshCache.get(`chunkgroup:${cacheKey}`)
    if (!chunkGroup) {
      chunkGroup = new THREE.Group()
      entityMeshCache.set(`chunkgroup:${cacheKey}`, chunkGroup)
    }
    chunkGroupMap.set(cacheKey, chunkGroup)
    group.add(chunkGroup)
  }
  const inFocusChunk = entity => focusChunk && entity.worldX >= focusChunk.worldX && entity.worldX < focusChunk.worldX + focusChunk.validWidthM && entity.worldZ >= focusChunk.worldZ && entity.worldZ < focusChunk.worldZ + focusChunk.validDepthM
  const inStreetFrame = entity => isStreetView && Math.hypot(entity.worldX - focusX, entity.worldZ - focusZ) < (isStreet ? 220 : 360)
  // LOD: focus chunk renders full detail; the 3x3 ring around it renders
  // coarse detail (fewer vegetation, simpler proxies) so chunk navigation
  // stays fast while the viewport is dominated by the focused chunk.
  const lodLevel = entity => {
    if (isSkyline || isStreetView) return 0
    if (inFocusChunk(entity)) return 0
    const d = Math.hypot(entity.worldX - focusX, entity.worldZ - focusZ)
    return d < 700 ? 1 : 2
  }
  const detailRoll = (entity, salt) => {
    const h = (Math.abs(entity.worldX * 31 + entity.worldZ * 17) ^ (salt * 2654435761)) >>> 0
    return h / 4294967296
  }
  const shouldRenderVegetation = (entity, salt) => {
    const level = lodLevel(entity)
    if (level === 0) return detailRoll(entity, salt) < .7
    if (level === 1) return detailRoll(entity, salt) < .45
    return detailRoll(entity, salt) < .2
  }
  // Cache a built entity group so navigating back to a chunk reuses the exact
  // mesh instead of rebuilding hundreds of THREE groups.
  const buildEntityCached = (entity, builder) => {
    let built = entityGroupCache.get(entity.entityId)
    if (!built) {
      built = builder()
      // Procedural building helpers keep their geometry in an entity-local
      // frame. Apply the serialized yaw at this boundary so the visible
      // footprint stays aligned with worldgen and the rules engine.
      if (built && !built.userData?.authoredWorldAxes && Number.isFinite(entity.rotationYDeg)) {
        built.rotation.y = THREE.MathUtils.degToRad(entity.rotationYDeg)
      }
      if (built && entity.grounding?.frontageRoadPoint) {
        const g=entity.grounding
        built.position.y+=(g.bottomOffsetM??0)+(g.roadbedOffsetM??0)-(g.pivot==='center'?entity.heightM*.5:0)
        built.userData.sourceEntityId=entity.entityId
      }
      entityGroupCache.set(entity.entityId, built)
    }
    return built
  }
  const addRendered = (entity, object) => {
    if (!object) return null
    if (entity?.entityId && object.userData) object.userData.sourceEntityId = entity.entityId
    group.add(object)
    return object
  }
  const addEntityToGroup = (entity, built) => {
    const cohort = entity.grounding?.constructionCohort
    const cohortKinds = new Set(['storefront', 'residential_block', 'residential_tower', 'urban_building', 'office_tower', 'market'])
    if (built && cohort && cohortKinds.has(entity.kind) && !built.userData.constructionCohortBand) {
      const cohortColors = {
        'old-core': '#8f5b46',
        renewal: '#a98a58',
        'new-growth': '#6f8791',
        periphery: '#7e8a69',
      }
      const plinth = new THREE.Mesh(
        new THREE.BoxGeometry((entity.widthM ?? 12) + .6, .55, (entity.depthM ?? 12) + .6),
        new THREE.MeshStandardMaterial({
          color: cohortColors[cohort] ?? '#80786b',
          roughness: .92,
          metalness: .02,
        }),
      )
      plinth.position.y = .275
      plinth.userData.constructionCohortBand = true
      built.add(plinth)
      built.userData.constructionCohortBand = true
    }
    if (built?.userData) built.userData.constructionCohort = cohort ?? null
    addRendered(entity, built)
  }
  const heightFields = []
  const waterTextures = []
  const animatedActors = []
  const animatedWater = []
  const sceneCacheKey = scene.sceneId
  const urbanScene = (scene.entities ?? []).some(entity =>
    ['residential_home', 'residential_block', 'residential_tower', 'storefront',
      'commercial_center', 'office_tower', 'town_hall', 'road'].includes(entity.kind))
  for (const chunk of activeChunks) {
    const cacheKey = `${sceneCacheKey}/${chunk.chunkX}/${chunk.chunkZ}`
    const chunkGroup = chunkGroupMap.get(cacheKey)
    const alreadyBuilt = chunkGroup.children.length > 0
    let heightView, surface, terrain
    const cached = chunkDataCache.get(cacheKey)
    if (cached) {
      heightView = cached.heightView
      surface = cached.surface
      terrain = cached.terrain
    } else {
      const [heightResponse, surfaceResponse, terrainResponse] = await Promise.all([
        fetch(`../scenes/${scene.sceneId}/${chunk.heightFile}`),
        fetch(`../scenes/${scene.sceneId}/${chunk.surfaceFile}`),
        chunk.terrainFile ? fetch(`../scenes/${scene.sceneId}/${chunk.terrainFile}`) : Promise.resolve(null),
      ])
      if (!heightResponse.ok || !surfaceResponse.ok || (terrainResponse && !terrainResponse.ok)) throw new Error('Terrain chunk HTTP failure')
      heightView = decodeHeightPayload(await heightResponse.arrayBuffer(),chunk.validWidthM,chunk.validDepthM)
      const expectedEncoding=world.sidecar?.heightEncoding==='gwhm-v1-i32le-millimeter'?'millimeter':'quarter-meter'
      if(heightView.encoding!==expectedEncoding)throw new Error('Height encoding disagrees with world sidecar')
      surface = new Uint8Array(await surfaceResponse.arrayBuffer())
      terrain = terrainResponse ? new Uint8Array(await terrainResponse.arrayBuffer()) : null
      if (buildToken !== nearBuildToken) return
      chunkDataCache.set(cacheKey, { heightView, surface, terrain })
    }
    const width = chunk.validWidthM; const depth = chunk.validDepthM
    heightFields.push({ chunk, heightView, surface, terrain, width, depth })
    // Only the focus chunk renders at 1m terrain detail; its immediate
    // neighbours use a finer 2m mesh (with a matching finer texture) so the
    // ground stays readable when panning. The outer ring keeps a coarse 8m
    // mesh so navigation doesn't rebuild huge geometries.
    const isFocusChunk = focusChunk && chunk.chunkX === focusChunk.chunkX && chunk.chunkZ === focusChunk.chunkZ
    const chunkDist = focusChunk ? Math.max(Math.abs(chunk.chunkX - focusChunk.chunkX), Math.abs(chunk.chunkZ - focusChunk.chunkZ)) : 9
    const meshStep = isFocusChunk ? (isHarbourOverview ? 4 : 1) : chunkDist <= 1 ? (isHarbourOverview ? 8 : 2) : 8
    const builtStep = chunkGroup.userData.meshStep ?? 0
    if (alreadyBuilt && builtStep === meshStep) continue
    // Remove any prior-step mesh before adding the correct one.
    for (let i = chunkGroup.children.length - 1; i >= 0; i--) {
      if (chunkGroup.children[i].isMesh) chunkGroup.remove(chunkGroup.children[i])
    }
    const meshCacheKey = `${cacheKey}/step${meshStep}`
    let mesh = entityMeshCache.get(meshCacheKey)
    if (!mesh) {
      // All themes use the same semantic terrain geometry and PBR response.
      // The previous urban branch used a dense legacy noise texture, hiding the
      // new grass/terrain look and making surface overlays hard to read.
      {
        const geometry = makeMCTerrain(heightView, surface, terrain, width, depth, meshStep, THREE, Number.isFinite(world.waterLevelM) ? world.waterLevelM : 0)
        // Use the tuned grass-ground shader on land-only chunks. Chunks that
        // contain water keep a plain vertex-colour PBR material so the water
        // datum remains blue and is not tinted by moss/grass grading.
        const hasWater = surface.some(value => value === 3)
        // Ground is a deterministic projection of the baked vertex colours.
        // The previous custom steppe shader compiled inconsistently across
        // Three.js revisions and left land-only chunks grey/white. Keep the
        // authoritative Rust surface colours visible; grass instances provide
        // the live detail instead of a browser-side colour rewrite.
        const material = new THREE.MeshBasicMaterial({
          color: 0xffffff,
          vertexColors: true,
          transparent: false,
          side: THREE.DoubleSide,
        })
        mesh = new THREE.Mesh(geometry, material)
      }
      entityMeshCache.set(meshCacheKey, mesh)
    }
    const meshClone = mesh.clone()
    meshClone.name='terrain:'+chunk.chunkX+','+chunk.chunkZ
    meshClone.userData.heightEncoding=heightView.encoding
    meshClone.userData.chunkX=chunk.chunkX;meshClone.userData.chunkZ=chunk.chunkZ
    meshClone.position.set(chunk.worldX - scene.originX + width / 2, 0, chunk.worldZ - scene.originZ + depth / 2)
    if (!hiddenRenderLayers.has('terrain')) chunkGroup.add(meshClone)
    chunkGroup.userData.meshStep = meshStep
  }
  const heightFieldMap = new Map()
  const heightAt = (worldX, worldZ) => {
    const key = `${Math.floor((worldX - scene.originX) / 512)},${Math.floor((worldZ - scene.originZ) / 512)}`
    let field = heightFieldMap.get(key)
    if (!field) {
      field = heightFields.find(item => worldX >= item.chunk.worldX && worldX < item.chunk.worldX + item.width && worldZ >= item.chunk.worldZ && worldZ < item.chunk.worldZ + item.depth)
      heightFieldMap.set(key, field ?? null)
    }
    if (!field && heightFields.length) {
      field = heightFields.reduce((nearest, item) => {
        const minX = item.chunk.worldX; const maxX = minX + item.width - 1
        const minZ = item.chunk.worldZ; const maxZ = minZ + item.depth - 1
        const dx = Math.max(minX - worldX, 0, worldX - maxX)
        const dz = Math.max(minZ - worldZ, 0, worldZ - maxZ)
        const distance = dx * dx + dz * dz
        return !nearest || distance < nearest.distance ? { item, distance } : nearest
      }, null)?.item ?? null
    }
    if (!field) return 0
    const x = Math.max(0, Math.min(field.width - 1, Math.floor(worldX - field.chunk.worldX)))
    const z = Math.max(0, Math.min(field.depth - 1, Math.floor(worldZ - field.chunk.worldZ)))
    return field.heightView.readMetres(z * field.width + x)
  }
  // Grounding must never use the nearest loaded chunk. That fallback is useful
  // for a camera between streamed chunks, but it would silently place an
  // off-screen building on a different chunk's terrain.
  const loadedHeightAt = (worldX, worldZ) => {
    const field = heightFields.find(item => worldX >= item.chunk.worldX && worldX < item.chunk.worldX + item.width && worldZ >= item.chunk.worldZ && worldZ < item.chunk.worldZ + item.depth)
    if (!field) return null
    const x = Math.max(0, Math.min(field.width - 1, Math.floor(worldX - field.chunk.worldX)))
    const z = Math.max(0, Math.min(field.depth - 1, Math.floor(worldZ - field.chunk.worldZ)))
    return field.heightView.readMetres(z * field.width + x)
  }
  for (const route of scene.routeEvolutionPlan?.routes ?? []) {
    if (route.status !== 'pass' || !(route.polyline?.length >= 2)) continue
    const halfWidth = Math.max(2, route.roadWidthM * .5)
    const bedGeometry = makePolylineRibbonGeometry(route.polyline, halfWidth, heightAt, THREE, .86, scene.originX, scene.originZ)
    if (!bedGeometry) continue
    const bed = new THREE.Mesh(bedGeometry, new THREE.MeshStandardMaterial({
      color: '#54483b',
      roughness: .96,
      polygonOffset: true,
      polygonOffsetFactor: -1,
      polygonOffsetUnits: -1,
    }))
    bed.name = 'route-evolution:' + route.id
    nearGroupRoot.add(bed)
    const centerGeometry = makePolylineRibbonGeometry(route.polyline, .32, heightAt, THREE, 1.08, scene.originX, scene.originZ)
    if (centerGeometry) {
      const center = new THREE.Mesh(centerGeometry, new THREE.MeshStandardMaterial({ color: '#d8c58e', roughness: .8 }))
      center.name = 'route-centerline:' + route.id
      nearGroupRoot.add(center)
    }
    for (const span of route.bridgeSpans ?? []) {
      let sincePier = 0
      for (let index = span.startIndex + 1; index <= span.endIndex; index++) {
        const previous = route.polyline[index - 1]
        const point = route.polyline[index]
        sincePier += Math.hypot(point.x - previous.x, point.z - previous.z)
        if (sincePier < 28) continue
        sincePier = 0
        const ground = heightAt(point.x, point.z)
        const height = Math.max(.4, point.designHeightM - ground)
        const support = new THREE.Mesh(
          new THREE.BoxGeometry(span.kind === 'bridge' ? 2.2 : 1.5, height, span.kind === 'bridge' ? 2.2 : 1.5),
          new THREE.MeshStandardMaterial({ color: span.kind === 'bridge' ? '#84745e' : '#756b5c', roughness: .95 }),
        )
        support.position.set(point.x - scene.originX, ground + height * .5, point.z - scene.originZ)
        support.name = 'route-support:' + route.id
        nearGroupRoot.add(support)
      }
    }
  }
  for (const building of scene.settlementEvolutionPlan?.buildings ?? []) {
    const ground = heightAt(building.worldX, building.worldZ)
    const material = new THREE.MeshStandardMaterial({
      color: building.kind === 'storefront' ? '#a85a43'
        : building.kind === 'office_tower' ? '#71848d'
        : '#c6aa80',
      roughness: .9,
      metalness: building.kind === 'office_tower' ? .12 : 0,
    })
    const mesh = new THREE.Mesh(
      new THREE.BoxGeometry(building.widthM, building.heightM, building.depthM),
      material,
    )
    mesh.position.set(
      building.worldX - scene.originX,
      ground + building.heightM * .5,
      building.worldZ - scene.originZ,
    )
    mesh.name = 'settlement-building:' + building.id
    nearGroupRoot.add(mesh)
    const roof = new THREE.Mesh(
      new THREE.BoxGeometry(building.widthM * 1.04, .35, building.depthM * 1.04),
      new THREE.MeshStandardMaterial({ color: '#4e4a43', roughness: .95 }),
    )
    roof.position.set(mesh.position.x, mesh.position.y + building.heightM * .5 + .18, mesh.position.z)
    nearGroupRoot.add(roof)
  }
  for (const market of scene.marketCatchmentPlan?.markets ?? []) {
    const ground = heightAt(market.worldX, market.worldZ)
    const hall = new THREE.Mesh(
      new THREE.BoxGeometry(18, 6, 22),
      new THREE.MeshStandardMaterial({ color: '#b87938', roughness: .92 }),
    )
    hall.position.set(market.worldX - scene.originX, ground + 3, market.worldZ - scene.originZ)
    hall.name = 'fresh-market:' + market.id
    nearGroupRoot.add(hall)
    const canopy = new THREE.Mesh(
      new THREE.BoxGeometry(20, .5, 24),
      new THREE.MeshStandardMaterial({ color: '#5d4630', roughness: .95 }),
    )
    canopy.position.set(hall.position.x, ground + 6.2, hall.position.z)
    nearGroupRoot.add(canopy)
  }
  const groundedRenderY = entity => {
    const scale = Number.isFinite(entity.scale) ? entity.scale : 1
    const halfW = Math.abs((entity.widthM || 0) * scale) * .5
    const halfD = Math.abs((entity.depthM || 0) * scale) * .5
    const yaw = THREE.MathUtils.degToRad(Number(entity.rotationYDeg) || 0)
    const cos = Math.cos(yaw); const sin = Math.sin(yaw)
    const samples = [[0, 0], [-halfW, -halfD], [halfW, -halfD], [-halfW, halfD], [halfW, halfD], [0, -halfD], [0, halfD], [-halfW, 0], [halfW, 0]]
      .map(([x, z]) => [x * cos + z * sin, -x * sin + z * cos])
    const levels = samples.map(([dx, dz]) => loadedHeightAt(entity.worldX + dx, entity.worldZ + dz))
    if (levels.some(level => level == null)) return entity.worldY ?? 0
    const highest = Math.max(...levels)
    const grounding = entity.grounding ?? {}
    const offsets = (grounding.bottomOffsetM ?? 0) + (grounding.roadbedOffsetM ?? 0)
    if (grounding.pivot === 'center') return highest - offsets + (entity.heightM || 0) * scale * .5
    return highest - offsets
  }

  const terrainFieldMap = new Map()
  const terrainAt = (worldX, worldZ) => {
    const key = `${Math.floor((worldX - scene.originX) / 512)},${Math.floor((worldZ - scene.originZ) / 512)}`
    let field = terrainFieldMap.get(key)
    if (!field) {
      field = heightFields.find(item => worldX >= item.chunk.worldX && worldX < item.chunk.worldX + item.width && worldZ >= item.chunk.worldZ && worldZ < item.chunk.worldZ + item.depth)
      terrainFieldMap.set(key, field ?? null)
    }
    if (!field) return { slope: 0, curvature: 0, wetness: .5, disturbance: 0, surface: 0 }
    const x = Math.max(0, Math.min(field.width - 1, Math.floor(worldX - field.chunk.worldX)))
    const z = Math.max(0, Math.min(field.depth - 1, Math.floor(worldZ - field.chunk.worldZ)))
    const index = z * field.width + x
    const offset = index * 4
    return {
      slope: field.terrain ? field.terrain[offset] / 255 : 0,
      curvature: (field.terrain ? field.terrain[offset + 1] / 255 : .5) * 4 - 2,
      wetness: field.terrain ? field.terrain[offset + 2] / 255 : .5,
      disturbance: field.terrain ? field.terrain[offset + 3] / 255 : 0,
      surface: field.surface?.[index] ?? 0,
    }
  }
  if (!worldWind) worldWind = createWorldWind(THREE)
  // Water is a world-space footprint contract, not merely a low terrain
  // height. This guard protects legacy/exported worlds where an authored
  // building was serialized separately from the generated entity list.
  const waterContains = (x, z) => {
    const water = (scene.landmarks ?? []).find(item => item.type === 'river' || item.type === 'lake')
    if (!water) return false
    if (water.type === 'lake') {
      const dx = (x - water.worldX) / Math.max(1, water.widthM * .5)
      const dz = (z - water.worldZ) / Math.max(1, water.depthM * .5)
      return dx * dx + dz * dz < 1
    }
    const minZ = water.worldZ - water.depthM * .5
    const maxZ = water.worldZ + water.depthM * .5
    if (z < minZ || z > maxZ) return false
    if (water.depthM < 3000) return Math.abs(x - water.worldX) < water.widthM * .5
    const t = (z - minZ) / Math.max(1, water.depthM)
    const halfWidth = water.widthM * .5 * (1 + Math.sin(t * Math.PI * 2 * 1.15) * .08 + Math.max(0, Math.sin((t - .28) * Math.PI * 2 * 3)) * .045)
    return Math.abs(x - water.worldX) < halfWidth
  }
  const waterCompatibleKind = kind => ['river', 'lake', 'canal', 'bridge', 'causeway', 'mudflat', 'mangrove', 'reed'].includes(kind)
  const footprintIntersectsWater = entity => {
    if (!entity || waterCompatibleKind(entity.kind || entity.type)) return false
    const hw = Math.abs(entity.widthM || 0) * .5
    const hd = Math.abs(entity.depthM || 0) * .5
    const yaw = THREE.MathUtils.degToRad(Number(entity.rotationYDeg) || 0)
    const points = [[0, 0], [-hw, -hd], [hw, -hd], [-hw, hd], [hw, hd], [0, -hd], [0, hd], [-hw, 0], [hw, 0]]
    return points.some(([x, z]) => waterContains(entity.worldX + x * Math.cos(yaw) + z * Math.sin(yaw), entity.worldZ - x * Math.sin(yaw) + z * Math.cos(yaw)))
  }
  // The generated terrain raster is the one ecological contract for all
  // visible natural detail. Legacy worlds may not contain the semantic sidecar,
  // so add an explicit geometric exclusion for road/sidewalk footprints. This
  // prevents reeds, rocks and fallback five-sided grass clumps from appearing
  // on top of a road even when disturbance bytes are absent.
  const footprintContains = (entity, worldX, worldZ, padding = 0) => {
    const hw = Math.abs(entity.widthM || 0) * .5 + padding
    const hd = Math.abs(entity.depthM || 0) * .5 + padding
    const yaw = THREE.MathUtils.degToRad(Number(entity.rotationYDeg) || 0)
    const dx = worldX - entity.worldX; const dz = worldZ - entity.worldZ
    const localX = dx * Math.cos(yaw) - dz * Math.sin(yaw)
    const localZ = dx * Math.sin(yaw) + dz * Math.cos(yaw)
    return Math.abs(localX) <= hw && Math.abs(localZ) <= hd
  }
  const roadFootprints = (scene.entities ?? []).filter(entity => entity.kind === 'road' || entity.kind === 'sidewalk')
  const onRoadFootprint = (entity, padding = 0) => roadFootprints.some(road => footprintContains(road, entity.worldX, entity.worldZ, padding + Math.max(entity.widthM || 0, entity.depthM || 0) * .5))
  const vegetationFitsTerrain = entity => {
    const terrain = terrainAt(entity.worldX, entity.worldZ)
    if (onRoadFootprint(entity, .35) || terrain.disturbance > .04 || terrain.surface === 3) return false
    if (entity.kind === 'tree') return terrain.slope < .78 && terrain.wetness > .12
    if (entity.kind === 'bush') return terrain.slope < .7 && terrain.wetness > .18 && terrain.wetness < .96 && terrain.surface !== 2
    if (entity.kind === 'grass_clump') return terrain.slope < .9 && terrain.wetness > .06 && terrain.surface !== 2
    if (entity.kind === 'reed') return terrain.slope < .82 && terrain.wetness > .42
    return true
  }
  // Grass blades are a shared natural-detail layer, not a steppe-only theme.
  // Urban scenes still need grass on parks, verges and unbuilt lots; the baked
  // terrain semantics reject roads, pads, water and hard surfaces.
  if (focusChunk && !new URLSearchParams(location.search).has('nograss')) {
    const sky = isSkyline
    // Soil Studio uses a 0.13 live density over a 200k demo cap. A 2.5D
    // GlyphWeave view spans a 3x3 active-chunk window, so every active chunk
    // receives its own deterministic layer; emitting only the focus chunk made
    // most of the visible green terrain look like a bare lawn.
    // Keep the browser-safe world budget bounded. A previous 3x3 allocation
    // created >1.5m CPU instances before the first frame and could kill the tab.
    // One detailed focus field plus small neighbour fields gives continuity
    // without turning a preview into a multi-million-blade allocation.
    const focusBudget = urbanScene
      ? (sky ? 38000 : 32000)
      : sky ? (window.__steppe ? 110000 : 90000) : (window.__steppe ? 100000 : 80000)
    const grassLayers = []
    let grassRequested = 0
    let grassPlaced = 0
    const managedYards=(scene.landcoverSurfaces ? scene.landcoverPlan?.cells : [])?.filter(c=>c.role==='managed_yard') ?? []
    const inManagedYard=(x,z)=>managedYards.some(c=>x>=c.boundary.x && x<c.boundary.x+c.boundary.widthM && z>=c.boundary.z && z<c.boundary.z+c.boundary.depthM)
    const farmRects=scene.landcoverSurfaces?.agriculture?.fields.map(f=>f.boundary)??[]
    const farmTrackCells=new Set()
    for(const patch of scene.landcoverSurfaces?.patches??[])if(patch.material==='farm_track') {
      for(let i=0;i<patch.vertices.length;i+=4){const [x,,z]=patch.vertices[i];farmTrackCells.add(x+','+z)}
    }
    const onFarmSurface=(x,z)=>farmRects.some(r=>x>=r.x&&x<r.x+r.widthM&&z>=r.z&&z<r.z+r.depthM)||farmTrackCells.has(Math.floor(x)+','+Math.floor(z))
    const routeSegments = (scene.routeEvolutionPlan?.routes ?? [])
      .filter(route => route.status === 'pass')
      .flatMap(route => (route.polyline ?? []).slice(1).map((point, index) => ({
        a: route.polyline[index],
        b: point,
        clearance: route.roadWidthM * .5 + 2,
      })))
    const onRouteFootprint = (x, z) => routeSegments.some(({ a, b, clearance }) => {
      const dx = b.x - a.x; const dz = b.z - a.z
      const lengthSq = dx * dx + dz * dz
      const t = lengthSq > 0 ? Math.max(0, Math.min(1, ((x - a.x) * dx + (z - a.z) * dz) / lengthSq)) : 0
      return Math.hypot(x - (a.x + dx * t), z - (a.z + dz * t)) <= clearance
    })
    // Suppress unowned procedural blades over Rust-managed yards/pavement.
    // Rust meshes below remain the only actual landscaping geometry there.
    const grassTerrainAt = (x, z) => inManagedYard(x,z) || onFarmSurface(x,z) || onRouteFootprint(x,z) || roadFootprints.some(road => footprintContains(road, x, z)) ? { slope: 0, curvature: 0, wetness: 0, disturbance: 1, surface: 2 } : terrainAt(x, z)
    // The terrain raster is authoritative for placement, but legacy semantic
    // bytes can be absent or overly conservative. Keep a deterministic neutral
    // fallback for genuinely land-covered chunks so the render adapter still
    // shows grass; roads/water remain hard-excluded by explicit footprints.
    const chunkTerrainAt = (chunk, x, z) => {
      const sample = grassTerrainAt(x, z)
      if (!chunk.terrain && sample.surface === 0 && sample.wetness === 0.5 && sample.disturbance === 0) return { slope: 0, curvature: 0, wetness: .7, disturbance: 0, surface: 0 }
      return sample
    }
    for (const chunk of activeChunks) {
      const distance = Math.max(Math.abs(chunk.chunkX - focusChunk.chunkX), Math.abs(chunk.chunkZ - focusChunk.chunkZ))
      const budget = distance === 0 ? focusBudget : distance === 1 ? Math.floor(focusBudget * .08) : 0
      if (budget < 200) continue
      const grass = makeGrassBlades(THREE, chunk.worldX, chunk.worldZ, chunk.validWidthM, chunk.validDepthM, heightAt, (x, z) => chunkTerrainAt(chunk, x, z), worldWind, budget, 7 + chunk.chunkX * 17 + chunk.chunkZ * 31, focusX, focusZ, {
        uniformDensity: true,
        coverageThreshold: urbanScene ? .2 : .075,
        bladeWidthScale: urbanScene ? 3.4 : sky ? 5.5 : 6.2,
        bladeHeightScale: urbanScene ? .96 : sky ? 2.1 : 2.35,
      })
      grass.uniforms.uSunDir.value.copy(window.__nearSteppeSunDir ?? new THREE.Vector3(-.45, .82, .3).normalize())
      group.add(grass.mesh)
      grassLayers.push(grass)
      grassRequested += budget
      grassPlaced += grass.count
    }
    window.__steppeGrassLayers = grassLayers
    window.__steppeGrass = grassLayers[0] ?? null
    const grassInfo = document.getElementById('info')
    if (grassInfo) {
      grassInfo.dataset.grassRequested = String(grassRequested)
      grassInfo.dataset.grassPlaced = String(grassPlaced)
      grassInfo.dataset.grassInstanceCount = String(grassPlaced)
      grassInfo.dataset.grassFocusChunk = String(focusChunk.chunkX) + ',' + String(focusChunk.chunkZ)
      grassInfo.dataset.grassLayers = String(grassLayers.length)
    }
  }
  for (const landmark of scene.landmarks) {
    if (footprintIntersectsWater({ ...landmark, kind: landmark.type })) {
      console.warn(`skip land landmark ${landmark.entityId}: footprint intersects water`)
      continue
    }
    const groundedLandmark = { ...landmark, worldY: heightAt(landmark.worldX, landmark.worldZ) }
    if (focusChunk && !isHarbourOverview && !isSkyline && !['bridge', 'river', 'pagoda', 'park', 'station', 'ferris_wheel', 'boat', 'building_tower', 'mangrove', 'nature_reserve', 'food_stall'].includes(landmark.type) && !inFocusChunk(landmark)) continue
    const type = landmark.type
    if (['rock_mountain', 'mountain', 'hill', 'shopping_mall', 'office_tower', 'convenience_store', 'fence', 'guardrail', 'boardwalk', 'scenic_belt', 'mudflat', 'night_market', 'community_plaza'].includes(type)) {
      addRendered(groundedLandmark, makeCoastalFeature(groundedLandmark, THREE))
      continue
    }
    if (type === 'bridge') {
      addRendered(landmark, makeBridgeStructure(landmark, THREE, heightAt))
      continue
    }
    if (type === 'food_stall') {
      addRendered(landmark, makeFoodStall(landmark, THREE))
      continue
    }
    if (type === 'mangrove' || type === 'nature_reserve') {
      addRendered(landmark, makeMangrove(landmark, THREE))
      continue
    }
    if (type === 'pavilion') {
      const pavilion = new THREE.Group()
      const wood = new THREE.MeshStandardMaterial({ color: '#756451', roughness: .9 })
      const roofMaterial = new THREE.MeshStandardMaterial({ color: '#3d4540', roughness: .95 })
      const deck = new THREE.Mesh(new THREE.BoxGeometry(1, .08, 1), wood)
      deck.scale.set(landmark.widthM, 1, landmark.depthM)
      pavilion.add(deck)
      for (const x of [-.34, .34]) for (const z of [-.34, .34]) {
        const column = new THREE.Mesh(new THREE.CylinderGeometry(.035, .045, .65, 8), wood)
        column.position.set(x * landmark.widthM, landmark.heightM * .38, z * landmark.depthM)
        pavilion.add(column)
      }
      const roof = new THREE.Mesh(new THREE.ConeGeometry(.52, .22, 4), roofMaterial)
      roof.scale.set(landmark.widthM, landmark.heightM, landmark.depthM)
      roof.position.y = landmark.heightM * .82
      pavilion.add(roof)
      pavilion.position.set(landmark.worldX, landmark.worldY, landmark.worldZ)
      addRendered(landmark, pavilion)
      continue
    }
    if (type === 'pagoda') {
      const pagoda = new THREE.Group()
      const red = new THREE.MeshStandardMaterial({ color: '#b5453a', roughness: .85 })
      const wall = new THREE.MeshStandardMaterial({ color: '#c9b8a0', roughness: .9 })
      const roof = new THREE.MeshStandardMaterial({ color: '#d9b13a', roughness: .7, metalness: .3 })
      const stone = new THREE.MeshStandardMaterial({ color: '#7d7d72', roughness: 1 })
      const hill = new THREE.Mesh(new THREE.ConeGeometry(landmark.widthM * .95, landmark.heightM * .72, 10), stone)
      hill.position.y = landmark.heightM * .36
      pagoda.add(hill)
      const floors = 7
      for (let i = 0; i < floors; i++) {
        const t = i / (floors - 1)
        const w = landmark.widthM * (.55 - t * .22)
        const body = new THREE.Mesh(new THREE.BoxGeometry(w, landmark.heightM * .1, w), wall)
        body.position.y = landmark.heightM * .72 + i * landmark.heightM * .3
        pagoda.add(body)
        const eave = new THREE.Mesh(new THREE.ConeGeometry(w * 1.5, landmark.heightM * .05, 6), roof)
        eave.position.y = landmark.heightM * .72 + i * landmark.heightM * .3 + landmark.heightM * .07
        pagoda.add(eave)
        const pillar = new THREE.Mesh(new THREE.CylinderGeometry(.4, .4, landmark.heightM * .1, 8), red)
        pillar.position.set(0, landmark.heightM * .72 + i * landmark.heightM * .3 + landmark.heightM * .05, 0)
        pagoda.add(pillar)
      }
      const spire = new THREE.Mesh(new THREE.ConeGeometry(landmark.widthM * .12, landmark.heightM * .16, 8), roof)
      spire.position.y = landmark.heightM * .72 + (floors - 1) * landmark.heightM * .3 + landmark.heightM * .22
      pagoda.add(spire)
      pagoda.position.set(landmark.worldX, landmark.worldY - landmark.heightM * .72, landmark.worldZ)
      addRendered(landmark, pagoda)
      continue
    }
    if (type === 'building_tower') {
      addRendered(groundedLandmark, makeHongKongTower(groundedLandmark, THREE))
      continue
    }
    if (type === 'park') {
      const park = new THREE.Group()
      // Paved square core: real stone/brick paving with a paver grid texture,
      // not bare lawn. The eye should read a hardscaped plaza with planting,
      // like an urban bayfront park 鈥?grass patches are inset beds, the ground
      // is paver.
      const paverTex = (() => {
        const c = document.createElement('canvas')
        c.width = 256; c.height = 256
        const g = c.getContext('2d')
        g.fillStyle = '#9a9388'
        g.fillRect(0, 0, 256, 256)
        // Paver grid: alternating stone slabs with visible joints.
        g.strokeStyle = '#7d7668'
        g.lineWidth = 3
        for (let i = 0; i < 8; i++) {
          g.beginPath(); g.moveTo(0, i * 32); g.lineTo(256, i * 32); g.stroke()
          g.beginPath(); g.moveTo(i * 32, 0); g.lineTo(i * 32, 256); g.stroke()
        }
        // Per-slab tone variation.
        for (let y = 0; y < 8; y++) for (let x = 0; x < 8; x++) {
          const shade = ((x * 31 + y * 17) % 100) / 100
          g.fillStyle = `rgba(${110 + shade * 40}, ${105 + shade * 30}, ${95 + shade * 25}, 0.35)`
          g.fillRect(x * 32 + 4, y * 32 + 4, 24, 24)
        }
        const t = new THREE.CanvasTexture(c)
        t.wrapS = THREE.RepeatWrapping; t.wrapT = THREE.RepeatWrapping
        t.repeat.set(Math.max(1, Math.round(landmark.widthM / 12)), Math.max(1, Math.round(landmark.depthM / 12)))
        t.colorSpace = THREE.SRGBColorSpace
        return t
      })()
      const paverMat = new THREE.MeshStandardMaterial({ map: paverTex, roughness: .92 })
      const plaza = new THREE.Mesh(new THREE.BoxGeometry(landmark.widthM, .6, landmark.depthM), paverMat)
      plaza.position.y = .3
      park.add(plaza)
      // Gravel/wood walking path down the middle, surfaced differently.
      const pathMat = new THREE.MeshStandardMaterial({ color: '#6f675c', roughness: 1 })
      const path = new THREE.Mesh(new THREE.BoxGeometry(landmark.widthM * .16, .62, landmark.depthM * .94), pathMat)
      path.position.y = .34
      park.add(path)
      // Ornamental planting beds (grass/flowers inset into the paving).
      const bedMat = new THREE.MeshStandardMaterial({ color: '#5d8a4a', roughness: 1 })
      const bedMat2 = new THREE.MeshStandardMaterial({ color: '#6f8f5a', roughness: 1 })
      for (const side of [-1, 1]) {
        const bed = new THREE.Mesh(new THREE.BoxGeometry(landmark.widthM * .3, .3, landmark.depthM * .3), side < 0 ? bedMat : bedMat2)
        bed.position.set(side * landmark.widthM * .34, .15, 0)
        park.add(bed)
      }
      // Waterfront guard rail: double rail + posts along the seaward edge so
      // the plaza reads as a real promenade railing, not a single line.
      const railMat = new THREE.MeshStandardMaterial({ color: '#7a7268', roughness: .6, metalness: .4 })
      const seawardZ = landmark.depthM / 2 - 1
      const postSpacing = 8
      const postCount = Math.max(4, Math.floor(landmark.widthM / postSpacing))
      for (let i = 0; i <= postCount; i++) {
        const x = -landmark.widthM * .5 + i * (landmark.widthM / postCount)
        for (const z of [seawardZ, -seawardZ]) {
          const post = new THREE.Mesh(new THREE.BoxGeometry(.35, 1.2, .35), railMat)
          post.position.set(x, .6, z)
          park.add(post)
          const rail1 = new THREE.Mesh(new THREE.BoxGeometry(postSpacing + .3, .12, .12), railMat)
          rail1.position.set(x + postSpacing / 2, 1.05, z)
          park.add(rail1)
          const rail2 = new THREE.Mesh(new THREE.BoxGeometry(postSpacing + .3, .12, .12), railMat)
          rail2.position.set(x + postSpacing / 2, .7, z)
          park.add(rail2)
        }
      }
      // Observation stairs down to the water at the seaward edge: a short
      // flight of steps connects the raised plaza to the shore.
      const stepMat = new THREE.MeshStandardMaterial({ color: '#8a8276', roughness: .9 })
      const steps = 5
      for (let i = 0; i < steps; i++) {
        const step = new THREE.Mesh(new THREE.BoxGeometry(landmark.widthM * .3, .25, .6), stepMat)
        step.position.set(0, .3 + i * .25, seawardZ - .3 - i * .6)
        park.add(step)
      }
      const fountain = new THREE.Mesh(new THREE.CylinderGeometry(8, 10, 1.4, 24), new THREE.MeshStandardMaterial({ color: '#8e9aa3', roughness: .7, metalness: .25 }))
      fountain.position.y = 1.2
      park.add(fountain)
      const jet = new THREE.Mesh(new THREE.ConeGeometry(2.4, 9, 8), new THREE.MeshStandardMaterial({ color: '#a8c8d8', roughness: .4, transparent: true, opacity: .65 }))
      jet.position.y = 6
      park.add(jet)
      for (let i = 0; i < 14; i++) {
        const a = i / 14 * Math.PI * 2
        const r = landmark.widthM * .38
        const tree = new THREE.Mesh(new THREE.ConeGeometry(3.2, 7, 7), new THREE.MeshStandardMaterial({ color: '#3f6a3d', roughness: .95 }))
        tree.position.set(Math.cos(a) * r, 4.5, Math.sin(a) * r * .8)
        park.add(tree)
      }
      park.position.set(landmark.worldX, landmark.worldY, landmark.worldZ)
      addRendered(landmark, park)
      continue
    }
    if (type === 'station') {
      const station = new THREE.Group()
      const wall = new THREE.MeshStandardMaterial({ color: '#c8b99a', roughness: .92 })
      const roof = new THREE.MeshStandardMaterial({ color: '#3f5a4a', roughness: .95 })
      const beam = new THREE.MeshStandardMaterial({ color: '#6a5f4a', roughness: .9 })
      const hall = new THREE.Mesh(new THREE.BoxGeometry(landmark.widthM * .7, landmark.heightM * .75, landmark.depthM * .6), wall)
      hall.position.y = landmark.heightM * .375
      station.add(hall)
      const roofSlab = new THREE.Mesh(new THREE.BoxGeometry(landmark.widthM * .78, 1.6, landmark.depthM * .68), roof)
      roofSlab.position.y = landmark.heightM * .78
      station.add(roofSlab)
      const clockTower = new THREE.Mesh(new THREE.BoxGeometry(landmark.widthM * .14, landmark.heightM * 1.15, landmark.depthM * .22), wall)
      clockTower.position.set(-landmark.widthM * .3, landmark.heightM * 1.0, 0)
      station.add(clockTower)
      const canopyW = landmark.widthM * .55
      const canopyZ = landmark.depthM * .85
      for (let i = 0; i < 7; i++) {
        const column = new THREE.Mesh(new THREE.CylinderGeometry(.7, .7, landmark.heightM * .5, 8), beam)
        column.position.set(-canopyW / 2 + i * canopyW / 6, landmark.heightM * .25, -canopyZ / 2)
        station.add(column)
      }
      const canopy = new THREE.Mesh(new THREE.BoxGeometry(canopyW, 1, landmark.depthM * .35), roof)
      canopy.position.set(0, landmark.heightM * .55, -landmark.depthM * .5)
      station.add(canopy)
      const track = new THREE.Mesh(new THREE.BoxGeometry(landmark.widthM * .7, .35, 2.6), beam)
      track.position.set(0, .2, landmark.depthM * .5)
      station.add(track)
      station.position.set(landmark.worldX, landmark.worldY, landmark.worldZ)
      addRendered(landmark, station)
      continue
    }
    if (type === 'ferris_wheel') {
      const wheel = new THREE.Group()
      const steel = new THREE.MeshStandardMaterial({ color: '#4a5258', roughness: .6, metalness: .6 })
      const red = new THREE.MeshStandardMaterial({ color: '#c03a2e', roughness: .7 })
      const ring = new THREE.Mesh(new THREE.TorusGeometry(landmark.heightM * .4, 1.1, 10, 48), steel)
      ring.rotation.x = Math.PI / 2
      ring.position.y = landmark.heightM * .72
      wheel.add(ring)
      for (let i = 0; i < 8; i++) {
        const a = i / 8 * Math.PI * 2
        const spoke = new THREE.Mesh(new THREE.CylinderGeometry(.4, .4, landmark.heightM * .8, 6), steel)
        spoke.rotation.z = Math.PI / 2 - a
        spoke.position.y = landmark.heightM * .72
        wheel.add(spoke)
      }
      for (let i = 0; i < 12; i++) {
        const a = i / 12 * Math.PI * 2
        const cabin = new THREE.Mesh(new THREE.BoxGeometry(3.4, 2.6, 2.6), red)
        cabin.position.set(Math.cos(a) * landmark.heightM * .4, landmark.heightM * .72 + Math.sin(a) * landmark.heightM * .4, 0)
        wheel.add(cabin)
      }
      const hub = new THREE.Mesh(new THREE.CylinderGeometry(2.4, 2.4, 3, 12), steel)
      hub.rotation.x = Math.PI / 2
      hub.position.y = landmark.heightM * .72
      wheel.add(hub)
      for (const side of [-1, 1]) {
        const leg = new THREE.Mesh(new THREE.BoxGeometry(1.6, landmark.heightM * .72, 1.6), steel)
        leg.position.set(side * landmark.widthM * .18, landmark.heightM * .36, 0)
        wheel.add(leg)
      }
      wheel.position.set(landmark.worldX, landmark.worldY, landmark.worldZ)
      addRendered(landmark, wheel)
      continue
    }
    if (type === 'boat') {
      const boat = new THREE.Group()
      const hull = new THREE.MeshStandardMaterial({ color: '#5a4a3e', roughness: .9 })
      const upper = new THREE.MeshStandardMaterial({ color: '#e8e2d4', roughness: .8 })
      const deck = new THREE.Mesh(new THREE.BoxGeometry(landmark.widthM, 2.4, landmark.depthM), hull)
      deck.position.y = 1.2
      boat.add(deck)
      const bow = new THREE.Mesh(new THREE.ConeGeometry(landmark.depthM * .5, landmark.widthM * .16, 4), hull)
      bow.rotation.z = Math.PI / 2
      bow.position.set(landmark.widthM * .58, 1.2, 0)
      boat.add(bow)
      const house = new THREE.Mesh(new THREE.BoxGeometry(landmark.widthM * .45, 4, landmark.depthM * .7), upper)
      house.position.set(-landmark.widthM * .12, 4.2, 0)
      boat.add(house)
      const funnel = new THREE.Mesh(new THREE.CylinderGeometry(1.6, 1.2, 5, 10), hull)
      funnel.position.set(landmark.widthM * .22, 8.6, 0)
      boat.add(funnel)
      boat.position.set(landmark.worldX, landmark.worldY, landmark.worldZ)
      addRendered(landmark, boat)
      continue
    }
    const isWater = type === 'lake' || type === 'river'
    const geometry = type === 'lake' ? new THREE.CircleGeometry(.5, 96).rotateX(-Math.PI / 2)
      : type === 'river' ? makeRiverGeometry(landmark, THREE)
      : type === 'island_hill' ? new THREE.SphereGeometry(.5, 24, 12)
      : type === 'ridge' ? new THREE.ConeGeometry(.5, 1, 8)
      : new THREE.BoxGeometry(1, 1, 1)
    const waterMaps = isWater ? makeWaterTexture(THREE, type === 'river') : null
    if (waterMaps) waterTextures.push(waterMaps)
    let waterUniforms = null
    let material
    if (isWater) {
      const wm = makeWaterMaterial(THREE, type === 'river', undefined, STATIC_PREVIEW)
      material = wm.material
      waterUniforms = wm.uniforms
      waterUniforms.uTime.value = waterUniforms.uTime.value
      animatedWater.push(wm.uniforms)
      // In static acceptance mode keep the calm water material flat and
      // legible. The animated ripple maps are intentionally skipped here;
      // otherwise the transparent river overlay becomes a dark striped sheet
      // over the baked water band.
      if (!STATIC_PREVIEW) {
        material.map = waterMaps?.texture ?? null
        material.normalMap = waterMaps?.normalTexture ?? null
        material.normalScale = new THREE.Vector2(.7, .7)
      }
    } else {
      material = new THREE.MeshStandardMaterial({
        color: type === 'causeway' ? '#b38b5a' : type === 'pavilion' ? '#c84f3f' : '#6f8557',
        map: waterMaps?.texture ?? null,
        normalMap: waterMaps?.normalTexture ?? null,
        normalScale: new THREE.Vector2(0, 0),
        roughness: .9,
        metalness: 0,
        envMapIntensity: .35,
        transparent: false,
        opacity: 1,
        depthWrite: true,
        side: THREE.FrontSide,
      })
    }
    const marker = new THREE.Mesh(geometry, material)
    if (type === 'island_hill') marker.scale.set(landmark.widthM, landmark.heightM, landmark.depthM)
    else if (type === 'ridge') marker.scale.set(landmark.widthM, landmark.heightM, landmark.depthM)
    else if (isWater && type === 'river') marker.scale.set(1, 1, 1)
    else if (isWater) marker.scale.set(landmark.widthM, 1, landmark.depthM)
    else marker.scale.set(landmark.widthM, landmark.heightM, landmark.depthM)
    marker.position.set(landmark.worldX, isWater ? .15 : landmark.worldY + landmark.heightM / 2, landmark.worldZ)
    if (isWater) { marker.castShadow = false; marker.receiveShadow = false; marker.renderOrder = 10 }
    addRendered(landmark, marker)
  }
  const inActiveChunk = entity => activeChunks.some(chunk => entity.worldX >= chunk.worldX && entity.worldX < chunk.worldX + chunk.validWidthM && entity.worldZ >= chunk.worldZ && entity.worldZ < chunk.worldZ + chunk.validDepthM)
  // Legacy manifest vegetation is deliberately not rendered as primitive fallback
  // geometry. `grass_clump` and `reed` used to become a green cone/pentagonal
  // cylinder as tall as a tree trunk, which is both visually wrong and easy to
  // place on a road in older worlds. The GPU grass layer is now the only generic
  // grass representation in 2.5D; reeds can be reintroduced later with a
  // dedicated wetland renderer and an explicit asset contract.
  const legacyGrassFallbackKinds = new Set(['grass_clump', 'reed'])
  const landmarkEntityIds = new Set((scene.landmarks ?? []).map(landmark => landmark.entityId).filter(Boolean))
  // All camera presets share the same water exclusion. Skyline is only a
  // visibility/LOD choice; it must never bypass the spatial safety contract.
  const gradedInActiveChunk = entity => {
    const p=entity.grounding?.roadProfile
    return p && verticesOverlapChunks([[Math.min(p.start[0],p.end[0])-4,0,Math.min(p.start[1],p.end[1])-4],[Math.max(p.start[0],p.end[0])+4,0,Math.max(p.start[1],p.end[1])+4]],activeChunks)
  }
  const focusEntityCandidates = (scene.entities ?? []).filter(entity =>
    !legacyGrassFallbackKinds.has(entity.kind) && !footprintIntersectsWater(entity) && (
      entity.kind === 'bridge'
      || ((inActiveChunk(entity) || gradedInActiveChunk(entity)) && (
        inFocusChunk(entity) || gradedInActiveChunk(entity)
        || inStreetFrame(entity)
        || ['storefront', 'stairs', 'pedestrian', 'residential_block', 'residential_home', 'residential_tower', 'school', 'industrial', 'town_hall', 'market', 'commercial_center', 'parking_lot'].includes(entity.kind)
        || (['tree', 'bush', 'rock'].includes(entity.kind) && shouldRenderVegetation(entity, 7) && vegetationFitsTerrain(entity))
      ))
    )
  )
  // Use a shallow render copy. The serialized scene remains the source of truth
  // for navigation, audits, and vegetation caches.
  const focusEntities = focusEntityCandidates.filter(entity => !legacyGrassFallbackKinds.has(entity.kind) && !(entity.kind === 'fallen_log' && onRoadFootprint(entity, .5))).map(entity => {
    if(entity.grounding?.frontageRoadPoint)return entity // Rust platform is authoritative; never renderer-reground planned frontage.
    const required = groundedRenderY(entity)
    return Number.isFinite(required) ? { ...entity, worldY: required } : entity
  })
  const plannedBuildingIds = new Set((scene.developmentPlan?.buildings ?? []).map(b => b.entity.entityId))
  for (const entity of focusEntities) {
    if (entity.grounding?.roadProfile) {
      addEntityToGroup(entity,buildEntityCached(entity,()=>makeGradedRoad(entity,THREE)))
      continue
    }
    if (entity.grounding?.frontageRoadPoint) {
      const settlementEvolutionPlan = entity.entityId.startsWith('generated.settlement-evolution.')
      if(!settlementEvolutionPlan && !plannedBuildingIds.has(entity.entityId))throw new Error('Frontage entity missing Rust building plan: '+entity.entityId)
      addEntityToGroup(entity,buildEntityCached(entity,()=>{
        if(entity.kind==='residential_home')return makePlannedResidentialHome(entity,THREE)
        if(entity.kind==='residential_block'||entity.kind==='residential_tower')return makeResidentialBlock(entity,THREE,heightAt)
        if(entity.kind==='storefront')return makeStorefront(entity,THREE)
        return makePlannedServiceBuilding(entity,THREE)
      }))
      continue
    }
    // Trees and rocks are rendered by their dedicated instanced systems below.
    // Do not let this generic fallback create tall low-poly cones/dodecahedra
    // (another source of the misleading green five-point objects).
    if (entity.kind === 'tree' || entity.kind === 'rock') continue
    if (entity.kind === 'storefront') {
      if (isStreetView && !inStreetFrame(entity)) continue
      addRendered(entity, makeStorefront(entity, THREE))
      continue
    }
    if (entity.kind === 'stairs') {
      const stairs = entity.grounding?.stairConnection
      if (!stairs) continue
      const group = new THREE.Group()
      const steps = Math.max(2, stairs.stepCount || 2)
      const dx = stairs.to[0] - stairs.from[0]
      const dz = stairs.to[1] - stairs.from[1]
      const run = Math.hypot(dx, dz) || 2
      const rise = Math.max(.12, (stairs.highMm - stairs.lowMm) / 1000)
      const stepRun = run / steps
      const stepRise = rise / steps
      const lowPoint = stairs.fromMm <= stairs.toMm ? stairs.from : stairs.to
      const direction = lowPoint === stairs.from ? 1 : -1
      const ux = dx / run * direction
      const uz = dz / run * direction
      const yaw = Math.atan2(uz, ux)
      const material = new THREE.MeshStandardMaterial({ color: '#9a9184', roughness: .96 })
      for (let index = 0; index < steps; index++) {
        const step = new THREE.Mesh(
          new THREE.BoxGeometry(
            stepRun + .04,
            stepRise * (index + 1),
            stairs.widthM,
          ),
          material,
        )
        const outward = stepRun * (index + .5)
        step.position.set(
          lowPoint[0] + ux * outward,
          stepRise * (index + 1) * .5,
          lowPoint[1] + uz * outward,
        )
        step.rotation.y = -yaw
        group.add(step)
      }
      group.position.y = Math.min(stairs.lowMm, stairs.highMm) / 1000
      addRendered(entity, group)
      continue
    }
    if (entity.kind === 'food_stall') {
      addRendered(entity, makeFoodStall(entity, THREE))
      continue
    }
    if (entity.kind === 'pedestrian') {
      const person = makePedestrian(entity, THREE)
      addRendered(entity, person)
      person.userData.actorKind = entity.kind
      animatedActors.push({ object: person, baseZ: entity.worldZ, phase: (entity.worldX + entity.worldZ) * .01, speed: .06 })
      continue
    }
    if (isSkyline && (entity.kind === 'building' || entity.kind === 'urban_building' || entity.kind === 'building_tower')) {
      const height = entity.kind === 'building_tower' ? entity.heightM : Math.max(6, entity.heightM)
      const skylineMaterial = new THREE.MeshStandardMaterial({ color: entity.kind === 'building_tower' ? '#536b78' : '#665f5b', roughness: .7, metalness: .2 })
      const skylineBuilding = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM, height, entity.depthM), skylineMaterial)
      skylineBuilding.position.set(entity.worldX, entity.worldY + height / 2, entity.worldZ)
      addRendered(entity, skylineBuilding)
      continue
    }
    if (entity.kind === 'bridge' && scene.landmarks.some(landmark => landmark.entityId === entity.entityId)) continue
    if (entity.kind === 'bridge') {
      addRendered(entity, makeBridgeStructure(entity, THREE, heightAt))
      continue
    }
    if (entity.kind === 'mountain_forest' || entity.kind === 'nature_reserve' || entity.kind === 'green_space') {
      const built = buildEntityCached(entity, () => makeForestPatch(entity, THREE, entity.kind === 'nature_reserve'))
      built.traverse(o => { if (o.isMesh) o.userData.vegetation = true })
      addEntityToGroup(entity, built)
      continue
    }
    if (['tree', 'bush', 'rock', 'building', 'urban_building'].includes(entity.kind)) continue
    if (entity.kind === 'building_tower') {
      addEntityToGroup(entity, buildEntityCached(entity, () => makeHongKongTower(entity, THREE)))
      continue
    }
    if (entity.kind === 'resort_lodge') {
      addEntityToGroup(entity, buildEntityCached(entity, () => {
        const built = makeResortLodge(entity, THREE, heightAt)
        applySlopeFoundation(built, entity, heightAt, THREE)
        return built
      }))
      continue
    }
    if (entity.kind !== 'residential_home' && plannedBuildingIds.has(entity.entityId)) {
      addEntityToGroup(entity, buildEntityCached(entity, () => makePlannedServiceBuilding(entity, THREE)))
      continue
    }
    if (entity.kind === 'residential_block') {
      addEntityToGroup(entity, buildEntityCached(entity, () => {
        const built = makeResidentialBlock(entity, THREE, heightAt)
        applySlopeFoundation(built, entity, heightAt, THREE)
        return built
      }))
      continue
    }
    if (entity.kind === 'residential_tower') {
      addEntityToGroup(entity, buildEntityCached(entity, () => {
        const built = makeResidentialTower(entity, THREE, heightAt)
        applySlopeFoundation(built, entity, heightAt, THREE)
        return built
      }))
      continue
    }
    if (entity.kind === 'residential_home') {
      addEntityToGroup(entity, buildEntityCached(entity, () => {
        const planned=scene.developmentPlan?.buildings.some(b=>b.entity.entityId===entity.entityId)
        const built = planned ? makePlannedResidentialHome(entity, THREE) : makeResidentialHome(entity, THREE, heightAt)
        // Planned foundations already belong to final Rust terrain; don't add
        // a larger browser-invented plinth outside the building footprint.
        if(!planned) applySlopeFoundation(built, entity, heightAt, THREE)
        return built
      }))
      continue
    }
    if (entity.kind === 'school') {
      addEntityToGroup(entity, buildEntityCached(entity, () => makeSchool(entity, THREE, heightAt)))
      continue
    }
    if (entity.kind === 'commercial_center') {
      addEntityToGroup(entity, buildEntityCached(entity, () => makeCommercialCenter(entity, THREE, heightAt)))
      continue
    }
    if (entity.kind === 'entertainment_center') {
      addEntityToGroup(entity, buildEntityCached(entity, () => makeEntertainmentCenter(entity, THREE)))
      continue
    }
    if (entity.kind === 'parking_lot') {
      addEntityToGroup(entity, buildEntityCached(entity, () => makeParkingLot(entity, THREE)))
      continue
    }
    if (entity.kind === 'temple') {
      addEntityToGroup(entity, buildEntityCached(entity, () => makeTemple(entity, THREE)))
      continue
    }
    if (entity.kind === 'church') {
      addEntityToGroup(entity, buildEntityCached(entity, () => makeChurch(entity, THREE)))
      continue
    }
    if (entity.kind === 'farmland') {
      addEntityToGroup(entity, buildEntityCached(entity, () => makeFarmland(entity, THREE)))
      continue
    }
    if (entity.kind === 'pasture') {
      addEntityToGroup(entity, buildEntityCached(entity, () => makePasture(entity, THREE)))
      continue
    }
    if (entity.kind === 'canal') {
      addEntityToGroup(entity, buildEntityCached(entity, () => makeCanal(entity, THREE)))
      continue
    }
    if (entity.kind === 'town_hall') {
      addEntityToGroup(entity, buildEntityCached(entity, () => makeTownHall(entity, THREE, heightAt)))
      continue
    }
    if (entity.kind === 'market') {
      addEntityToGroup(entity, buildEntityCached(entity, () => makeMarket(entity, THREE, heightAt)))
      continue
    }
    if (entity.kind === 'industrial') {
      addEntityToGroup(entity, buildEntityCached(entity, () => makeIndustrial(entity, THREE, heightAt)))
      continue
    }
    if (entity.kind === 'water_well') {
      addRendered(entity, makeWaterWell(entity, THREE))
      continue
    }
    if (entity.kind === 'road_sign') {
      addRendered(entity, makeRoadSign(entity, THREE))
      continue
    }
    if (entity.kind === 'street_lamp') {
      addRendered(entity, makeStreetLamp(entity, THREE))
      continue
    }
    const zoneKinds = ['parking_lot', 'commercial_center', 'entertainment_center', 'school', 'residential_block', 'canal', 'farmland', 'mountain_forest', 'green_space', 'temple', 'church', 'pasture', 'nature_reserve']
    const geometry = entity.kind === 'road' || entity.kind === 'sidewalk' ? makeStripGeometry(entity, entity.kind === 'sidewalk' ? 3 : 7, 0, heightAt, THREE, .45, undefined, undefined, entity.depthM > entity.widthM)
      : entity.kind === 'bridge' ? new THREE.BoxGeometry(entity.widthM || 120, entity.heightM || 8, entity.depthM || 24)
      : entity.kind === 'building_cluster' ? new THREE.BoxGeometry(120, 32, 100)
      : entity.kind === 'building_tower' ? new THREE.BoxGeometry(entity.widthM, entity.heightM, entity.depthM)
       : entity.kind === 'car' ? new THREE.BoxGeometry(entity.widthM, entity.heightM, entity.depthM)
       : entity.kind === 'pedestrian' ? new THREE.CapsuleGeometry(.28, 1.15, 4, 6)
      : entity.kind === 'bench' ? new THREE.BoxGeometry(3.4, 1.1, 1.2)
      : entity.kind === 'lamp' ? new THREE.CylinderGeometry(.16, .24, 4.8, 6)
      : entity.kind === 'fallen_log' ? new THREE.CylinderGeometry(.38, .5, 3.2, 8)
      : entity.kind === 'tree' ? new THREE.ConeGeometry(7, 28, 7)
      : entity.kind === 'rock' ? new THREE.DodecahedronGeometry(8, 0)
      : zoneKinds.includes(entity.kind) ? new THREE.BoxGeometry(entity.widthM, entity.heightM, entity.depthM)
      : entity.kind === 'building' ? new THREE.BoxGeometry(18, 24, 18)
      : null
    if (!geometry) continue
      const zoneColor = { parking_lot: '#6d7070', commercial_center: '#b06d4d', entertainment_center: '#9b5c91', school: '#c29b61', residential_block: '#807b70', canal: '#477c87', farmland: '#9a8747', mountain_forest: '#456344', green_space: '#4d7c4b', temple: '#a74e3c', church: '#777f91', pasture: '#78904b', nature_reserve: '#3d6948' }[entity.kind]
      const material = new THREE.MeshStandardMaterial({ color: zoneColor ?? (entity.kind === 'road' ? '#786b5a' : entity.kind === 'sidewalk' ? '#b0a58f' : entity.kind === 'bridge' ? '#807664' : entity.kind === 'building_cluster' ? '#6c665d' : entity.kind === 'building_tower' ? '#7d8a94' : entity.kind === 'car' ? '#b8463a' : entity.kind === 'pedestrian' ? '#3e536b' : entity.kind === 'bench' ? '#756451' : entity.kind === 'lamp' ? '#6c675b' : entity.kind === 'fallen_log' ? '#70503a' : entity.kind === 'tree' ? '#47704a' : entity.kind === 'bush' ? '#5e8a4c' : entity.kind === 'rock' ? '#76716a' : '#746d63'), roughness: .95 })
    const prop = new THREE.Mesh(geometry, material)
     const baseHeight = zoneKinds.includes(entity.kind) ? entity.heightM / 2 : entity.kind === 'road' || entity.kind === 'sidewalk' ? 0 : entity.kind === 'bridge' ? 4 : entity.kind === 'building_cluster' ? 16       : entity.kind === 'building_tower' ? entity.heightM / 2 : entity.kind === 'car' ? .9 : entity.kind === 'pedestrian' ? .9 : entity.kind === 'building' ? 12 : entity.kind === 'bench' ? .55 : entity.kind === 'lamp' ? 2.4 : entity.kind === 'fallen_log' ? .35 : entity.kind === 'tree' ? 14 : entity.kind === 'bush' ? 2 : 10
    // GroundingSpec owns the map/asset height contract. Old baked worlds omit
    // it and retain the legacy bottom-pivot behaviour.
    const grounding = entity.grounding ?? {}
    const groundY = entity.worldY + (grounding.bottomOffsetM ?? 0) + (grounding.roadbedOffsetM ?? 0)
    const renderY = grounding.pivot === 'center' ? groundY : groundY + baseHeight
    prop.position.set(entity.worldX, renderY, entity.worldZ)
    if (entity.kind === 'fallen_log') prop.rotation.z = Math.PI / 2
    if (Number.isFinite(entity.rotationYDeg) && entity.rotationYDeg !== 0) prop.rotation.y = THREE.MathUtils.degToRad(entity.rotationYDeg)
    else if ((entity.kind === 'road' || entity.kind === 'sidewalk') && (entity.entityId.includes('road-ns') || entity.entityId.includes('north-south'))) prop.rotation.y = Math.PI / 2
    if (entity.kind === 'car') prop.rotation.y = Math.PI / 2
    prop.scale.setScalar(entity.scale * (entity.kind === 'bush' ? 1.4 : 1))
    if (entity.kind === 'road' || entity.kind === 'sidewalk') {
      // Center the visible strip on the focus so a long road stays on screen
      // instead of extending to the far scene edge.
      // Keep the strip centered on its serialized entity. Re-centering a long
      // rotated road in world X was a source of false offsets and apparent
      // sinking at the visible end; the active-chunk filter already controls
      // how much of the road is shown.
      const roadLift = entity.kind === 'road' ? .42 : .28
      const authoredWidth = Math.max(4, Number(entity.depthM) || (entity.kind === 'sidewalk' ? 4 : 8))
      const bed = new THREE.Mesh(makeStripGeometry(entity, authoredWidth * .5, 0, heightAt, THREE, roadLift), new THREE.MeshStandardMaterial({ color: entity.kind === 'sidewalk' ? '#877e70' : '#4c4438', roughness: 1, polygonOffset: true, polygonOffsetFactor: -1, polygonOffsetUnits: -1 }))
      bed.position.set(0, 0, 0)
      prop.add(bed)
      if (entity.kind === 'road') {
        const marking = new THREE.Mesh(makeStripGeometry(entity, .28, 0, heightAt, THREE, .72), new THREE.MeshStandardMaterial({ color: '#d9c98c', roughness: .8 }))
        marking.position.set(0, 0, 0)
        prop.add(marking)
        for (const side of [-1, 1]) {
          const curb = new THREE.Mesh(makeStripGeometry(entity, .18, side * 6.2, heightAt, THREE, .66), new THREE.MeshStandardMaterial({ color: '#a3957e', roughness: .95 }))
          curb.position.set(0, 0, 0)
          prop.add(curb)
        }
      }
    } else if (entity.kind === 'lamp') {
      group.remove(prop)
      addRendered(entity, makeStreetLamp(entity, THREE))
      continue
    } else if (entity.kind === 'bridge') {
      for (const side of [-1, 1]) {
        const rail = new THREE.Mesh(new THREE.BoxGeometry(120, 2.8, .35), new THREE.MeshStandardMaterial({ color: '#806346', roughness: .9 }))
        rail.position.set(0, 5.5, side * 10.5)
        prop.add(rail)
      }
    } else if (entity.kind === 'car') {
      const cab = new THREE.Mesh(new THREE.BoxGeometry(2.2, .8, 1.7), new THREE.MeshStandardMaterial({ color: '#2e3438', roughness: .35, metalness: .3 }))
      cab.position.set(-.3, .95, 0)
      prop.add(cab)
      for (const z of [-.8, .8]) {
        for (const x of [-1.4, 1.4]) {
          const wheel = new THREE.Mesh(new THREE.CylinderGeometry(.38, .38, .3, 10), new THREE.MeshStandardMaterial({ color: '#171a1c', roughness: .95 }))
          wheel.rotation.x = Math.PI / 2
          wheel.position.set(x, .32, z)
          prop.add(wheel)
        }
      }
    }
    if (['bush', 'rock', 'tree'].includes(entity.kind)) prop.userData.vegetation = true
    addRendered(entity, prop)
    if (entity.kind === 'car' || entity.kind === 'pedestrian') {
      prop.userData.actorKind = entity.kind
      animatedActors.push({ object: prop, baseZ: entity.worldZ, phase: (entity.worldX + entity.worldZ) * .01, speed: entity.kind === 'car' ? .12 : .06 })
    }
  }
  const loader = new GLTFLoader()
  const objLoader = new OBJLoader()
  const prototypes = new Map()
  for (const [kind, file] of [['tree', 'CommonTree_1.gltf'], ['bush', 'Bush_Common.gltf'], ['rock', 'Pebble_Round_1.gltf']]) {
    try { prototypes.set(kind, (await loader.loadAsync(`../assets/${file}`)).scene) } catch (error) { console.warn(`asset load failed: ${file}`, error) }
  }
  frameAssets.push(...prototypes.values())
  if (buildToken !== nearBuildToken) return
  let buildingPrototype
  try { buildingPrototype = await objLoader.loadAsync('../assets/2Story_GableRoof.obj') } catch (error) { console.warn('asset load failed: 2Story_GableRoof.obj', error) }
  if (buildingPrototype) frameAssets.push(buildingPrototype)
  if (buildToken !== nearBuildToken) return
  const buildingVariants = []
  for (const file of ['1Story_GableRoof.obj', '2Story_Wide.obj', '2Story_RoundRoof.obj']) {
    try { buildingVariants.push(await objLoader.loadAsync(`../assets/${file}`)) } catch (error) { console.warn(`asset load failed: ${file}`, error) }
  }
  frameAssets.push(...buildingVariants)
  if (buildToken !== nearBuildToken) return
  // Pavement meshes are authored by Rust on the final raster. Do not round
  // coordinates, divide Y by four, re-ground, or invent missing paths here.
  for (const surface of scene.accessSurfaces ?? []) {
    if (hiddenRenderLayers.has('access')) continue
    if (surface.version !== 1) throw new Error('Unsupported access surface version')
    const vertices = surface.vertices
    if (!verticesOverlapChunks(vertices,activeChunks)) continue
    const key = 'access:' + JSON.stringify([scene.sceneId, surface.entityId, surface.anchorId])
    let pavement = entityMeshCache.get(key)
    if (!pavement) {
      const geometry = new THREE.BufferGeometry()
      geometry.setAttribute('position', new THREE.Float32BufferAttribute(vertices.flat(), 3))
      geometry.setIndex(surface.triangles.flat())
      geometry.computeVertexNormals()
      pavement = new THREE.Mesh(geometry, new THREE.MeshStandardMaterial({
        color: 0x4a4941, roughness: 1, metalness: 0,
        polygonOffset: true, polygonOffsetFactor: -1, polygonOffsetUnits: -1,
      }))
      pavement.name = key
      pavement.receiveShadow = true
      entityMeshCache.set(key, pavement)
    }
    group.add(pavement)
  }
  const landscaping=scene.landcoverSurfaces
  let renderedYardPatches=0
  if (landscaping) {
    if (hiddenRenderLayers.has('landcover')) {
      // Diagnostic layer switch; ordinary previews keep every baked surface.
    } else
    if(![1,2].includes(landscaping.version)) throw new Error('Unsupported landcover surface version')
    for(const patch of landscaping.patches) {
      // Pavement patches only account for clipped areas of the existing
      // accessSurfaces mesh: drawing them again would create z-fighting.
      if(patch.material==='access_pavement') continue
      const palette={
        managed_grass:0x65804a,access_pavement:0x99938a,public_open_space:0x7f9a62,
        pasture_ground:0x8fa65f,cultivated_soil:0x79603d,field_headland:0xa1946c,
        farm_track:0x958576,conservation_ground:0x466b55,
        conservation_forest_floor:0x355b42,conservation_wetland:0x527b67,
        rural_meadow:0x6f8c4f,rural_woodland_floor:0x496b4a,
        rural_rough_ground:0x777264,urban_vacant_ground:0x7d8a5d,
        unassigned_ground:0x6c7361,water_surface:0x365e6d
      }
      const landcoverColor=palette[patch.material]
      if(landcoverColor===undefined){console.warn('unknown landcover material; using neutral ground',patch.material)}
      const vertices=patch.vertices
      if(!verticesOverlapChunks(vertices,activeChunks)) continue
      const key='landcover:'+patch.id
      let mesh=entityMeshCache.get(key)
      if(!mesh){
        const geometry=new THREE.BufferGeometry()
        geometry.setAttribute('position',new THREE.Float32BufferAttribute(vertices.flat(),3))
        geometry.setIndex(patch.triangles.flat());geometry.computeVertexNormals()
        mesh=new THREE.Mesh(geometry,new THREE.MeshStandardMaterial({color:landcoverColor??0x6b7d5a,roughness:1,metalness:0,polygonOffset:true,polygonOffsetFactor:-1,polygonOffsetUnits:-1}))
        mesh.name=key;mesh.receiveShadow=true;mesh.userData.landcoverMaterial=patch.material
        entityMeshCache.set(key,mesh)
      }
      group.add(mesh);if(patch.material==='managed_grass')renderedYardPatches+=1
    }
  }
  window.glyphweaveFeedback.visualChecks.landcoverGrassPatches=renderedYardPatches
  window.glyphweaveFeedback.visualChecks.agricultureFields=landscaping?.agriculture?.fields.length??0
  // Consume Rust-authoritative plants through bounded chunk/type instance batches.
  ecologyRenderer ??= createEcologyRenderer(THREE)
  const ecologyStats = ecologyRenderer.update(scene, activeChunks, group)
  window.glyphweaveFeedback.visualChecks.ecologyPlants = ecologyStats.activePlants
  window.glyphweaveFeedback.visualChecks.ecologyInstancing = ecologyStats

  // Trees are instanced per chunk so a 3x3 area with hundreds of trees costs
  // just a few draw calls instead of one Group per tree. We collect every tree
  // in the active chunks (not LOD-filtered) so the cached InstancedMesh stays
  // complete across navigations.
  const treeGroups = new Map()
  for (const entity of scene.entities ?? []) {
    if (entity.kind !== 'tree' || !vegetationFitsTerrain(entity)) continue
    const inActive = activeChunks.some(chunk => entity.worldX >= chunk.worldX && entity.worldX < chunk.worldX + chunk.validWidthM && entity.worldZ >= chunk.worldZ && entity.worldZ < chunk.worldZ + chunk.validDepthM)
    if (!inActive) continue
    const key = `${Math.floor(entity.worldX / 512)},${Math.floor(entity.worldZ / 512)}`
    let list = treeGroups.get(key)
    if (!list) { list = []; treeGroups.set(key, list) }
    list.push(entity)
  }
  for (const [key, list] of treeGroups) {
    const cacheKey = `trees:${scene.sceneId}/${key}`
    let inst = entityMeshCache.get(cacheKey)
    if (!inst) {
      inst = buildTreeInstances(list, THREE, terrainAt, worldWind)
      entityMeshCache.set(cacheKey, inst)
    }
    const eligibleTreeIds = list.filter(entity => {
      const terrain = terrainAt?.(entity.worldX, entity.worldZ)
      return !terrain || (terrain.disturbance <= .04 && terrain.surface !== 3 && terrain.slope < .78 && terrain.wetness > .12)
    }).map(entity => entity.entityId)
    const treeParts = inst.children.filter(child => child.isInstancedMesh && child.geometry)
      .map(child => ({batchUuid: child.uuid, geometryUuid: child.geometry.uuid, part: child.name || child.type}))
    inst.userData.entityInstanceBindings = eligibleTreeIds.map((id, instanceIndex) => ({
      id, instanceIndex, count: eligibleTreeIds.length,
      parts: treeParts.map(part => ({...part, instanceIndex, instanceCount: inst.children.find(child => child.uuid === part.batchUuid)?.count ?? 0}))
    }))
    inst.traverse(o => { if (o.isMesh) o.userData.vegetation = true })
    group.add(inst)
    if (inst.userData?.wind) window.__treeWind = inst.userData.wind
  }
  // Rocks are instanced per scene so a field of boulders costs one draw call
  // (the LOD-filtered set already keeps the visible count reasonable).
  const activeRocks = (scene.entities ?? []).filter(entity => entity.kind === 'rock' && vegetationFitsTerrain(entity) && activeChunks.some(chunk => entity.worldX >= chunk.worldX && entity.worldX < chunk.worldX + chunk.validWidthM && entity.worldZ >= chunk.worldZ && entity.worldZ < chunk.worldZ + chunk.validDepthM))
  if (activeRocks.length) {
    const rockCacheKey = `rocks:${scene.sceneId}`
    let rockInst = entityMeshCache.get(rockCacheKey)
    if (!rockInst) {
      rockInst = buildRockInstances(activeRocks, THREE)
      entityMeshCache.set(rockCacheKey, rockInst)
    }
    const rockParts = rockInst.children.filter(child => child.isInstancedMesh && child.geometry)
      .map(child => ({batchUuid: child.uuid, geometryUuid: child.geometry.uuid, part: child.name || child.type}))
    rockInst.userData.entityInstanceBindings = activeRocks.map((entity, instanceIndex) => ({
      id: entity.entityId, instanceIndex, count: activeRocks.length,
      parts: rockParts.map(part => ({...part, instanceIndex, instanceCount: rockInst.children.find(child => child.uuid === part.batchUuid)?.count ?? 0}))
    }))
    rockInst.traverse(o => { if (o.isMesh) o.userData.vegetation = true })
    group.add(rockInst)
  }
  for (const entity of focusEntities) {
    // A planned building already has its authoritative envelope above.
    // Never draw a second coastal recipe or asset prototype on top of it.
    if (plannedBuildingIds.has(entity.entityId)) continue
    if (landmarkEntityIds.has(entity.entityId)) continue
    if (['rock_mountain', 'mountain', 'hill', 'shopping_mall', 'office_tower', 'convenience_store', 'fence', 'guardrail', 'boardwalk', 'scenic_belt', 'mudflat', 'night_market', 'community_plaza'].includes(entity.kind)) {
      addRendered(entity, makeCoastalFeature({ ...entity, type: entity.kind, worldY: groundedRenderY(entity) }, THREE))
      continue
    }
    if (entity.kind === 'mangrove') {
      addRendered(entity, makeMangrove(entity, THREE))
      continue
    }
    if (entity.kind === 'tree') continue
    if (entity.kind === 'storefront') continue
    if (['bush', 'fallen_log'].includes(entity.kind) && !vegetationFitsTerrain(entity)) continue
    const prototype = prototypes.get(entity.kind)
    if (['building', 'urban_building'].includes(entity.kind) && (buildingPrototype || buildingVariants.length)) {
      const source = [buildingPrototype, ...buildingVariants].filter(Boolean)[Math.abs(entity.worldX + entity.worldZ) % (buildingVariants.length + 1)]
      const prop = source.clone(true)
      prop.userData.sourceEntityId = entity.entityId
      const facadePalette = ['#8d7567', '#6c7d82', '#b08d69', '#7e8a6d', '#85758c', '#6f7770']
      const facadeColor = facadePalette[Math.abs(entity.worldX * 17 + entity.worldZ * 31) % facadePalette.length]
      prop.traverse(child => { if (child.material) child.material = new THREE.MeshStandardMaterial({ color: facadeColor, roughness: .82, metalness: .08 }) })
      fitAssetToEntity(prop, entity, THREE, { width: 12, depth: 12, height: 7 })
      const sign = new THREE.Mesh(new THREE.BoxGeometry(5, 2.2, .2), new THREE.MeshStandardMaterial({ color: '#c9b28a', roughness: .8 }))
      sign.position.set(0, 4.2, entity.depthM / 2 + .3)
      prop.add(sign)
      addRendered(entity, prop)
      continue
    }
    if (!prototype) continue
    const prop = prototype.clone(true)
    prop.userData.sourceEntityId = entity.entityId
    fitAssetToEntity(prop, entity, THREE, entity.kind === 'tree' ? { width: .7, depth: .7, height: 5 } : entity.kind === 'bush' ? { width: 1.5, depth: 1.5, height: 1.2 } : { width: 1, depth: 1, height: 1 })
    prop.traverse(o => { if (o.isMesh) o.userData.vegetation = true })
    addRendered(entity, prop)
  }
  // Shadows are costly with thousands of meshes; enable them only on the
  // terrain and built structures in the focus chunk, not vegetation/rocks,
  // so the focused frame renders fast.
  group.traverse(object => {
    if (!object.isMesh) return
    const isVegetation = object.userData?.vegetation === true
    object.receiveShadow = !isVegetation
    object.castShadow = !isVegetation
  })
  // Cinematic post-processing (DoF opt-in for street/explore, bloom + film
  // grade always on). Fall back to a plain render if the composer fails.
  let nearFx = null
  if (enableFx && postAddons) {
    try {
      nearFx = createPostFX({ THREE, addons: postAddons, renderer: nearRenderer, scene: nearScene, camera: nearCamera })
      if (window.__steppe) nearFx.bloom.strength = .2
      window.__nearFx = nearFx
    } catch (error) {
      console.warn('postfx unavailable, falling back to direct render', error)
    }
  }
  if (buildToken !== nearBuildToken) return
  nearRenderer.render(nearScene, nearCamera)
  window.glyphweaveFeedback.visualChecks.highDetailChunk = focusChunk ? `${focusChunk.chunkX},${focusChunk.chunkZ}` : null
  window.glyphweaveFeedback.visualChecks.loadedChunks = activeChunks.map(chunk => `${chunk.chunkX},${chunk.chunkZ}`)
  window.glyphweaveFeedback.visualChecks.nearCanvasReady = Boolean(webglCanvas.width && webglCanvas.height)
  window.glyphweaveFeedback.visualChecks.nearSceneRendered = true
  previewState = {...previewState, status: 'ready', completedGeneration: buildToken, error: null}
  setNearLoading(false)
  renderNearFrame = () => {
    if (!nearRenderer || !nearCamera || !nearScene) return
    // Do not call OrbitControls.update() here: update() dispatches `change`,
    // whose static-preview handler calls this function. Calling it recursively
    // exhausted the stack on the first 2.5D frame.
    if (nearFx) nearFx.render(1 / 60)
    else nearRenderer.render(nearScene, nearCamera)
  }
  if (!STATIC_PREVIEW) nearRenderer.setAnimationLoop(() => {
    const now = performance.now() * .001
    for (const layer of window.__steppeGrassLayers ?? []) {
      layer.update(now)
      layer.uniforms.uCameraPos.value.copy(nearCamera.position)
    }
    if (!STATIC_PREVIEW) {
      if (worldWind) worldWind.uTime.value = now
      for (const actor of animatedActors) {
        actor.object.position.z = actor.baseZ + Math.sin(now * actor.speed + actor.phase) * (actor.object.userData.actorKind === 'car' ? 8 : 3)
      }
      for (const maps of waterTextures) {
        maps.texture.offset.x = (maps.texture.offset.x + .00025) % 1
        maps.texture.offset.y = (maps.texture.offset.y + .00055) % 1
        maps.normalTexture.offset.x = (maps.normalTexture.offset.x + .00025) % 1
        maps.normalTexture.offset.y = (maps.normalTexture.offset.y + .00055) % 1
      }
      for (const wu of animatedWater) wu.uTime.value = now
    }
    if (edgePointer) {
      const edge = 54
      const speed = 1.8
      let dx = 0; let dz = 0
      if (edgePointer.x < edge) dx = -speed * (1 - edgePointer.x / edge)
      if (edgePointer.x > edgePointer.width - edge) dx = speed * (1 - (edgePointer.width - edgePointer.x) / edge)
      if (edgePointer.y < edge) dz = -speed * (1 - edgePointer.y / edge)
      if (edgePointer.y > edgePointer.height - edge) dz = speed * (1 - (edgePointer.height - edgePointer.y) / edge)
      const nextX = Math.max(0, Math.min(scene.widthM, nearControls.target.x + dx))
      const nextZ = Math.max(0, Math.min(scene.depthM, nearControls.target.z + dz))
      const actualDx = nextX - nearControls.target.x
      const actualDz = nextZ - nearControls.target.z
      nearCamera.position.x += actualDx; nearCamera.position.z += actualDz
      nearControls.target.x = nextX; nearControls.target.z = nextZ
    }
    if (isExplore && exploreKeys.size) {
      const forward = new THREE.Vector3()
      nearCamera.getWorldDirection(forward)
      forward.y = 0
      forward.normalize()
      const right = new THREE.Vector3(-forward.z, 0, forward.x)
      const speed = exploreKeys.has('ShiftLeft') ? 2.8 : 1.2
      const move = new THREE.Vector3()
      if (exploreKeys.has('KeyW')) move.add(forward)
      if (exploreKeys.has('KeyS')) move.sub(forward)
      if (exploreKeys.has('KeyD')) move.add(right)
      if (exploreKeys.has('KeyA')) move.sub(right)
      if (move.lengthSq()) {
        move.normalize().multiplyScalar(speed)
        nearCamera.position.add(move)
        nearControls.target.add(move)
      }
    }
    if (isExplore) {
      const ground = heightAt(nearCamera.position.x, nearCamera.position.z)
      nearCamera.position.y = Math.max(nearCamera.position.y, ground + 6)
      nearControls.target.y = Math.max(nearControls.target.y, nearCamera.position.y + 12)
    }
    // Throttle renders so a large scene (thousands of meshes) doesn't block
    // the main thread every frame; static ortho views render ~8fps, while
    // explore/street (perspective, need smooth motion) keep full frame rate.
    const frameMs = performance.now()
    if (isExplore || isStreet || frameMs - lastRenderMs >= 120) {
      nearControls.update()
      if (nearFx) nearFx.render(1 / 60)
      else nearRenderer.render(nearScene, nearCamera)
      lastRenderMs = frameMs
    }
  })
  if (STATIC_PREVIEW) {
    nearRenderer.setAnimationLoop(null)
    renderNearFrame()
  }
}

function updateNearCoordinateInfo() {
  if (!scene || !nearControls) return
  const x = Math.max(0, Math.min(scene.widthM - 1, Math.floor(nearControls.target.x)))
  const z = Math.max(0, Math.min(scene.depthM - 1, Math.floor(nearControls.target.z)))
  window.glyphweaveFeedback.cameraDistance = nearControls.getDistance()
  const viewName = nearPreset === 'explore' ? 'first-person' : nearPreset === 'skyline' ? 'skyline-2.5d' : nearPreset === 'street' ? 'street-2.5d' : nearPreset === 'waterfront' ? 'waterfront-2.5d' : nearPreset === 'bridgehead' ? 'bridgehead-2.5d' : 'harbour-2.5d'
  info.textContent = `${scene.sceneId}  X=${x} Z=${z}  terrain detail=${window.glyphweaveFeedback.visualChecks.highDetailChunk ? '1m geometry / 1m data' : 'coarse'}  view=${viewName}`
  const focusChunk = scene.chunks.find(chunk => x >= chunk.worldX - scene.originX && x < chunk.worldX - scene.originX + chunk.validWidthM && z >= chunk.worldZ - scene.originZ && z < chunk.worldZ - scene.originZ + chunk.validDepthM)
  const label = document.getElementById('chunk-label')
  if (label && focusChunk) {
    label.textContent = `chunk (${focusChunk.chunkX}, ${focusChunk.chunkZ})  world ${focusChunk.worldX},${focusChunk.worldZ}`
  }
}

function draw() {
  const entities = scene.entities ?? []
  const kinds = entities.reduce((result, entity) => {
    result[entity.kind] = (result[entity.kind] ?? 0) + 1
    return result
  }, {})
  window.glyphweaveFeedback = {
    sceneId: scene.sceneId,
    sizeM: [scene.widthM, scene.depthM],
    mode,
    chunks: scene.chunks.length,
    landmarks: scene.landmarks.length,
    entities: entities.length,
    entityKinds: kinds,
    visualChecks: {
      sceneLoaded: true,
      chunkIndexComplete: scene.chunks.length === scene.chunkCountX * scene.chunkCountZ,
      nearCanvasReady: mode === 'near' ? Boolean(webglCanvas?.width && webglCanvas?.height) : null,
    },
  }
  info.textContent = `${scene.sceneId}  ${scene.widthM}m 脳 ${scene.depthM}m  ${scene.chunkCountX}脳${scene.chunkCountZ} chunks`
  const isNearMode = mode === 'near'
  for (const id of ['chunk-up', 'chunk-down', 'chunk-left', 'chunk-right', 'chunk-label']) {
    const el = document.getElementById(id)
    if (el) el.classList.toggle('near', isNearMode)
  }
  if (mode === 'strategic') drawStrategic(); else requestNearDraw()
}
const bindClick = (id, handler) => {
  const element = document.querySelector(id)
  if (element) element.onclick = handler
}
bindClick('#strategic', () => { mode = 'strategic'; setNearLoading(false); if (webglCanvas) webglCanvas.style.display = 'none'; canvas.style.display = 'block'; draw() })
bindClick('#close', () => {
  const harbour = (scene?.entities ?? []).some(entity => ['boardwalk', 'boat', 'quay'].includes(entity.kind))
    || (scene?.landmarks ?? []).some(landmark => ['river', 'sea', 'boardwalk'].includes(landmark.type))
  // A non-harbour world must enter its near view around actual Rust content.
  // The harbour overview cameras assume a full 3x3 shoreline composition; on
  // a natural/steppe scene they exposed dark chunk frontiers and oversized
  // coarse terrain tiles instead of a readable near view.
  nearPreset = harbour ? 'harbour' : 'focus'
  mode = 'near'
  draw()
})
// Optional preview modes are intentionally omitted from the compact UI, but
// keep these bindings safe for older exported HTML pages that still include
// one or more of the buttons.
bindClick('#skyline', () => { nearPreset = 'skyline'; mode = 'near'; draw() })
bindClick('#street', () => { nearPreset = 'street'; mode = 'near'; draw() })
bindClick('#bridgehead', () => { nearPreset = 'bridgehead'; mode = 'near'; draw() })
bindClick('#explore', () => { nearPreset = 'explore'; mode = 'near'; draw() })
bindClick('#fullscreen', async () => { if (!document.fullscreenElement) await document.documentElement.requestFullscreen(); else await document.exitFullscreen(); resize() })
bindClick('#chunk-up', () => { if (window.__chunkNav) window.__chunkNav(0, 1) })
bindClick('#chunk-down', () => { if (window.__chunkNav) window.__chunkNav(0, -1) })
bindClick('#chunk-left', () => { if (window.__chunkNav) window.__chunkNav(-1, 0) })
bindClick('#chunk-right', () => { if (window.__chunkNav) window.__chunkNav(1, 0) })
sceneSelect.onchange = loadScene
window.onresize = resize
fetch('../world.json').then(response => response.json()).then(async data => { world = data; title.textContent = data.name; for (const path of data.scenes) { const option = document.createElement('option'); option.textContent = path.split('/')[1]; sceneSelect.append(option) } await loadScene(); resize() })
