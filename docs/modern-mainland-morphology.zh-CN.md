# 现代社会随机大陆形态基线

本文档把中国、欧洲、美国的公开城市规划与开放 GIS 资料归纳为可被 Rust
生成器使用的先验。它不是现实城市复刻数据，也不把任何一个城市当作模板；
城市名称只用于研究样本，最终地图由 `world.seed` 和规则参数决定。

## 研究样本

中国：北京、上海、广州、深圳、成都、武汉、西安、南京、重庆、天津、青岛、杭州。

欧洲：伦敦、巴黎、阿姆斯特丹、柏林、巴塞罗那、米兰、维也纳、斯德哥尔摩、
哥本哈根、布拉格、华沙、里斯本。

美国：纽约、芝加哥、洛杉矶、旧金山、西雅图、波士顿、华盛顿特区、费城、
波特兰、丹佛、休斯敦、明尼阿波利斯。

## 公开资料入口

资料优先级为政府规划文本、规划局开放数据、官方 GIS 门户，其次为官方公共
空间或交通资料。代表性入口如下：

- 北京总体规划：<https://www.beijing.gov.cn/gongkai/guihua/wngh/csztgh/201907/t20190701_100008.html>
- 上海总体规划：<https://www.shanghai.gov.cn/newshanghai/xxgkfj/2035004.pdf>
- 广州国土空间规划：<https://www.gz.gov.cn/gkmlpt/content/9/9960/post_9960346.html>
- 深圳国土空间规划：<https://www.sz.gov.cn/cn/xxgk/zfxxgj/ghjh/csgh/zxgh/content/post_12097290.html>
- 成都生态修复规划：<https://mpnr.chengdu.gov.cn/ghhzrzyj/tzgg/2024-08/21/97e8fd940aa04e2f9735fb75b8c60110/files/bff1eee922864412a083407cd2e3e8d0.pdf>
- 武汉国土空间规划：<https://www.wuhan.gov.cn/zwgk/xxgk/zfwj/szfwj/202504/t20250425_2573228.shtml>
- 西安国土空间规划批复：<https://www.gov.cn/zhengce/zhengceku/202501/content_7000441.htm>
- 南京国土空间规划批复：<https://www.gov.cn/zhengce/zhengceku/202409/content_6975142.htm>
- 重庆国土空间规划批复：<https://www.gov.cn/zhengce/zhengceku/202402/content_6934307.htm>
- 天津国土空间规划批复：<https://www.gov.cn/zhengce/zhengceku/202408/content_6968299.htm>
- London Datastore：<https://data.london.gov.uk/>
- Paris Open Data：<https://opendata.paris.fr/>
- Amsterdam Open Data：<https://data.amsterdam.nl/>
- Berlin Open Data：<https://daten.berlin.de/>
- Barcelona Open Data：<https://opendata-ajuntament.barcelona.cat/>
- Vienna Open Data：<https://data.wien.gv.at/>
- Copenhagen Open Data：<https://data.kk.dk/>
- NYC Open Data：<https://data.cityofnewyork.us/>
- Chicago Data Portal：<https://data.cityofchicago.org/>
- Los Angeles Open Data：<https://data.lacity.org/>
- DataSF：<https://data.sfgov.org/>
- Seattle Open Data：<https://data.seattle.gov/>
- OpenDataPhilly：<https://www.opendataphilly.org/>
- Houston Open Data：<https://data.houstontx.gov/>

## 归纳出的空间逻辑

现代城市的稳定共性不是一个固定土地百分比，而是：交通节点集聚、自然要素
约束、中心多级分化、居住与公共服务邻近、产业沿物流走廊布局、生态空间形成
连续网络。

| 地块层级 | 人口/建筑密度 | 主要功能 | 生成位置关系 |
| --- | ---: | --- | --- |
| 高密度核心 | 0.75-1.00 | 商业中心、办公、住宅、公共设施、娱乐中心 | 位于交通汇聚点、港口、河湾、历史中心或可建设台地 |
| 普通城区 | 0.45-0.75 | 中高层住宅、社区商业、学校、小公园、基层服务 | 围绕核心和轨道/主干路形成 400-900m 生活圈 |
| 郊区组团 | 0.20-0.50 | 低层住宅、产业园、大型商业、停车场 | 沿快速路、铁路、机场或城市边缘节点展开 |
| 乡村 | 0.05-0.25 | 村落、农田、沟渠、集市、学校、宗教建筑 | 沿河谷、道路、等高线和田块边界形成紧凑斑块 |
| 自然保护区 | 0.00-0.10 | 山地林场、湿地、牧场、保护设施 | 位于陡坡、源头、海岸、生态廊道和城市外缘 |

## 程序参数基线

下表是归一化生成参数。`landRatio` 是场景面积比例，`density` 是同类地块内
的生成强度，二者不能混为一谈。Rust 可按主题、地形和 seed 在区间内取值。

| 类型 | landRatio | density | 尺度/间距 | 地形约束 |
| --- | ---: | ---: | --- | --- |
| 道路与路口 | 0.08-0.22 | 0.55-1.00 | 主干路 18-32m，支路 8-16m | 沿等高线；最大纵坡 6-8% |
| 绿化与公园 | 0.08-0.30 | 0.35-0.85 | 80-500m 斑块，连续廊道优先 | 沿河、坡脚、道路和社区边缘 |
| 停车场 | 0.01-0.08 | 0.20-0.75 | 20-160m 地块 | 靠近商业、学校、枢纽，不占湿地 |
| 娱乐中心 | 0.005-0.04 | 0.10-0.50 | 60-240m | 靠近核心、滨水、轨道节点 |
| 商业中心 | 0.02-0.12 | 0.35-1.00 | 150-700m 中心，沿街店面连续 | 主干路/轨道交汇，滨水优先 |
| 学校 | 0.01-0.06 | 0.25-0.70 | 服务半径 400-900m | 住宅区邻近，避开主干路和陡坡 |
| 住宅区 | 0.18-0.48 | 0.40-0.95 | 42-120m 街区/组团 | 台地、平缓坡地，靠近学校和商业 |
| 水渠与滞洪带 | 0.01-0.10 | 0.20-0.70 | 6-40m 水渠，30-120m 缓冲 | 低洼地、农田边界和河流支线 |
| 农田 | 0.15-0.55 | 0.25-0.85 | 40-300m 田块 | 平原、河谷、缓坡，靠近灌渠 |
| 山地林场 | 0.10-0.60 | 0.35-0.95 | 20-80m 林窗 | 高程和坡度高的连续区域 |
| 寺庙/教堂 | 0.001-0.02 | 0.05-0.30 | 20-80m | 村落、老城、山脚或广场节点 |
| 牧场 | 0.05-0.35 | 0.15-0.65 | 100-600m 围栏地块 | 缓坡、草原、林缘，避开密集城区 |
| 自然保护区 | 0.10-0.70 | 0.35-1.00 | 连续大斑块 | 河源、湿地、陡坡、海岸和生态廊道 |

## 生成顺序

1. 生成大陆边界、海岸、河流、湖泊、山脉、坡度和排水网络。
2. 按可建设性、交通可达性和生态敏感度生成自然区、农田、牧场和林场。
3. 选择多个交通节点，生成核心、次中心、产业节点、郊区组团和乡村聚落。
4. 生成主干路、支路、桥梁、渠道和绿廊，确保路网连接而不是随机线段。
5. 以生活圈为单位布置住宅、学校、社区商业、公园和停车场。
6. 沿主干路、轨道、港口和河湾布置商业中心、娱乐中心和高层天际线。
7. 最后生成树、灌木、草、路灯、车辆等细节，并执行水体、坡度、footprint
   和跨分块连续性审计。

## 主题参数

大陆主题只改变分布先验，不改变硬约束：

- `river-delta`：平原、河网、农田和多中心城市比例提高。
- `coastal-bay`：港湾、滨水商业、海岸湿地和高密度核心概率提高。
- `mountain-valley`：山地林场、谷地村落和等高线道路概率提高。
- `temperate-plain`：规则道路、农田、郊区组团和林带概率提高。
- `arid-oasis`：绿洲核心、灌渠、牧场和荒漠保护区形成强对比。
- `tropical-rainforest`：河流、湿地、林场和少量沿岸聚落占主导。

Agent 只负责选择主题、seed、少量剧情主体和用户确认的宏观约束；所有普通
地块、道路、建筑、植物和装饰由 Rust 根据本基线确定性生成。

## 城市形态 profile 抽象

36 个样本城市被抽象为 6 种可调的城市形态。每种形态只改分布先验，
不改变硬约束，并在 `generate-procedural-world OUTPUT WIDTH DEPTH SEED THEME`
中可直接选用：

| profile | 代表城市 | 核心结构 | 路网 | 密度梯度 | 自然倾向 |
| --- | --- | --- | --- | --- | --- |
| `dense-core` | 上海、纽约曼哈顿 | 单高密度核心 | 密网格 260m | 0.95 | 公园少、城市包围绿心 |
| `river-delta` | 阿姆斯特丹、武汉 | 3 核心沿河道 | 主轴 + 支路 | 0.78 | 双水渠、农田沿河 |
| `coastal-bay` | 旧金山、青岛 | 双核心靠湾 | 环状放射 | 0.88 | 环状路网、滨水商业 |
| `mountain-valley` | 重庆、旧金山湾 | 双核心沿谷 | 谷轴 + 稀疏支路 | 0.66 | 林场沿高坡 |
| `temperate-plain` | 北京、巴黎 | 单核心 | 正交网格 360m | 0.75 | 规则农田、郊区组团 |
| `low-density-suburban` | 洛杉矶、休斯敦 | 单核心低密 | 宽网格 460m | 0.42 | 大住宅块、绿地多 |

### 多核心布局

`river-delta` / `coastal-bay` / `mountain-valley` 会派生多个次级核心，
每个次级核心附带商业、学校、停车场和绿地组团；`river-delta` 沿河道主轴、
`mountain-valley` 沿谷轴排列次核心，其余形态用黄金角在 seed 上确定性散开。
密度梯度取「到最近核心」的距离，多核心形态的自然衔接由该距离自动完成。

### 面积区间审计

生成后 `quality-report` 计算城市/乡村/自然三大区 footprint 面积，
与 `landUseProfile` 的目标 share 归一化对比（容差 0.22）。这也是把
研究基线中的 `landRatio` 转成可验证产物指标的第一步：后续若面积持续
偏离目标，应优先调整对应 profile 的地块尺寸与填充带，而不是修改硬约束。
