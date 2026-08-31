# GlyphWeave 地图逻辑改造方案（基于 mapgen4 水文地形）

## 背景

当前地形是"噪声 landform_field → 河谷/平原/丘陵/山地分类"，本质是
噪声拼贴，缺乏真实地形骨架。借鉴 redblobgames/mapgen4 的核心算法，
重写地形生成逻辑，让地图"为什么长这样"有据可依。

## mapgen4 核心算法提炼（来源：map.ts，Apache-2.0）

### 1. 山峰-距离场（地形骨架）
不是纯噪声。先选山峰点，BFS 从峰向外扩散距离场：
```js
// calculateMountainDistance: 从峰出发 BFS，distance += spacing*(1+jaggedness*rand)
// 山体 = 1 - slope/sharpness * distance   (距峰越远越低)
```
→ 地形是"以峰为核、距离衰减"的**真实山体骨架**，不是随机起伏。

### 2. 距海地形演化
```
e = constraintAt(距海岸的场)   // 近海=0(海平面)，内陆=1
陆地：e = (1-e²)*hill + e²*mountain   // 近海平地，内陆山峰，weight=e²
海洋：e *= ocean_depth + noise        // 水下加深
水位0 = 海岸线（e>0 陆，e<0 海）
```
→ 地形随"距海距离"演化，海岸线由水位自然决定，不是噪声边界。

### 3. 水文下切（河流）
```
assignDownslope: 从海洋三角开始，按海拔优先队列向上遍历，
  每个三角指向最低邻接三角（s_downslope_t），形成流域树
assignFlow: 水量沿下坡汇聚，河床在下切处加深
```
→ 河流从高处自然流到低处/海，河谷是水流的结果。

### 4. 气候湿度场（biome）
```
风方向排序 → 顺风湿度累积 → 遇山地形雨 → 降雨量
湿度>阈值 → 森林，低湿度 → 草原/沙漠
```
→ biome 由气候驱动（风/雨/地形），不是随机分类。

## GlyphWeave 落地路径

### 阶段 A：地形骨架（worldgen.rs）
1. 新增 `mountain_peaks(seed, scene)`：确定性选 N 个峰点。
2. 新增 `mountain_distance_field`：BFS/近似距离场（比 mapgen4 简单，
   用区域网格近似）。
3. `landform_field` 改为 = 山峰距离场 + 低振幅噪声（保河流蜿蜒），
   不再让噪声主导。
4. `terrain_height` 用 `e = (1-e²)*hill + e²*mountain` 距海演化，
   水位 0 是海岸线。

### 阶段 B：水文河流
5. 在 `generate_chunk_with_geometry` 前，预计算整场景的
   `downslope` + `flow`（低分辨率网格，如 16m/格），存成 sidecar。
6. `terrain_height` 读取水流：河床沿 flow 下切，谷地加深。
   → 河流从高处自然流到海，河谷连续。

### 阶段 C：气候 biome
7. 湿度场（风 + 降雨），`surface_kind` 由湿度决定森林/草原/沙。
8. 城市区域仍由 urbanization_field 覆盖（城区逻辑保留）。

## 约束
- 保持纯函数：山峰/距离场/水流都是 (seed, x, z) 的确定性函数，
  实体 worldY 和 chunk 烘焙仍一致。
- 城市/湾区平坦不受影响（城区 mask 保留）。
- 逐阶段实现 + 测试，不一次性大改。

## 验收
- 地图有清晰的山峰/山脊/河谷结构，不是噪声糊。
- 河流从高处流到低处/海，河床下切，谷地自然。
- 海岸线由水位决定，近海平缓、内陆起伏。
- 143+ 测试全通过，scale-audit 保持 pass。
