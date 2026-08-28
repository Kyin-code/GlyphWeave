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

canvas.addEventListener('click', (event) => {
  if (!scene || mode !== 'strategic') return
  const rect = canvas.getBoundingClientRect()
  const scale = Math.min(canvas.clientWidth / scene.widthM, canvas.clientHeight / scene.depthM)
  const offsetX = (canvas.clientWidth - scene.widthM * scale) / 2
  const offsetZ = (canvas.clientHeight - scene.depthM * scale) / 2
  const worldX = Math.floor((event.clientX - rect.left - offsetX) / scale) + scene.originX
  const worldZ = Math.floor((event.clientY - rect.top - offsetZ) / scale) + scene.originZ
  info.textContent = `${scene.sceneId}  X=${worldX} Z=${worldZ} Y=heightfield`
})

function resize() {
  canvas.width = canvas.clientWidth * devicePixelRatio
  canvas.height = canvas.clientHeight * devicePixelRatio
  context.setTransform(devicePixelRatio, 0, 0, devicePixelRatio, 0, 0)
  if (scene) draw()
}

function color(surface) { return ['#526a4f', '#6f8050', '#69737a', '#456b73'][surface] ?? '#526a4f' }

async function loadScene() {
  scene = await fetch(`../${world.scenes[sceneSelect.selectedIndex]}`).then(response => response.json())
  draw()
}

function drawStrategic() {
  const scale = Math.min(canvas.clientWidth / scene.widthM, canvas.clientHeight / scene.depthM)
  const ox = (canvas.clientWidth - scene.widthM * scale) / 2
  const oz = (canvas.clientHeight - scene.depthM * scale) / 2
  context.fillStyle = '#0c100e'; context.fillRect(0, 0, canvas.clientWidth, canvas.clientHeight)
  for (const chunk of scene.chunks) {
    fetch(`../scenes/${scene.sceneId}/${chunk.lod2File}`).then(response => response.arrayBuffer()).then(bytes => {
      const data = new Uint8Array(bytes)
      const cols = Math.ceil(chunk.validWidthM / 64)
      const rows = Math.ceil(chunk.validDepthM / 64)
      for (let z = 0; z < rows; z++) for (let x = 0; x < cols; x++) {
        const i = (z * cols + x) * 3
        const height = new DataView(bytes).getInt16(i, true)
        context.fillStyle = color(data[i + 2])
        context.fillRect(ox + (chunk.worldX - scene.originX + x * 64) * scale, oz + (chunk.worldZ - scene.originZ + z * 64) * scale, 64 * scale + 1, 64 * scale + 1)
        if (height > 600) { context.fillStyle = 'rgba(220,230,225,.2)'; context.fillRect(ox + (chunk.worldX - scene.originX + x * 64) * scale, oz + (chunk.worldZ - scene.originZ + z * 64) * scale, 64 * scale, 64 * scale) }
      }
      for (const landmark of scene.landmarks) {
        context.fillStyle = '#d5a85e'; context.fillRect(ox + (landmark.worldX - scene.originX) * scale - 3, oz + (landmark.worldZ - scene.originZ) * scale - 3, 6, 6)
      }
    })
  }
}

async function drawNear() {
  const THREE = await import('https://cdn.jsdelivr.net/npm/three@0.178.0/build/three.module.js')
  const viewport = document.querySelector('#viewport')
  const viewportWidth = viewport.clientWidth
  const viewportHeight = viewport.clientHeight
  canvas.style.display = 'none'
  webglCanvas ??= document.createElement('canvas')
  webglCanvas.style.width = '100%'
  webglCanvas.style.height = '100%'
  webglCanvas.style.display = 'block'
  document.querySelector('#viewport').append(webglCanvas)
  if (nearRenderer) { nearRenderer.setAnimationLoop(null); nearRenderer.dispose(); nearRenderer = null }
  nearRenderer = new THREE.WebGLRenderer({ canvas: webglCanvas, antialias: true })
  nearRenderer.setSize(viewportWidth, viewportHeight, false)
  nearRenderer.setPixelRatio(Math.min(devicePixelRatio, 2))
  nearRenderer.setClearColor('#0c100e')
  nearCamera = new THREE.PerspectiveCamera(48, viewportWidth / viewportHeight, 1, 10000)
  nearCamera.position.set(scene.widthM * .45, Math.max(scene.widthM, scene.depthM) * .7, scene.depthM * .7)
  nearCamera.lookAt(scene.widthM / 2, 0, scene.depthM / 2)
  nearScene = new THREE.Scene()
  nearScene.add(new THREE.HemisphereLight(0xdce8df, 0x202820, 2))
  const sun = new THREE.DirectionalLight(0xffe6b0, 2); sun.position.set(-400, 900, 300); nearScene.add(sun)
  const group = new THREE.Group(); nearScene.add(group)
  for (const chunk of scene.chunks) {
    const response = await fetch(`../scenes/${scene.sceneId}/${chunk.heightFile}`)
    const bytes = await response.arrayBuffer(); const width = chunk.validWidthM; const depth = chunk.validDepthM
    const geometry = new THREE.PlaneGeometry(width, depth, Math.min(width / 8, 64), Math.min(depth / 8, 64)); geometry.rotateX(-Math.PI / 2)
    const positions = geometry.attributes.position
    for (let i = 0; i < positions.count; i++) { const x = Math.min(width - 1, Math.max(0, Math.round(positions.getX(i) + width / 2))); const z = Math.min(depth - 1, Math.max(0, Math.round(positions.getZ(i) + depth / 2))); positions.setY(i, new DataView(bytes).getInt16((z * width + x) * 2, true) / 4) }
    geometry.computeVertexNormals(); const mesh = new THREE.Mesh(geometry, new THREE.MeshStandardMaterial({ color: '#667d58', roughness: 1 })); mesh.position.set(chunk.worldX + width / 2, 0, chunk.worldZ + depth / 2); group.add(mesh)
  }
  for (const landmark of scene.landmarks) {
    const type = landmark.type
    const geometry = type === 'island_hill' ? new THREE.SphereGeometry(.5, 24, 12)
      : type === 'ridge' ? new THREE.ConeGeometry(.5, 1, 8)
      : type === 'pavilion' ? new THREE.CylinderGeometry(.5, .5, 1, 8)
      : new THREE.BoxGeometry(1, 1, 1)
    const material = new THREE.MeshStandardMaterial({
      color: type === 'lake' ? '#326b78' : type === 'causeway' ? '#b38b5a' : type === 'pavilion' ? '#c84f3f' : '#6f8557',
      roughness: type === 'lake' ? .25 : .9,
      metalness: type === 'lake' ? .1 : 0,
      transparent: type === 'lake',
      opacity: type === 'lake' ? .82 : 1,
    })
    const marker = new THREE.Mesh(geometry, material)
    if (type === 'island_hill') marker.scale.set(landmark.widthM, landmark.heightM, landmark.depthM)
    else if (type === 'ridge') marker.scale.set(landmark.widthM, landmark.heightM, landmark.depthM)
    else marker.scale.set(landmark.widthM, landmark.heightM, landmark.depthM)
    marker.position.set(landmark.worldX, landmark.worldY + landmark.heightM / 2, landmark.worldZ)
    group.add(marker)
  }
  for (const entity of scene.entities ?? []) {
    const geometry = entity.kind === 'tree' ? new THREE.ConeGeometry(7, 28, 7)
      : entity.kind === 'rock' ? new THREE.DodecahedronGeometry(8, 0)
      : entity.kind === 'building' ? new THREE.BoxGeometry(18, 24, 18)
      : null
    if (!geometry) continue
    const material = new THREE.MeshStandardMaterial({ color: entity.kind === 'tree' ? '#47704a' : entity.kind === 'rock' ? '#76716a' : '#a35d43', roughness: .95 })
    const prop = new THREE.Mesh(geometry, material)
    prop.position.set(entity.worldX, entity.worldY + (entity.kind === 'tree' ? 14 : 10), entity.worldZ)
    prop.scale.setScalar(entity.scale)
    group.add(prop)
  }
  nearRenderer.render(nearScene, nearCamera)
  nearRenderer.setAnimationLoop(() => nearRenderer.render(nearScene, nearCamera))
}

function draw() { info.textContent = `${scene.sceneId}  ${scene.widthM}m × ${scene.depthM}m  ${scene.chunkCountX}×${scene.chunkCountZ} chunks`; if (mode === 'strategic') drawStrategic(); else drawNear() }
document.querySelector('#strategic').onclick = () => { mode = 'strategic'; if (webglCanvas) webglCanvas.style.display = 'none'; canvas.style.display = 'block'; draw() }
document.querySelector('#close').onclick = () => { mode = 'near'; draw() }
sceneSelect.onchange = loadScene
window.onresize = resize
fetch('../world.json').then(response => response.json()).then(async data => { world = data; title.textContent = data.name; for (const path of data.scenes) { const option = document.createElement('option'); option.textContent = path.split('/')[1]; sceneSelect.append(option) } await loadScene(); resize() })
