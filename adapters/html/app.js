import { color, colorRgb, hashNoise, smoothNoise, fractalNoise, deterministicRand } from './core/shared.js'
import { makeSurfaceTexture, makeLodTexture, makeStripGeometry, makeArtisticGroundTexture, makeSteppeGrass, makeMCTerrain, makeSteppeGroundMaterial } from './render/terrain.js'
import { makeGrassBlades } from './render/grass.js'
import { createPostFX } from './render/postfx.js'
import { makeWaterTexture, makeRiverGeometry, makeWaterMaterial } from './render/water.js'
import { makeSkyTexture } from './render/sky.js'
import { makeVoxelTree, buildTreeInstances, makeMangrove, makeForestPatch, buildRockInstances } from './render/vegetation.js'
import { makeWaterWell, makeRoadSign, makeStreetLamp, makePedestrian, makeFoodStall } from './render/props.js'
import { fitAssetToEntity, addBridgeBeam, makeBridgeStructure, makeHongKongTower, applySlopeFoundation, makeResidentialBlock, makeResidentialTower, makeResidentialHome, makeResortLodge, makeSchool, makeCommercialCenter, makeEntertainmentCenter, makeParkingLot, makeTemple, makeChurch, makeFarmland, makePasture, makeCanal, makeTownHall, makeMarket, makeIndustrial, makeStorefront } from './render/buildings.js'

const canvas = document.querySelector('#map')
const context = canvas.getContext('2d')
const title = document.querySelector('#title')
const info = document.querySelector('#info')
const sceneSelect = document.querySelector('#scene')
let world
let scene
let mode = 'strategic'
let nearPreset = 'harbour'
// URL presets: ?view=near|skyline|street|explore opens straight into a 3D view.
const urlView = new URLSearchParams(location.search).get('view')
if (urlView) { mode = 'near'; nearPreset = ['skyline', 'street', 'explore'].includes(urlView) ? urlView : 'harbour' }
let webglCanvas
let nearRenderer
let nearScene
let nearCamera
let nearControls
let exploreKeys = new Set()
let exploreInputCleanup
const chunkDataCache = new Map()
const entityMeshCache = new Map()
const entityGroupCache = new Map()
let nearGroupRoot = null
let activeChunkKeys = new Set()

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
  if (window.__nearFx) {
    window.__nearFx.setSize(window.innerWidth, window.innerHeight)
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


async function loadScene() {
  scene = await fetch(`../${world.scenes[sceneSelect.selectedIndex]}`).then(response => response.json())
  draw()
}

function drawStrategicOverlays(ox, oz, scale) {
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

function drawStrategic() {
  const scale = Math.min(canvas.clientWidth / scene.widthM, canvas.clientHeight / scene.depthM)
  const ox = (canvas.clientWidth - scene.widthM * scale) / 2
  const oz = (canvas.clientHeight - scene.depthM * scale) / 2
  const background = context.createLinearGradient(0, 0, 0, canvas.clientHeight)
  background.addColorStop(0, '#152a38'); background.addColorStop(.72, '#23332f'); background.addColorStop(1, '#111816')
  context.fillStyle = background; context.fillRect(0, 0, canvas.clientWidth, canvas.clientHeight)
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

async function drawNear() {
  const THREE = await import('three')
  const { GLTFLoader } = await import('three/addons/loaders/GLTFLoader.js')
  const { OrbitControls } = await import('three/addons/controls/OrbitControls.js')
  const { OBJLoader } = await import('three/addons/loaders/OBJLoader.js')
  const postAddons = await Promise.all([
    import('three/addons/postprocessing/EffectComposer.js'),
    import('three/addons/postprocessing/RenderPass.js'),
    import('three/addons/postprocessing/BokehPass.js'),
    import('three/addons/postprocessing/UnrealBloomPass.js'),
    import('three/addons/postprocessing/ShaderPass.js'),
    import('three/addons/postprocessing/OutputPass.js'),
  ]).then(modules => ({ EffectComposer: modules[0].EffectComposer, RenderPass: modules[1].RenderPass, BokehPass: modules[2].BokehPass, UnrealBloomPass: modules[3].UnrealBloomPass, ShaderPass: modules[4].ShaderPass, OutputPass: modules[5].OutputPass }))
  const viewport = document.querySelector('#viewport')
  const viewportWidth = viewport.clientWidth
  const viewportHeight = viewport.clientHeight
  canvas.style.display = 'none'
  webglCanvas ??= document.createElement('canvas')
  webglCanvas.style.width = '100%'
  webglCanvas.style.height = '100%'
  webglCanvas.style.display = 'block'
  document.querySelector('#viewport').append(webglCanvas)
  if (nearControls) { nearControls.dispose(); nearControls = null }
  if (exploreInputCleanup) { exploreInputCleanup(); exploreInputCleanup = null }
  if (!nearRenderer) {
    nearRenderer = new THREE.WebGLRenderer({ canvas: webglCanvas, antialias: true })
    nearRenderer.setSize(viewportWidth, viewportHeight, false)
    nearRenderer.setPixelRatio(Math.min(devicePixelRatio, 2))
    nearRenderer.shadowMap.enabled = true
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
  const isStreetView = isStreet || isExplore
  // Focus / 2.5d near view uses a street-block scale (~120m) so doors,
  // windows, street lamps and lane markings are readable inside a single
  // 512m chunk. Skyline stays full-world; street/explore are perspective.
  const orthoHeight = isSkyline ? Math.max(scene.widthM, scene.depthM) * .92 : isStreet ? 220 : 120
  const orthoWidth = orthoHeight * viewportWidth / Math.max(1, viewportHeight)
  nearCamera = isExplore || isStreet
    ? new THREE.PerspectiveCamera(isStreet ? 54 : 68, viewportWidth / viewportHeight, .1, 12000)
    : new THREE.OrthographicCamera(-orthoWidth / 2, orthoWidth / 2, orthoHeight / 2, -orthoHeight / 2, -orthoHeight, 12000)
  const localCoordinate = entity => ({ x: entity.worldX - scene.originX, z: entity.worldZ - scene.originZ })
  const focusEntity = (scene.entities ?? []).find(entity => entity.kind === 'bridge') ?? (scene.entities ?? []).find(entity => {
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
  }) ?? (scene.entities ?? []).find(entity => ['tree', 'bench', 'grass_clump'].includes(entity.kind))
  const bridgeLength = focusEntity?.kind === 'bridge' ? (focusEntity.widthM || 4588) : 0
  const focusOverride = window.__nearFocusOverride
  const harbour = (scene.landmarks ?? []).find(landmark => landmark.type === 'river')
  const centreDist = e => Math.hypot(e.worldX - (scene.originX + scene.widthM / 2), e.worldZ - (scene.originZ + scene.depthM / 2))
  const streetAnchor = (scene.entities ?? []).filter(entity => entity.kind === 'storefront').sort((a, b) => centreDist(a) - centreDist(b))[0]
    ?? (scene.entities ?? []).filter(entity => entity.kind === 'street_lamp').sort((a, b) => centreDist(a) - centreDist(b))[0]
    ?? (scene.entities ?? []).filter(entity => entity.kind === 'road').sort((a, b) => centreDist(a) - centreDist(b))[0]
  const streetX = streetAnchor?.worldX ?? (harbour ? harbour.worldX + harbour.widthM / 2 + 150 : scene.widthM * .68)
  const streetZ = streetAnchor?.worldZ ?? scene.depthM * .5
  const focusX = focusOverride ? focusOverride.x : isSkyline ? scene.widthM * .5 : isStreetView ? streetX : focusLandmark ? focusLandmark.worldX - scene.originX : (focusEntity ? focusEntity.worldX - scene.originX + (nearPreset === 'bridgehead' ? bridgeLength / 2 - 70 : 0) : scene.widthM * .5)
  const focusZ = focusOverride ? focusOverride.z : isSkyline ? scene.depthM * .5 : isStreetView ? streetZ : focusLandmark ? focusLandmark.worldZ - scene.originZ : (focusEntity ? focusEntity.worldZ - scene.originZ : scene.depthM * .5)
  const focusY = isSkyline || isStreetView ? 0 : focusLandmark ? focusLandmark.worldY : focusEntity ? focusEntity.worldY : 0
  const isBridgeFocus = focusEntity?.kind === 'bridge'
  const isLandmarkFocus = Boolean(focusLandmark) && !isSkyline && !isStreetView
  const focusTargetY = isBridgeFocus ? focusY + 10 : isLandmarkFocus ? focusY + (focusLandmark.heightM || 100) * .45 : focusY + 6
  const focusTargetZ = isStreet ? focusZ + 90 : focusZ
  const viewOffsetX = nearPreset === 'bridgehead' ? 100 : 0
  const viewOffsetZ = nearPreset === 'bridgehead' ? 105 : 72
  const landmarkViewDistance = Math.max(180, (focusLandmark?.heightM || 100) * 1.15)
  nearCamera.position.set(
    focusX + viewOffsetX + (isExplore ? 130 : isStreet ? 150 : isLandmarkFocus ? orthoHeight * .72 : isSkyline ? orthoHeight * .3 : orthoHeight * .5),
    focusY + (isExplore ? 8 : isStreet ? 10 : isBridgeFocus ? 34 : isLandmarkFocus ? (focusLandmark.heightM || 100) * 1.08 : isSkyline ? orthoHeight * .78 : orthoHeight * .95) + (isLandmarkFocus && !isExplore ? orthoHeight * .2 : 0),
    focusZ + (isExplore ? 125 : isStreet ? -90 : isBridgeFocus ? viewOffsetZ : isLandmarkFocus ? landmarkViewDistance : isSkyline ? orthoHeight * .72 : orthoHeight * .42),
  )
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
    const urban = (scene.entities ?? []).some(e => ['storefront', 'residential_block', 'residential_tower', 'building_tower'].includes(e.kind))
    const steppe = !urban
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
      nearScene.add(new THREE.HemisphereLight(0xdce8df, 0x202820, 2))
      const sun = new THREE.DirectionalLight(0xffe6b0, 2.8)
      sun.position.set(-500, 900, 420)
      sun.castShadow = false
      nearScene.add(sun)
    }
  }
  nearControls = new OrbitControls(nearCamera, webglCanvas)
  nearControls.target.set(focusX, isExplore ? focusY + 26 : isStreet ? focusY + 9 : focusTargetY, focusTargetZ)
  nearControls.enableDamping = true
  nearControls.enablePan = true
  nearControls.enableRotate = true
  nearControls.mouseButtons.LEFT = isExplore || isStreet ? THREE.MOUSE.ROTATE : THREE.MOUSE.PAN
  nearControls.mouseButtons.MIDDLE = THREE.MOUSE.PAN
  nearControls.mouseButtons.RIGHT = isExplore ? THREE.MOUSE.PAN : THREE.MOUSE.ROTATE
  nearControls.minDistance = 10
  nearControls.maxDistance = 2400
  nearControls.maxPolarAngle = isExplore ? Math.PI * .8 : Math.PI * .48
  nearControls.addEventListener('change', updateNearCoordinateInfo)
  window.__nearControls = nearControls
  window.__nearCamera = nearCamera
  window.__focusAt = (x, z) => { window.__nearFocusOverride = { x, z }; nearPreset = 'focus'; mode = 'near'; draw() }
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
  nearGroupRoot.add(group)
  const focusChunk = scene.chunks.find(chunk => focusX >= chunk.worldX - scene.originX && focusX < chunk.worldX - scene.originX + chunk.validWidthM && focusZ >= chunk.worldZ - scene.originZ && focusZ < chunk.worldZ - scene.originZ + chunk.validDepthM)
  const activeChunks = focusChunk
    ? scene.chunks.filter(chunk => Math.abs(chunk.chunkX - focusChunk.chunkX) <= 1 && Math.abs(chunk.chunkZ - focusChunk.chunkZ) <= 1)
    : scene.chunks.slice(0, 1)
  // Cache each active chunk's terrain mesh so navigating back is instant.
  const chunkGroupMap = new Map()
  for (const chunk of activeChunks) {
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
      entityGroupCache.set(entity.entityId, built)
    }
    return built
  }
  const addEntityToGroup = (entity, built) => {
    if (built) group.add(built)
  }
  const heightFields = []
  const waterTextures = []
  const animatedActors = []
  const animatedWater = []
  const sceneCacheKey = scene.sceneId
  for (const chunk of activeChunks) {
    const cacheKey = `${sceneCacheKey}/${chunk.chunkX}/${chunk.chunkZ}`
    const chunkGroup = chunkGroupMap.get(cacheKey)
    const alreadyBuilt = chunkGroup.children.length > 0
    let heightView, surface
    const cached = chunkDataCache.get(cacheKey)
    if (cached) {
      heightView = cached.heightView
      surface = cached.surface
    } else {
      const [heightResponse, surfaceResponse] = await Promise.all([fetch(`../scenes/${scene.sceneId}/${chunk.heightFile}`), fetch(`../scenes/${scene.sceneId}/${chunk.surfaceFile}`)])
      const bytes = await heightResponse.arrayBuffer()
      heightView = new DataView(bytes)
      surface = new Uint8Array(await surfaceResponse.arrayBuffer())
      chunkDataCache.set(cacheKey, { heightView, surface })
    }
    const width = chunk.validWidthM; const depth = chunk.validDepthM
    heightFields.push({ chunk, heightView, width, depth })
    // Only the focus chunk renders at 1m terrain detail; its immediate
    // neighbours use a finer 2m mesh (with a matching finer texture) so the
    // ground stays readable when panning. The outer ring keeps a coarse 8m
    // mesh so navigation doesn't rebuild huge geometries.
    const isFocusChunk = focusChunk && chunk.chunkX === focusChunk.chunkX && chunk.chunkZ === focusChunk.chunkZ
    const chunkDist = focusChunk ? Math.max(Math.abs(chunk.chunkX - focusChunk.chunkX), Math.abs(chunk.chunkZ - focusChunk.chunkZ)) : 9
    const meshStep = isFocusChunk ? 1 : chunkDist <= 1 ? 2 : 8
    const builtStep = chunkGroup.userData.meshStep ?? 0
    if (alreadyBuilt && builtStep === meshStep) continue
    // Remove any prior-step mesh before adding the correct one.
    for (let i = chunkGroup.children.length - 1; i >= 0; i--) {
      if (chunkGroup.children[i].isMesh) chunkGroup.remove(chunkGroup.children[i])
    }
    const meshCacheKey = `${cacheKey}/step${meshStep}`
    let mesh = entityMeshCache.get(meshCacheKey)
    if (!mesh) {
      // Steppe: flat vertex colours graded by height (MC / Journey style, no
      // texture map). Urban: the existing noise-mapped terrain.
      if (window.__steppe) {
        const geometry = makeMCTerrain(heightView, width, depth, meshStep, THREE)
        // MeshStandardMaterial handles vertex-color colour management correctly
        // (Lambert can wash the greens toward tan under these lights).
        const material = new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 1 })
        mesh = new THREE.Mesh(geometry, material)
      } else {
        const geometry = new THREE.PlaneGeometry(width, depth, Math.min(width - 1, Math.ceil(width / meshStep)), Math.min(depth - 1, Math.ceil(depth / meshStep))); geometry.rotateX(-Math.PI / 2)
        const positions = geometry.attributes.position
        for (let i = 0; i < positions.count; i++) { const x = Math.min(width - 1, Math.max(0, Math.round(positions.getX(i) + width / 2))); const z = Math.min(depth - 1, Math.max(0, Math.round(positions.getZ(i) + depth / 2))); positions.setY(i, heightView.getInt16((z * width + x) * 2, true) / 4) }
        geometry.computeVertexNormals(); const material2 = meshStep <= 2 ? (() => { const surfaceMaps = makeSurfaceTexture(surface, width, depth, THREE); surfaceMaps.texture.anisotropy = Math.min(8, nearRenderer.capabilities.getMaxAnisotropy()); surfaceMaps.normalTexture.anisotropy = surfaceMaps.texture.anisotropy; return new THREE.MeshStandardMaterial({ map: surfaceMaps.texture, normalMap: surfaceMaps.normalTexture, normalScale: new THREE.Vector2(.45, .45), roughness: .92 }) })() : new THREE.MeshStandardMaterial({ map: makeLodTexture(surface, width, depth, THREE), roughness: 1 }); mesh = new THREE.Mesh(geometry, material2)
      }
      entityMeshCache.set(meshCacheKey, mesh)
    }
    const meshClone = mesh.clone()
    meshClone.position.set(chunk.worldX - scene.originX + width / 2, 0, chunk.worldZ - scene.originZ + depth / 2)
    chunkGroup.add(meshClone)
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
    if (!field) return 0
    const x = Math.max(0, Math.min(field.width - 1, Math.floor(worldX - field.chunk.worldX)))
    const z = Math.max(0, Math.min(field.depth - 1, Math.floor(worldZ - field.chunk.worldZ)))
    return field.heightView.getInt16((z * field.width + x) * 2, true) / 4
  }
  // Steppe grass: GPU-instanced curved blades with wind + backlight over the
  // focused chunk (Journey/Flower style). Density concentrates near the camera
  // so the visible foreground reads lush. Skyline (whole-world view) gets a
  // higher count + wider core so the distant plain stays grassy.
  if (window.__steppe && focusChunk && !new URLSearchParams(location.search).has('nograss')) {
    const sky = isSkyline
    const grass = makeGrassBlades(THREE, focusChunk.worldX, focusChunk.worldZ, focusChunk.validWidthM, focusChunk.validDepthM, heightAt, sky ? 1200000 : 900000, 7, focusX, focusZ, { focusR: sky ? 480 : 320, farR: sky ? 900 : 700 })
    grass.uniforms.uSunDir.value.copy(window.__nearSteppeSunDir ?? new THREE.Vector3(-.45, .82, .3).normalize())
    group.add(grass.mesh)
    window.__steppeGrass = grass
  }
  for (const landmark of scene.landmarks) {
    const groundedLandmark = { ...landmark, worldY: heightAt(landmark.worldX, landmark.worldZ) }
    if (focusChunk && !['bridge', 'river', 'pagoda', 'park', 'station', 'ferris_wheel', 'boat', 'building_tower', 'mangrove', 'nature_reserve', 'food_stall'].includes(landmark.type) && !inFocusChunk(landmark)) continue
    const type = landmark.type
    if (type === 'bridge') {
      group.add(makeBridgeStructure(landmark, THREE, heightAt))
      continue
    }
    if (type === 'food_stall') {
      group.add(makeFoodStall(landmark, THREE))
      continue
    }
    if (type === 'mangrove' || type === 'nature_reserve') {
      group.add(makeMangrove(landmark, THREE))
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
      group.add(pavilion)
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
      group.add(pagoda)
      continue
    }
    if (type === 'building_tower') {
      group.add(makeHongKongTower(groundedLandmark, THREE))
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
      group.add(park)
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
      group.add(station)
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
      group.add(wheel)
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
      group.add(boat)
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
      const wm = makeWaterMaterial(THREE, type === 'river')
      material = wm.material
      waterUniforms = wm.uniforms
      waterUniforms.uTime.value = waterUniforms.uTime.value
      animatedWater.push(wm.uniforms)
      material.map = waterMaps?.texture ?? null
      material.normalMap = waterMaps?.normalTexture ?? null
      material.normalScale = new THREE.Vector2(.7, .7)
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
    if (isWater) { marker.castShadow = false; marker.receiveShadow = false }
    group.add(marker)
  }
  const focusEntities = isSkyline
    ? (scene.entities ?? [])
    : (scene.entities ?? []).filter(entity =>
        entity.kind === 'bridge'
        || inFocusChunk(entity)
        || inStreetFrame(entity)
        || entity.kind === 'storefront'
        || entity.kind === 'pedestrian'
        || ['residential_block', 'residential_home', 'residential_tower', 'school', 'industrial', 'town_hall', 'market', 'commercial_center', 'parking_lot'].includes(entity.kind)
        || (['tree', 'bush', 'rock', 'grass_clump'].includes(entity.kind) && shouldRenderVegetation(entity, 7))
      )
  for (const entity of focusEntities) {
    if (entity.kind === 'storefront') {
      if (isStreetView && !inStreetFrame(entity)) continue
      group.add(makeStorefront(entity, THREE))
      continue
    }
    if (entity.kind === 'food_stall') {
      group.add(makeFoodStall(entity, THREE))
      continue
    }
    if (entity.kind === 'pedestrian') {
      const person = makePedestrian(entity, THREE)
      group.add(person)
      person.userData.actorKind = entity.kind
      animatedActors.push({ object: person, baseZ: entity.worldZ, phase: (entity.worldX + entity.worldZ) * .01, speed: .06 })
      continue
    }
    if (isSkyline && (entity.kind === 'building' || entity.kind === 'urban_building' || entity.kind === 'building_tower')) {
      const height = entity.kind === 'building_tower' ? entity.heightM : Math.max(6, entity.heightM)
      const skylineMaterial = new THREE.MeshStandardMaterial({ color: entity.kind === 'building_tower' ? '#536b78' : '#665f5b', roughness: .7, metalness: .2 })
      const skylineBuilding = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM, height, entity.depthM), skylineMaterial)
      skylineBuilding.position.set(entity.worldX, entity.worldY + height / 2, entity.worldZ)
      group.add(skylineBuilding)
      continue
    }
    if (entity.kind === 'bridge' && scene.landmarks.some(landmark => landmark.entityId === entity.entityId)) continue
    if (entity.kind === 'bridge') {
      group.add(makeBridgeStructure(entity, THREE, heightAt))
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
        const built = makeResidentialHome(entity, THREE, heightAt)
        applySlopeFoundation(built, entity, heightAt, THREE)
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
      group.add(makeWaterWell(entity, THREE))
      continue
    }
    if (entity.kind === 'road_sign') {
      group.add(makeRoadSign(entity, THREE))
      continue
    }
    if (entity.kind === 'street_lamp') {
      group.add(makeStreetLamp(entity, THREE))
      continue
    }
    const zoneKinds = ['parking_lot', 'commercial_center', 'entertainment_center', 'school', 'residential_block', 'canal', 'farmland', 'mountain_forest', 'green_space', 'temple', 'church', 'pasture', 'nature_reserve']
    const geometry = entity.kind === 'road' || entity.kind === 'sidewalk' ? makeStripGeometry(entity, entity.kind === 'sidewalk' ? 3 : 7, 0, heightAt, THREE, .45, undefined, undefined, entity.depthM > entity.widthM)
      : entity.kind === 'bridge' ? new THREE.BoxGeometry(entity.widthM || 120, entity.heightM || 8, entity.depthM || 24)
      : entity.kind === 'building_cluster' ? new THREE.BoxGeometry(120, 32, 100)
      : entity.kind === 'building_tower' ? new THREE.BoxGeometry(entity.widthM, entity.heightM, entity.depthM)
       : entity.kind === 'car' ? new THREE.BoxGeometry(entity.widthM, entity.heightM, entity.depthM)
       : entity.kind === 'pedestrian' ? new THREE.CapsuleGeometry(.28, 1.15, 4, 6)
      : entity.kind === 'grass_clump' ? new THREE.ConeGeometry(1.2, 3.5, 5)
      : entity.kind === 'reed' ? new THREE.CylinderGeometry(.16, .24, 3.8, 5)
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
      const material = new THREE.MeshStandardMaterial({ color: zoneColor ?? (entity.kind === 'road' ? '#786b5a' : entity.kind === 'sidewalk' ? '#b0a58f' : entity.kind === 'bridge' ? '#807664' : entity.kind === 'building_cluster' ? '#6c665d' : entity.kind === 'building_tower' ? '#7d8a94' : entity.kind === 'car' ? '#b8463a' : entity.kind === 'pedestrian' ? '#3e536b' : entity.kind === 'grass_clump' ? '#6d944f' : entity.kind === 'reed' ? '#718e45' : entity.kind === 'bench' ? '#756451' : entity.kind === 'lamp' ? '#6c675b' : entity.kind === 'fallen_log' ? '#70503a' : entity.kind === 'tree' ? '#47704a' : entity.kind === 'bush' ? '#5e8a4c' : entity.kind === 'rock' ? '#76716a' : '#746d63'), roughness: .95 })
    const prop = new THREE.Mesh(geometry, material)
     const baseHeight = zoneKinds.includes(entity.kind) ? entity.heightM / 2 : entity.kind === 'road' || entity.kind === 'sidewalk' ? 0 : entity.kind === 'bridge' ? 4 : entity.kind === 'building_cluster' ? 16       : entity.kind === 'building_tower' ? entity.heightM / 2 : entity.kind === 'car' ? .9 : entity.kind === 'pedestrian' ? .9 : entity.kind === 'building' ? 12 : entity.kind === 'grass_clump' ? 1.75 : entity.kind === 'reed' ? 1.9 : entity.kind === 'bench' ? .55 : entity.kind === 'lamp' ? 2.4 : entity.kind === 'fallen_log' ? .35 : entity.kind === 'tree' ? 14 : entity.kind === 'bush' ? 2 : 10
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
      const roadCenter = isStreetView ? entity.worldX : focusX
      const bed = new THREE.Mesh(makeStripGeometry(entity, entity.kind === 'sidewalk' ? 3.2 : 8, 0, heightAt, THREE, .1, undefined, roadCenter), new THREE.MeshStandardMaterial({ color: entity.kind === 'sidewalk' ? '#877e70' : '#4c4438', roughness: 1 }))
      bed.position.set(roadCenter - entity.worldX, 0, 0)
      prop.add(bed)
      if (entity.kind === 'road') {
        const marking = new THREE.Mesh(makeStripGeometry(entity, .28, 0, heightAt, THREE, .58, undefined, roadCenter), new THREE.MeshStandardMaterial({ color: '#d9c98c', roughness: .8 }))
        marking.position.set(roadCenter - entity.worldX, 0, 0)
        prop.add(marking)
        for (const side of [-1, 1]) {
          const curb = new THREE.Mesh(makeStripGeometry(entity, .18, side * 6.2, heightAt, THREE, .52, undefined, roadCenter), new THREE.MeshStandardMaterial({ color: '#a3957e', roughness: .95 }))
          curb.position.set(roadCenter - entity.worldX, 0, 0)
          prop.add(curb)
        }
      }
    } else if (entity.kind === 'lamp') {
      group.remove(prop)
      group.add(makeStreetLamp(entity, THREE))
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
    if (['bush', 'rock', 'tree', 'grass_clump', 'reed'].includes(entity.kind)) prop.userData.vegetation = true
    group.add(prop)
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
  let buildingPrototype
  try { buildingPrototype = await objLoader.loadAsync('../assets/2Story_GableRoof.obj') } catch (error) { console.warn('asset load failed: 2Story_GableRoof.obj', error) }
  const buildingVariants = []
  for (const file of ['1Story_GableRoof.obj', '2Story_Wide.obj', '2Story_RoundRoof.obj']) {
    try { buildingVariants.push(await objLoader.loadAsync(`../assets/${file}`)) } catch (error) { console.warn(`asset load failed: ${file}`, error) }
  }
  // Trees are instanced per chunk so a 3x3 area with hundreds of trees costs
  // just a few draw calls instead of one Group per tree. We collect every tree
  // in the active chunks (not LOD-filtered) so the cached InstancedMesh stays
  // complete across navigations.
  const treeGroups = new Map()
  for (const entity of scene.entities ?? []) {
    if (entity.kind !== 'tree') continue
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
      inst = buildTreeInstances(list, THREE)
      entityMeshCache.set(cacheKey, inst)
    }
    inst.traverse(o => { if (o.isMesh) o.userData.vegetation = true })
    group.add(inst)
    if (inst.userData?.wind) window.__treeWind = inst.userData.wind
  }
  // Rocks are instanced per scene so a field of boulders costs one draw call
  // (the LOD-filtered set already keeps the visible count reasonable).
  const activeRocks = (scene.entities ?? []).filter(entity => entity.kind === 'rock' && activeChunks.some(chunk => entity.worldX >= chunk.worldX && entity.worldX < chunk.worldX + chunk.validWidthM && entity.worldZ >= chunk.worldZ && entity.worldZ < chunk.worldZ + chunk.validDepthM))
  if (activeRocks.length) {
    const rockCacheKey = `rocks:${scene.sceneId}`
    let rockInst = entityMeshCache.get(rockCacheKey)
    if (!rockInst) {
      rockInst = buildRockInstances(activeRocks, THREE)
      entityMeshCache.set(rockCacheKey, rockInst)
    }
    rockInst.traverse(o => { if (o.isMesh) o.userData.vegetation = true })
    group.add(rockInst)
  }
  for (const entity of focusEntities) {
    if (entity.kind === 'tree') continue
    if (entity.kind === 'storefront') continue
    const prototype = prototypes.get(entity.kind)
    if (['building', 'urban_building'].includes(entity.kind) && (buildingPrototype || buildingVariants.length)) {
      const source = [buildingPrototype, ...buildingVariants].filter(Boolean)[Math.abs(entity.worldX + entity.worldZ) % (buildingVariants.length + 1)]
      const prop = source.clone(true)
      const facadePalette = ['#8d7567', '#6c7d82', '#b08d69', '#7e8a6d', '#85758c', '#6f7770']
      const facadeColor = facadePalette[Math.abs(entity.worldX * 17 + entity.worldZ * 31) % facadePalette.length]
      prop.traverse(child => { if (child.material) child.material = new THREE.MeshStandardMaterial({ color: facadeColor, roughness: .82, metalness: .08 }) })
      fitAssetToEntity(prop, entity, THREE, { width: 12, depth: 12, height: 7 })
      const sign = new THREE.Mesh(new THREE.BoxGeometry(5, 2.2, .2), new THREE.MeshStandardMaterial({ color: '#c9b28a', roughness: .8 }))
      sign.position.set(0, 4.2, entity.depthM / 2 + .3)
      prop.add(sign)
      group.add(prop)
      continue
    }
    if (!prototype) continue
    const prop = prototype.clone(true)
    fitAssetToEntity(prop, entity, THREE, entity.kind === 'tree' ? { width: .7, depth: .7, height: 5 } : entity.kind === 'bush' ? { width: 1.5, depth: 1.5, height: 1.2 } : { width: 1, depth: 1, height: 1 })
    prop.traverse(o => { if (o.isMesh) o.userData.vegetation = true })
    group.add(prop)
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
  if (!new URLSearchParams(location.search).has('nofx')) {
    try {
      nearFx = createPostFX({ THREE, addons: postAddons, renderer: nearRenderer, scene: nearScene, camera: nearCamera })
      if (window.__steppe) nearFx.bloom.strength = .2
      window.__nearFx = nearFx
    } catch (error) {
      console.warn('postfx unavailable, falling back to direct render', error)
    }
  }
  nearRenderer.render(nearScene, nearCamera)
  window.glyphweaveFeedback.visualChecks.highDetailChunk = focusChunk ? `${focusChunk.chunkX},${focusChunk.chunkZ}` : null
  window.glyphweaveFeedback.visualChecks.loadedChunks = activeChunks.map(chunk => `${chunk.chunkX},${chunk.chunkZ}`)
  window.glyphweaveFeedback.visualChecks.nearCanvasReady = Boolean(webglCanvas.width && webglCanvas.height)
  window.glyphweaveFeedback.visualChecks.nearSceneRendered = true
  nearRenderer.setAnimationLoop(() => {
    const now = performance.now() * .001
    if (window.__steppeGrass) {
      window.__steppeGrass.update(now)
      window.__steppeGrass.uniforms.uCameraPos.value.copy(nearCamera.position)
    }
    if (window.__treeWind) window.__treeWind.uTime.value = now
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
  let lastRenderMs = performance.now()
}

function updateNearCoordinateInfo() {
  if (!scene || !nearControls) return
  const x = Math.max(0, Math.min(scene.widthM - 1, Math.floor(nearControls.target.x)))
  const z = Math.max(0, Math.min(scene.depthM - 1, Math.floor(nearControls.target.z)))
  window.glyphweaveFeedback.cameraDistance = nearControls.getDistance()
  const viewName = nearPreset === 'explore' ? 'first-person' : nearPreset === 'skyline' ? 'skyline-2.5d' : nearPreset === 'street' ? 'street-2.5d' : nearPreset === 'bridgehead' ? 'bridgehead-2.5d' : 'harbour-2.5d'
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
  if (mode === 'strategic') drawStrategic(); else drawNear()
}
document.querySelector('#strategic').onclick = () => { mode = 'strategic'; if (webglCanvas) webglCanvas.style.display = 'none'; canvas.style.display = 'block'; draw() }
document.querySelector('#skyline').onclick = () => { nearPreset = 'skyline'; mode = 'near'; draw() }
document.querySelector('#close').onclick = () => { nearPreset = 'harbour'; mode = 'near'; draw() }
document.querySelector('#street').onclick = () => { nearPreset = 'street'; mode = 'near'; draw() }
document.querySelector('#bridgehead').onclick = () => { nearPreset = 'bridgehead'; mode = 'near'; draw() }
document.querySelector('#explore').onclick = () => { nearPreset = 'explore'; mode = 'near'; draw() }
document.querySelector('#fullscreen').onclick = async () => { if (!document.fullscreenElement) await document.documentElement.requestFullscreen(); else await document.exitFullscreen(); resize() }
document.querySelector('#chunk-up').onclick = () => { if (window.__chunkNav) window.__chunkNav(0, 1) }
document.querySelector('#chunk-down').onclick = () => { if (window.__chunkNav) window.__chunkNav(0, -1) }
document.querySelector('#chunk-left').onclick = () => { if (window.__chunkNav) window.__chunkNav(-1, 0) }
document.querySelector('#chunk-right').onclick = () => { if (window.__chunkNav) window.__chunkNav(1, 0) }
sceneSelect.onchange = loadScene
window.onresize = resize
fetch('../world.json').then(response => response.json()).then(async data => { world = data; title.textContent = data.name; for (const path of data.scenes) { const option = document.createElement('option'); option.textContent = path.split('/')[1]; sceneSelect.append(option) } await loadScene(); resize() })


