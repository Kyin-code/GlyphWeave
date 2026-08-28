const canvas = document.querySelector('#map')
const context = canvas.getContext('2d')
const title = document.querySelector('#title')
const info = document.querySelector('#info')
const sceneSelect = document.querySelector('#scene')
let world
let scene
let mode = 'strategic'
let webglCanvas

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
  canvas.style.display = 'none'
  webglCanvas ??= document.createElement('canvas')
  webglCanvas.style.width = '100%'
  webglCanvas.style.height = '100%'
  document.querySelector('#viewport').append(webglCanvas)
  const renderer = new THREE.WebGLRenderer({ canvas: webglCanvas, antialias: true })
  renderer.setSize(canvas.clientWidth, canvas.clientHeight, false)
  renderer.setClearColor('#0c100e')
  const camera = new THREE.PerspectiveCamera(48, canvas.clientWidth / canvas.clientHeight, 1, 10000)
  camera.position.set(scene.widthM * .45, Math.max(scene.widthM, scene.depthM) * .7, scene.depthM * .7)
  camera.lookAt(scene.widthM / 2, 0, scene.depthM / 2)
  const light = new THREE.HemisphereLight(0xdce8df, 0x202820, 2); renderer.scene = new THREE.Scene(); renderer.scene.add(light)
  const group = new THREE.Group(); renderer.scene.add(group)
  for (const chunk of scene.chunks.slice(0, 9)) {
    const response = await fetch(`../scenes/${scene.sceneId}/${chunk.heightFile}`)
    const bytes = await response.arrayBuffer(); const width = chunk.validWidthM; const depth = chunk.validDepthM
    const geometry = new THREE.PlaneGeometry(width, depth, Math.min(width / 8, 64), Math.min(depth / 8, 64)); geometry.rotateX(-Math.PI / 2)
    const positions = geometry.attributes.position
    for (let i = 0; i < positions.count; i++) { const x = Math.min(width - 1, Math.max(0, Math.round(positions.getX(i) + width / 2))); const z = Math.min(depth - 1, Math.max(0, Math.round(positions.getZ(i) + depth / 2))); positions.setY(i, new DataView(bytes).getInt16((z * width + x) * 2, true) / 4) }
    geometry.computeVertexNormals(); const mesh = new THREE.Mesh(geometry, new THREE.MeshStandardMaterial({ color: '#667d58', roughness: 1 })); mesh.position.set(chunk.worldX + width / 2, 0, chunk.worldZ + depth / 2); group.add(mesh)
  }
  renderer.setAnimationLoop(() => renderer.render(renderer.scene, camera))
}

function draw() { info.textContent = `${scene.sceneId}  ${scene.widthM}m × ${scene.depthM}m  ${scene.chunkCountX}×${scene.chunkCountZ} chunks`; if (mode === 'strategic') drawStrategic(); else drawNear() }
document.querySelector('#strategic').onclick = () => { mode = 'strategic'; if (webglCanvas) webglCanvas.style.display = 'none'; canvas.style.display = 'block'; draw() }
document.querySelector('#close').onclick = () => { mode = 'near'; draw() }
sceneSelect.onchange = loadScene
window.onresize = resize
fetch('../world.json').then(response => response.json()).then(async data => { world = data; title.textContent = data.name; for (const path of data.scenes) { const option = document.createElement('option'); option.textContent = path.split('/')[1]; sceneSelect.append(option) } await loadScene(); resize() })
