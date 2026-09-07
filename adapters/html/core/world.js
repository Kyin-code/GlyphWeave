import { decodeHeightPayload } from './height.js'
import { state } from './state.js'

// Fetch a scene JSON and cache it into state.
export async function loadSceneData(sceneId) {
  const response = await fetch('../scenes/' + sceneId + '/scene.json')
  const loaded = await response.json()
  loaded.landUseLedger = await loadLandUseLedger(loaded)
  state.scene = loaded
  return loaded
}

// Read-only bake diagnostics; release acceptance is recomputed by the CLI.
// Older worlds can omit this additive report without breaking rendering.
export async function loadLandUseLedger(sceneData) {
  if (!sceneData.developmentPlan) return null
  try {
    const response = await fetch('../scenes/' + sceneData.sceneId + '/land-use-ledger.json')
    if (!response.ok) return null
    const ledger = await response.json()
    const valid = ledger.version === 1 && ledger.accounting === 'disjoint-purpose-cover-v1'
      && ledger.sceneId === sceneData.sceneId && ledger.sceneAreaM2 === sceneData.widthM * sceneData.depthM
      && ['urban', 'rural', 'nature', 'unassigned'].every(kind => {
        const row = ledger.byPurpose?.[kind]
        return row && ['targetRatio', 'plannedM2', 'realizedM2', 'unresolvedM2'].every(key => Number.isFinite(row[key]) && row[key] >= 0)
      })
    return valid ? ledger : null
  } catch { return null }
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
      cached = { heightView: decodeHeightPayload(bytes,chunk.validWidthM,chunk.validDepthM) }
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
    return field.heightView.readMetres(z * field.width + x)
  }
  return state.heightAt
}
