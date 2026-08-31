export function makeWaterWell(entity, THREE) {
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

export function makeRoadSign(entity, THREE) {
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

export function makeStreetLamp(entity, THREE) {
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

export function makePedestrian(entity, THREE) {
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
export function makeFoodStall(entity, THREE) {
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
