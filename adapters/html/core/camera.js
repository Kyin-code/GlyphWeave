import { state } from './state.js'

// Wrap OrbitControls change event to update coordinate info.
export function attachCoordinateInfo(controls, infoEl) {
  controls.addEventListener('change', () => {
    if (!state.scene || !state.nearCamera) return
    const p = state.nearCamera.position
    const target = state.nearControls ? state.nearControls.target : null
    const chunkX = Math.floor((target ? target.x : p.x) / 512)
    const chunkZ = Math.floor((target ? target.z : p.z) / 512)
    const label = document.getElementById('chunk-label')
    if (label) label.textContent = `chunk (${chunkX}, ${chunkZ}) world ${chunkX * 512},${chunkZ * 512}`
    if (state.feedback) state.feedback.cameraDistance = state.nearCamera.position.distanceTo(state.nearControls ? state.nearControls.target : new (window.THREE_VECTOR3 || Object)())
    const info = infoEl
    if (info) info.textContent = `${state.scene.sceneId}  X=${Math.round(p.x)} Z=${Math.round(p.z)}`
  })
}
