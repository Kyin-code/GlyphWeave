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

// ---- 未来材质占位（Kyin 会逐步回填）--------------------------------------
export const groundPresets = {
  // 地形起伏：Mound Scale / Mound Height 应根据地形数据自动给出（未定）
  mounds: { enabled: true, note: '根据地形高度自动', },
}

export const mossPresets = {
  enabled: false, // Moss Cover 默认关，Kyin 调后回填
}
