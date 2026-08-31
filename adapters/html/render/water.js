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

// Stylized water material with Fresnel-driven transparency + animated normal
// shimmer. The base color deepens toward the horizon (view angle), and the
// roughness/metallic read gives it a glassy, reflective surface — Journey-like
// calm water rather than flat paint. `opacity` here is the min opacity (viewed
// straight down); grazing angles become near-opaque so the shore reads solid.
export function makeWaterMaterial(THREE, isRiver, baseColor) {
  const material = new THREE.MeshStandardMaterial({
    color: baseColor ?? (isRiver ? '#2e5f6e' : '#326b78'),
    roughness: .18,
    metalness: .35,
    transparent: true,
    opacity: .86,
    depthWrite: false,
    side: THREE.DoubleSide,
    envMapIntensity: 1.4,
  })
  const uniforms = {
    uTime: { value: 0 },
  }
  material.onBeforeCompile = (shader) => {
    Object.assign(shader.uniforms, uniforms)
    shader.vertexShader = shader.vertexShader
      .replace(
        '#include <common>',
        /* glsl */ `#include <common>
        varying vec3 vWorldPos;
        varying vec3 vWorldNormal;
        uniform float uTime;
        `
      )
      .replace(
        '#include <begin_vertex>',
        /* glsl */ `#include <begin_vertex>
        vec4 wp = modelMatrix * vec4(transformed, 1.0);
        vWorldPos = wp.xyz;
        vWorldNormal = normalize(mat3(modelMatrix) * normal);
        // Gentle animated vertex ripple so the surface is never flat.
        float wave = sin(vWorldPos.x * .35 + uTime * 1.4) * .04
                   + sin(vWorldPos.z * .29 - uTime * 1.1) * .035;
        transformed.y += wave;
        `
      )
    shader.fragmentShader = shader.fragmentShader
      .replace(
        '#include <common>',
        /* glsl */ `#include <common>
        varying vec3 vWorldPos;
        varying vec3 vWorldNormal;
        uniform float uTime;
        `
      )
      .replace(
        '#include <opaque_fragment>',
        /* glsl */ `#include <opaque_fragment>
        // Fresnel: grazing angles become opaque, straight-down stays translucent.
        vec3 V = normalize(cameraPosition - vWorldPos);
        float fres = pow(1.0 - max(dot(normalize(vWorldNormal), V), 0.0), 2.5);
        gl_FragColor.a = mix(0.82, 1.0, clamp(fres * 1.6, 0.0, 1.0));
        `
      )
  }
  material.customProgramCacheKey = () => 'water-fresnel-v1'
  return { material, uniforms }
}
