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
export function buildTreeInstances(treeEntities, THREE) {
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
