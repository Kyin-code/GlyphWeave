// 材质预设（Material Presets）
// ============================================================
// 所有视觉参数集中在此，方便 Kyin 调参后落地到实际渲染。
// 由 Kyin（任开炎）在原工程（Soil Studio / GrassSystemThreeJS）
// 中调好参数后回填。每个材质一个文件，按需加载。
// ============================================================

// ---- 草 (Grass) -----------------------------------------------------------
// 来源：Kyin 在 02-grass-system (Soil Studio, :5174) 中人工调参
// 记录时间：2026-08-31
// 三档草：浅草 / 中草 / 高草（按地形/覆盖区混合，形成层次）
export const grassPresets = {
  // 浅草 (low)：低矮贴地草，适合大面积铺底 / 缓坡
  low: {
    bladeHeight: 0.2,
    bladeWidth: 0.02,
    curl: 0.25,
  },
  // 中草 (mid)：正常草原主体
  mid: {
    bladeHeight: 0.78,
    bladeWidth: 0.042,
    curl: 0.48,
  },
  // 高草 (high)：高秆草丛，点缀 / 湿地
  high: {
    bladeHeight: 1.54,
    bladeWidth: 0.05,
    curl: 0.73,
  },
}

// 草的颜色与光照（保留 02 工程手感，可再调）
export const grassLook = {
  colorBase: '#33421b', // 底部暗绿
  colorTip: '#9bc24a',  // 尖端亮绿
  colorVarAmt: 0.5,     // 每丛颜色差异
  translucency: 1.1,    // 逆光透光强度
  roughness: 0.47,
}

// ---- 地面 (Ground) ---------------------------------------------------------
// 原则（Kyin）：地面染成草的颜色（看季节），立体草叶片只做动态效果。
// 当前为春夏季草绿调色板（makeMCTerrain），随季节可整体调整色相。
export const groundPresets = {
  season: 'summer', // 季节：spring / summer / autumn / winter
  // 低/中/高海拔草色 (RGB 0-255)，夏季草绿
  palette: {
    low: [66, 118, 52],
    mid: [88, 132, 60],
    high: [132, 148, 70],
  },
  tonalAmount: 0.18,  // 大尺度色调变化幅度
  warmStrength: 0.22, // 阳光暖化强度
  mossColor: [70, 130, 50], // 草甸 cover 色
  mossCoverage: 0.38,
  note: '远景因雾与光照自然过渡，近景保持草绿',
}

// 季节调色板（未来可切换）
export const seasonPalettes = {
  spring: { low: [80, 128, 58], mid: [104, 142, 64], high: [148, 156, 76] },
  summer: { low: [66, 118, 52], mid: [88, 132, 60], high: [132, 148, 70] },
  autumn: { low: [150, 128, 60], mid: [170, 140, 66], high: [184, 152, 72] },
  winter: { low: [140, 148, 120], mid: [150, 154, 128], high: [158, 158, 132] },
}

export const mossPresets = {
  enabled: false, // Moss Cover 默认关，Kyin 调后回填
}
