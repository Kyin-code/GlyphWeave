import { state } from './state.js'

// Fetch a scene JSON and cache it into state.
export async function loadSceneData(sceneId) {
  const response = await fetch(`../scenes/${sceneId}/scene.json`)
  state.scene = await response.json()
  return state.scene
}

// Build a heightAt query function over active chunks. Reads chunk height
// files lazily and caches them.
export async function createHeightField(activeChunks, sceneCacheKey) {
  const heightFields = []
  for (const chunk of activeChunks) {
    const cacheKey = `${sceneCacheKey}/${chunk.chunkX}/${chunk.chunkZ}`
    let cached = state.chunkDataCache.get(cacheKey)
    if (!cached) {
      const [heightResponse] = await Promise.all([
        fetch(`../scenes/${state.scene.sceneId}/${chunk.heightFile}`),
      ])
      const bytes = await heightResponse.arrayBuffer()
      cached = { heightView: new DataView(bytes) }
      state.chunkDataCache.set(cacheKey, cached)
    }
    heightFields.push({ chunk, heightView: cached.heightView, width: chunk.validWidthM, depth: chunk.validDepthM })
  }
  const heightFieldMap = new Map()
  state.heightAt = (worldX, worldZ) => {
    const key = `${Math.floor(worldX / 512)},${Math.floor(worldZ / 512)}`
    let field = heightFieldMap.get(key)
    if (!field) {
      field = heightFields.find(item => worldX >= item.chunk.worldX && worldX < item.chunk.worldX + item.width && worldZ >= item.chunk.worldZ && worldZ < item.chunk.worldZ + item.depth)
      heightFieldMap.set(key, field ?? null)
    }
    if (!field) return 0
    const x = Math.max(0, Math.min(field.width - 1, Math.floor(worldX - field.chunk.worldX)))
    const z = Math.max(0, Math.min(field.depth - 1, Math.floor(worldZ - field.chunk.worldZ)))
    return field.heightView.getInt16((z * field.width + x) * 2, true) / 4
  }
  return state.heightAt
}
