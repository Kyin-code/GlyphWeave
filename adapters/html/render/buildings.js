import { deterministicRand } from '../core/shared.js'

export function fitAssetToEntity(asset, entity, THREE, fallback) {
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

export function addBridgeBeam(group, x1, y1, x2, y2, z, thickness, material, THREE) {
  const dx = x2 - x1; const dy = y2 - y1
  const length = Math.hypot(dx, dy)
  const beam = new THREE.Mesh(new THREE.BoxGeometry(length, thickness, thickness), material)
  beam.position.set((x1 + x2) / 2, (y1 + y2) / 2, z)
  beam.rotation.z = -Math.atan2(dy, dx)
  group.add(beam)
}

export function makeBridgeStructure(spec, THREE, heightAt) {
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

export function makeHongKongTower(spec, THREE) {
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

// Slope-aware plinth (Mountain-City style). A building with a large footprint
// on uneven ground either floats or sinks at its corners; this adds a concrete
// foundation that reaches from the building anchor down to the local terrain
// so the structure reads as built into the slope (like Chongqing / stilt
// houses) instead of clipping through the grass.
export function applySlopeFoundation(group, entity, heightAt, THREE, plinthColor = '#9a938a') {
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

export function makeResidentialBlock(entity, THREE) {
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
export function makeResidentialTower(entity, THREE, heightAt) {
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
export function makeResidentialHome(entity, THREE, heightAt) {
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
export function makeResortLodge(entity, THREE, heightAt) {
  return cgaBuild(entity, CGA_RESORT, THREE)
}

export function makeSchool(entity, THREE, heightAt) {
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

export function makeCommercialCenter(entity, THREE, heightAt) {
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

export function makeEntertainmentCenter(entity, THREE) {
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

export function makeParkingLot(entity, THREE) {
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

export function makeTemple(entity, THREE) {
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

export function makeChurch(entity, THREE) {
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

export function makeFarmland(entity, THREE) {
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

export function makePasture(entity, THREE) {
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

export function makeCanal(entity, THREE) {
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

export function makeTownHall(entity, THREE, heightAt) {
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

export function makeMarket(entity, THREE, heightAt) {
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

export function makeIndustrial(entity, THREE, heightAt) {
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

export function makeStorefront(entity, THREE) {
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
export const CGA_MATS = {
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

export function cgaMat(name, THREE) {
  const spec = CGA_MATS[name] ?? { color: name ?? '#cccccc', roughness: .8 }
  return new THREE.MeshStandardMaterial(spec)
}

export function cgaSize(token, entity, axis) {
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

export function cgaCond(cond, entity) {
  if (!cond) return true
  if (cond === 'floors>1') return Math.max(1, Math.floor(entity.heightM / 3.2)) > 1
  if (cond === 'width>14') return entity.widthM > 14
  if (cond === 'width>20') return entity.widthM > 20
  if (cond === 'height>8') return entity.heightM > 8
  return true
}

export function cgaExpand(block, entity, THREE, group) {
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

export function cgaBuild(entity, style, THREE) {
  const group = new THREE.Group()
  for (const block of style) cgaExpand(block, entity, THREE, group)
  group.position.set(entity.worldX, entity.worldY, entity.worldZ)
  return group
}

export const CGA_RESORT = [
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

export const CGA_STOREFRONT = [
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

export const CGA_STYLES = {
  storefront: CGA_STOREFRONT,
  resort_lodge: CGA_RESORT,
}

