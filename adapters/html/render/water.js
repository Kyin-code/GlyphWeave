export function makeWaterTexture(THREE, river) {
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

export function makeRiverGeometry(landmark, THREE) {
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
