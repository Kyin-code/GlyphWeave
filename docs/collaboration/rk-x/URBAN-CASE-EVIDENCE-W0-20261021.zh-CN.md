# 48 城核心切面证据矩阵 W0

日期：2026-10-21

状态：**调查进行中；34 个代表案例已有公开来源，其中 12 个完成首轮 OSM
米制观测，另 14 个仍为机制筛选**

上游设计：

- `CORE-CITY-V3-NATURAL-HUMAN-EVOLUTION-REVIEW-20261021.zh-CN.md`
- `assets/data/urban-case-priors-48.json`

机器可读结果：

- `assets/data/urban-case-evidence.json`
- `scripts/check-urban-case-evidence.mjs`
- `assets/data/urban-core-measurements/`
- `scripts/measure-urban-case-osm.mjs`
- `scripts/check-urban-core-measurements.mjs`

## 1. 本轮目的

本轮只推进审核稿 W0，不改生成算法。目标是回答三个问题：

1. 当前 48 条先验到底属于什么证据等级；
2. 哪些代表案例已经能追到政府规划、法规、开放数据、遗产或独立研究；
3. 哪些结论仍然只是机制先验，不能拿来冒充真实城市统计。

验证命令：

```powershell
node scripts/check-urban-case-evidence.mjs
node scripts/check-urban-case-priors.mjs
node scripts/check-urban-core-measurements.mjs
```

当前结果：

```json
{
  "status": "pass",
  "cases": 48,
  "sourceAuditedCases": 34,
  "screenedCases": 14,
  "sources": 65,
  "partialOsmWindows": 12
}
```

## 2. 证据分级

| 等级 | 含义 | 当前用途 |
|---|---|---|
| `source-audited-core-cut` | 至少两个不同发布者来源；能支持核心切面机制 | 代表性案例 |
| `partial-osm-core-window` | 已有约 1km² 当前 OSM 道路、建筑覆盖和设施点观测 | 12 个代表案例 |
| `screened-heuristic` | 已进入 48 案机制筛选，尚无逐案公开来源矩阵 | 14 案参数先验 |
| `measured-core-window` | 已定义米制 crop，并完成路网/地块/密度/市场等测量 | **尚无案例达到** |
| `bake-calibrated` | 已用真实或合成控制世界完成烘焙对照 | **尚无 V3 案例达到** |

本轮把 34 案提升到 `source-audited-core-cut`，其中前 12 案同时达到
`partial-osm-core-window`。没有案例被错误提升为完整的
`measured-core-window`，因为地块、FAR、市场容量和网络时距仍未全部测量。

## 3. 代表性案例与来源

### 3.1 北京

来源：

- [Beijing Master Plan 2016-2035](https://ghzrzyw.beijing.gov.cn/zhengwuxinxi/zxzt/bjcsztgh20162035/)
- [Beijing Urban Design Guideline](https://ghzrzyw.beijing.gov.cn/biaozhunguanli/bz/qt/202106/P020211108567546343857.pdf)
- [Cultural Innovation Breathes New Life into Beijing's Hutongs](https://english.beijing.gov.cn/latest/news/202307/t20230701_3152487.html)

已支持：

- 总体规划把首都功能、历史名城保护、资源与城乡关系作为长期空间框架；
- 城市设计导则可用于街道、街块和公共空间控制；
- 官方文章明确把胡同描述为高楼城市中的窄巷居住肌理，并以再利用方式更新。

尚未测量：

- 1km 核心切面边界；
- 胡同地块面宽/进深分布；
- 道路密度、FAR、建筑高度分布；
- 菜场与超市网络时距。

### 3.2 上海

来源：

- [Shanghai Master Plan 2017-2035](https://www.shanghai.gov.cn/newshanghai/xxgkfj/2035004.pdf)
- [Shanghai Master Plan portal](https://english.shanghai.gov.cn/en-MasterPlan/index.html)
- [The Shanghai lilong: Approaches to rehabilitation and reuse](https://www.iias.asia/the-newsletter/article/shanghai-lilong-approaches-rehabilitation-and-reuse)

已支持：

- 上海市级长期空间规划存在并可核验；
- 里弄起源于 19 世纪，在 20 世纪成为主导住宅类型；
- 到 1949 年，约一半建成区和 70–80% 居民与里弄有关；
- 2000 年后里弄出现商业、住宅和混合再利用等多条更新路径。

这组证据直接支持 V3 的“旧城小地块—更新带—混合底商”而不是用高层统一覆盖。

### 3.3 重庆

来源：

- [国务院关于重庆市国土空间总体规划的批复](https://www.gov.cn/zhengce/zhengceku/202402/content_6934307.htm)
- [ADB: Chongqing Gaoxin District Green and Low-Carbon Urban Development Program](https://www.adb.org/projects/56182-001/main)

已支持：

- 2021–2035 重庆市国土空间总体规划已获国务院批复；
- 重庆存在绿色低碳城市发展项目，适合检验山地城市的工程与生态压力。

尚未测量：

- 两江核心切面的坡度和高差分布；
- 台地、挡墙、梯道与桥隧密度；
- 山地道路实际纵坡；
- 削坡/填方工程量。

当前 `mountainous=1.72` 等土地压力仍是先验，不是该来源直接给出的统计。

### 3.4 深圳

来源：

- [深圳市国土空间总体规划（2021–2035年）](https://www.sz.gov.cn/cn/xxgk/zfxxgj/ghjh/csgh/zxgh/content/post_12097290.html)
- [State Council approval release](https://english.www.gov.cn/policies/latestreleases/202409/30/content_WS66fa5310c6d0868f4e8eb6c9.html)
- [UNEP-WCMC: Urban planning for the future in Shenzhen](https://www.unep-wcmc.org/en/news/urban-planning-for-the-future-in-shenzhen)

已支持：

- 深圳市级国土空间规划已批复并由市政府印发；
- 存在面向未来城市发展的规划研究。

尚未测量：

- 城中村地块与街巷样本；
- 填海面积和岸线变化；
- 湾区公共空间连续性；
- 居住—就业—商业网络。

“特区、城中村、填海新区并置”目前仍是机制先验，下一轮需要用深圳官方开放
数据和规划图件做米制测量。

### 3.5 纽约

来源：

- [New York City Zoning Resolution](https://zoningresolution.planning.nyc.gov/)
- [NYC Street Design Manual](https://www.nyc.gov/html/dot/html/pedestrians/streetdesignmanual.shtml)
- [NYC Planning Open Data](https://www.nyc.gov/site/planning/data-maps/open-data.page)

已支持：

- 纽约以住宅、商业、制造区和特别区规则控制开发；
- 街道与步行空间有独立设计导则；
- 规划部门公开空间数据，可用于地块、路网和设施测量。

用途：

- 校验“高密网格不等于全城同一塔楼”；
- 对比老网格、商业走廊、滨水再开发和收入混合；
- 建立道路密度、地块面宽和 FAR 样本。

### 3.6 芝加哥

来源：

- [Chicago Department of Planning and Development](https://www.chicago.gov/city/en/depts/dcd.html)
- [Chicago Data Portal](https://data.cityofchicago.org/)
- [Chicago History Museum](https://www.chicagohistory.org/)

已支持：

- 芝加哥有正式规划与开发部门；
- 城市提供开放数据门户；
- 独立城市史机构可作为历史层资料的入口。

下一轮应优先测量：

- 平原网格街区尺度；
- 湖滨和河流关系；
- 铁路、工业和仓储地块；
- 商业走廊与住宅组团差异。

### 3.7 巴黎

来源：

- [Paris Open Data](https://opendata.paris.fr/)
- [APUR: Parisian urban planning workshop](https://www.apur.org/en/)
- [Paris open-air markets dataset](https://opendata.paris.fr/explore/dataset/marches-decouverts/)

已支持：

- 巴黎公开市政空间数据；
- APUR 专门分析巴黎与巴黎大区的城市空间；
- 露天/周期市场有独立空间数据集，可用于建立周期市场网络。

用途：

- 把“市场辐射”同时覆盖固定菜场/超市与周期市场；
- 用连续街墙、庭院街区、市场街和轨道节点对照新区塔楼；
- 以 5/10/15/30 分钟网络时距做服务场，而不是均匀半径。

### 3.8 布拉格

来源：

- [Prague City official portal](https://www.praha.eu/jnp/en/index.html)
- [Prague Open Data](https://opendata.praha.eu/)
- [UNESCO: Historic Centre of Prague](https://whc.unesco.org/en/list/616/)

已支持：

- 布拉格历史中心位于伏尔塔瓦河两岸，是中世纪城市形态的重要实例；
- 大规模城市更新和拆除被避免，整体街道格局与空间构成得到保留；
- 城市提供开放数据入口。

用途：

- 避免把“更新”误做成整片推平；
- 对历史中心优先保留街网、地块和公共空间，再添加填充/改造；
- 用河岸和历史核心约束新城扩张与桥接。

### 3.9 东京下町/台东

来源：

- [Tokyo Urban Planning portal](https://www.toshiseibi.metro.tokyo.lg.jp/)
- [MLIT Street Design Guidelines](https://www.mlit.go.jp/toshi/toshi_gairo_fr_000055.html)
- [Taito City official portal](https://www.city.taito.lg.jp/)

已支持：

- 东京都提供城市规划入口；
- 日本国土交通省街区设计导则覆盖大城市到中小城市的步行公共空间；
- 台东区提供地方社区、灾害和规划信息入口。

尚未测量：

- 台东区细地块和窄巷分布；
- 轨道站影响范围；
- 木造密集区防火更新；
- 商店街和社区市场网络。

### 3.10 神户三宫

来源：

- [Kobe: Development and Urban Planning](https://www.city.kobe.lg.jp/a57337/business/kaihatsu/index.html)
- [Kobe City official portal](https://www.city.kobe.lg.jp/)
- [MLIT Street Design Guidelines](https://www.mlit.go.jp/toshi/toshi_gairo_fr_000055.html)

已支持：

- 神户公布开发许可、开发指导、市街化调整区域、用途地域检索和高度利用地区等
  规划制度；
- 城市提供市政规划入口；
- 日本有面向步行街区的设计导则。

用途：

- 山—海狭窄走廊不能套用平原同心圆；
- 地块、道路和建筑需要联合考虑坡地、挡墙、开发许可和港湾。

### 3.11 香港中环

来源：

- [Hong Kong Planning Standards and Guidelines](https://www.pland.gov.hk/pland_en/tech_doc/hkpsg/)
- [Hong Kong Planning Department](https://www.pland.gov.hk/)
- [Antiquities and Monuments Office](https://www.amo.gov.hk/en/home/index.html)

已支持：

- HKPSG 规定不同土地用途和设施的规模、选址与用地标准；
- 规划署公开法定规划信息；
- 古迹办提供历史建筑与古迹资料。

用途：

- 金融中心、坡地、天桥/步行系统和高密混合街区不能由普通商业地块模板生成；
- 历史对象和法定规划必须进入保留/更新逻辑。

### 3.12 香港沙田

来源：

- [Hong Kong Planning Standards and Guidelines](https://www.pland.gov.hk/pland_en/tech_doc/hkpsg/)
- [Hong Kong Planning Department](https://www.pland.gov.hk/)
- [Hong Kong Housing Authority](https://www.housingauthority.gov.hk/en/)

已支持：

- 新市镇设施和土地用途可用 HKPSG 标准核验；
- 规划署公开法定规划；
- 房委会提供公共屋邨和公共住房空间资料。

用途：

- 新市镇不是“低密郊区”，而是轨道、公共屋邨、学校、社区商业、
  河道和公园的组合；
- 社区菜场/邻里商业应在屋邨和站点网络中核定，而不是一个市场覆盖全图。

### 3.13 第二批来源审计

第二批把中国剩余 8 个核心切面和 4 个美国城市提升到有官方规划入口与
独立 OSM 数据源的状态：

| 案例 | 官方入口 | 当前用途 |
|---|---|---|
| 广州 | [2021–2035 国土空间总体规划](https://www.gz.gov.cn/gkmlpt/content/9/9960/post_9960346.html) | 江港、骑楼旧城、珠江新城对照 |
| 成都 | [生态修复规划文件](https://mpnr.chengdu.gov.cn/ghhzrzyj/tzgg/2024-08/21/97e8fd940aa04e2f9735fb75b8c60110/files/bff1eee922864412a083407cd2e3e8d0.pdf) | 平原旧城、水系与生态修复 |
| 西安 | [国务院国土空间规划批复](https://www.gov.cn/zhengce/zhengceku/202501/content_7000441.htm) | 城墙核心、保护与新区 |
| 杭州 | [杭州市政府门户](https://www.hangzhou.gov.cn/) | 西湖、运河与滨水新区 |
| 武汉 | [2021–2035 国土空间总体规划](https://www.wuhan.gov.cn/zwgk/xxgk/zfwj/szfwj/202504/t20250425_2573228.shtml) | 江汇、湖群与三镇 |
| 青岛 | [青岛政务网](https://www.qingdao.gov.cn/) | 山—海—港与阶梯街道 |
| 厦门 | [厦门市政府门户](https://www.xm.gov.cn/) | 岛屿、骑楼与填海边缘 |
| 昆明 | [昆明市政府门户](https://www.km.gov.cn/) | 高原湖盆与坝区 |
| 旧金山 | [SF Planning](https://sfplanning.org/) | 陡坡网格、湾岸与就业核心 |
| 洛杉矶 | [LA City Planning](https://planning.lacity.gov/) | 多中心、商业走廊与低密蔓延 |
| 西雅图 | [Seattle OPCD](https://www.seattle.gov/opcd/) | 山海坡地、城市村与轨道更新 |
| 波士顿 | [City of Boston Planning Department](https://www.bostonplans.org/) | 殖民巷网、填海滨水与再开发 |

这批案例的第二个来源是 [OpenStreetMap map data API](https://api.openstreetmap.org/api/0.6/map)，
只证明当前可取得有界窗口的道路、建筑和设施几何，**不证明历史事实**。
它们尚未完成 1km² OSM 米制测量，因此保持 `source-audited-core-cut`，
不带 `partial-osm-core-window` 标记。

### 3.14 第三批欧洲来源审计

第三批把剩余 10 个欧洲案例提升到双来源覆盖：

| 案例 | 来源 A | 来源 B | 证据限制 |
|---|---|---|---|
| 伦敦 | [London Datastore](https://data.london.gov.uk/) | [UNESCO Westminster](https://whc.unesco.org/en/list/426/) | 伦敦规划页受反爬保护，暂用数据与遗产组合 |
| 阿姆斯特丹 | [Urban development](https://www.amsterdam.nl/en/policy/urban-development/) | [Data Amsterdam](https://data.amsterdam.nl/) | 运河地块仍需实测 |
| 柏林 | [Berlin urban development](https://www.berlin.de/sen/stadtentwicklung/) | [Berlin Open Data](https://daten.berlin.de/) | 分裂城市更新仍需历史档案 |
| 苏黎世 | [Planning and Construction](https://www.stadt-zuerich.ch/de/planen-und-bauen.html) | [Open Data Zurich](https://data.stadt-zuerich.ch/) | 湖岸坡地仍需实测 |
| 米兰 | [Urban planning and building](https://www.comune.milano.it/aree-tematiche/urbanistica-ed-edilizia) | [Milan open data](https://dati.comune.milano.it/) | 两站均通过阅读器访问 |
| 罗马 | [Roma Open Data](https://dati.comune.roma.it/) | [UNESCO Historic Centre](https://whc.unesco.org/en/list/91/) | 规划控制与考古范围仍需细化 |
| 巴塞罗那 | [Barcelona urbanism dataset API](https://opendata-ajuntament.barcelona.cat/data/api/action/package_search?q=urbanisme&rows=1) | [UNESCO Gaudí](https://whc.unesco.org/en/list/320/) | 还缺完整市政规划文档 |
| 里斯本 | [Visit Lisboa](https://www.visitlisboa.com/) | [UNESCO Belém](https://whc.unesco.org/en/list/263/) | **缺官方市政规划来源，优先级最高** |
| 斯德哥尔摩 | [Stockholm City](https://start.stockholm/en/) | [Stockholm Data Portal](https://dataportalen.stockholm.se/) | 岛屿核心仍需实测 |
| 维也纳 | [Urban development](https://www.wien.gv.at/stadtentwicklung/) | [Vienna Open Data](https://data.wien.gv.at/) | Ring/公共住宅对照仍需实测 |

这批案例只有两个不同发布者的来源入口，不代表已完成规划史、地块或密度测量。
其中里斯本与巴塞罗那的官方规划材料覆盖明显弱于其他案例，不能用于高置信
规划参数。

## 4. 首轮 1km² 米制观测

测量脚本以命名核心锚点为中心，取面积约 1km² 的圆形当前 OSM 快照：

```powershell
$env:HTTP_PROXY="http://127.0.0.1:7890"
$env:HTTPS_PROXY="http://127.0.0.1:7890"
$env:NODE_USE_ENV_PROXY="1"
node scripts/measure-urban-case-osm.mjs `
  --case-id cn-shanghai-bund `
  --anchor "The Bund, Shanghai" `
  --output assets/data/urban-core-measurements/cn-shanghai-bund-1km2.json
```

这不是历史统计，也不是全城平均；它只用于校准核心切面的空间量级。
无代理环境或本地可直接访问 OSM 时，可删除前三行环境变量。

| 核心窗口 | 可驾驶中心线 km/km² | 建筑数 | 覆盖率 | 生鲜点 | 市场型中位步行 | 10 分钟覆盖 |
|---|---:|---:|---:|---:|---:|---:|
| 北京王府井 | 15.37 | 283 | 0.286 | 2 | — | — |
| 上海外滩 | 21.42 | 301 | 0.193 | 8 | — | — |
| 重庆解放碑 | 20.28 | 233 | 0.313 | 16 | 8.96 min | 0.609 |
| 深圳福田 CBD | 22.69 | 177 | 0.351 | 5 | 19.12 min | 0.249 |
| 纽约时代广场 | 18.50 | 784 | 0.569 | 42 | 3.98 min | 0.939 |
| 芝加哥 Loop | 20.50 | 325 | 0.231 | 6 | — | — |
| 巴黎市政厅 | 20.68 | 1,085 | 0.246 | 66 | 2.42 min | 0.962 |
| 布拉格老城 | 18.28 | 942 | 0.372 | 63 | 2.93 min | 0.973 |
| 东京浅草 | 36.70 | 4,153 | 0.368 | 50 | 2.46 min | 0.998 |
| 神户三宫 | 33.30 | 2,406 | 0.376 | 65 | 3.87 min | 0.996 |
| 香港中环 | 32.72 | 410 | 0.302 | 21 | 2.63 min | 0.985 |
| 香港沙田站 | 14.26 | 379 | 0.223 | 10 | 7.64 min | 0.736 |

`—` 表示该窗口没有可路由的市场型 OSM 设施记录，不表示真实供给为零。

汇总：

| 指标 | 最小 | 中位数 | 最大 |
|---|---:|---:|---:|
| 可驾驶道路中心线密度 | 14.26 | 20.59 | 36.70 |
| 建筑 centroid 数 | 177 | 394.5 | 4,153 |
| 建筑覆盖率 | 0.193 | 0.3075 | 0.569 |
| OSM 生鲜设施点 | 2 | 18.5 | 66 |
| 建筑层数标注覆盖率 | 0.075 | 0.2255 | 0.800 |
| 市场型中位步行时间（9 窗口） | 2.42 | 3.87 | 19.12 |
| 市场型 10 分钟覆盖（9 窗口） | 0.249 | 0.962 | 0.998 |

### 4.1 旧控制范围需要修正

V3 早期把中心网格/混合区的“路网密度”写成 10–18 km/km²。12 个核心样本显示，
只看可驾驶中心线，中位数已经约 20.6，范围 14.3–36.7。旧范围对高密核心偏低，
应把中心区控制带改为 18–34，历史细街网改为 14–23。

包含步行道、服务道、平台和楼梯的总中心线密度会更高；巴黎样本达到
123 km/km²，其中 86.2 km/km² 是 footway。生成器不能把这些口径混成一个
“道路密度”。

### 4.2 建筑数量不等于密度

深圳福田样本只有 177 个 inclusion building，覆盖率为 0.351；东京浅草有
4,153 个建筑，覆盖率 0.368。楼栋数相差 23 倍，但地面覆盖率接近。

因此：

- 建筑数量必须结合占地、楼层、FAR 和用途一起看；
- “楼太少”不能只靠增加 count 解决；
- 塔楼片区可能楼栋少但覆盖/容积高，旧城可能楼栋多但高度低。

### 4.3 生鲜设施点只能证明点位，不证明服务

OSM 的 `marketplace/shop=supermarket/convenience/...` 点可用于空间开场，
但北京王府井只有 2 个记录点，不代表实际只有两家；它更可能反映 OSM
标注完整度。生鲜容量、营业面积、摊位、开放时段、竞争和网络时距仍需另测。

### 4.4 步行网络首次校准

测量把建筑 centroid 和生鲜设施吸附到 120m 内的 OSM 步行/道路网络，以
`1.3m/s` 计算网络步行时间。9 个能路由市场型设施的窗口中：

- 市场型中位步行时间的中位数为 3.87 分钟；
- 10 分钟覆盖比例的中位数为 96.2%；
- 欧洲/日本核心普遍在 2–4 分钟内形成高频生鲜网络；
- 重庆解放碑为 8.96 分钟、10 分钟覆盖 60.9%；
- 香港沙田站为 7.64 分钟、73.6%；
- 深圳福田 CBD 为 19.12 分钟、24.9%，是明显的待调查异常。

北京、上海、芝加哥窗口没有映射到市场型设施，不能据此判定“没有市场”。
这要求正式生成前增加数据完整性 gate：设施数量、分类和道路连通性不足时，
只能输出“数据不足”，不能输出“服务缺口”。

这里的建筑等权、设施点位和 OSM 连通性都是代理，尚不能代替容量竞争模型。

### 4.5 建筑层数与 FAR 代理

只有 7.5%–80.0% 的建筑具有明确 OSM 层数标签，中位数 22.55%。因此
`observedFloorAreaRatioProxy` 只能作为下界参考，不能作为样本 FAR 统计。
要获得可靠 FAR，仍须使用官方建筑轮廓/楼层数据或规划控制资料。

### 4.6 观测限制

- 当前 OSM 是协作地图快照，不是历史人口普查；
- 建筑覆盖用 centroid 进入圆窗和投影多边形面积，边界裁剪是近似；
- 道路是中心线，不代表车道容量或道路红线面积；
- 设施点不代表容量、质量或服务覆盖；
- 步行网络等权建筑，不代表住户、就业或游客人数；
- OSM 设施分类和连通性缺失会改变覆盖结果；
- 指定锚点只代表一个核心窗口，不能外推全城。

## 5. 本轮调查得到的跨案规律

### 5.1 自然先于城市

所有已审案例都显示城市核心与水系、海岸、河谷、平原、山麓或湖岸有关。
但这批来源首先证明的是空间制度和城市形态；具体地貌过程与河流动力的米制
参数仍要回到自然地理来源矩阵。

### 5.2 旧城不是低密，而是细粒度

北京胡同和上海里弄说明，旧城常见的是小地块、窄街、连续街墙和高混合，
而不是简单“低层低密”。因此新区不能仅靠提高楼层来体现现代化。

### 5.3 更新是多种动作并存

上海里弄资料明确出现商业、住宅、混合再利用；布拉格资料强调保留整体街网
和空间构成。生成器必须支持保留、局部改造、拆合、填充和功能替换，
不能把更新简化为整片拆除重建。

### 5.4 制度差异会改变空间

纽约区划、神户开发许可、香港 HKPSG 和北京/上海/深圳总规都说明，
同样的人口或就业需求，在不同制度下会得到不同地块、FAR、退界、停车、
公共空间和设施组合。

### 5.5 市场必须是网络

巴黎周期市场数据、香港设施标准和上海里弄商业肌理都指向：

- 旧城存在小型、连续、街面型商业；
- 新城需要社区/邻里商业中心；
- 周期市场、菜场、商店街、超市和批发市场承担不同功能；
- 单点固定半径不能证明覆盖。

## 6. 仍未完成的证据工作

### 6.1 14 个待补案例

`assets/data/urban-case-evidence.json` 已为全部 48 案登记状态。其余 14 案
目前只能是 `screened-heuristic`，不能用于声称官方密度、人口、路网或市场统计。

### 6.2 米制核心窗口

12 案虽已有首轮 OSM 观测，但尚未完成：

- 统一 1km/2km 窗口边界；
- 坐标参考系和朝向；
- 建筑/地块/道路的米制测量；
- 自然高度、坡度、水位和岸线；
- 中心性、服务时距和人口/就业权重；
- 数据缺失和冲突记录。

### 6.3 历史时间轴

现有 48 案每案已有三段机制性历史，但除上海、布拉格等少数有直接来源外，
多数尚需政府历史、规划档案或独立研究逐段核实。

### 6.4 市场专用证据

巴黎周期市场数据集是起点。还需补：

- 中国社区菜场/农贸市场建设标准；
- 香港街市与屋邨商业标准；
- 纽约食品零售和超市可达政策；
- 日本商店街与社区商业资料；
- 批发市场/冷链物流空间资料。

## 7. 下一步

W0 继续按以下顺序推进：

1. 对 24 个已审案例定义米制 core-window；
2. 使用官方开放数据/OSM 辅助测量道路、地块、建筑高度和设施；
3. 每个数值分别标记 `observed`、`derived` 或 `heuristic`；
4. 补齐市场专用来源；
5. 将 14 个案例按地区逐批升级；
6. 只有通过来源 gate 后，才把参数写入生成 Manifest 或 Rust fixture。

在这些工作完成前，V3 审核稿中的道路密度、FAR、市场和工程系数仍只能当作
待校准范围，不能当作真实城市统计。
