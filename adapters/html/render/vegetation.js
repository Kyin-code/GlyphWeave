export function makeVoxelTree(entity, THREE) {
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
// Canopy shaders get a Journey-style wind sway (circular-arc bend + travelling
// gust + tip flutter) so the forest ripples in the wind; trunks stay rigid.
export function buildTreeInstances(treeEntities, THREE) {
  const group = new THREE.Group()
  if (!treeEntities.length) return group
  const count = treeEntities.length
  const trunkGeo = new THREE.CylinderGeometry(.5, .72, 2, 7)
  const foliageGeoLow = new THREE.DodecahedronGeometry(1, 1)
  const foliageGeoHigh = new THREE.DodecahedronGeometry(1, 0)
  const trunkMat = new THREE.MeshStandardMaterial({ color: '#5d4330', roughness: .97 })
  const foliageMat = new THREE.MeshStandardMaterial({ color: '#3d6b44', roughness: .94 })
  // Shared wind uniforms for the canopy sway.
  const wind = {
    uTime: { value: 0 },
    uWindStrength: { value: .5 },
    uWindSpeed: { value: 1.6 },
    uWindScale: { value: .22 },
    uGust: { value: .6 },
  }
  const windVertexChunk = /* glsl */ `
    uniform float uTime;
    uniform float uWindStrength;
    uniform float uWindSpeed;
    uniform float uWindScale;
    uniform float uGust;
    varying vec3 vWorldPos;
  `
  const windVertexBody = /* glsl */ `
    // World position of this instance's origin (for the travelling gust).
    vec4 wp = instanceMatrix * vec4(0.0, 0.0, 0.0, 1.0);
    vec3 base = wp.xyz;
    // Normalized height along the canopy (0 base .. 1 tip), used to weight the
    // bend so the crown arcs over while the trunk stays stiff.
    float t = clamp(position.y, 0.0, 1.0);
    // Travelling gust front sweeping downwind (world XZ sampled at base).
    float along = base.x * 0.7071 + base.z * 0.7071;
    float gustPhase = along * uWindScale - uTime * uWindSpeed * 0.6;
    float gust = pow(sin(gustPhase) * 0.5 + 0.5, 1.6);
    float chop = sin(along * uWindScale * 2.7 - uTime * uWindSpeed * 1.3) * 0.5 + 0.5;
    float intensity = 0.25 + gust * 0.85 + chop * 0.18;
    // Per-instance phase desync so trees don't all sway in lockstep.
    float seed = fract(sin(base.x * 127.1 + base.z * 311.7) * 43758.5453);
    float bladePhase = seed * 6.28318;
    float ampVar = 0.65 + fract(sin((seed * 100.0) * 127.1 + 311.7) * 43758.5453) * 0.7;
    // Circular-arc bend angle (radians), weighted toward the tip.
    float shaped = pow(t, 1.5);
    float phi = clamp(uWindStrength * intensity * ampVar * 3.0, 0.0, 1.4);
    float a = phi * shaped;
    float safePhi = max(phi, 1e-3);
    float R = CANOPY_RADIUS / safePhi;
    float u = R * (1.0 - cos(a));
    float dv = R * sin(a) - position.y;
    // Tip flutter (perpendicular shimmer, upper canopy only).
    float flutterMask = smoothstep(0.5, 1.0, t);
    float flutterAmt = sin(uTime * 10.0 + bladePhase * 3.0 + along * 0.8) * uGust * 0.08 * flutterMask;
    vec2 windDir = vec2(0.7071, 0.7071);
    vec2 perpDir = vec2(-windDir.y, windDir.x);
    vec3 sway = vec3(windDir.x * u + perpDir.x * flutterAmt, dv, windDir.y * u + perpDir.y * flutterAmt);
    transformed += sway;
    vec4 wpos = instanceMatrix * vec4(transformed, 1.0);
    vWorldPos = wpos.xyz;
  `
  const applyWind = (mat, amount) => {
    const floatLit = Number.isInteger(amount) ? `${amount}.0` : String(amount)
    const body = windVertexBody.replace('CANOPY_RADIUS', floatLit)
    mat.onBeforeCompile = (shader) => {
      Object.assign(shader.uniforms, wind)
      shader.vertexShader = shader.vertexShader
        .replace('#include <common>', `#include <common>\n${windVertexChunk}`)
        .replace(
          '#include <begin_vertex>',
          `#include <begin_vertex>\n${body}`
        )
    }
    mat.customProgramCacheKey = () => `tree-wind-v${amount}`
  }
  applyWind(foliageMat, 7.0)
  const foliageMat2 = new THREE.MeshStandardMaterial({ color: '#4d7a45', roughness: .93 })
  applyWind(foliageMat2, 9.0)
  const trunks = new THREE.InstancedMesh(trunkGeo, trunkMat, count)
  const foliageLow = new THREE.InstancedMesh(foliageGeoLow, foliageMat, count)
  const foliageHigh = new THREE.InstancedMesh(foliageGeoHigh, foliageMat2, count)
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
  group.userData.wind = wind
  return group
}

export function makeMangrove(entity, THREE) {
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

export function makeForestPatch(entity, THREE, reserve = false) {
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

// Instanced low-poly rocks with per-vertex noise displacement — a handful of
// draw calls for a whole field of boulders. Each rock gets a random yaw, scale
// and tone, and the shared geometry is displaced by world-position noise so no
// two rocks read identical even when the base mesh is the same.
export function buildRockInstances(rockEntities, THREE) {
  const group = new THREE.Group()
  if (!rockEntities.length) return group
  const count = rockEntities.length
  const baseGeo = new THREE.IcosahedronGeometry(1, 1)
  const pos = baseGeo.attributes.position
  // Displace vertices by deterministic noise for an organic, uneven surface.
  const displaced = new Float32Array(pos.count * 3)
  for (let i = 0; i < pos.count; i++) {
    const x = pos.getX(i), y = pos.getY(i), z = pos.getZ(i)
    const n = 0.12 + Math.sin(x * 3.1 + y * 2.3 + z * 1.7) * .06 + Math.sin(x * 7.7 - z * 5.3) * .04
    displaced[i * 3] = x * (1 + n)
    displaced[i * 3 + 1] = y * (1 + n)
    displaced[i * 3 + 2] = z * (1 + n)
  }
  pos.array = displaced
  baseGeo.computeVertexNormals()
  const rockMat = new THREE.MeshStandardMaterial({ color: '#7a7468', roughness: .95, flatShading: true })
  const rocks = new THREE.InstancedMesh(baseGeo, rockMat, count)
  const m = new THREE.Matrix4()
  const q = new THREE.Quaternion()
  const s = new THREE.Vector3()
  const p = new THREE.Vector3()
  const color = new THREE.Color()
  const rand = (a, b) => { const v = Math.sin(a * 127.1 + b * 311.7) * 43758.5453; return v - Math.floor(v) }
  for (let i = 0; i < count; i++) {
    const e = rockEntities[i]
    const size = Math.max(e.widthM, e.depthM) * (1.5 + rand(e.worldX, e.worldZ) * 1.8)
    p.set(e.worldX, e.worldY + size * .45, e.worldZ)
    s.set(size, size * (.7 + rand(e.worldX * 3, e.worldZ * 5) * .6), size)
    q.setFromEuler(new THREE.Euler(rand(e.worldX * 7, e.worldZ * 11) * .35, rand(e.worldX * 13, e.worldZ * 17) * Math.PI * 2, rand(e.worldX * 19, e.worldZ * 23) * .35))
    m.compose(p, q, s)
    rocks.setMatrixAt(i, m)
    const shade = .55 + rand(e.worldX * 29, e.worldZ * 31) * .25
    color.setRGB(shade, shade * .97, shade * .92)
    rocks.setColorAt(i, color)
  }
  rocks.instanceMatrix.needsUpdate = true
  rocks.instanceColor.needsUpdate = true
  group.add(rocks)
  return group
}
