const canvas = document.querySelector('#map')
const context = canvas.getContext('2d')
const title = document.querySelector('#title')
const info = document.querySelector('#info')
const sceneSelect = document.querySelector('#scene')
let world
let scene
let mode = 'strategic'
let webglCanvas
let nearRenderer
let nearScene
let nearCamera
let nearControls
let nearPreset = 'harbour'
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
  if (scene) draw()
}

function color(surface) { return ['#526a4f', '#6f8050', '#69737a', '#456b73', '#aa9560', '#809357', '#3f6548', '#8d765c'][surface] ?? '#526a4f' }

function colorRgb(surface) { return [[.32, .42, .31], [.43, .52, .31], [.41, .45, .48], [.16, .42, .46], [.67, .58, .38], [.50, .60, .34], [.24, .39, .28], [.55, .46, .36]][surface] ?? [.32, .42, .31] }

function hashNoise(x, z) {
  const value = Math.sin(x * 127.1 + z * 311.7) * 43758.5453
  return value - Math.floor(value)
}

function smoothNoise(x, z) {
  const x0 = Math.floor(x); const z0 = Math.floor(z)
  const tx = x - x0; const tz = z - z0
  const sx = tx * tx * (3 - 2 * tx); const sz = tz * tz * (3 - 2 * tz)
  const a = hashNoise(x0, z0); const b = hashNoise(x0 + 1, z0)
  const c = hashNoise(x0, z0 + 1); const d = hashNoise(x0 + 1, z0 + 1)
  return (a + (b - a) * sx) * (1 - sz) + (c + (d - c) * sx) * sz
}

function fractalNoise(x, z) {
  return smoothNoise(x, z) * .58 + smoothNoise(x * 2.7, z * 2.7) * .29 + smoothNoise(x * 7.1, z * 7.1) * .13
}

function makeSurfaceTexture(surface, width, depth, THREE) {
  const textureCanvas = document.createElement('canvas')
  const textureScale = 3
  textureCanvas.width = width * textureScale
  textureCanvas.height = depth * textureScale
  const textureContext = textureCanvas.getContext('2d')
  const image = textureContext.createImageData(textureCanvas.width, textureCanvas.height)
  const variationField = new Float32Array(textureCanvas.width * textureCanvas.height)
  const isGrass = material => [0, 1, 5, 6].includes(material)
  for (let z = 0; z < textureCanvas.height; z++) for (let x = 0; x < textureCanvas.width; x++) {
    const cellX = Math.min(width - 1, Math.floor(x / textureScale))
    const cellZ = Math.min(depth - 1, Math.floor(z / textureScale))
    const material = surface[cellZ * width + cellX]
    const base = colorRgb(material)
    const detail = fractalNoise(x * .035 + material * 11, z * .035 - material * 7) - .5
    const grain = fractalNoise(x * .12, z * .12) - .5
    const wave = material === 3 ? Math.sin((x + z) * .16) * .028 + Math.sin(x * .42) * .014 : 0
    // Stone / paved surfaces (2) get a real paver grid: visible joints every
    // ~1.2m (textureScale 3 => 3.6 texels) plus per-slab tone variation, so
    // plazas and pavements read as tiled stone, not grey noise.
    const stoneVein = material === 2 ? (fractalNoise(x * .055, z * .055) - .5) * .06 : 0
    let paverGrid = 0
    if (material === 2) {
      const gx = Math.floor(x / 3.6)
      const gz = Math.floor(z / 3.6)
      const jx = (x / 3.6) - gx
      const jz = (z / 3.6) - gz
      // Mortar joints (dark lines) at slab edges.
      const joint = (jx > .94 || jz > .94) ? -.16 : 0
      // Offset alternate rows like a running-bond brick wall.
      const offset = (gx % 2 === 0) ? 0 : 1.8
      const shifted = Math.abs(((z / 3.6) + offset) % 1)
      const rowJoint = shifted > .94 ? -.16 : 0
      // Per-slab tone (hash by grid cell) + subtle stains.
      const slab = hashNoise(gx * 17 + gz * 31, gz * 7 + gx * 13)
      const tone = (slab - .5) * .14
      paverGrid = joint + rowJoint + tone
    }
    const earthPatch = [4, 7].includes(material) ? (fractalNoise(x * .025, z * .025) - .5) * .06 : 0
    let variation = detail * .085 + grain * .026 + wave + stoneVein + earthPatch + paverGrid
    if (isGrass(material)) {
      // Large-scale mowing patches (10-30m) for tonal variation.
      const patch = fractalNoise(x * .08, z * .08) - .5
      variation += patch * .17
      // Mid-scale grass tufts (1-3m): dense clusters of lighter blades so the
      // ground reads as meadow rather than flat paint up close.
      const tuftN = fractalNoise(x * .42, z * .47) - .5
      variation += tuftN > .12 ? .16 : tuftN < -.12 ? -.11 : tuftN * .3
      // High-frequency individual blade tips (0.2-0.5m) for texture grain.
      const blade = hashNoise(Math.floor(x * 2.2), Math.floor(z * 3.6))
      variation += blade > .72 ? .11 : blade < .16 ? -.09 : 0
      const blade2 = hashNoise(Math.floor(x * 5.1), Math.floor(z * 4.3))
      variation += blade2 > .8 ? .07 : 0
      // Occasional wildflower / clover flecks in brighter tone.
      const fleck = hashNoise(Math.floor(x * 1.1), Math.floor(z * .9))
      variation += fleck > .955 ? .2 : 0
      const clump = hashNoise(Math.floor(x * .55), Math.floor(z * .45))
      variation += clump > .93 ? .14 : clump < .06 ? -.1 : 0
    }
    if (material === 4 || material === 7) {
      const pebble = hashNoise(Math.floor(x * .42), Math.floor(z * .37))
      variation += pebble > .8 ? .09 : pebble < .18 ? -.07 : 0
    }
    const index = (z * textureCanvas.width + x) * 4
    variationField[z * textureCanvas.width + x] = variation
    image.data[index] = Math.max(0, Math.min(255, (base[0] + variation) * 255))
    image.data[index + 1] = Math.max(0, Math.min(255, (base[1] + variation) * 255))
    image.data[index + 2] = Math.max(0, Math.min(255, (base[2] + variation) * 255))
    image.data[index + 3] = 255
  }
  textureContext.putImageData(image, 0, 0)
  const normalCanvas = document.createElement('canvas')
  normalCanvas.width = textureCanvas.width
  normalCanvas.height = textureCanvas.height
  const normalContext = normalCanvas.getContext('2d')
  const normalImage = normalContext.createImageData(normalCanvas.width, normalCanvas.height)
  for (let z = 0; z < normalCanvas.height; z++) for (let x = 0; x < normalCanvas.width; x++) {
    const current = variationField[z * normalCanvas.width + x]
    const left = variationField[z * normalCanvas.width + Math.max(0, x - 1)]
    const right = variationField[z * normalCanvas.width + Math.min(normalCanvas.width - 1, x + 1)]
    const up = variationField[Math.max(0, z - 1) * normalCanvas.width + x]
    const down = variationField[Math.min(normalCanvas.height - 1, z + 1) * normalCanvas.width + x]
    const nx = (left - right) * 3; const ny = 1; const nz = (up - down) * 3
    const length = Math.hypot(nx, ny, nz)
    const index = (z * normalCanvas.width + x) * 4
    normalImage.data[index] = ((nx / length) * .5 + .5) * 255
    normalImage.data[index + 1] = ((ny / length) * .5 + .5) * 255
    normalImage.data[index + 2] = ((nz / length) * .5 + .5) * 255
    normalImage.data[index + 3] = 255
  }
  normalContext.putImageData(normalImage, 0, 0)
  const texture = new THREE.CanvasTexture(textureCanvas)
  const normalTexture = new THREE.CanvasTexture(normalCanvas)
  texture.colorSpace = THREE.SRGBColorSpace
  normalTexture.colorSpace = THREE.NoColorSpace
  texture.minFilter = THREE.LinearMipmapLinearFilter
  texture.magFilter = THREE.LinearFilter
  texture.generateMipmaps = true
  normalTexture.minFilter = THREE.LinearMipmapLinearFilter
  normalTexture.magFilter = THREE.LinearFilter
  normalTexture.generateMipmaps = true
  return { texture, normalTexture }
}

function makeLodTexture(surface, width, depth, THREE) {
  const step = 4
  const tw = Math.max(2, Math.ceil(width / step))
  const th = Math.max(2, Math.ceil(depth / step))
  const canvas = document.createElement('canvas')
  canvas.width = tw
  canvas.height = th
  const context = canvas.getContext('2d')
  const image = context.createImageData(tw, th)
  for (let z = 0; z < th; z++) for (let x = 0; x < tw; x++) {
    let r = 0; let g = 0; let b = 0; let n = 0
    for (let dz = 0; dz < step; dz++) for (let dx = 0; dx < step; dx++) {
      const sx = x * step + dx
      const sz = z * step + dz
      if (sx >= width || sz >= depth) continue
      const c = colorRgb(surface[sz * width + sx])
      r += c[0]; g += c[1]; b += c[2]; n++
    }
    const index = (z * tw + x) * 4
    image.data[index] = n ? r / n * 255 : 127
    image.data[index + 1] = n ? g / n * 255 : 127
    image.data[index + 2] = n ? b / n * 255 : 127
    image.data[index + 3] = 255
  }
  context.putImageData(image, 0, 0)
  const texture = new THREE.CanvasTexture(canvas)
  texture.colorSpace = THREE.SRGBColorSpace
  texture.minFilter = THREE.LinearMipmapLinearFilter
  texture.magFilter = THREE.LinearFilter
  texture.generateMipmaps = true
  return texture
}

function makeWaterTexture(THREE, river) {
  const textureCanvas = document.createElement('canvas')
  textureCanvas.width = 256
  textureCanvas.height = 256
  const textureContext = textureCanvas.getContext('2d')
  const image = textureContext.createImageData(256, 256)
  for (let y = 0; y < 256; y++) for (let x = 0; x < 256; x++) {
    const ripple = Math.sin((x + y) * .15) * 7 + Math.sin(x * .33 - y * .08) * 4
    const index = (y * 256 + x) * 4
    image.data[index] = Math.max(0, Math.min(255, 35 + ripple))
    image.data[index + 1] = Math.max(0, Math.min(255, 102 + ripple))
    image.data[index + 2] = Math.max(0, Math.min(255, 116 + ripple))
    image.data[index + 3] = 255
  }
  textureContext.putImageData(image, 0, 0)
  const normalCanvas = document.createElement('canvas')
  normalCanvas.width = 256
  normalCanvas.height = 256
  const normalContext = normalCanvas.getContext('2d')
  const normalImage = normalContext.createImageData(256, 256)
  for (let y = 0; y < 256; y++) for (let x = 0; x < 256; x++) {
    const left = Math.sin((x - 1 + y) * .15) * 7 + Math.sin((x - 1) * .33 - y * .08) * 4
    const right = Math.sin((x + 1 + y) * .15) * 7 + Math.sin((x + 1) * .33 - y * .08) * 4
    const up = Math.sin((x + y - 1) * .15) * 7 + Math.sin(x * .33 - (y - 1) * .08) * 4
    const down = Math.sin((x + y + 1) * .15) * 7 + Math.sin(x * .33 - (y + 1) * .08) * 4
    const nx = (left - right) * 1.8; const ny = 1; const nz = (up - down) * 1.8
    const length = Math.hypot(nx, ny, nz)
    const index = (y * 256 + x) * 4
    normalImage.data[index] = ((nx / length) * .5 + .5) * 255
    normalImage.data[index + 1] = ((ny / length) * .5 + .5) * 255
    normalImage.data[index + 2] = ((nz / length) * .5 + .5) * 255
    normalImage.data[index + 3] = 255
  }
  normalContext.putImageData(normalImage, 0, 0)
  const texture = new THREE.CanvasTexture(textureCanvas)
  const normalTexture = new THREE.CanvasTexture(normalCanvas)
  texture.colorSpace = THREE.SRGBColorSpace
  normalTexture.colorSpace = THREE.NoColorSpace
  texture.wrapS = THREE.RepeatWrapping
  texture.wrapT = THREE.RepeatWrapping
  texture.repeat.set(river ? 0.8 : 4, river ? 3.2 : 3)
  normalTexture.wrapS = THREE.RepeatWrapping
  normalTexture.wrapT = THREE.RepeatWrapping
  normalTexture.repeat.set(river ? 0.8 : 4, river ? 3.2 : 3)
  return { texture, normalTexture }
}

function makeRiverGeometry(landmark, THREE) {
  const segments = 64
  const baseHalfWidth = landmark.widthM * .5
  const depth = landmark.depthM
  const positions = []
  const uvs = []
  const indices = []
  const halfWidthAt = index => {
    const t = index / segments
    const broadBend = Math.sin(t * Math.PI * 2 * 1.15) * .08
    const harbourBay = Math.max(0, Math.sin((t - .28) * Math.PI * 2 * 3)) * .045
    return baseHalfWidth * (1 + broadBend + harbourBay)
  }
  for (let index = 0; index <= segments; index++) {
    const z = -depth / 2 + depth * index / segments
    const halfWidth = halfWidthAt(index)
    positions.push(-halfWidth, 0, z, halfWidth, 0, z)
    uvs.push(0, index / segments, 1, index / segments)
    if (index < segments) {
      const vertex = index * 2
      indices.push(vertex, vertex + 1, vertex + 2, vertex + 1, vertex + 3, vertex + 2)
    }
  }
  const geometry = new THREE.BufferGeometry()
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3))
  geometry.setAttribute('uv', new THREE.Float32BufferAttribute(uvs, 2))
  geometry.setIndex(indices)
  geometry.computeVertexNormals()
  return geometry
}

function makeStripGeometry(entity, halfWidth, centerZ, heightAt, THREE, lift, maxLength, centerX, alongZ) {
  // Roads are long strips; in the near view we cap them so a 6km road doesn't
  // extend far beyond the visible neighbourhood. When `centerX` is given, the
  // strip is re-centered on the focus so the visible stretch is on screen.
  // `alongZ` draws the strip along Z (for roads whose length runs NS / data
  // roads that are wider in depth than width).
  const rawLength = Math.max(entity.widthM || 850, entity.depthM || 850)
  const length = maxLength ? Math.min(rawLength, maxLength) : Math.min(rawLength, 1500)
  const origin = centerX ?? entity.worldX
  const step = 8
  const segments = Math.ceil(length / step)
  const vertices = []
  const indices = []
  for (let i = 0; i <= segments; i++) {
    const t = -length / 2 + Math.min(length, i * step)
    const edge = t
    for (const off of [centerZ - halfWidth, centerZ + halfWidth]) {
      if (alongZ) {
        const x = off
        const z = edge
        vertices.push(x, heightAt(entity.worldX + x, entity.worldZ + z) - entity.worldY + lift, z)
      } else {
        const x = edge
        const z = off
        vertices.push(x, heightAt(origin + x, entity.worldZ + z) - entity.worldY + lift, z)
      }
    }
  }
  for (let i = 0; i < segments; i++) {
    const a = i * 2
    indices.push(a, a + 1, a + 2, a + 1, a + 3, a + 2)
  }
  const geometry = new THREE.BufferGeometry()
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(vertices, 3))
  geometry.setIndex(indices)
  geometry.computeVertexNormals()
  return geometry
}

function fitAssetToEntity(asset, entity, THREE, fallback) {
  asset.updateMatrixWorld(true)
  const sourceBounds = new THREE.Box3().setFromObject(asset)
  const sourceSize = sourceBounds.getSize(new THREE.Vector3())
  const targetWidth = (entity.widthM || fallback.width) * entity.scale
  const targetDepth = (entity.depthM || fallback.depth) * entity.scale
  const targetHeight = (entity.heightM || fallback.height) * entity.scale
  asset.scale.set(targetWidth / Math.max(sourceSize.x, .001), targetHeight / Math.max(sourceSize.y, .001), targetDepth / Math.max(sourceSize.z, .001))
  asset.updateMatrixWorld(true)
  const fittedBounds = new THREE.Box3().setFromObject(asset)
  const center = fittedBounds.getCenter(new THREE.Vector3())
  asset.position.set(entity.worldX - center.x, entity.worldY - fittedBounds.min.y, entity.worldZ - center.z)
  return asset
}

function addBridgeBeam(group, x1, y1, x2, y2, z, thickness, material, THREE) {
  const dx = x2 - x1; const dy = y2 - y1
  const length = Math.hypot(dx, dy)
  const beam = new THREE.Mesh(new THREE.BoxGeometry(length, thickness, thickness), material)
  beam.position.set((x1 + x2) / 2, (y1 + y2) / 2, z)
  beam.rotation.z = -Math.atan2(dy, dx)
  group.add(beam)
}

function makeBridgeStructure(spec, THREE, heightAt) {
  const bridge = new THREE.Group()
  const steel = new THREE.MeshStandardMaterial({ color: '#58636a', roughness: .78, metalness: .42 })
  const darkSteel = new THREE.MeshStandardMaterial({ color: '#303b40', roughness: .82, metalness: .35 })
  const concrete = new THREE.MeshStandardMaterial({ color: '#a29d91', roughness: .96 })
  const road = new THREE.MeshStandardMaterial({ color: '#343b3d', roughness: .92 })
  const tower = new THREE.MeshStandardMaterial({ color: '#d0c3a7', roughness: .9 })
  const flag = new THREE.MeshStandardMaterial({ color: '#a43c32', roughness: .82 })
  const marking = new THREE.MeshStandardMaterial({ color: '#c7c9bd', roughness: .78 })
  const length = spec.widthM || 4588
  const width = spec.depthM || 19.5
  const deck = new THREE.Mesh(new THREE.BoxGeometry(length, 1.2, width), road)
  bridge.add(deck)
  for (const z of [-4.5, 4.5]) {
    const laneLine = new THREE.Mesh(new THREE.BoxGeometry(length, .035, .12), marking)
    laneLine.position.set(0, .62, z)
    bridge.add(laneLine)
  }
  for (const z of [-width / 2 - .45, width / 2 + .45]) {
    const parapet = new THREE.Mesh(new THREE.BoxGeometry(length, 1.4, .35), darkSteel)
    parapet.position.set(0, 1.25, z)
    bridge.add(parapet)
  }
  const railDeck = new THREE.Mesh(new THREE.BoxGeometry(length, .9, 14), darkSteel)
  railDeck.position.y = -4.5
  bridge.add(railDeck)
  for (const z of [-4.2, 4.2]) {
    const rail = new THREE.Mesh(new THREE.BoxGeometry(length, .18, .18), steel)
    rail.position.set(0, -3.92, z)
    bridge.add(rail)
  }
  for (let x = -length / 2 + 2; x < length / 2; x += 4) {
    const sleeper = new THREE.Mesh(new THREE.BoxGeometry(2.4, .16, 11.5), concrete)
    sleeper.position.set(x, -4.02, 0)
    bridge.add(sleeper)
  }
  const mainStart = -length * .18
  const mainEnd = length * .18
  const panel = Math.max(20, Math.min(64, length * .02))
  for (const side of [-1, 1]) {
    const z = side * 9
    addBridgeBeam(bridge, mainStart, 5, mainEnd, 5, z, .8, steel, THREE)
    addBridgeBeam(bridge, mainStart, 15, mainEnd, 15, z, .8, steel, THREE)
    for (let x = mainStart; x < mainEnd; x += panel) {
      const next = Math.min(mainEnd, x + panel)
      addBridgeBeam(bridge, x, 5, x, 15, z, .55, steel, THREE)
      addBridgeBeam(bridge, x, 5, next, 15, z, .45, steel, THREE)
      addBridgeBeam(bridge, x, 15, next, 5, z, .45, steel, THREE)
    }
  }
  const pierCount = Math.max(2, Math.round((mainEnd - mainStart) / 160) - 1)
  for (let index = 1; index <= pierCount; index++) {
    const x = mainStart + index * ((mainEnd - mainStart) / (pierCount + 1))
    const pier = new THREE.Mesh(new THREE.BoxGeometry(10, 30, 16), concrete)
    // Deck is 1.2m thick centred on y=0, so its underside is at -0.6. The
    // pier must rise to exactly that level — not punch up through the deck.
    pier.position.set(x, -15.6, 0)
    bridge.add(pier)
  }
  const towerHeight = Math.max(10, spec.heightM || 70)
  for (const side of [-1, 1]) {
    for (const z of [-8, 8]) {
      // Tower pedestal sits ON the deck (top at +0.6), not buried in it.
      const towerBase = new THREE.Mesh(new THREE.BoxGeometry(16, 14, 10), tower)
      towerBase.position.set(side * (length / 2 - 70), 7.6, z)
      bridge.add(towerBase)
      const towerBody = new THREE.Mesh(new THREE.BoxGeometry(11, towerHeight, 8), tower)
      towerBody.position.set(side * (length / 2 - 70), towerHeight / 2, z)
      bridge.add(towerBody)
      const flagPole = new THREE.Mesh(new THREE.CylinderGeometry(.35, .35, 16, 8), darkSteel)
      flagPole.position.set(side * (length / 2 - 70), towerHeight + 8, z)
      bridge.add(flagPole)
      const flagShape = new THREE.Mesh(new THREE.BoxGeometry(.16, 5, 8), flag)
      flagShape.position.set(side * (length / 2 - 70) + side * 3.5, towerHeight + 11, z)
      bridge.add(flagShape)
    }
    const portal = new THREE.Mesh(new THREE.BoxGeometry(13, 10, 31), tower)
    portal.position.set(side * (length / 2 - 70), towerHeight - 4, 0)
    bridge.add(portal)
  }
  bridge.position.set(spec.worldX, spec.worldY, spec.worldZ)
  // Approach ramps: both ends step down from the deck to the shore so the
  // bridge never ends in a floating vertical cut. Each ramp is a short sloped
  // road slab from the deck edge down to the ground height at that end.
  if (typeof heightAt === 'function') {
    const deckEdge = length / 2
    const rampLength = 46
    const rampStep = 4
    for (const side of [-1, 1]) {
      const x0 = side * (deckEdge - rampLength)
      const x1 = side * deckEdge
      const xEnd = side * (deckEdge + rampLength)
      const groundAtEnd = heightAt(spec.worldX + xEnd, spec.worldZ) - spec.worldY
      const segments = Math.ceil(rampLength / rampStep)
      for (let seg = 0; seg < segments; seg++) {
        const xA = x0 + seg * rampStep * side
        const xB = Math.min(Math.abs(xA) + rampStep, deckEdge) * side
        // Ramp surface height drops linearly from deck (0) to ground at the
        // outer end, and joins the deck flush at the inner end.
        const fracA = Math.max(0, (Math.abs(xA) - deckEdge) / rampLength)
        const fracB = Math.max(0, (Math.abs(xB) - deckEdge) / rampLength)
        const yA = fracA * groundAtEnd
        const yB = fracB * groundAtEnd
        const rampGeo = new THREE.BufferGeometry()
        const half = width / 2 + .5
        const verts = [
          xA, yA, -half, xB, yB, -half,
          xA, yA, half, xB, yB, half,
        ]
        rampGeo.setAttribute('position', new THREE.Float32BufferAttribute(verts, 3))
        rampGeo.setIndex([0, 1, 2, 1, 3, 2])
        rampGeo.computeVertexNormals()
        const ramp = new THREE.Mesh(rampGeo, road)
        bridge.add(ramp)
      }
      // Parapet continues along the ramp.
      for (const z of [-width / 2 - .45, width / 2 + .45]) {
        for (let seg = 0; seg < segments; seg++) {
          const xA = x0 + seg * rampStep * side
          const xB = Math.min(Math.abs(xA) + rampStep, deckEdge) * side
          const fracA = Math.max(0, (Math.abs(xA) - deckEdge) / rampLength)
          const fracB = Math.max(0, (Math.abs(xB) - deckEdge) / rampLength)
          const yA = fracA * groundAtEnd
          const yB = fracB * groundAtEnd
          const rail = new THREE.Mesh(new THREE.BoxGeometry(Math.abs(xB - xA), .35, .35), darkSteel)
          rail.position.set((xA + xB) / 2, (yA + yB) / 2 + .6, z)
          rail.rotation.x = -Math.atan2(yB - yA, xB - xA)
          bridge.add(rail)
        }
      }
    }
  }
  return bridge
}

function makeHongKongTower(spec, THREE) {
  const tower = new THREE.Group()
  const glass = new THREE.MeshStandardMaterial({ color: '#718894', roughness: .28, metalness: .52 })
  const darkGlass = new THREE.MeshStandardMaterial({ color: '#263f4b', roughness: .22, metalness: .62 })
  const edge = new THREE.MeshStandardMaterial({ color: '#b8c5c7', roughness: .38, metalness: .7 })
  const light = new THREE.MeshStandardMaterial({ color: '#f6d993', emissive: '#a77b32', emissiveIntensity: .7, roughness: .35 })
  const width = spec.widthM || 60
  const depth = spec.depthM || width
  const height = spec.heightM || 100
  const isBank = spec.assetId === 'landmark.bank-of-china'
  const isIcc = spec.assetId === 'landmark.icc'
  const isCentralPlaza = spec.assetId === 'landmark.central-plaza'
  const segments = Math.max(8, Math.min(28, Math.floor(height / 18)))
  for (let i = 0; i < segments; i++) {
    const t = i / Math.max(1, segments - 1)
    const taper = isIcc ? 1 - t * .18 : isBank ? 1 - t * .28 : 1 - t * .1
    const segment = new THREE.Mesh(new THREE.BoxGeometry(width * taper, height / segments * .92, depth * taper), i % 3 === 0 ? glass : darkGlass)
    segment.position.y = height * (t + .5 / segments)
    if (isBank) segment.rotation.y = Math.PI / 4
    tower.add(segment)
    const band = new THREE.Mesh(new THREE.BoxGeometry(width * taper * 1.03, .7, depth * taper * 1.03), edge)
    band.position.y = height * (t + .95 / segments)
    if (isBank) band.rotation.y = Math.PI / 4
    tower.add(band)
    if (i % 2 === 0) {
      const window = new THREE.Mesh(new THREE.BoxGeometry(width * taper * .72, .9, .08), light)
      window.position.set(0, segment.position.y, depth * taper / 2 + .08)
      tower.add(window)
    }
  }
  if (isCentralPlaza) {
    const crown = new THREE.Mesh(new THREE.ConeGeometry(width * .42, height * .16, 6), edge)
    crown.position.y = height * 1.04
    tower.add(crown)
  }
  const antenna = new THREE.Mesh(new THREE.CylinderGeometry(.8, 1.4, Math.max(12, height * .12), 6), edge)
  antenna.position.y = height * 1.08
  tower.add(antenna)
  const contactShadow = new THREE.Mesh(new THREE.CircleGeometry(Math.max(width, depth) * .72, 24), new THREE.MeshBasicMaterial({ color: '#101820', transparent: true, opacity: .28, depthWrite: false }))
  contactShadow.rotation.x = -Math.PI / 2
  contactShadow.scale.set(1.35, 1.35, 1)
  contactShadow.position.y = .32
  contactShadow.renderOrder = 4
  contactShadow.material.depthTest = false
  tower.add(contactShadow)
  tower.position.set(spec.worldX, spec.worldY, spec.worldZ)
  return tower
}

function makeVoxelTree(entity, THREE) {
  const tree = new THREE.Group()
  const trunk = new THREE.MeshStandardMaterial({ color: '#604632', roughness: .96 })
  const foliage = new THREE.MeshStandardMaterial({ color: '#356447', roughness: .92 })
  const foliageLight = new THREE.MeshStandardMaterial({ color: '#588553', roughness: .9 })
  const crown = Math.max(entity.widthM, entity.depthM) * .5
  const treePit = new THREE.Mesh(new THREE.CylinderGeometry(crown * .42, crown * .5, .18, 8), new THREE.MeshStandardMaterial({ color: '#514538', roughness: 1 }))
  treePit.position.y = .09
  tree.add(treePit)
  const height = entity.heightM
  const stem = new THREE.Mesh(new THREE.CylinderGeometry(crown * .11, crown * .16, height * .54, 7), trunk)
  stem.position.y = height * .27
  tree.add(stem)
  const lower = new THREE.Mesh(new THREE.DodecahedronGeometry(crown * .72, 1), foliage)
  lower.scale.y = .72
  lower.position.y = height * .57
  tree.add(lower)
  const upper = new THREE.Mesh(new THREE.IcosahedronGeometry(crown * .55, 1), foliageLight)
  upper.scale.y = .8
  upper.position.y = height * .78
  tree.add(upper)
  const side = new THREE.Mesh(new THREE.DodecahedronGeometry(crown * .38, 0), foliage)
  side.position.set(crown * .48, height * .58, crown * .1)
  tree.add(side)
  tree.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return tree
}

// Build all trees of one chunk into a few InstancedMeshes (trunk + layered
// foliage canopy) so thousands of trees cost a handful of draw calls instead
// of one THREE.Group per tree. Each tree keeps its own scale / position /
// tint. The canopy is stacked from two offset Dodecahedra per tree so it reads
// as a dense, shading mass rather than a single lollipop sphere.
function buildTreeInstances(treeEntities, THREE) {
  const group = new THREE.Group()
  if (!treeEntities.length) return group
  const count = treeEntities.length
  const trunkGeo = new THREE.CylinderGeometry(.5, .72, 2, 7)
  const foliageGeoLow = new THREE.DodecahedronGeometry(1, 1)
  const foliageGeoHigh = new THREE.DodecahedronGeometry(1, 0)
  const trunkMat = new THREE.MeshStandardMaterial({ color: '#5d4330', roughness: .97 })
  const foliageMat = new THREE.MeshStandardMaterial({ color: '#3d6b44', roughness: .94 })
  const trunks = new THREE.InstancedMesh(trunkGeo, trunkMat, count)
  const foliageLow = new THREE.InstancedMesh(foliageGeoLow, foliageMat, count)
  const foliageHigh = new THREE.InstancedMesh(foliageGeoHigh, foliageMat, count)
  const m = new THREE.Matrix4()
  const q = new THREE.Quaternion()
  const s = new THREE.Vector3()
  const pos = new THREE.Vector3()
  const color = new THREE.Color()
  for (let i = 0; i < count; i++) {
    const e = treeEntities[i]
    const crown = Math.max(e.widthM, e.depthM) * .85
    const h = e.heightM
    const tintSeed = e.worldX * .017 + e.worldZ * .023
    const leanX = ((tintSeed * 137.5) % 1 + 1) % 1 * .16 - .08
    // Trunk: thicker so the canopy reads as supported, not a floating ball.
    pos.set(e.worldX, e.worldY + h * .3, e.worldZ)
    s.set(crown * .16, h * .62, crown * .16)
    q.setFromEuler(new THREE.Euler(0, 0, leanX, 'XYZ'))
    m.compose(pos, q, s)
    trunks.setMatrixAt(i, m)
    // Lower canopy: the widest, densest mass — broad enough to cast shade
    // over the ground and read as a real tree silhouette.
    pos.set(e.worldX, e.worldY + h * .52, e.worldZ)
    s.set(crown * 1.25, crown * .95, crown * 1.25)
    m.compose(pos, q, s)
    foliageLow.setMatrixAt(i, m)
    // Upper canopy: smaller, slightly offset lobe for a natural crown.
    pos.set(e.worldX + crown * .2, e.worldY + h * .82, e.worldZ + crown * .14)
    s.set(crown * .78, crown * .62, crown * .78)
    m.compose(pos, q, s)
    foliageHigh.setMatrixAt(i, m)
    const hue = .33 + ((tintSeed % .09) - .045)
    color.setHSL(hue, .46, .3 + ((tintSeed * 3 % .08) + .02))
    foliageLow.setColorAt(i, color)
    color.setHSL(hue + .02, .5, .36)
    foliageHigh.setColorAt(i, color)
  }
  trunks.instanceMatrix.needsUpdate = true
  foliageLow.instanceMatrix.needsUpdate = true
  foliageHigh.instanceMatrix.needsUpdate = true
  foliageLow.instanceColor.needsUpdate = true
  foliageHigh.instanceColor.needsUpdate = true
  group.add(trunks)
  group.add(foliageLow)
  group.add(foliageHigh)
  return group
}

function makeMangrove(entity, THREE) {
  const patch = new THREE.Group()
  // Mudflat base with tidal creeks.
  const mud = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM, .5, entity.depthM), new THREE.MeshStandardMaterial({ color: '#7a6a4d', roughness: 1 }))
  mud.position.y = .25
  patch.add(mud)
  // Dense mangrove thicket: many low aerial-rooted trees.
  const trunkMat = new THREE.MeshStandardMaterial({ color: '#5a4436', roughness: .95 })
  const foliageMat = new THREE.MeshStandardMaterial({ color: '#3c6b33', roughness: .95 })
  const foliageLight = new THREE.MeshStandardMaterial({ color: '#5d8a4a', roughness: .92 })
  const count = 26
  for (let i = 0; i < count; i++) {
    const phase = (entity.worldX * 31 + entity.worldZ * 17 + i * 97) % 997
    const x = ((phase * 37) % 1000 / 1000 - .5) * entity.widthM * .9
    const z = ((phase * 71) % 1000 / 1000 - .5) * entity.depthM * .9
    const h = 4 + (phase % 5)
    // Prop roots
    for (let r = 0; r < 4; r++) {
      const root = new THREE.Mesh(new THREE.CylinderGeometry(.12, .16, h * .35, 5), trunkMat)
      root.position.set(x + Math.cos(r) * 1.4, h * .18, z + Math.sin(r) * 1.4)
      root.rotation.z = Math.cos(r) * .5
      root.rotation.x = Math.sin(r) * .5
      patch.add(root)
    }
    const trunk = new THREE.Mesh(new THREE.CylinderGeometry(.18, .24, h * .55, 5), trunkMat)
    trunk.position.set(x, h * .28, z)
    patch.add(trunk)
    const crown = new THREE.Mesh(new THREE.DodecahedronGeometry(1.6 + (phase % 3) * .4, 1), i % 2 ? foliageMat : foliageLight)
    crown.position.set(x, h * .62, z)
    crown.scale.y = .9
    patch.add(crown)
  }
  patch.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return patch
}

function makeForestPatch(entity, THREE, reserve = false) {
  const patch = new THREE.Group()
  const ground = new THREE.Mesh(
    new THREE.BoxGeometry(entity.widthM, .28, entity.depthM),
    new THREE.MeshStandardMaterial({ color: reserve ? '#3e7047' : '#4d6d3d', roughness: 1 })
  )
  ground.position.y = .14
  patch.add(ground)
  const trunk = new THREE.MeshStandardMaterial({ color: '#604632', roughness: .96 })
  const foliage = new THREE.MeshStandardMaterial({ color: reserve ? '#2e623c' : '#39633d', roughness: .94 })
  const highlight = new THREE.MeshStandardMaterial({ color: reserve ? '#5b8e52' : '#547a45', roughness: .92 })
  const count = reserve ? 11 : 7
  for (let index = 0; index < count; index++) {
    const phase = (entity.worldX * 17 + entity.worldZ * 31 + index * 97) % 997
    const x = ((phase * 37) % 1000 / 1000 - .5) * entity.widthM * .78
    const z = ((phase * 71) % 1000 / 1000 - .5) * entity.depthM * .78
    const height = 7 + (phase % 6)
    const crown = 3 + (phase % 4) * .45
    const stem = new THREE.Mesh(new THREE.CylinderGeometry(crown * .12, crown * .18, height * .48, 6), trunk)
    stem.position.set(x, height * .24, z)
    patch.add(stem)
    const lower = new THREE.Mesh(new THREE.DodecahedronGeometry(crown, 0), foliage)
    lower.scale.y = .8
    lower.position.set(x, height * .58, z)
    patch.add(lower)
    const upper = new THREE.Mesh(new THREE.ConeGeometry(crown * .72, height * .52, 6), highlight)
    upper.position.set(x, height * .85, z)
    patch.add(upper)
  }
  patch.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return patch
}

function deterministicRand(x, z, salt) {
  let h = (x * 374761393 + z * 668265263 + salt * 69069) >>> 0
  h = (h ^ (h >> 13)) >>> 0
  h = Math.imul(h, 1274126177) >>> 0
  return ((h ^ (h >> 16)) >>> 0) / 4294967296
}

// Slope-aware plinth (Mountain-City style). A building with a large footprint
// on uneven ground either floats or sinks at its corners; this adds a concrete
// foundation that reaches from the building anchor down to the local terrain
// so the structure reads as built into the slope (like Chongqing / stilt
// houses) instead of clipping through the grass.
function applySlopeFoundation(group, entity, heightAt, THREE, plinthColor = '#9a938a') {
  if (!heightAt || !entity) return
  const hw = entity.widthM * .5
  const hd = entity.depthM * .5
  const corners = [
    [entity.worldX - hw, entity.worldZ - hd],
    [entity.worldX + hw, entity.worldZ - hd],
    [entity.worldX - hw, entity.worldZ + hd],
    [entity.worldX + hw, entity.worldZ + hd],
    [entity.worldX, entity.worldZ],
  ]
  const groundLevels = corners.map(([cx, cz]) => heightAt(cx, cz))
  const minGround = Math.min(...groundLevels)
  const maxGround = Math.max(...groundLevels)
  const anchor = entity.worldY
  // Building bottom sits at the anchor; terrain may be higher (sunk) or lower
  // (floating). Add a plinth from the lowest ground up to the building bottom.
  const top = anchor + .4
  const bottom = minGround - .3
  if (top <= bottom + .4) return
  const plinth = new THREE.Mesh(
    new THREE.BoxGeometry(entity.widthM + .6, top - bottom, entity.depthM + .6),
    new THREE.MeshStandardMaterial({ color: plinthColor, roughness: 1 })
  )
  plinth.position.set(entity.worldX, (top + bottom) / 2, entity.worldZ)
  group.add(plinth)
  // Keep a reference so callers can tilt the whole block on steep ground.
  group.userData.plinth = plinth
  group.userData.slope = maxGround - minGround
}

function makeResidentialBlock(entity, THREE) {
  const group = new THREE.Group()
  const ground = new THREE.Mesh(
    new THREE.BoxGeometry(entity.widthM, .35, entity.depthM),
    new THREE.MeshStandardMaterial({ color: '#6d7168', roughness: 1 })
  )
  ground.position.y = .175
  group.add(ground)
  const wallPalette = ['#d8c4a8', '#b8bcc6', '#d4b48a', '#a5b49a', '#c4b8c6']
  const roofPalette = ['#8a4f38', '#7a5a4a', '#b05038', '#6d5b72', '#5d7a6a']
  const windowMat = new THREE.MeshStandardMaterial({ color: '#7fc0d6', roughness: .2, metalness: .3 })
  const doorMat = new THREE.MeshStandardMaterial({ color: '#4a3426', roughness: .85 })
  const rows = 2
  const cols = 3
  for (let i = 0; i < rows; i++) {
    for (let j = 0; j < cols; j++) {
      const r = deterministicRand(entity.worldX, entity.worldZ, i * 7 + j * 3 + 1)
      const houseW = entity.widthM / cols * .78
      const houseD = entity.depthM / rows * .76
      const houseH = 3.2 + r * 2.4
      const wall = new THREE.MeshStandardMaterial({ color: wallPalette[Math.floor(r * wallPalette.length) % wallPalette.length], roughness: .9 })
      const roof = new THREE.MeshStandardMaterial({ color: roofPalette[Math.floor(r * roofPalette.length) % roofPalette.length], roughness: .85 })
      const body = new THREE.Mesh(new THREE.BoxGeometry(houseW, houseH, houseD), wall)
      const px = -entity.widthM / 2 + houseW / 2 + (entity.widthM - houseW * cols) / 2 + j * (houseW + (entity.widthM - houseW * cols) / (cols - 1))
      const pz = -entity.depthM / 2 + houseD / 2 + (entity.depthM - houseD * rows) / 2 + i * (houseD + (entity.depthM - houseD * rows) / (rows - 1))
      body.position.set(px, houseH / 2, pz)
      group.add(body)
      // Gable roof (Minecraft-like) instead of a cone: two sloped prisms
      const roofH = houseH * .42
      const gable = new THREE.Mesh(new THREE.ConeGeometry(Math.max(houseW, houseD) * .55, roofH, 4), roof)
      gable.rotation.y = Math.PI / 4
      gable.position.set(px, houseH + roofH * .55, pz)
      group.add(gable)
      // Chimney for a readable silhouette
      const chimney = new THREE.Mesh(new THREE.BoxGeometry(houseW * .14, roofH * .7, houseW * .14), new THREE.MeshStandardMaterial({ color: '#6d5646', roughness: .95 }))
      chimney.position.set(px + houseW * .22, houseH + roofH * .62, pz - houseD * .16)
      group.add(chimney)
      // Door on the +z face
      const door = new THREE.Mesh(new THREE.BoxGeometry(houseW * .26, houseH * .42, .14), doorMat)
      door.position.set(px, houseH * .21, pz + houseD / 2 + .08)
      group.add(door)
      // Front windows (+z face)
      for (const wx of [-houseW * .22, houseW * .22]) {
        const win = new THREE.Mesh(new THREE.BoxGeometry(houseW * .18, houseH * .26, .12), windowMat)
        win.position.set(px + wx, houseH * .6, pz + houseD / 2 + .08)
        group.add(win)
      }
      // Side windows (+x and -x faces) so tilted views show life
      for (const sx of [-1, 1]) {
        const sideWin = new THREE.Mesh(new THREE.BoxGeometry(.12, houseH * .22, houseD * .2), windowMat)
        sideWin.position.set(px + sx * (houseW / 2 + .08), houseH * .58, pz)
        group.add(sideWin)
      }
    }
  }
  // Street trees inside the block for a lived-in feel
  const treeCount = 3
  for (let t = 0; t < treeCount; t++) {
    const tr = deterministicRand(entity.worldX, entity.worldZ, 100 + t)
    const treeGroup = new THREE.Group()
    const trunk = new THREE.Mesh(new THREE.CylinderGeometry(.22, .3, 2.1, 5), new THREE.MeshStandardMaterial({ color: '#6d5238', roughness: .95 }))
    trunk.position.y = 1.05
    treeGroup.add(trunk)
    const crown = new THREE.Mesh(new THREE.DodecahedronGeometry(1.5 + tr * .7, 0), new THREE.MeshStandardMaterial({ color: tr > .5 ? '#4a7a3a' : '#5d8f47', roughness: .95 }))
    crown.position.y = 2.7 + tr * .5
    treeGroup.add(crown)
    const tx = -entity.widthM / 2 + 10 + tr * (entity.widthM - 20)
    const tz = -entity.depthM / 2 + 6 + ((t * 37 + Math.floor(tr * 100)) % (entity.depthM - 14))
    treeGroup.position.set(tx, .1, tz)
    group.add(treeGroup)
  }
  group.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return group
}

// Chinese-style gated compound: a cluster of high-rise towers around a shared
// court, with a wall line and internal greenery. Reads as a dense urban
// residential block rather than a handful of small cottages.
function makeResidentialTower(entity, THREE, heightAt) {
  const group = new THREE.Group()
  const plinthMat = new THREE.MeshStandardMaterial({ color: '#6e726c', roughness: 1 })
  const base = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM, .6, entity.depthM), plinthMat)
  base.position.y = .3
  group.add(base)
  const towerW = entity.widthM * .32
  const towerD = entity.depthM * .32
  const towers = 4
  const wallCols = ['#c8b8a4', '#b5b9c2', '#a9b0a2', '#c4b8c6']
  const windowMat = new THREE.MeshStandardMaterial({ color: '#7fc0d6', roughness: .2, metalness: .3 })
  for (let i = 0; i < towers; i++) {
    const col = i % 2
    const row = Math.floor(i / 2)
    const tx = -entity.widthM / 2 + towerW / 2 + col * (entity.widthM - towerW)
    const tz = -entity.depthM / 2 + towerD / 2 + row * (entity.depthM - towerD)
    const h = entity.heightM * (.82 + (i % 3) * .08)
    const body = new THREE.Mesh(new THREE.BoxGeometry(towerW, h, towerD), new THREE.MeshStandardMaterial({ color: wallCols[i % wallCols.length], roughness: .85 }))
    body.position.set(tx, h / 2, tz)
    group.add(body)
    for (let rowI = 1; rowI <= 4; rowI++) {
      for (const side of [-1, 1]) {
        const balcony = new THREE.Mesh(new THREE.BoxGeometry(towerW * .9, .4, .5), new THREE.MeshStandardMaterial({ color: '#d8d4c8', roughness: .8 }))
        balcony.position.set(tx, h * rowI / 5, tz + side * (towerD / 2 + .2))
        group.add(balcony)
      }
    }
    for (let rowI = 0; rowI < 5; rowI++) {
      for (const colI of [-1, 0, 1]) {
        const win = new THREE.Mesh(new THREE.BoxGeometry(towerW * .2, h * .12, .12), windowMat)
        win.position.set(tx + colI * towerW * .24, h * (rowI + .5) / 5.2, tz + towerD / 2 + .3)
        group.add(win)
      }
    }
  }
  const court = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM * .34, .2, entity.depthM * .34), new THREE.MeshStandardMaterial({ color: '#5d8a4a', roughness: 1 }))
  court.position.y = .5
  group.add(court)
  group.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return group
}

// American-style detached home on its own lot, with a front yard and a single
// family house — low-rise and spread out, distinct from the dense core.
function makeResidentialHome(entity, THREE, heightAt) {
  const group = new THREE.Group()
  const lotMat = new THREE.MeshStandardMaterial({ color: '#6d9a52', roughness: 1 })
  const lot = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM, .3, entity.depthM), lotMat)
  lot.position.y = .15
  group.add(lot)
  const homeW = entity.widthM * .62
  const homeD = entity.depthM * .6
  const homeH = entity.heightM * .85
  const wall = new THREE.MeshStandardMaterial({ color: ['#e6d8c4', '#d9d3c2', '#d4cfc6'][Math.abs(entity.worldX) % 3], roughness: .9 })
  const roof = new THREE.MeshStandardMaterial({ color: '#8a5a42', roughness: .85 })
  const body = new THREE.Mesh(new THREE.BoxGeometry(homeW, homeH, homeD), wall)
  body.position.y = homeH / 2
  group.add(body)
  const roofH = homeH * .4
  const gable = new THREE.Mesh(new THREE.ConeGeometry(Math.max(homeW, homeD) * .5, roofH, 4), roof)
  gable.rotation.y = Math.PI / 4
  gable.position.y = homeH + roofH * .5
  group.add(gable)
  const door = new THREE.Mesh(new THREE.BoxGeometry(homeW * .18, homeH * .42, .12), new THREE.MeshStandardMaterial({ color: '#5a4030', roughness: .9 }))
  door.position.set(0, homeH * .2, homeD / 2 + .1)
  group.add(door)
  const windowMat = new THREE.MeshStandardMaterial({ color: '#8fc4d8', roughness: .25, metalness: .3 })
  for (const side of [-1, 1]) {
    const win = new THREE.Mesh(new THREE.BoxGeometry(homeW * .2, homeH * .3, .1), windowMat)
    win.position.set(side * homeW * .26, homeH * .55, homeD / 2 + .1)
    group.add(win)
  }
  const tree = new THREE.Mesh(new THREE.ConeGeometry(1.8, 5, 6), new THREE.MeshStandardMaterial({ color: '#3f6a3d', roughness: .95 }))
  tree.position.set(homeW * .4, 2.5, -homeD * .5)
  group.add(tree)
  group.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return group
}

// Waterfront holiday resort lodge in the Shenzhen Bay / OCT Happy Harbour
// idiom. Now declarative: the CGA rule set CGA_RESORT produces the white
// plaster body, flush louver cladding, glass strip and flat parapet roof.
function makeResortLodge(entity, THREE, heightAt) {
  return cgaBuild(entity, CGA_RESORT, THREE)
}

function makeSchool(entity, THREE, heightAt) {
  const group = new THREE.Group()
  const wall = new THREE.MeshStandardMaterial({ color: '#c9a86a', roughness: .92 })
  const roof = new THREE.MeshStandardMaterial({ color: '#7a5f44', roughness: .85 })
  const track = new THREE.MeshStandardMaterial({ color: '#9c4a38', roughness: .95 })
  const field = new THREE.MeshStandardMaterial({ color: '#5d8a4a', roughness: 1 })
  const ground = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM, .3, entity.depthM), new THREE.MeshStandardMaterial({ color: '#7d7a68', roughness: 1 }))
  ground.position.y = .15
  group.add(ground)
  const mainW = entity.widthM * .5
  const mainD = entity.depthM * .34
  const mainH = entity.heightM * .72
  const main = new THREE.Mesh(new THREE.BoxGeometry(mainW, mainH, mainD), wall)
  main.position.set(-entity.widthM * .15, mainH / 2, -entity.depthM * .14)
  group.add(main)
  const roofSlab = new THREE.Mesh(new THREE.BoxGeometry(mainW * 1.04, 1, mainD * 1.04), roof)
  roofSlab.position.set(-entity.widthM * .15, mainH + .6, -entity.depthM * .14)
  group.add(roofSlab)
  const gym = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM * .3, entity.heightM * .5, entity.depthM * .3), wall)
  gym.position.set(entity.widthM * .28, entity.heightM * .25, entity.depthM * .18)
  group.add(gym)
  const gymRoof = new THREE.Mesh(new THREE.CylinderGeometry(entity.widthM * .16, entity.widthM * .16, entity.depthM * .3, 8), roof)
  gymRoof.rotation.x = Math.PI / 2
  gymRoof.position.set(entity.widthM * .28, entity.heightM * .5, entity.depthM * .18)
  group.add(gymRoof)
  const trackW = entity.widthM * .62
  const trackD = entity.depthM * .4
  const trackBed = new THREE.Mesh(new THREE.BoxGeometry(trackW, .2, trackD), track)
  trackBed.position.set(entity.widthM * .18, .25, entity.depthM * .28)
  group.add(trackBed)
  const fieldPatch = new THREE.Mesh(new THREE.BoxGeometry(trackW * .62, .15, trackD * .5), field)
  fieldPatch.position.set(entity.widthM * .18, .38, entity.depthM * .28)
  group.add(fieldPatch)
  for (const side of [-1, 1]) {
    const goal = new THREE.Mesh(new THREE.BoxGeometry(4, 2.2, .6), new THREE.MeshStandardMaterial({ color: '#f2efe6', roughness: .7 }))
    goal.position.set(entity.widthM * .18, 1.4, entity.depthM * .28 + side * trackD * .26)
    group.add(goal)
  }
  applySlopeFoundation(group, entity, heightAt, THREE)
  group.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return group
}

function makeCommercialCenter(entity, THREE, heightAt) {
  const group = new THREE.Group()
  const ground = new THREE.Mesh(
    new THREE.BoxGeometry(entity.widthM, .35, entity.depthM),
    new THREE.MeshStandardMaterial({ color: '#7a7d80', roughness: .98 })
  )
  ground.position.y = .175
  group.add(ground)
  const glass = new THREE.MeshStandardMaterial({ color: '#3f6a78', roughness: .22, metalness: .4 })
  const frame = new THREE.MeshStandardMaterial({ color: '#b3a37e', roughness: .7, metalness: .25 })
  const stores = 4
  for (let i = 0; i < stores; i++) {
    const r = deterministicRand(entity.worldX, entity.worldZ, i + 5)
    const storeW = entity.widthM / stores * .8
    const storeH = 7 + r * 9
    const storeD = entity.depthM * .62
    const body = new THREE.Mesh(new THREE.BoxGeometry(storeW, storeH, storeD), frame)
    const sx = -entity.widthM / 2 + storeW / 2 + (entity.widthM - storeW * stores) / 2 + i * (storeW + (entity.widthM - storeW * stores) / (stores - 1))
    body.position.set(sx, storeH / 2, -entity.depthM * .08)
    group.add(body)
    const face = new THREE.Mesh(new THREE.BoxGeometry(storeW * .9, storeH * .6, .3), glass)
    face.position.set(sx, storeH * .5, -entity.depthM * .08 + storeD / 2 + .18)
    group.add(face)
    const canopy = new THREE.Mesh(new THREE.BoxGeometry(storeW * 1.02, .5, 1.6), frame)
    canopy.position.set(sx, storeH * .78, -entity.depthM * .08 + storeD / 2 + 1)
    group.add(canopy)
    const roof = new THREE.Mesh(new THREE.BoxGeometry(storeW * 1.06, .7, storeD * 1.06), new THREE.MeshStandardMaterial({ color: '#5d6670', roughness: .8 }))
    roof.position.set(sx, storeH + .4, -entity.depthM * .08)
    group.add(roof)
  }
  const sign = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM * .7, 2.4, .5), new THREE.MeshStandardMaterial({ color: '#c0403a', roughness: .6, emissive: '#7a1f1a', emissiveIntensity: .35 }))
  sign.position.set(0, entity.heightM * .9, entity.depthM / 2 - .3)
  group.add(sign)
  applySlopeFoundation(group, entity, heightAt, THREE)
  group.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return group
}

function makeEntertainmentCenter(entity, THREE) {
  const group = new THREE.Group()
  const wall = new THREE.MeshStandardMaterial({ color: '#8f6a92', roughness: .85 })
  const glass = new THREE.MeshStandardMaterial({ color: '#5a88a8', roughness: .25, metalness: .35, emissive: '#2a4a6a', emissiveIntensity: .25 })
  const roof = new THREE.MeshStandardMaterial({ color: '#4f5a66', roughness: .8 })
  const main = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM * .7, entity.heightM * .7, entity.depthM * .7), wall)
  main.position.y = entity.heightM * .35
  group.add(main)
  const marquee = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM * .55, entity.heightM * .2, entity.depthM * .4), glass)
  marquee.position.set(0, entity.heightM * .78, entity.depthM * .2)
  group.add(marquee)
  const arch = new THREE.Mesh(new THREE.CylinderGeometry(entity.widthM * .28, entity.widthM * .34, entity.depthM * .55, 6), roof)
  arch.rotation.x = Math.PI / 2
  arch.position.set(0, entity.heightM * 1.02, entity.depthM * .12)
  group.add(arch)
  const ground = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM * 1.1, .3, entity.depthM * 1.1), new THREE.MeshStandardMaterial({ color: '#585c60', roughness: .98 }))
  ground.position.y = .15
  group.add(ground)
  for (let i = 0; i < 5; i++) {
    const a = i / 5 * Math.PI * 2
    const lamp = new THREE.Mesh(new THREE.CylinderGeometry(.35, .5, 4.5, 6), new THREE.MeshStandardMaterial({ color: '#3e4648', roughness: .7 }))
    lamp.position.set(Math.cos(a) * entity.widthM * .48, 2.25, Math.sin(a) * entity.depthM * .48)
    group.add(lamp)
    const bulb = new THREE.Mesh(new THREE.SphereGeometry(.6, 6, 4), new THREE.MeshStandardMaterial({ color: '#ffe9a8', emissive: '#ffd76a', emissiveIntensity: 1.6 }))
    bulb.position.set(Math.cos(a) * entity.widthM * .48, 4.6, Math.sin(a) * entity.depthM * .48)
    group.add(bulb)
  }
  group.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return group
}

function makeParkingLot(entity, THREE) {
  const group = new THREE.Group()
  const asphalt = new THREE.Mesh(
    new THREE.BoxGeometry(entity.widthM, .3, entity.depthM),
    new THREE.MeshStandardMaterial({ color: '#5a5d60', roughness: 1 })
  )
  asphalt.position.y = .15
  group.add(asphalt)
  const rows = Math.max(2, Math.round(entity.depthM / 4.5))
  const lanes = Math.max(2, Math.round(entity.widthM / 3.2))
  const lineH = .04
  // Parking stall dividers so the lot reads as a parking area even when empty
  const dividerMat = new THREE.MeshStandardMaterial({ color: '#cfcba8', roughness: .95 })
  for (let lane = 0; lane <= lanes; lane++) {
    const divider = new THREE.Mesh(new THREE.BoxGeometry(.18, lineH, entity.depthM), dividerMat)
    divider.position.set(-entity.widthM / 2 + lane * (entity.widthM / lanes), .34, 0)
    group.add(divider)
  }
  for (let row = 0; row <= rows; row++) {
    const divider = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM, lineH, .18), dividerMat)
    divider.position.set(0, .34, -entity.depthM / 2 + row * (entity.depthM / rows))
    group.add(divider)
  }
  for (let lane = 0; lane < lanes; lane++) {
    for (let row = 0; row < rows; row++) {
      const rx = -entity.widthM / 2 + (lane + .5) * (entity.widthM / lanes)
      const rz = -entity.depthM / 2 + (row + .5) * (entity.depthM / rows)
      if (deterministicRand(entity.worldX, entity.worldZ, lane * 13 + row * 29) > .78) continue
      const car = new THREE.Group()
      const carMat = new THREE.MeshStandardMaterial({ color: ['#b8463a', '#3f6f91', '#e0d8c0', '#3c3f42', '#7a9448'][Math.floor(deterministicRand(entity.worldX, entity.worldZ, lane + row * 3 + 17) * 5) % 5], roughness: .4, metalness: .35 })
      const carBody = new THREE.Mesh(new THREE.BoxGeometry(2, .55, 4.1), carMat)
      car.add(carBody)
      const cab = new THREE.Mesh(new THREE.BoxGeometry(1.6, .5, 2.1), new THREE.MeshStandardMaterial({ color: '#2e3438', roughness: .3, metalness: .4 }))
      cab.position.y = .55
      cab.position.z = -.2
      car.add(cab)
      for (const wz of [-1.4, 1.4]) {
        for (const wx of [-.7, .7]) {
          const wheel = new THREE.Mesh(new THREE.CylinderGeometry(.3, .3, .24, 8), new THREE.MeshStandardMaterial({ color: '#15181a', roughness: .95 }))
          wheel.rotation.x = Math.PI / 2
          wheel.position.set(wx, .32, wz)
          car.add(wheel)
        }
      }
      car.position.set(rx, .35, rz)
      car.rotation.y = (row % 2) * Math.PI / 2
      group.add(car)
    }
  }
  group.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return group
}

function makeTemple(entity, THREE) {
  const group = new THREE.Group()
  const wall = new THREE.MeshStandardMaterial({ color: '#c0402e', roughness: .85 })
  const roof = new THREE.MeshStandardMaterial({ color: '#2e3a34', roughness: .9 })
  const gold = new THREE.MeshStandardMaterial({ color: '#e0b64a', roughness: .4, metalness: .4 })
  const base = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM, .8, entity.depthM), new THREE.MeshStandardMaterial({ color: '#c9b8a0', roughness: .95 }))
  base.position.y = .4
  group.add(base)
  const hall = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM * .6, entity.heightM * .5, entity.depthM * .6), wall)
  hall.position.y = .8 + entity.heightM * .25
  group.add(hall)
  const roofSlab = new THREE.Mesh(new THREE.ConeGeometry(entity.widthM * .6, entity.heightM * .35, 4), roof)
  roofSlab.rotation.y = Math.PI / 4
  roofSlab.position.y = .8 + entity.heightM * .72
  group.add(roofSlab)
  const finial = new THREE.Mesh(new THREE.SphereGeometry(.5, 6, 4), gold)
  finial.position.y = .8 + entity.heightM * .9
  group.add(finial)
  for (const side of [-1, 1]) {
    const pillar = new THREE.Mesh(new THREE.CylinderGeometry(.45, .5, entity.heightM * .6, 8), new THREE.MeshStandardMaterial({ color: '#b5453a', roughness: .85 }))
    pillar.position.set(side * entity.widthM * .22, .8 + entity.heightM * .3, 0)
    group.add(pillar)
  }
  group.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return group
}

function makeChurch(entity, THREE) {
  const group = new THREE.Group()
  const wall = new THREE.MeshStandardMaterial({ color: '#c9b8a0', roughness: .92 })
  const roof = new THREE.MeshStandardMaterial({ color: '#6d5548', roughness: .85 })
  const glass = new THREE.MeshStandardMaterial({ color: '#4a7a9a', roughness: .2, emissive: '#2a4a6a', emissiveIntensity: .3 })
  const body = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM * .7, entity.heightM * .6, entity.depthM * .72), wall)
  body.position.y = entity.heightM * .3
  group.add(body)
  const gable = new THREE.Mesh(new THREE.ConeGeometry(entity.widthM * .5, entity.heightM * .5, 3), roof)
  gable.rotation.y = Math.PI
  gable.position.set(0, entity.heightM * .72, -entity.depthM * .2)
  group.add(gable)
  const tower = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM * .2, entity.heightM * 1.05, entity.depthM * .24), wall)
  tower.position.set(entity.widthM * .26, entity.heightM * .62, 0)
  group.add(tower)
  const spire = new THREE.Mesh(new THREE.ConeGeometry(entity.widthM * .1, entity.heightM * .4, 6), roof)
  spire.position.set(entity.widthM * .26, entity.heightM * 1.3, 0)
  group.add(spire)
  const rose = new THREE.Mesh(new THREE.CircleGeometry(entity.widthM * .12, 12), glass)
  rose.rotation.y = Math.PI / 2
  rose.position.set(0, entity.heightM * .48, -entity.depthM * .36 - .05)
  group.add(rose)
  group.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return group
}

function makeFarmland(entity, THREE) {
  const group = new THREE.Group()
  const soil = new THREE.Mesh(
    new THREE.BoxGeometry(entity.widthM, .4, entity.depthM),
    new THREE.MeshStandardMaterial({ color: '#6b5638', roughness: 1 })
  )
  soil.position.y = .2
  group.add(soil)
  const cropMat = new THREE.MeshStandardMaterial({ color: '#9a8747', roughness: 1 })
  const cropLight = new THREE.MeshStandardMaterial({ color: '#b5a860', roughness: .95 })
  const rows = Math.max(3, Math.round(entity.depthM / 8))
  const cols = Math.max(3, Math.round(entity.widthM / 8))
  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      const roll = deterministicRand(entity.worldX, entity.worldZ, r * 31 + c * 17)
      if (roll < .18) continue
      const crop = new THREE.Mesh(
        new THREE.BoxGeometry(2.4, 1.1, 2.4),
        roll > .55 ? cropMat : cropLight
      )
      crop.position.set(
        -entity.widthM / 2 + (c + .5) * (entity.widthM / cols),
        .75,
        -entity.depthM / 2 + (r + .5) * (entity.depthM / rows)
      )
      group.add(crop)
    }
  }
  group.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return group
}

function makePasture(entity, THREE) {
  const group = new THREE.Group()
  const grass = new THREE.Mesh(
    new THREE.BoxGeometry(entity.widthM, .35, entity.depthM),
    new THREE.MeshStandardMaterial({ color: '#78904b', roughness: 1 })
  )
  grass.position.y = .175
  group.add(grass)
  for (let i = 0; i < 4; i++) {
    const r = deterministicRand(entity.worldX, entity.worldZ, i + 41)
    const cow = new THREE.Group()
    const body = new THREE.Mesh(new THREE.BoxGeometry(2.2, 1.1, 1.2), new THREE.MeshStandardMaterial({ color: r > .5 ? '#e8e2d8' : '#6d5a48', roughness: .9 }))
    cow.add(body)
    const head = new THREE.Mesh(new THREE.BoxGeometry(.6, .7, .6), new THREE.MeshStandardMaterial({ color: r > .5 ? '#e8e2d8' : '#6d5a48', roughness: .9 }))
    head.position.set(1.3, .55, 0)
    cow.add(head)
    for (const side of [-1, 1]) {
      const leg = new THREE.Mesh(new THREE.BoxGeometry(.3, .8, .3), new THREE.MeshStandardMaterial({ color: '#4a4238', roughness: .9 }))
      leg.position.set(side * .6, -.45, side * .35)
      cow.add(leg)
    }
    cow.position.set(
      (r - .5) * entity.widthM * .7,
      .6,
      ((deterministicRand(entity.worldX, entity.worldZ, i + 67) - .5) * entity.depthM * .7)
    )
    cow.rotation.y = r * Math.PI * 2
    group.add(cow)
  }
  group.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return group
}

function makeCanal(entity, THREE) {
  const group = new THREE.Group()
  const water = new THREE.Mesh(
    new THREE.BoxGeometry(entity.widthM, .5, entity.depthM),
    new THREE.MeshStandardMaterial({ color: '#477c87', roughness: .25, metalness: .3, transparent: true, opacity: .85 })
  )
  water.position.y = .25
  group.add(water)
  const bankMat = new THREE.MeshStandardMaterial({ color: '#6d6a5e', roughness: 1 })
  for (const side of [-1, 1]) {
    const bank = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM + 2, .8, 2.4), bankMat)
    bank.position.set(0, .35, side * (entity.depthM / 2 + 1.1))
    group.add(bank)
  }
  group.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return group
}

function makeTownHall(entity, THREE, heightAt) {
  const group = new THREE.Group()
  const plaza = new THREE.Mesh(
    new THREE.BoxGeometry(entity.widthM * 1.25, .3, entity.depthM * 1.25),
    new THREE.MeshStandardMaterial({ color: '#8a8578', roughness: 1 })
  )
  plaza.position.y = .15
  group.add(plaza)
  const wall = new THREE.MeshStandardMaterial({ color: '#d8ccb2', roughness: .85 })
  const roof = new THREE.MeshStandardMaterial({ color: '#7a6a52', roughness: .9 })
  const accent = new THREE.MeshStandardMaterial({ color: '#b05038', roughness: .7 })
  const hall = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM * .7, entity.heightM * .55, entity.depthM * .6), wall)
  hall.position.y = entity.heightM * .275
  group.add(hall)
  const columns = 3
  for (let i = 0; i < columns; i++) {
    const column = new THREE.Mesh(new THREE.CylinderGeometry(.7, .8, entity.heightM * .4, 10), wall)
    column.position.set(-entity.widthM * .26 + i * entity.widthM * .26, entity.heightM * .2, entity.depthM * .31)
    group.add(column)
  }
  const pediment = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM * .72, entity.heightM * .14, entity.depthM * .62), accent)
  pediment.position.set(0, entity.heightM * .64, 0)
  group.add(pediment)
  const roofSlab = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM * .78, entity.heightM * .1, entity.depthM * .68), roof)
  roofSlab.position.set(0, entity.heightM * .74, 0)
  group.add(roofSlab)
  const tower = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM * .12, entity.heightM * 1.2, entity.depthM * .18), wall)
  tower.position.set(entity.widthM * .26, entity.heightM * .62, 0)
  group.add(tower)
  const dome = new THREE.Mesh(new THREE.SphereGeometry(entity.widthM * .06, 10, 8), accent)
  dome.position.set(entity.widthM * .26, entity.heightM * 1.32, 0)
  group.add(dome)
  for (const side of [-1, 1]) {
    const flag = new THREE.Mesh(new THREE.BoxGeometry(1.6, entity.heightM * .14, .12), accent)
    flag.position.set(entity.widthM * .26 + side * 1, entity.heightM * 1.2, 0)
    group.add(flag)
  }
  applySlopeFoundation(group, entity, heightAt, THREE)
  group.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return group
}

function makeMarket(entity, THREE, heightAt) {
  const group = new THREE.Group()
  const ground = new THREE.Mesh(
    new THREE.BoxGeometry(entity.widthM, .3, entity.depthM),
    new THREE.MeshStandardMaterial({ color: '#9a8a72', roughness: 1 })
  )
  ground.position.y = .15
  group.add(ground)
  const canopyMat = new THREE.MeshStandardMaterial({ color: '#c65b43', roughness: .8 })
  const stallMat = new THREE.MeshStandardMaterial({ color: '#e8e0cc', roughness: .9 })
  const stalls = 4
  for (let i = 0; i < stalls; i++) {
    const stall = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM / stalls * .7, entity.heightM * .6, entity.depthM * .5), stallMat)
    const sx = -entity.widthM / 2 + (i + .5) * (entity.widthM / stalls)
    stall.position.set(sx, entity.heightM * .3, -entity.depthM * .05)
    group.add(stall)
    const canopy = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM / stalls * .85, .4, entity.depthM * .55), canopyMat)
    canopy.position.set(sx, entity.heightM * .66, -entity.depthM * .05)
    group.add(canopy)
    for (const side of [-1, 1]) {
      const post = new THREE.Mesh(new THREE.CylinderGeometry(.16, .16, entity.heightM * .62, 6), new THREE.MeshStandardMaterial({ color: '#6d5a48', roughness: .9 }))
      post.position.set(sx + side * entity.widthM / stalls * .38, entity.heightM * .31, -entity.depthM * .32)
      group.add(post)
    }
    const crate = new THREE.Mesh(new THREE.BoxGeometry(1.2, .8, 1.2), new THREE.MeshStandardMaterial({ color: ['#c0392b', '#d18c32', '#5d8a4a', '#7a5db0'][i % 4], roughness: .85 }))
    crate.position.set(sx, .4, entity.depthM * .3)
    group.add(crate)
  }
  applySlopeFoundation(group, entity, heightAt, THREE)
  group.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return group
}

function makeIndustrial(entity, THREE, heightAt) {
  const group = new THREE.Group()
  const yard = new THREE.Mesh(
    new THREE.BoxGeometry(entity.widthM, .3, entity.depthM),
    new THREE.MeshStandardMaterial({ color: '#6a6460', roughness: 1 })
  )
  yard.position.y = .15
  group.add(yard)
  const steel = new THREE.MeshStandardMaterial({ color: '#7d8791', roughness: .7, metalness: .5 })
  const roof = new THREE.MeshStandardMaterial({ color: '#5a6670', roughness: .8 })
  const brick = new THREE.MeshStandardMaterial({ color: '#a06a4a', roughness: .9 })
  const sheds = 3
  for (let i = 0; i < sheds; i++) {
    const shedW = entity.widthM * .28
    const shedH = entity.heightM * .55
    const shed = new THREE.Mesh(new THREE.BoxGeometry(shedW, shedH, entity.depthM * .7), i === 1 ? brick : steel)
    shed.position.set(-entity.widthM * .28 + i * entity.widthM * .28, shedH / 2, -entity.depthM * .05)
    group.add(shed)
    const sawtooth = new THREE.Mesh(new THREE.ConeGeometry(shedW * .6, shedH * .35, 3), roof)
    sawtooth.rotation.y = i % 2 ? Math.PI : 0
    sawtooth.position.set(-entity.widthM * .28 + i * entity.widthM * .28, shedH + shedH * .17, -entity.depthM * .05)
    group.add(sawtooth)
    for (const side of [-1, 1]) {
      const chimney = new THREE.Mesh(new THREE.CylinderGeometry(1, 1.2, entity.heightM * .9, 10), brick)
      chimney.position.set(-entity.widthM * .28 + i * entity.widthM * .28 + side * entity.widthM * .1, shedH + entity.heightM * .45, -entity.depthM * .35)
      group.add(chimney)
    }
  }
  const gate = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM * .14, entity.heightM * .3, 1.5), steel)
  gate.position.set(0, entity.heightM * .15, entity.depthM / 2 - .5)
  group.add(gate)
  for (let i = 0; i < 3; i++) {
    const crate = new THREE.Mesh(new THREE.BoxGeometry(3, 2.5, 3), new THREE.MeshStandardMaterial({ color: '#8a7a5a', roughness: .9 }))
    crate.position.set(-entity.widthM * .35 + i * 4, 1.3, entity.depthM * .34)
    group.add(crate)
  }
  applySlopeFoundation(group, entity, heightAt, THREE)
  group.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return group
}

function makeWaterWell(entity, THREE) {
  const group = new THREE.Group()
  const ring = new THREE.Mesh(
    new THREE.CylinderGeometry(entity.widthM * .5, entity.widthM * .55, entity.heightM * .7, 10),
    new THREE.MeshStandardMaterial({ color: '#b0a898', roughness: .9 })
  )
  ring.position.y = entity.heightM * .35
  group.add(ring)
  const water = new THREE.Mesh(new THREE.CircleGeometry(entity.widthM * .42, 12), new THREE.MeshStandardMaterial({ color: '#3a6a78', roughness: .3, metalness: .2 }))
  water.rotation.x = -Math.PI / 2
  water.position.y = entity.heightM * .62
  group.add(water)
  for (const side of [-1, 1]) {
    const post = new THREE.Mesh(new THREE.CylinderGeometry(.1, .12, entity.heightM * .6, 6), new THREE.MeshStandardMaterial({ color: '#6d5a48', roughness: .9 }))
    post.position.set(side * entity.widthM * .5, entity.heightM * .3, 0)
    group.add(post)
  }
  const roof = new THREE.Mesh(new THREE.ConeGeometry(entity.widthM * .6, entity.heightM * .55, 4), new THREE.MeshStandardMaterial({ color: '#6d5a48', roughness: .9 }))
  roof.rotation.y = Math.PI / 4
  roof.position.y = entity.heightM * .62
  group.add(roof)
  group.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return group
}

function makeRoadSign(entity, THREE) {
  const group = new THREE.Group()
  const pole = new THREE.Mesh(new THREE.CylinderGeometry(.07, .09, entity.heightM, 6), new THREE.MeshStandardMaterial({ color: '#3e4648', roughness: .6, metalness: .5 }))
  pole.position.y = entity.heightM / 2
  group.add(pole)
  const sign = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM, entity.heightM * .32, .1), new THREE.MeshStandardMaterial({ color: '#2a6a8a', roughness: .5, emissive: '#1a4a6a', emissiveIntensity: .3 }))
  sign.position.y = entity.heightM * .82
  group.add(sign)
  const stripe = new THREE.Mesh(new THREE.BoxGeometry(entity.widthM, entity.heightM * .1, .11), new THREE.MeshStandardMaterial({ color: '#f2efe6', roughness: .5 }))
  stripe.position.y = entity.heightM * .82
  group.add(stripe)
  group.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return group
}

function makeStorefront(entity, THREE) {
  const shop = cgaBuild(entity, CGA_STYLES.storefront, THREE)
  // Face the street: shops sit in a frontage row along Z, so rotate the whole
  // lot so the display window / door point at the road.
  shop.rotation.y = entity.worldX < scene.widthM * .5 ? Math.PI / 2 : -Math.PI / 2
  return shop
}

// ---------------------------------------------------------------------------
// Lightweight CGA-style building rules.
//
// A "style" is pure data: a list of blocks that the interpreter expands into
// BoxGeometry meshes. Each block is anchored to the building pad (0,0,0 = the
// ground centre of the lot) and sized with fractions of the entity, absolute
// metres, or a small expression language. Blocks may repeat along Y (floors),
// mirror across an axis, or gate on a condition, so a single rule set adapts
// to any lot size — the same idea as symbios' Split / Extrude / Comp grammar,
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
//   part:  for repeatY — the block repeated each floor.
// ---------------------------------------------------------------------------
const CGA_MATS = {
  glass: { color: '#385e68', roughness: .18, metalness: .35 },
  glass2: { color: '#9cc9dd', roughness: .18, metalness: .3 },
  white: { color: '#f0e9dd', roughness: .82 },
  trim: { color: '#c6a56e', roughness: .62 },
  louver: { color: '#8a6a4c', roughness: .9 },
  accent: { color: '#3f6f8f', roughness: .6 },
  dark: { color: '#4a3b2c', roughness: .9 },
  wall: { color: '#756b61', roughness: .86 },
  terrace: { color: '#cfc8b8', roughness: .96 },
  roof: { color: '#7a5f44', roughness: .85 },
  green: { color: '#5d8a4a', roughness: 1 },
  metal: { color: '#58636a', roughness: .5, metalness: .6 },
}

function cgaMat(name, THREE) {
  const spec = CGA_MATS[name] ?? { color: name ?? '#cccccc', roughness: .8 }
  return new THREE.MeshStandardMaterial(spec)
}

// Measure a size token against the entity: "~0.8w" -> fraction of width,
// "3" -> 3 metres, "~0.5h" -> fraction of height, "~0.3d" -> fraction depth.
function cgaSize(token, entity, axis) {
  if (typeof token === 'number') return token
  if (typeof token === 'string') {
    const m = token.match(/^~([\d.]+)([whd])$/)
    if (m) {
      const base = m[2] === 'w' ? entity.widthM : m[2] === 'd' ? entity.depthM : entity.heightM
      return base * parseFloat(m[1])
    }
    return parseFloat(token) || 0
  }
  if (Array.isArray(token)) {
    // min/max range, deterministic from position.
    const r = deterministicRand(entity.worldX, entity.worldZ, axis * 31 + 7)
    return token[0] + r * (token[1] - token[0])
  }
  return 0
}

function cgaCond(cond, entity) {
  if (!cond) return true
  if (cond === 'floors>1') return Math.max(1, Math.floor(entity.heightM / 3.2)) > 1
  if (cond === 'width>14') return entity.widthM > 14
  if (cond === 'width>20') return entity.widthM > 20
  if (cond === 'height>8') return entity.heightM > 8
  return true
}

// Expand a single block into one or more meshes added to `group`.
function cgaExpand(block, entity, THREE, group) {
  if (!cgaCond(block.cond, entity)) return
  const w = cgaSize(block.w, entity, 0)
  const d = cgaSize(block.d ?? block.w, entity, 1)
  const h = cgaSize(block.h, entity, 2)
  const mat = cgaMat(block.mat ?? 'wall', THREE)
  const x = cgaSize(block.x ?? 0, entity, 3)
  const z = cgaSize(block.z ?? 0, entity, 4)
  const y = cgaSize(block.y ?? 0, entity, 5)

  if (block.kind === 'repeatY') {
    const start = cgaSize(block.start ?? 0, entity, 6)
    const step = cgaSize(block.step ?? 3, entity, 7)
    const count = block.count ?? Math.max(1, Math.floor((entity.heightM - start) / step))
    for (let i = 0; i < count; i++) {
      const cy = start + i * step
      if (cy > entity.heightM - h) break
      cgaExpand({ ...block.part, y: (block.part.y ?? 0) + cy }, entity, THREE, group)
    }
    return
  }
  if (block.kind === 'repeatX') {
    const start = cgaSize(block.start ?? 0, entity, 6)
    const step = cgaSize(block.step ?? 4, entity, 7)
    const count = block.count ?? Math.max(1, Math.floor((w - start) / step))
    for (let i = 0; i < count; i++) {
      const cx = start + i * step
      if (cx > w - h) break
      cgaExpand({ ...block.part, x: (block.part.x ?? 0) + cx }, entity, THREE, group)
    }
    return
  }
  if (block.kind === 'mirrorX') {
    for (const s of [-1, 1]) {
      cgaExpand({ ...block.part, x: (block.part.x ?? 0) * s }, entity, THREE, group)
    }
    return
  }

  // Default: a box.
  const mesh = new THREE.Mesh(new THREE.BoxGeometry(w, h, d), mat)
  mesh.position.set(x, y, z)
  if (block.rotY) mesh.rotation.y = block.rotY
  if (block.tiltZ) mesh.rotation.z = block.tiltZ
  group.add(mesh)
}

// Build a building from a CGA style rule set.
function cgaBuild(entity, style, THREE) {
  const group = new THREE.Group()
  for (const block of style) cgaExpand(block, entity, THREE, group)
  group.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return group
}

// ---------------------------------------------------------------------------
// Style rule sets.
// ---------------------------------------------------------------------------

// Shenzhen Bay / OCT resort-lodge idiom: white plaster, wood louver band,
// glass curtain strip, flat roof with parapet. Declarative equivalent of
// makeResortLodge.
const CGA_RESORT = [
  { kind: 'box', w: '~1w', d: '~1d', h: .35, y: .175, mat: 'terrace' },
  { kind: 'box', w: '~0.82w', d: '~0.72d', h: '~0.8h', y: '~0.4h', mat: 'white' },
  // Wood louver band flush on all four faces (cladding, not floating planks).
  { kind: 'mirrorX', part: { kind: 'box', w: '~0.02w', d: '~0.95d', h: '~0.14h', y: '~0.4h', x: '~0.41w', mat: 'louver' } },
  { kind: 'mirrorX', part: { kind: 'box', w: '~0.98w', d: '~0.02d', h: '~0.14h', y: '~0.4h', z: '~0.36d', mat: 'louver' } },
  // Glass curtain strip + teal accent band.
  { kind: 'mirrorX', part: { kind: 'box', w: '~0.7w', d: .14, h: '~0.34h', y: '~0.62h', z: '~0.36d', mat: 'glass2' } },
  { kind: 'box', w: '~0.8w', d: '~0.82d', h: '~0.12h', y: '~0.12h', mat: 'accent' },
  // Flat roof + parapet.
  { kind: 'box', w: '~1.04w', d: '~1.04d', h: .3, y: '~0.8h', mat: 'terrace' },
  { kind: 'mirrorX', part: { kind: 'box', w: '~1.1w', d: .14, h: .45, y: '~0.8h', z: '~0.52d', mat: 'white' } },
]

// Commercial storefront: a real 3.2m ground floor (window, awning, sign,
// door) plus repeated upper-floor window bands. No element floats mid-air.
const CGA_STOREFRONT = [
  { kind: 'box', w: '~1w', d: '~1d', h: '~1h', y: '~0.5h', mat: 'wall' },
  // Upper-floor window bands (repeat every 3m above the 3.2m shopfront).
  { kind: 'repeatY', start: 4.6, step: 3, count: 0, part: { kind: 'box', w: '~0.78w', d: .12, h: .9, z: '~0.5d', mat: 'glass' } },
  // Shopfront header bar separating glazing from upper facade.
  { kind: 'box', w: '~0.98w', d: '~0.96d', h: .5, y: 2.95, mat: 'trim' },
  // Ground-floor display window.
  { kind: 'box', w: '~0.72w', d: .12, h: 1.9, y: 1.35, z: '~0.5d', mat: 'glass' },
  // Awning just under the header (door height, not mid-tower).
  { kind: 'box', w: '~0.84w', d: .8, h: .5, y: 2.6, z: '~0.5d', mat: 'trim' },
  // Door.
  { kind: 'box', w: [1.6, 2.2], d: .14, h: 2.24, y: 1.12, z: '~0.5d', mat: 'dark' },
  // Signboard directly under header.
  { kind: 'box', w: '~0.8w', d: .18, h: .55, y: 2.05, z: '~0.5d', mat: 'accent', cond: 'width>14' },
  // Window frames.
  { kind: 'mirrorX', part: { kind: 'box', w: .18, d: .2, h: 1.9, y: 1.35, x: '~0.35w', z: '~0.5d', mat: 'trim' } },
  // Planter by the door.
  { kind: 'box', w: '~0.18w', d: .7, h: .5, y: .25, x: '~0.28w', z: '~0.5d', mat: 'green' },
]

const CGA_STYLES = {
  storefront: CGA_STOREFRONT,
  resort_lodge: CGA_RESORT,
}


function makeStreetLamp(entity, THREE) {
  const lamp = new THREE.Group()
  const metal = new THREE.MeshStandardMaterial({ color: '#3e4648', roughness: .78, metalness: .4 })
  const glow = new THREE.MeshStandardMaterial({ color: '#f4c66a', emissive: '#f4a52e', emissiveIntensity: 1.8, roughness: .28 })
  const pole = new THREE.Mesh(new THREE.CylinderGeometry(.13, .2, 4.8, 8), metal)
  pole.position.y = 2.4
  lamp.add(pole)
  const arm = new THREE.Mesh(new THREE.BoxGeometry(1.1, .12, .12), metal)
  arm.position.set(.42, 4.55, 0)
  lamp.add(arm)
  const head = new THREE.Mesh(new THREE.SphereGeometry(.22, 8, 6), glow)
  head.position.set(.92, 4.42, 0)
  lamp.add(head)
  lamp.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return lamp
}

function makePedestrian(entity, THREE) {
  const person = new THREE.Group()
  const shirt = new THREE.MeshStandardMaterial({ color: entity.worldZ % 2 ? '#c65b43' : '#3f6f91', roughness: .82 })
  const skin = new THREE.MeshStandardMaterial({ color: '#c18c6d', roughness: .9 })
  const body = new THREE.Mesh(new THREE.CapsuleGeometry(.3, .72, 4, 6), shirt)
  body.position.y = .78
  person.add(body)
  const head = new THREE.Mesh(new THREE.SphereGeometry(.27, 8, 6), skin)
  head.position.y = 1.55
  person.add(head)
  person.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return person
}

// Street-corner food stall (街角小吃摊): a shaded cart with a stove, a small
// seating area, a tiled apron underfoot, planted greenery and a shade tree.
// This is a self-contained micro-scene that reads as a lived-in corner even
// though every part is primitive geometry.
function makeFoodStall(entity, THREE) {
  const stall = new THREE.Group()
  const W = entity.widthM || 6
  const D = entity.depthM || 4
  const wood = new THREE.MeshStandardMaterial({ color: '#8a6a4a', roughness: .88 })
  const woodDark = new THREE.MeshStandardMaterial({ color: '#5f4a36', roughness: .92 })
  const steel = new THREE.MeshStandardMaterial({ color: '#4a4f52', roughness: .5, metalness: .6 })
  const roof = new THREE.MeshStandardMaterial({ color: '#b0302a', roughness: .85 })
  const roofLight = new THREE.MeshStandardMaterial({ color: '#e8d8c0', roughness: .8 })
  const glass = new THREE.MeshStandardMaterial({ color: '#9fc8d8', roughness: .15, metalness: .3 })
  const warm = new THREE.MeshStandardMaterial({ color: '#f4c66a', emissive: '#e8a02c', emissiveIntensity: 1.4, roughness: .4 })
  const foodMat = new THREE.MeshStandardMaterial({ color: '#c9802c', roughness: .95 })

  // --- Tiled apron (地砖): a small paved corner with paver grid -----------
  const paverTex = (() => {
    const c = document.createElement('canvas')
    c.width = 128; c.height = 128
    const g = c.getContext('2d')
    g.fillStyle = '#9a9388'
    g.fillRect(0, 0, 128, 128)
    g.strokeStyle = '#7d7668'; g.lineWidth = 4
    for (let i = 0; i < 4; i++) {
      g.beginPath(); g.moveTo(0, i * 32); g.lineTo(128, i * 32); g.stroke()
      g.beginPath(); g.moveTo(i * 32, 0); g.lineTo(i * 32, 128); g.stroke()
    }
    for (let y = 0; y < 4; y++) for (let x = 0; x < 4; x++) {
      g.fillStyle = ((x * 31 + y * 17) % 2) ? 'rgba(110,105,95,.4)' : 'rgba(140,132,120,.3)'
      g.fillRect(x * 32 + 5, y * 32 + 5, 22, 22)
    }
    const t = new THREE.CanvasTexture(c)
    t.wrapS = THREE.RepeatWrapping; t.wrapT = THREE.RepeatWrapping
    t.repeat.set(2, 1.4)
    t.colorSpace = THREE.SRGBColorSpace
    return t
  })()
  const apron = new THREE.Mesh(new THREE.BoxGeometry(W * 1.6, .14, D * 1.6), new THREE.MeshStandardMaterial({ map: paverTex, roughness: .92 }))
  apron.position.y = .07
  stall.add(apron)

  // --- Cart body + counter -------------------------------------------------
  const cart = new THREE.Mesh(new THREE.BoxGeometry(W * .8, .8, D * .5), wood)
  cart.position.set(-W * .12, .4, 0)
  stall.add(cart)
  // Front panel with a small serving shelf.
  const front = new THREE.Mesh(new THREE.BoxGeometry(W * .8, .5, .06), woodDark)
  front.position.set(-W * .12, .45, D * .25 + .03)
  stall.add(front)
  // Wheels.
  for (const sx of [-W * .3, W * .14]) for (const sz of [-D * .15, D * .15]) {
    const wheel = new THREE.Mesh(new THREE.CylinderGeometry(.22, .22, .12, 10), steel)
    wheel.rotation.x = Math.PI / 2
    wheel.position.set(-W * .12 + sx, .22, sz)
    stall.add(wheel)
  }
  // Stove + pot on the counter.
  const stove = new THREE.Mesh(new THREE.CylinderGeometry(.4, .42, .5, 10), steel)
  stove.position.set(-W * .12, .9, 0)
  stall.add(stove)
  const pot = new THREE.Mesh(new THREE.CylinderGeometry(.3, .28, .5, 10), steel)
  pot.position.set(-W * .12, 1.3, 0)
  stall.add(pot)
  const lid = new THREE.Mesh(new THREE.CylinderGeometry(.32, .32, .06, 10), foodMat)
  lid.position.set(-W * .12, 1.58, 0)
  stall.add(lid)
  // Steam.
  for (let i = 0; i < 3; i++) {
    const puff = new THREE.Mesh(new THREE.SphereGeometry(.14, 6, 5), new THREE.MeshStandardMaterial({ color: '#ffffff', transparent: true, opacity: .5, roughness: 1 }))
    puff.position.set(-W * .12 + (i - 1) * .1, 1.75 + i * .08, (i - 1) * .04)
    stall.add(puff)
  }

  // --- Awning (遮阳棚): striped canopy over the cart ----------------------
  const awningW = W * 1.05
  for (let i = 0; i < 8; i++) {
    const stripe = new THREE.Mesh(new THREE.BoxGeometry(awningW / 8, .05, D * .62), i % 2 ? roof : roofLight)
    stripe.position.set(-W * .12 - awningW / 2 + (i + .5) * awningW / 8, 2.15, 0)
    stripe.rotation.x = .06
    stall.add(stripe)
  }
  // Awning supports.
  for (const sx of [-W * .4, W * .16]) {
    const post = new THREE.Mesh(new THREE.CylinderGeometry(.06, .07, 2.1, 6), steel)
    post.position.set(-W * .12 + sx, 1.05, D * .22)
    stall.add(post)
  }
  // Sign hanging under the awning.
  const signCanvas = document.createElement('canvas')
  signCanvas.width = 128; signCanvas.height = 32
  const sctx = signCanvas.getContext('2d')
  sctx.fillStyle = '#c0392b'; sctx.fillRect(0, 0, 128, 32)
  sctx.fillStyle = '#fff1c4'; sctx.font = 'bold 22px serif'
  sctx.textAlign = 'center'; sctx.textBaseline = 'middle'
  sctx.fillText('小吃摊', 64, 18)
  const signTex = new THREE.CanvasTexture(signCanvas)
  signTex.colorSpace = THREE.SRGBColorSpace
  const sign = new THREE.Mesh(new THREE.BoxGeometry(W * .5, .32, .04), new THREE.MeshStandardMaterial({ map: signTex, emissive: '#43150f', emissiveIntensity: .4 }))
  sign.position.set(-W * .12, 1.9, D * .3)
  stall.add(sign)
  // Warm bulb.
  const bulb = new THREE.Mesh(new THREE.SphereGeometry(.09, 6, 5), warm)
  bulb.position.set(-W * .12, 2.0, -D * .15)
  stall.add(bulb)

  // --- Seating (小桌凳) ----------------------------------------------------
  const seatMat = new THREE.MeshStandardMaterial({ color: '#7a9b5a', roughness: .8 })
  for (let i = 0; i < 2; i++) {
    const tableX = W * .34 + i * W * .55
    const tableTop = new THREE.Mesh(new THREE.BoxGeometry(.7, .06, .7), seatMat)
    tableTop.position.set(tableX, .55, D * .1)
    stall.add(tableTop)
    const leg = new THREE.Mesh(new THREE.CylinderGeometry(.03, .03, .52, 5), steel)
    leg.position.set(tableX, .28, D * .1)
    stall.add(leg)
    for (const side of [-1, 1]) {
      const stool = new THREE.Mesh(new THREE.CylinderGeometry(.14, .14, .4, 6), seatMat)
      stool.position.set(tableX + side * .35, .2, D * .1 + side * .3)
      stall.add(stool)
    }
  }

  // --- Greenery (绿化): planter box + potted plants -----------------------
  const planterMat = new THREE.MeshStandardMaterial({ color: '#7d5a3e', roughness: .9 })
  const leafMat = new THREE.MeshStandardMaterial({ color: '#3f7a3a', roughness: .95 })
  const leafLight = new THREE.MeshStandardMaterial({ color: '#5d8f47', roughness: .95 })
  const planter = new THREE.Mesh(new THREE.BoxGeometry(W * .5, .4, .5), planterMat)
  planter.position.set(W * .5, .2, -D * .55)
  stall.add(planter)
  for (let i = 0; i < 3; i++) {
    const bush = new THREE.Mesh(new THREE.DodecahedronGeometry(.3, 0), i % 2 ? leafMat : leafLight)
    bush.position.set(W * .5 - .12 + i * .12, .55, -D * .55 + (i % 2 ? .1 : -.08))
    stall.add(bush)
  }

  // --- Shade tree (树木) ----------------------------------------------------
  const trunk = new THREE.Mesh(new THREE.CylinderGeometry(.18, .26, 2.6, 6), new THREE.MeshStandardMaterial({ color: '#5d4330', roughness: .96 }))
  trunk.position.set(-W * .7, 1.3, -D * .55)
  stall.add(trunk)
  const crown = new THREE.Mesh(new THREE.DodecahedronGeometry(1.6, 1), leafMat)
  crown.position.set(-W * .7, 3.2, -D * .55)
  stall.add(crown)
  const crown2 = new THREE.Mesh(new THREE.DodecahedronGeometry(1.0, 0), leafLight)
  crown2.position.set(-W * .7 + .4, 3.7, -D * .55 + .2)
  stall.add(crown2)

  stall.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return stall
}

function makeSkyTexture(THREE) {
  const canvas = document.createElement('canvas')
  canvas.width = 4
  canvas.height = 256
  const context = canvas.getContext('2d')
  const gradient = context.createLinearGradient(0, 0, 0, canvas.height)
  gradient.addColorStop(0, '#08131e')
  gradient.addColorStop(.48, '#294658')
  gradient.addColorStop(1, '#9b8062')
  context.fillStyle = gradient
  context.fillRect(0, 0, canvas.width, canvas.height)
  const texture = new THREE.CanvasTexture(canvas)
  texture.colorSpace = THREE.SRGBColorSpace
  return texture
}

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
  const THREE = await import('https://cdn.jsdelivr.net/npm/three@0.178.0/build/three.module.js')
  const { GLTFLoader } = await import('https://cdn.jsdelivr.net/npm/three@0.178.0/examples/jsm/loaders/GLTFLoader.js')
  const { OrbitControls } = await import('https://cdn.jsdelivr.net/npm/three@0.178.0/examples/jsm/controls/OrbitControls.js')
  const { OBJLoader } = await import('https://cdn.jsdelivr.net/npm/three@0.178.0/examples/jsm/loaders/OBJLoader.js')
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
    nearScene.background = makeSkyTexture(THREE)
    nearScene.fog = new THREE.Fog('#294658', 900, 3600)
    nearScene.add(new THREE.HemisphereLight(0xdce8df, 0x202820, 2))
    const sun = new THREE.DirectionalLight(0xffe6b0, 2.8)
    sun.position.set(-500, 900, 420)
    sun.castShadow = false
    nearScene.add(sun)
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
      const geometry = new THREE.PlaneGeometry(width, depth, Math.min(width - 1, Math.ceil(width / meshStep)), Math.min(depth - 1, Math.ceil(depth / meshStep))); geometry.rotateX(-Math.PI / 2)
      const positions = geometry.attributes.position
      for (let i = 0; i < positions.count; i++) { const x = Math.min(width - 1, Math.max(0, Math.round(positions.getX(i) + width / 2))); const z = Math.min(depth - 1, Math.max(0, Math.round(positions.getZ(i) + depth / 2))); positions.setY(i, heightView.getInt16((z * width + x) * 2, true) / 4) }
      geometry.computeVertexNormals(); const material = meshStep <= 2 ? (() => { const surfaceMaps = makeSurfaceTexture(surface, width, depth, THREE); surfaceMaps.texture.anisotropy = Math.min(8, nearRenderer.capabilities.getMaxAnisotropy()); surfaceMaps.normalTexture.anisotropy = surfaceMaps.texture.anisotropy; return new THREE.MeshStandardMaterial({ map: surfaceMaps.texture, normalMap: surfaceMaps.normalTexture, normalScale: new THREE.Vector2(.45, .45), roughness: .92 }) })() : new THREE.MeshStandardMaterial({ map: makeLodTexture(surface, width, depth, THREE), roughness: 1 }); mesh = new THREE.Mesh(geometry, material); entityMeshCache.set(meshCacheKey, mesh)
    }
    const meshClone = mesh.clone()
    meshClone.position.set(chunk.worldX - scene.originX + width / 2, 0, chunk.worldZ - scene.originZ + depth / 2)
    chunkGroup.add(meshClone)
    chunkGroup.userData.meshStep = meshStep
  }
  const heightFieldMap = new Map()
  const heightAt = (worldX, worldZ) => {
    const key = `${Math.floor(worldX / 512)},${Math.floor(worldZ / 512)}`
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
      // like an urban bayfront park — grass patches are inset beds, the ground
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
    const material = new THREE.MeshStandardMaterial({
      color: isWater ? '#326b78' : type === 'causeway' ? '#b38b5a' : type === 'pavilion' ? '#c84f3f' : '#6f8557',
      map: waterMaps?.texture ?? null,
      normalMap: waterMaps?.normalTexture ?? null,
      normalScale: waterMaps ? new THREE.Vector2(.7, .7) : new THREE.Vector2(0, 0),
      roughness: isWater ? .12 : .9,
      metalness: isWater ? .28 : 0,
      envMapIntensity: isWater ? 1.25 : .35,
      transparent: isWater,
      opacity: isWater ? .82 : 1,
      depthWrite: !isWater,
      side: isWater ? THREE.DoubleSide : THREE.FrontSide,
    })
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
    prop.position.set(entity.worldX, entity.worldY + baseHeight, entity.worldZ)
    if (entity.kind === 'fallen_log') prop.rotation.z = Math.PI / 2
    if ((entity.kind === 'road' || entity.kind === 'sidewalk') && (entity.entityId.includes('road-ns') || entity.entityId.includes('north-south'))) prop.rotation.y = Math.PI / 2
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
  nearRenderer.render(nearScene, nearCamera)
  window.glyphweaveFeedback.visualChecks.highDetailChunk = focusChunk ? `${focusChunk.chunkX},${focusChunk.chunkZ}` : null
  window.glyphweaveFeedback.visualChecks.loadedChunks = activeChunks.map(chunk => `${chunk.chunkX},${chunk.chunkZ}`)
  window.glyphweaveFeedback.visualChecks.nearCanvasReady = Boolean(webglCanvas.width && webglCanvas.height)
  window.glyphweaveFeedback.visualChecks.nearSceneRendered = true
  nearRenderer.setAnimationLoop(() => {
    const now = performance.now() * .001
    for (const actor of animatedActors) {
      actor.object.position.z = actor.baseZ + Math.sin(now * actor.speed + actor.phase) * (actor.object.userData.actorKind === 'car' ? 8 : 3)
    }
    for (const maps of waterTextures) {
      maps.texture.offset.x = (maps.texture.offset.x + .00025) % 1
      maps.texture.offset.y = (maps.texture.offset.y + .00055) % 1
      maps.normalTexture.offset.x = (maps.normalTexture.offset.x + .00025) % 1
      maps.normalTexture.offset.y = (maps.normalTexture.offset.y + .00055) % 1
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
      nearControls.update(); nearRenderer.render(nearScene, nearCamera)
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
  info.textContent = `${scene.sceneId}  ${scene.widthM}m × ${scene.depthM}m  ${scene.chunkCountX}×${scene.chunkCountZ} chunks`
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
