// Shared mutable state across modules. Kept in one place so render/camera/
// world modules don't pass dozens of parameters around.
export const state = {
  world: null,          // world.json
  scene: null,          // current scene JSON
  mode: 'strategic',    // 'strategic' | 'near'
  webglCanvas: null,
  nearRenderer: null,
  nearScene: null,
  nearCamera: null,
  nearControls: null,
  nearPreset: 'harbour',
  nearGroupRoot: null,
  exploreKeys: new Set(),
  exploreInputCleanup: null,
  chunkDataCache: new Map(),      // sceneId/chunkX/chunkZ -> {heightView, surface}
  entityMeshCache: new Map(),
  entityGroupCache: new Map(),
  feedback: null,       // window.glyphweaveFeedback object
  heightAt: null,       // height field query fn (set by world module)
  buildEntityCached: null, // set by world/engine module
  animatedActors: [],
  waterTextures: [],
  edgePointer: null,
  lastRenderMs: 0,
}
