// 材质预设（Material Presets）
// ============================================================
// 所有视觉参数集中在此，方便 Kyin 调参后落地到实际渲染。
// 由 Kyin（任开炎）在原工程（Soil Studio / GrassSystemThreeJS）
// 中调好参数后回填。每个材质一个文件，按需加载。
// ============================================================

// ---- 开源参考合同 ---------------------------------------------------------
// 逐项核对来源：G:\difyllmwiki\render-experiments\02-grass-system\src\grass.js
// 与 src/main.js（Soil Studio / achrefelouafi/GrassSystemThreeJS 的本地副本）。
// 这些是参考工程的实际默认值；GlyphWeave 只在世界尺度上提高实例预算，
// 并保留自己的地形湿润度/坡度筛选，不把测试场景坐标写进渲染器。
export const grassReferenceTuning = {
  liveDensity: 0.13,
  maxCount: 200000,
  coverage: 0.62,
  maskScale: 0.15,
  maskEdge: 0.251,
  curl: 1.14,
  height: 1.5,
  width: 0.049,
  colorBase: '#33421b',
  colorTip: '#9bc24a',
  colorVariation: 0.47,
  translucency: 0.61,
  wind: { strength: 0.5, speed: 1.8, scale: 0.35, gust: 0.6 },
}

// ---- 草 (Grass) -----------------------------------------------------------
// 基础外观沿用开源参考工程；三档高度是 GlyphWeave 对 200m+ 世界的适配层。
// 参考值详见 grassReferenceTuning，避免把“手调值”误写成上游默认值。
// 记录时间：2026-09-03
// 三档草：浅草 / 中草 / 高草（按地形/覆盖区混合，形成层次）
export const grassPresets = {
  // 浅草 (low)：低矮贴地草，适合大面积铺底 / 缓坡
  low: {
    bladeHeight: 0.34,
    bladeWidth: 0.09,
    curl: 0.25,
  },
  // 中草 (mid)：正常草原主体
  mid: {
    bladeHeight: 1.08,
    bladeWidth: 0.14,
    curl: 0.48,
  },
  // 高草 (high)：高秆草丛，点缀 / 湿地
  high: {
    bladeHeight: 1.85,
    bladeWidth: 0.18,
    curl: 0.73,
  },
}

// 草的颜色与光照（保留 02 工程手感，可再调）
export const grassLook = {
  colorBase: grassReferenceTuning.colorBase, // 上游 uColorBase
  colorTip: grassReferenceTuning.colorTip,  // 上游 uColorTip
  colorVarAmt: grassReferenceTuning.colorVariation, // 上游 uColorVarAmt
  translucency: grassReferenceTuning.translucency,   // 上游 uTranslucency
  roughness: 0.47,
}

// ---- 地面 (Ground) ---------------------------------------------------------
// 原则（Kyin）：地面染成草的颜色（看季节），立体草叶片只做动态效果。
// 当前为春夏季草绿调色板（makeMCTerrain），随季节可整体调整色相。
export const groundPresets = {
  season: 'summer', // 季节：spring / summer / autumn / winter
  // 低/中/高海拔草色 (RGB 0-255)，夏季草绿
  palette: {
    low: [48, 112, 45],
    mid: [67, 132, 53],
    high: [118, 151, 67],
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
