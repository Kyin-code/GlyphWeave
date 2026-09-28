import { PRESETS } from './presets'

type Grid = string[][]

function setCell(grid: Grid, x: number, y: number, id: string) {
  if (y >= 0 && y < grid.length && x >= 0 && x < grid[0].length) {
    grid[y][x] = id
  }
}

function fillRect(grid: Grid, x: number, y: number, w: number, h: number, id: string) {
  for (let dy = 0; dy < h; dy++)
    for (let dx = 0; dx < w; dx++)
      setCell(grid, x + dx, y + dy, id)
}

function placePreset(grid: Grid, presetId: string, ox: number, oy: number) {
  const preset = PRESETS.find(p => p.id === presetId)
  if (!preset) return
  for (let py = 0; py < preset.grid.length; py++) {
    for (let px = 0; px < preset.grid[py].length; px++) {
      const cell = preset.grid[py][px]
      if (cell !== 'void') {
        setCell(grid, ox + px, oy + py, cell)
      }
    }
  }
}

function carveHCorridor(grid: Grid, x: number, y: number, len: number) {
  for (let dx = 0; dx < len; dx++) {
    if (grid[y]?.[x + dx] === 'void') setCell(grid, x + dx, y, 'floor')
    if (grid[y - 1]?.[x + dx] === 'void') setCell(grid, x + dx, y - 1, 'wall')
    if (grid[y + 1]?.[x + dx] === 'void') setCell(grid, x + dx, y + 1, 'wall')
  }
}

function carveVCorridor(grid: Grid, x: number, y: number, len: number) {
  for (let dy = 0; dy < len; dy++) {
    if (grid[y + dy]?.[x] === 'void') setCell(grid, x, y + dy, 'floor')
    if (grid[y + dy]?.[x - 1] === 'void') setCell(grid, x - 1, y + dy, 'wall')
    if (grid[y + dy]?.[x + 1] === 'void') setCell(grid, x + 1, y + dy, 'wall')
  }
}

export function generateDemoMap(): Record<string, string | null> {
  const W = 80
  const H = 48
  const grid: Grid = Array.from({ length: H }, () => Array(W).fill('void'))

  // border wall
  for (let x = 0; x < W; x++) { setCell(grid, x, 0, 'wall'); setCell(grid, x, H - 1, 'wall') }
  for (let y = 0; y < H; y++) { setCell(grid, 0, y, 'wall'); setCell(grid, W - 1, y, 'wall') }

  // ── Entrance Hall ──
  placePreset(grid, 'entrance-hall', 3, 2)
  // carve door into entrance
  setCell(grid, 10, 0, 'wall')
  setCell(grid, 10, 1, 'wall')
  setCell(grid, 10, 2, 'door')

  // ── Upper corridor ──
  carveHCorridor(grid, 10, 5, 10)
  carveHCorridor(grid, 24, 5, 10)
  carveHCorridor(grid, 38, 5, 10)
  carveHCorridor(grid, 52, 5, 16)

  // ── Upper rooms ──
  placePreset(grid, 'pillared-hall', 26, 2)
  setCell(grid, 33, 9, 'door')
  placePreset(grid, 'fountain-hall', 40, 2)
  setCell(grid, 47, 9, 'door')

  // ── Left vertical corridor ──
  carveVCorridor(grid, 8, 9, 6)
  carveHCorridor(grid, 3, 15, 10)
  carveHCorridor(grid, 17, 15, 10)

  // ── Cross junction (center hub) ──
  placePreset(grid, 'cross-room', 20, 12)
  setCell(grid, 27, 12, 'door')
  setCell(grid, 27, 21, 'door')

  // ── Right rooms from upper ──
  carveVCorridor(grid, 52, 9, 8)

  // dungeon features on right
  placePreset(grid, 'graveyard', 54, 18)
  setCell(grid, 61, 25, 'door')

  placePreset(grid, 'prison', 64, 2)
  setCell(grid, 71, 9, 'door')

  // ── Central stairs room ──
  placePreset(grid, 'stairs-set', 13, 18)
  setCell(grid, 16, 18, 'door')

  // ── Lower left: lake, bridge, forest ──
  placePreset(grid, 'lake', 2, 24)
  setCell(grid, 7, 31, 'floor')
  setCell(grid, 7, 32, 'floor')
  carveHCorridor(grid, 2, 33, 16)

  placePreset(grid, 'bridge', 9, 28)
  placePreset(grid, 'forest', 14, 24)

  // ── Lower center: throne vault lava ──
  carveVCorridor(grid, 24, 22, 8)
  placePreset(grid, 'throne-room', 20, 30)
  setCell(grid, 28, 30, 'door')

  carveHCorridor(grid, 32, 34, 8)
  placePreset(grid, 'vault', 40, 32)
  setCell(grid, 45, 32, 'door')

  // ── Lava fissure (bottom) ──
  fillRect(grid, 2, 38, 10, 3, 'lava')
  for (let x = 0; x < 12; x++) {
    if (Math.random() > 0.3) setCell(grid, x, 37, 'wall')
    if (Math.random() > 0.3) setCell(grid, x, 41, 'wall')
  }
  // bridge over lava
  placePreset(grid, 'bridge', 5, 38)
  // blood near lava
  setCell(grid, 3, 36, 'blood')
  setCell(grid, 4, 36, 'blood')

  // ── Bottom corridor and rooms ──
  carveHCorridor(grid, 14, 38, 10)
  carveHCorridor(grid, 28, 38, 10)
  carveVCorridor(grid, 38, 36, 3)

  placePreset(grid, 'shop', 28, 40)
  setCell(grid, 35, 40, 'door')

  placePreset(grid, 'altar-room', 40, 38)
  setCell(grid, 45, 38, 'door')

  // ── Trap corridor ──
  carveVCorridor(grid, 55, 26, 14)
  placePreset(grid, 'trap-corridor', 52, 34)
  setCell(grid, 60, 34, 'door')

  // ── Lower right rooms ──
  placePreset(grid, 'kitchen', 58, 38)
  setCell(grid, 65, 38, 'door')

  // ── Extra decorations ──
  // random grass patches
  for (let i = 0; i < 20; i++) {
    const gx = 2 + Math.floor(Math.random() * 76)
    const gy = 2 + Math.floor(Math.random() * 44)
    if (grid[gy][gx] === 'void') setCell(grid, gx, gy, 'grass')
  }

  // some random tombs
  for (let i = 0; i < 8; i++) {
    const gx = 2 + Math.floor(Math.random() * 76)
    const gy = 2 + Math.floor(Math.random() * 44)
    if (grid[gy][gx] === 'void') setCell(grid, gx, gy, 'grave')
  }

  // convert grid to tiles record
  const tiles: Record<string, string | null> = {}
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      const id = grid[y][x]
      if (id !== 'void') {
        tiles[`${x},${y}`] = id
      }
    }
  }
  return tiles
}
