export function color(surface) { return ['#526a4f', '#6f8050', '#69737a', '#456b73', '#aa9560', '#809357', '#3f6548', '#8d765c'][surface] ?? '#526a4f' }

export function colorRgb(surface) { return [[.32, .42, .31], [.43, .52, .31], [.41, .45, .48], [.16, .42, .46], [.67, .58, .38], [.50, .60, .34], [.24, .39, .28], [.55, .46, .36]][surface] ?? [.32, .42, .31] }

export function hashNoise(x, z) {
  const value = Math.sin(x * 127.1 + z * 311.7) * 43758.5453
  return value - Math.floor(value)
}

export function smoothNoise(x, z) {
  const x0 = Math.floor(x); const z0 = Math.floor(z)
  const tx = x - x0; const tz = z - z0
  const sx = tx * tx * (3 - 2 * tx); const sz = tz * tz * (3 - 2 * tz)
  const a = hashNoise(x0, z0); const b = hashNoise(x0 + 1, z0)
  const c = hashNoise(x0, z0 + 1); const d = hashNoise(x0 + 1, z0 + 1)
  return (a + (b - a) * sx) * (1 - sz) + (c + (d - c) * sx) * sz
}

export function fractalNoise(x, z) {
  return smoothNoise(x, z) * .58 + smoothNoise(x * 2.7, z * 2.7) * .29 + smoothNoise(x * 7.1, z * 7.1) * .13
}

export function deterministicRand(x, z, salt) {
  let h = (x * 374761393 + z * 668265263 + salt * 69069) >>> 0
  h = (h ^ (h >> 13)) >>> 0
  h = Math.imul(h, 1274126177) >>> 0
  return ((h ^ (h >> 16)) >>> 0) / 4294967296
}
