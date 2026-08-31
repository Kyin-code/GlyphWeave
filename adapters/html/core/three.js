// Central lazy loader for Three.js and helpers. drawNear() dynamically
// imports these at runtime; modules that build meshes receive THREE as an
// argument, so this is the single place that resolves the CDN modules.
let cached = null
export async function ensureThree() {
  if (cached) return cached
  const THREE = await import('https://cdn.jsdelivr.net/npm/three@0.178.0/build/three.module.js')
  const [GLTFLoader, OrbitControls, OBJLoader] = await Promise.all([
    import('https://cdn.jsdelivr.net/npm/three@0.178.0/examples/jsm/loaders/GLTFLoader.js'),
    import('https://cdn.jsdelivr.net/npm/three@0.178.0/examples/jsm/controls/OrbitControls.js'),
    import('https://cdn.jsdelivr.net/npm/three@0.178.0/examples/jsm/loaders/OBJLoader.js'),
  ])
  cached = { THREE, GLTFLoader: GLTFLoader.GLTFLoader, OrbitControls: OrbitControls.OrbitControls, OBJLoader: OBJLoader.OBJLoader }
  return cached
}
