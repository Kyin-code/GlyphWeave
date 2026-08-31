import { colorRgb, fractalNoise, hashNoise, smoothNoise } from '../core/shared.js'

export function makeSurfaceTexture(surface, width, depth, THREE) {
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

export function makeLodTexture(surface, width, depth, THREE) {
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

export function makeStripGeometry(entity, halfWidth, centerZ, heightAt, THREE, lift, maxLength, centerX, alongZ) {
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

// Artistic steppe ground — painterly, Journey/Flower-style colour field.
// Instead of per-cell noise texture, the ground is a soft gradient across a
// warm steppe palette: dry grass yellow-greens on high ground, richer green
// in dips, warm earth along waterlines. Large-scale noise drives broad tonal
// regions, so it reads as a painted plain rather than a pixel-noise smear.
export function makeArtisticGroundTexture(surface, width, depth, THREE) {
  const c = document.createElement('canvas')
  const scale = 2
  c.width = width * scale
  c.height = depth * scale
  const g = c.getContext('2d')
  const img = g.createImageData(c.width, c.height)
  // Steppe palette (base rgb per surface id: 0 grass,1 light,2 rock,3 water,
  // 4 mud,5 soil,6 forest,7 dry).
  const P = [
    [164, 178, 110], [186, 188, 126], [150, 138, 120], [110, 160, 168],
    [150, 132, 92],  [172, 158, 108],  [118, 140, 96], [172, 160, 112],
  ]
  for (let y = 0; y < c.height; y++) {
    for (let x = 0; x < c.width; x++) {
      const cellX = Math.min(width - 1, Math.floor(x / scale))
      const cellZ = Math.min(depth - 1, Math.floor(y / scale))
      const m = surface[cellZ * width + cellX]
      const base = P[m] ?? P[0]
      // Broad tonal region (hundreds of metres) so the plain has soft painting
      // patches, not speckle.
      const region = fractalNoise(x * .004 + m * 9, y * .004 - m * 5)
      // Fine grass strand variance for a living field.
      const strand = smoothNoise(x * .06, y * .06)
      const tonal = region * 26 + strand * 10
      let r = base[0] + tonal
      let gg = base[1] + tonal
      let b = base[2] + tonal * .55
      // Warm light on sunny slopes via a broad NW-SE gradient.
      const sun = (x + y) / (c.width + c.height) * 18
      r += sun; gg += sun * .9; b += sun * .5
      const i = (y * c.width + x) * 4
      img.data[i] = Math.max(0, Math.min(255, r))
      img.data[i + 1] = Math.max(0, Math.min(255, gg))
      img.data[i + 2] = Math.max(0, Math.min(255, b))
      img.data[i + 3] = 255
    }
  }
  g.putImageData(img, 0, 0)
  const texture = new THREE.CanvasTexture(c)
  texture.colorSpace = THREE.SRGBColorSpace
  texture.minFilter = THREE.LinearMipmapLinearFilter
  texture.magFilter = THREE.LinearFilter
  texture.generateMipmaps = true
  return texture
}

// Wind-animated steppe grass: thousands of billboard grass blades swaying in a
// travelling wind (Journey/Flower style). Built with InstancedMesh + a vertex
// shader that bends each blade by a wind sine, so the whole field ripples.
export function makeSteppeGrass(THREE, worldX, worldZ, width, depth, heightAt, count, salt) {
  const n = Math.max(200, count)
  // Thin tapered blade: two base verts + one tip, so it reads as a blade of
  // grass rather than a flat square (the main reason it looked like dots).
  const geo = new THREE.InstancedBufferGeometry()
  geo.setAttribute('position', new THREE.BufferAttribute(new Float32Array([-1, 0, 0, 1, 0, 0, 0, 1, 0]), 3))
  const inst = new THREE.InstancedMesh(geo, null, n)
  const dummy = new THREE.Object3D()
  const tints = new Float32Array(n * 3)
  const rnd = (a, b, s) => { let h = Math.imul(a + s * 1013904223, 2654435761) ^ Math.imul(b + s * 340573321, 1597334677); h = (h ^ (h >> 13)) * 1274126177; h = h ^ (h >> 16); return ((h >>> 0) % 100000) / 100000 }
  let placed = 0
  let guard = 0
  let x = 0, z = 0, gy = 0
  while (placed < n && guard < n * 40) {
    guard++
    x = worldX + rnd(placed, 0, salt) * width
    z = worldZ + rnd(placed, 1, salt) * depth
    gy = heightAt(x, z)
    const h = .45 + rnd(placed, 2, salt) * .75
    const tilt = (rnd(placed, 3, salt) - .5) * .35
    dummy.position.set(x, gy, z)
    dummy.rotation.set(tilt, rnd(placed, 4, salt) * Math.PI * 2, 0)
    dummy.scale.set(.12, h, 1)
    dummy.updateMatrix()
    inst.setMatrixAt(placed, dummy.matrix)
    const tt = rnd(placed, 5, salt)
    tints[placed * 3] = .45 + tt * .25
    tints[placed * 3 + 1] = .55 + tt * .22
    tints[placed * 3 + 2] = .22 + tt * .14
    placed++
  }
  if (placed < n) inst.count = placed
  inst.instanceMatrix.needsUpdate = true
  geo.setAttribute('tint', new THREE.InstancedBufferAttribute(tints, 3))
  const mat = new THREE.ShaderMaterial({
    uniforms: {
      time: { value: 0 },
      fogColor: { value: new THREE.Color(0xe8d9b8) },
      fogNear: { value: 800 },
      fogFar: { value: 5000 },
    },
    vertexShader: `
      attribute vec3 tint;
      varying vec3 vColor;
      varying float vDepth;
      uniform float time;
      void main() {
        vColor = tint;
        vec3 p = position;
        float sway = sin(time * 1.5 + instanceMatrix[3][0] * .02 + instanceMatrix[3][2] * .03) * .35;
        p.x += position.y * sway;
        p.z += position.y * sway * .5;
        vec4 mv = modelViewMatrix * instanceMatrix * vec4(p, 1.0);
        vDepth = -mv.z;
        gl_Position = projectionMatrix * mv;
      }
    `,
    fragmentShader: `
      varying vec3 vColor;
      varying float vDepth;
      uniform vec3 fogColor;
      uniform float fogNear;
      uniform float fogFar;
      void main() {
        float fog = clamp((vDepth - fogNear) / (fogFar - fogNear), 0.0, 1.0);
        vec3 c = mix(vColor, fogColor, fog);
        gl_FragColor = vec4(c, 1.0);
      }
    `,
    side: THREE.DoubleSide,
  })
  inst.material = mat
  inst.frustumCulled = false
  return { mesh: inst, update: t => { mat.uniforms.time.value = t } }
}


// Minecraft / Journey-style terrain: no texture map, just flat vertex colours
// graded by height and slope, lit by a simple Lambert light. This gives the
// clean "painted blocks" look instead of a noisy photoreal smear.
export function makeMCTerrain(heightView, width, depth, meshStep, THREE) {
  const geo = new THREE.PlaneGeometry(width, depth, Math.min(width - 1, Math.ceil(width / meshStep)), Math.min(depth - 1, Math.ceil(depth / meshStep)))
  geo.rotateX(-Math.PI / 2)
  const pos = geo.attributes.position
  const colors = new Float32Array(pos.count * 3)
  let minH = Infinity, maxH = -Infinity
  for (let i = 0; i < pos.count; i++) {
    const x = Math.min(width - 1, Math.max(0, Math.round(pos.getX(i) + width / 2)))
    const z = Math.min(depth - 1, Math.max(0, Math.round(pos.getZ(i) + depth / 2)))
    const h = heightView.getInt16((z * width + x) * 2, true) / 4
    pos.setY(i, h)
    if (h < minH) minH = h
    if (h > maxH) maxH = h
  }
  // Store height for grading (re-read from positions after set).
  const range = Math.max(1, maxH - minH)
  for (let i = 0; i < pos.count; i++) {
    const x = Math.min(width - 1, Math.max(0, Math.round(pos.getX(i) + width / 2)))
    const z = Math.min(depth - 1, Math.max(0, Math.round(pos.getZ(i) + depth / 2)))
    const h = heightView.getInt16((z * width + x) * 2, true) / 4
    // Steppe palette: deep grass in dips -> dry gold on rises.
    const t = (h - minH) / range
    let r, g, b
    if (t < .35) { const s = t / .35; r = 84; g = 116; b = 62; }
    else if (t < .75) { const s = (t - .35) / .4; r = 84 + (156 - 84) * s; g = 116 + (140 - 116) * s; b = 62 + (86 - 62) * s; }
    else { const s = (t - .75) / .25; r = 156 + (182 - 156) * s; g = 140 + (150 - 140) * s; b = 86 + (98 - 86) * s; }
    // Subtle per-cell block variance (Minecraft-ish) from position hash.
    const v = ((x * 31 + z * 17) % 13) / 13 - .5
    colors[i * 3] = r / 255 + v * .02
    colors[i * 3 + 1] = g / 255 + v * .02
    colors[i * 3 + 2] = b / 255 + v * .02
  }
  geo.setAttribute('color', new THREE.BufferAttribute(colors, 3))
  geo.computeVertexNormals()
  return geo
}
