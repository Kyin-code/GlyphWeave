/**
 * Generate a grand demo map: Grand Realm of Aethra
 *
 * Layer 1 — Terrain   (grass, water, lava, floor base)
 * Layer 2 — Structures (walls, buildings, roads, bridges)
 * Layer 3 — Details   (trees, furniture, decorations)
 *
 * Usage: node scripts/generate-grand-map.mjs
 * Output: grand-realm-of-aethra.gemap
 */

import { writeFileSync } from 'fs'

const W = 120
const H = 80

// Helper: 2D grid type
const layer1 = Array.from({ length: H }, () => Array(W).fill(null)) // Terrain
const layer2 = Array.from({ length: H }, () => Array(W).fill(null)) // Structures
const layer3 = Array.from({ length: H }, () => Array(W).fill(null)) // Details

function set(layer, x, y, id) {
  if (x >= 0 && x < W && y >= 0 && y < H) layer[y][x] = id
}

function fillRect(layer, x, y, w, h, id) {
  for (let dy = 0; dy < h; dy++)
    for (let dx = 0; dx < w; dx++)
      set(layer, x + dx, y + dy, id)
}

function hLine(layer, x, y, len, id) {
  for (let dx = 0; dx < len; dx++) set(layer, x + dx, y, id)
}

function vLine(layer, x, y, len, id) {
  for (let dy = 0; dy < len; dy++) set(layer, x, y + dy, id)
}

function rect(layer, x, y, w, h, id) {
  hLine(layer, x, y, w, id)
  hLine(layer, x, y + h - 1, w, id)
  vLine(layer, x, y, h, id)
  vLine(layer, x + w - 1, y, h, id)
}

function fillRectRng(layer, x, y, w, h, id, chance) {
  for (let dy = 0; dy < h; dy++)
    for (let dx = 0; dx < w; dx++)
      if (Math.random() < chance) set(layer, x + dx, y + dy, id)
}

function room(layer2, layer3, x, y, w, h, floorType = 'floor') {
  fillRect(layer1, x + 1, y + 1, w - 2, h - 2, floorType)
  rect(layer2, x, y, w, h, 'wall')
}

function circle(layer, cx, cy, r, id) {
  for (let dy = -r; dy <= r; dy++) {
    for (let dx = -r; dx <= r; dx++) {
      if (dx * dx + dy * dy <= r * r) set(layer, cx + dx, cy + dy, id)
    }
  }
}

function circleRng(layer, cx, cy, r, id, chance) {
  for (let dy = -r; dy <= r; dy++) {
    for (let dx = -r; dx <= r; dx++) {
      if (dx * dx + dy * dy <= r * r && Math.random() < chance)
        set(layer, cx + dx, cy + dy, id)
    }
  }
}

function roadH(layer, x, y, len) {
  for (let dx = 0; dx < len; dx++) {
    set(layer, x + dx, y, 'floorAlt')
    set(layer, x + dx, y - 1, 'floorAlt')
  }
}

function roadV(layer, x, y, len) {
  for (let dy = 0; dy < len; dy++) {
    set(layer, x, y + dy, 'floorAlt')
    set(layer, x - 1, y + dy, 'floorAlt')
  }
}

// ════════════════════════════════════════════════════════════
//  1. TERRAIN — Layer 1
// ════════════════════════════════════════════════════════════

// Default grass
fillRect(layer1, 0, 0, W, H, 'grass')

// ── Mountains (NW: 5..30, 3..25) ──
circleRng(layer1, 16, 12, 16, 'wall', 0.55)
fillRectRng(layer1, 4, 3, 28, 24, 'wall', 0.3)
// mountain peaks with stone/darker look
circleRng(layer1, 14, 10, 8, 'wall', 0.7)
circleRng(layer1, 20, 16, 6, 'wall', 0.65)
// some high grass around mountains
fillRectRng(layer1, 2, 2, 34, 28, 'grass', 0.3)

// ── Great Lake (15..50, 20..55) with varying depth ──
circle(layer1, 35, 38, 18, 'deepWater')
circle(layer1, 30, 42, 14, 'deepWater')
circle(layer1, 42, 35, 12, 'deepWater')
// shallow edges
circleRng(layer1, 35, 38, 22, 'water', 0.5)
circleRng(layer1, 30, 42, 18, 'water', 0.5)
// shoreline smoothing
for (let y = 18; y < 60; y++) {
  for (let x = 12; x < 60; x++) {
    if (layer1[y][x] === 'water' || layer1[y][x] === 'deepWater') {
      // check neighbors — if surrounded by grass on 3+ sides, keep shallow; otherwise deep
      let grassNeighbors = 0
      for (const [dx, dy] of [[0,-1],[0,1],[-1,0],[1,0],[-1,-1],[1,1],[-1,1],[1,-1]]) {
        const nx = x + dx, ny = y + dy
        if (nx >= 0 && nx < W && ny >= 0 && ny < H && layer1[ny][nx] === 'grass') grassNeighbors++
      }
      if (grassNeighbors >= 3 && layer1[y][x] !== 'deepWater') {
        layer1[y][x] = 'water'
      }
    }
  }
}

// ── River from lake flowing SE to ocean (60,50) ──
for (let i = 0; i < 40; i++) {
  const rx = 50 + i
  const ry = 38 + Math.floor(i * 0.6) + Math.floor(Math.sin(i * 0.3) * 2)
  set(layer1, rx, ry, 'water')
  set(layer1, rx, ry + 1, 'water')
  set(layer1, rx - 1, ry, 'water')
  set(layer1, rx, ry + 2, 'water')
  // some deep spots
  if (i % 5 === 0) set(layer1, rx, ry, 'deepWater')
}

// ── Lava Fissure (NE: 90..115, 2..25) ──
for (let x = 85; x < 115; x++) {
  const baseY = 8 + Math.floor(Math.sin(x * 0.15) * 5) + Math.floor(Math.random() * 2)
  const width = 2 + Math.floor(Math.random() * 3)
  for (let w = 0; w < width; w++) {
    set(layer1, x, baseY + w, 'lava')
  }
}
// lava pool
circleRng(layer1, 100, 16, 8, 'lava', 0.7)
// cracked earth around lava
circleRng(layer1, 100, 16, 12, 'grass', 0.4)
fillRectRng(layer1, 85, 2, 30, 28, 'wall', 0.15)  // rocky terrain

// ── Volcano near lava ──
circleRng(layer1, 105, 6, 7, 'wall', 0.6)
set(layer1, 105, 6, 'lava')
set(layer1, 104, 5, 'lava')
set(layer1, 106, 5, 'lava')
set(layer1, 105, 4, 'lava')

// ── Forest (SW: 2..25, 50..75) ──
fillRectRng(layer1, 2, 50, 24, 26, 'grass', 0.7) // dense grass
for (let y = 50; y < 76; y++) {
  for (let x = 2; x < 26; x++) {
    if (Math.random() < 0.4) set(layer3, x, y, 'tree')
    else if (Math.random() < 0.15) set(layer3, x, y, 'grass')
  }
}

// ════════════════════════════════════════════════════════════
//  2. STRUCTURES — Layer 2
// ════════════════════════════════════════════════════════════

// ── Roads ──
// Main road: Village → Crossroads → City
roadH(layer2, 40, 62, 30)   // village to east
roadV(layer2, 70, 50, 16)   // north-south connector
roadH(layer2, 72, 38, 20)   // park road
roadV(layer2, 82, 30, 25)   // city roads

// ── VILLAGE (around 32..48, 58..67) ──
// Village center
room(layer2, layer3, 32, 60, 6, 6, 'floorAlt')
set(layer2, 35, 63, 'fountain')
// Houses
room(layer2, layer3, 28, 55, 5, 4, 'floor')
set(layer2, 30, 56, 'table')
room(layer2, layer3, 38, 55, 5, 4, 'floor')
set(layer2, 40, 56, 'shop')
room(layer2, layer3, 28, 65, 5, 4, 'floor')
room(layer2, layer3, 38, 65, 5, 4, 'floor')
set(layer2, 40, 66, 'table')
// Well
set(layer1, 33, 62, 'floor')
set(layer2, 33, 62, 'fountain')
// Village paths
roadH(layer2, 30, 59, 12)
roadH(layer2, 30, 64, 12)

// ── CITY (around 75..100, 30..48) ──
// Outer wall
rect(layer2, 74, 29, 28, 22, 'wall')
// Gates
set(layer2, 74, 40, 'door')   // west gate
set(layer2, 101, 40, 'door')  // east gate
set(layer2, 88, 29, 'door')   // north gate
set(layer2, 88, 50, 'door')   // south gate
// Inner floors
fillRect(layer1, 76, 31, 24, 18, 'floorAlt')
// Keep/Castle in center
room(layer2, layer3, 82, 34, 12, 10, 'floor')
// Castle interior
set(layer2, 84, 36, 'stairsDown')
set(layer2, 90, 36, 'stairsUp')
set(layer2, 88, 38, 'throne')
set(layer2, 86, 40, 'treasure')
set(layer2, 90, 40, 'treasure')
// Barracks
room(layer2, layer3, 77, 32, 4, 5, 'floor')
set(layer2, 78, 33, 'cage')
// Temple
room(layer2, layer3, 94, 32, 5, 5, 'floor')
set(layer2, 96, 34, 'altar')
// Market stalls
fillRect(layer1, 77, 42, 22, 6, 'floor')
for (let mx = 78; mx < 98; mx += 4) {
  for (let my = 43; my < 47; my += 2) {
    if (Math.random() < 0.6) set(layer2, mx, my, 'table')
  }
}
// Shops
set(layer2, 78, 44, 'shop')
set(layer2, 88, 44, 'shop')
set(layer2, 96, 45, 'shop')
// Houses along the walls
for (let i = 0; i < 4; i++) {
  room(layer2, layer3, 76 + i * 6, 31, 4, 3, 'floor')
  room(layer2, layer3, 76 + i * 6, 46, 4, 3, 'floor')
}

// ── PARK / GARDEN (60..73, 30..42) ──
fillRect(layer1, 60, 30, 14, 13, 'grass')
rect(layer2, 60, 30, 14, 13, 'wall')
set(layer2, 60, 36, 'door')
// Garden inside
for (let gx = 62; gx < 72; gx += 2) {
  for (let gy = 32; gy < 41; gy += 2) {
    set(layer3, gx, gy, 'fountain')
  }
}
// Trees in park
for (let i = 0; i < 10; i++) {
  const tx = 61 + Math.floor(Math.random() * 12)
  const ty = 31 + Math.floor(Math.random() * 11)
  if (!layer3[ty][tx] && layer1[ty][tx] === 'grass') set(layer3, tx, ty, 'tree')
}
// Path through garden
for (let px = 61; px < 73; px += 2) {
  set(layer2, px, 36, 'floorAlt')
}

// ── DUNGEON (SE: 95..114, 55..74) ──
// Outer wall
rect(layer2, 94, 55, 22, 20, 'wall')
// Entrance
set(layer2, 94, 65, 'door')
fillRect(layer1, 96, 57, 18, 16, 'floor')
// Inner rooms
room(layer2, layer3, 100, 57, 7, 7, 'floor')
set(layer2, 103, 60, 'stairsDown')
set(layer2, 104, 57, 'treasure')
set(layer2, 101, 57, 'trap')
room(layer2, layer3, 96, 65, 5, 5, 'floor')
set(layer2, 98, 67, 'altar')
set(layer2, 97, 66, 'blood')
room(layer2, layer3, 106, 65, 6, 6, 'floor')
set(layer2, 108, 67, 'throne')
set(layer2, 106, 68, 'cage')
set(layer2, 110, 66, 'treasure')
// Torture chamber
room(layer2, layer3, 100, 73, 7, 4, 'floor')
set(layer2, 102, 74, 'cage')
set(layer2, 104, 74, 'blood')
set(layer2, 106, 74, 'trap')
// Graveyard
for (let gx = 100; gx < 110; gx += 2) {
  for (let gy = 57; gy < 64; gy += 2) {
    if (Math.random() < 0.5) set(layer3, gx + Math.floor(Math.random() * 2), gy, 'grave')
  }
}
// Outer decorations
for (let i = 0; i < 6; i++) {
  const dx = 95 + Math.floor(Math.random() * 20)
  const dy = 56 + Math.floor(Math.random() * 18)
  if (!layer2[dy]?.[dx] && !layer3[dy]?.[dx] && layer1[dy]?.[dx] === 'grass') {
    set(layer3, dx, dy, 'grave')
  }
}

// ── Bridges ──
// Bridge over river near village
for (let bx = 53; bx < 57; bx++) {
  set(layer2, bx, 44, 'bridge')
  set(layer2, bx, 45, 'bridge')
}
// Bridge over river near city
for (let bx = 63; bx < 67; bx++) {
  set(layer2, bx, 52, 'bridge')
  set(layer2, bx, 53, 'bridge')
}

// ════════════════════════════════════════════════════════════
//  3. DETAILS — Layer 3  (trees, grass tufts, decorations)
// ════════════════════════════════════════════════════════════

// Scattered trees on open plains
for (let i = 0; i < 60; i++) {
  const tx = 30 + Math.floor(Math.random() * 60)
  const ty = 10 + Math.floor(Math.random() * 60)
  if (!layer3[ty]?.[tx] && layer1[ty]?.[tx] === 'grass' && !layer2[ty]?.[tx]) {
    set(layer3, tx, ty, 'tree')
  }
}

// Grass tufts
for (let i = 0; i < 120; i++) {
  const gx = Math.floor(Math.random() * W)
  const gy = Math.floor(Math.random() * H)
  if (!layer3[gy]?.[gx] && layer1[gy]?.[gx] === 'grass' && !layer2[gy]?.[gx]) {
    set(layer3, gx, gy, 'grass')
  }
}

// Scattered graves in wilderness
for (let i = 0; i < 15; i++) {
  const gx = 10 + Math.floor(Math.random() * 100)
  const gy = 5 + Math.floor(Math.random() * 70)
  if (!layer3[gy]?.[gx] && !layer2[gy]?.[gx]) {
    set(layer3, gx, gy, 'grave')
  }
}

// ════════════════════════════════════════════════════════════
//  BUILD OUTPUT
// ════════════════════════════════════════════════════════════

function layerToRecord(layer) {
  const record = {}
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      const val = layer[y][x]
      if (val) record[`${x},${y}`] = val
    }
  }
  return record
}

const mapData = {
  tiles: {}, // flat backward-compat (flattened from all layers)
  layerTiles: {
    'layer-1': layerToRecord(layer1),
    'layer-2': layerToRecord(layer2),
    'layer-3': layerToRecord(layer3),
  },
  layers: [
    { id: 'layer-1', name: 'Terrain', visible: true, locked: false },
    { id: 'layer-2', name: 'Structures', visible: true, locked: false },
    { id: 'layer-3', name: 'Details', visible: true, locked: false },
  ],
  worldName: 'Grand Realm of Aethra',
  tileSize: 24,
  themeId: 'ansi-16',
  version: 2,
}

// flattened
const allTiles = {}
for (const layerId of ['layer-1', 'layer-2', 'layer-3']) {
  Object.assign(allTiles, mapData.layerTiles[layerId])
}
mapData.tiles = allTiles

// Stats
const totalTiles = Object.keys(allTiles).length
const layerCounts = Object.fromEntries(
  Object.entries(mapData.layerTiles).map(([id, t]) => [id, Object.keys(t).length])
)

const outputPath = new URL('../grand-realm-of-aethra.gemap', import.meta.url)
writeFileSync(outputPath, JSON.stringify(mapData, null, 2), 'utf-8')

console.log('[OK] Generated: grand-realm-of-aethra.gemap')
console.log(`   Map size: ${W}×${H} (${W * H} cells)`)
console.log(`   Total tiles: ${totalTiles}`)
console.log(`   Layers:`)
for (const [id, count] of Object.entries(layerCounts)) {
  const name = mapData.layers.find(l => l.id === id)?.name || id
  console.log(`     ${name} (${id}): ${count} tiles`)
}
