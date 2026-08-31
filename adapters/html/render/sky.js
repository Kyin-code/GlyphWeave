export function makeSkyTexture(THREE) {
  const canvas = document.createElement('canvas')
  canvas.width = 4
  canvas.height = 256
  const context = canvas.getContext('2d')
  const gradient = context.createLinearGradient(0, 0, 0, canvas.height)
  gradient.addColorStop(0, '#0e1c2a')
  gradient.addColorStop(.35, '#5b7d8f')
  gradient.addColorStop(.62, '#d8a85c')
  gradient.addColorStop(.85, '#f2c98a')
  gradient.addColorStop(1, '#e8b06a')
  context.fillStyle = gradient
  context.fillRect(0, 0, canvas.width, canvas.height)
  const texture = new THREE.CanvasTexture(canvas)
  texture.colorSpace = THREE.SRGBColorSpace
  return texture
}
