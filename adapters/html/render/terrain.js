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
