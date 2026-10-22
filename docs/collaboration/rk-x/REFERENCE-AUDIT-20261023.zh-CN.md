# 自然地貌、水岸、道路与建筑参考审计（2026-10-23）

状态：**资料审计完成；仅形成设计与尺度依据，不代表任何参考项目已被复刻。**

本文回答两个问题：

1. 48 城样本怎样使用才不会被误读成“必须生成多少栋”；
2. 现有公开资料能支持哪些山体、水体、道路和高密度建筑关系，哪些内容
   只能做构图参考，不能直接量取或复制。

## 1. 结论先行

- 样本用途是**尺度和机制证据**，不是完成数量目标。OSM building way 不
  展开 multipolygon，也不包含所有内部隔间、棚屋和未测绘附属体量；因此
  建筑数量只可当作下界，覆盖率也受 centroid inclusion 影响。
- 48 个一平方公里核心窗口的差异非常大。建筑数量中位数为 410，但四分位
  上界为 942，最大为 4,153；建筑覆盖率中位数约为 32%，范围约
  2.2%–56.9%。这说明“中心城区”不能用一个全局密度系数解决。
- 香港内部同样存在多种城市形态：旺角是高密度小街廓和极高覆盖率；中环
  是金融塔楼、平台和陡坡；观塘是工业更新与大型公共屋邨；沙田和西贡又
  受河道、山体和郊野边界影响。旧城、更新区、新区和郊外必须分阶段生成。
- 道路不能根据现有贴图材质反推。应先由交通等级确定车道数、车道宽、中央
  隔离、人行道、路缘、纵坡和碰撞断面，再由道路断面选择表现资源。
- 山体、水体、建筑不能只由随机噪声生成。至少应先有地形骨架、汇水、
  山脊/谷地、岸线和可建设台地，再由道路与聚落反向雕刻地形。

## 2. 仓库样本复核

数据来源：

- `assets/data/urban-core-measurements/*-1km2.json`
- `assets/data/urban-case-evidence.json`

复核日期：2026-10-23。

48 个窗口的当前统计：

| 指标 | 最小值 | 中位数 | P75 | 最大值 |
|---|---:|---:|---:|---:|
| 建筑数量 | 48 | 410 | 942 | 4,153 |
| 建筑覆盖 | 2.2% | 32.0% | — | 56.9% |
| 可驾驶中心线密度 | 12.10 km/km² | 20.33 km/km² | — | 36.70 km/km² |

香港样本的形态差异：

| 样本 | 建筑数 | 覆盖 | 可驾驶道路密度 | 10 分钟鲜食网络覆盖 |
|---|---:|---:|---:|---:|
| 旺角 | 1,370 | 45% | 29.29 km/km² | 96% |
| 中环皇后像广场 | 410 | 30% | 32.72 km/km² | 98% |
| 观塘 | 401 | 40% | 23.36 km/km² | 100% |
| 铜锣湾 | 477 | 27% | 21.81 km/km² | 100% |
| 沙田站 | 379 | 22% | 14.26 km/km² | 76% |
| 西贡 | 937 | 13% | 12.10 km/km² | 97% |

使用边界：

- 中环 410 栋是**当前 OSM 可观测下界**，不是要求生成器必须命中 410；
- 西贡建筑多但覆盖低，说明建筑数量和覆盖不能互相替代；
- 旺角的高覆盖不能照搬到山坡、海湾或旅游区；
- 鲜食覆盖是等权建筑、步行网络和设施存在性的代理，不包含容量、营业
  时间、竞争、排队和品质；
- 道路密度是中心线长度，不包含车道容量和路权面积。

因此密度验收至少应同时报告：

- 建筑数量；
- building footprint 总面积和覆盖率；
- 建筑用途构成；
- 中心距离分带的建筑数量/覆盖曲线；
- 道路等级、车道数、中心线密度和交叉口数量；
- 市场 5/10/15 分钟覆盖及容量利用；
- 自然保育、水域、不可建设坡地和工程改造面积。

## 3. 公开参考

### 3.1 道路与街道断面

1. **NACTO, Urban Street Design Guide: Lane Width**
   - URL:
     https://nacto.org/publication/urban-street-design-guide/street-design-elements/lane-width/
   - 访问：2026-10-23。直接请求返回 HTTP 403；Reader 代理遇到
     Cloudflare/CAPTCHA，未能提取正文。
   - 用途：街道断面、车道宽和城市道路分级的概念参考。
   - 限制：当前会话没有可复现的正文证据，不能据此引用具体数值；正式
     使用前应由可正常访问的审阅者核对页码和数值。

2. **NACTO, Neighborhood Street**
   - URL:
     https://nacto.org/publication/urban-street-design-guide/streets/neighborhood-street/
   - 访问：2026-10-23。直接请求返回 HTTP 403；Reader 代理遇到
     Cloudflare/CAPTCHA，未能提取正文。
   - 用途：本地街道、步行、街边活动和窄断面概念参考。
   - 限制：同上，不能把当前未取得的正文当作已核对证据。

3. **Hong Kong Highways Department, Structures Design Manual for
   Highways and Railways (2013 Edition)**
   - URL:
     https://www.hyd.gov.hk/en/technical_references/technical_document/structures_design_manual_2013/index.html
   - 访问：2026-10-23，HTTP 200。页面明确说明该手册为香港公路和铁路
     结构设计提供指导和标准，2013 版迁移至 Eurocodes，并持续纳入修订。
   - 用途：桥梁、高架、挡墙、道路结构与道路表面分离建模。
   - 对本项目的要求：桥梁/高架应有独立结构、支座、桥面连续体和碰撞，
     不能把普通道路贴图拉长后冒充跨越结构。

### 3.2 山体与陡坡

4. **Hong Kong CEDD, Natural Terrain and Slope Safety**
   - URL:
     https://hkss.cedd.gov.hk/hkss/eng/natural_terrain.aspx
   - 访问：2026-10-23，HTTP 200。页面为动态站点，静态抓取只得到站点
     外壳，无法可靠提取正文数字。
   - 用途：自然山坡、坡面安全、滑坡与人工边坡治理的官方入口。
   - 对本项目的要求：
     - 自然山体先形成连续山脊、谷地、汇水和坡面；
     - 陡坡上的建筑需要台地、挡墙、阶梯和排水；
     - “中央城区可以开山”不等于所有坡面都能无限削平；
     - 任何削坡/填方都必须进入工程账本，并留下坡脚、坡顶与支护体。

### 3.3 水岸与高密度城市

5. **Hong Kong Harbourfront Commission, Urban Design Study for the New
   Central Harbourfront**
   - URL:
     https://www.hfc.org.hk/filemanager/files/urban-design-study-for-the-new-central-harbourfront/uds-nch-es-(english-version-only)_r.pdf
   - 访问：2026-10-23，HTTP 200，PDF，约 6.34 MB，50 页。
   - 用途：海滨长廊、公众可达岸线、城市与海港界面、低层公共空间与
     背后高密度塔楼的关系。
   - 对本项目的要求：
     - 水体应有独立水面、岸线、湿边、浅水和公共步道；
     - 海堤与填海区应有明确边界与工程断面；
     - 滨水建筑应形成可读的城市界面，不应把水面当作背景蓝色贴图。

6. **Hong Kong Planning Department, Planning and Urban Design for a
   Liveable High-Density City**
   - URL:
     https://www.pland.gov.hk/file/planning_studies/comp_s/hk2030plus/document/Planning%20and%20Urban%20Design%20for%20a%20Liveable%20High-Density%20City_Eng.pdf
   - 访问：2026-10-23，HTTP 200，PDF，约 9.40 MB，82 页。
   - 用途：高密度城市的建筑形态、公共空间、街道生活、塔楼与平台关系。
   - 对本项目的要求：
     - 高密度不等于均匀复制塔楼；
     - 中心区应有塔楼、裙房、街墙、街角公共空间和不同进深的建筑；
     - 建筑面积、覆盖率和建筑数量必须分别核查；
     - 中心密度应向更新区和新区渐退，并与道路、市场和公共空间共同组织。

7. **Hong Kong Planning Department, Hong Kong Planning Standards and
   Guidelines (HKPSG)**
   - URL: https://www.pland.gov.hk/pland_en/tech_doc/hkpsg/
   - 访问：2026-10-23，HTTP 200。
   - 页面说明 HKPSG 是确定各类用地与设施规模、位置和场地要求的政府
     技术手册。
   - 用途：学校、市场、公共设施与人口/岗位需求的规划关系。
   - 限制：具体设施的选址标准应按设施章节逐项核对，不能用一个统一半径
     代替。

### 3.4 可复用三维资源

8. **Quaternius, Ultimate Buildings Pack**
   - URL: https://quaternius.com/packs/ultimatetexturedbuildings.html
   - 访问：2026-10-23，HTTP 200。页面标注 76 个模型，FBX/OBJ/Blend
     格式并带贴图。
   - 仓库证据：
     - `assets/third_party/quaternius/README.md`
     - `assets/third_party/quaternius/buildings/NOTICE.txt`
   - 用途：作为理解低多边形建筑体量、屋顶、门窗比例的参考。
   - 限制：本地四个 OBJ 是派生导出；精确来源和下载日期仍记录在 NOTICE
     中。不得把上游原始资产包再次作为独立素材包分发。

9. **Quaternius, Quaternius Asset License v1.0**
   - URL: https://quaternius.com/license.html
   - 访问：2026-10-23，HTTP 200。当前页面标识 QAL v1.0。
   - 用途：资源合法性与再分发限制核对。
   - 本项目策略：继续优先使用仓库内已注释来源的资源；新增资源必须先
     写入 registry 和 provenance notice，不能只复制模型文件。

10. **Quaternius, Stylized Nature MegaKit**
    - 仓库说明：`assets/third_party/quaternius/README.md`
    - 用途：树、灌木和岩石的低多边形自然表现参考。
    - 限制：当前只选择了一个 tree、bush、pebble 进入预览 MVP；不能让
      所有植被都退化为黑色球体或竹竿树。

## 4. 对当前视觉问题的可执行建议

### 4.1 地面上的不规则白色/三角碎片

优先级：P0。

- 道路、车道线、人行道和地形必须来自同一个设计面；
- 有 Rust 设计高程时，渲染端不得再对左右边分别取样地形后取最高值；
- 交叉口应有连续的交叉口模板，不能由圆裁剪任意打断车道线；
- 弯道连接应使用有界 miter/圆角过渡，禁止出现折叠三角面；
- 白色碎片若属于车道线，必须有端部封口和最小可见长度。

### 4.2 草场呈圆斑

优先级：P1。

- 草群分布应先由坡度、朝向、湿度、土壤、林缘、路径和放牧压力形成
  不规则斑块；
- 噪声只能调制边界，不能作为唯一生成器；
- 城市中的草坪应被建筑、道路、围墙和步道切割，不能留下统一的圆形岛；
- 树、灌木和草丛必须分别有尺度、地表与禁区合同。

### 4.3 不够现代、中心土地利用率异常

优先级：P0。

- 中心区优先形成连续街墙和混合用途裙房，不应只有稀疏独立盒子；
- 建筑宽度、进深、层数、屋顶和立面必须随 archetype 与地块变化；
- 中心应有办公塔楼、商业裙房、街角店铺、市场、公共建筑和住宅混合；
- 旧城窄临街面、更新区中等街廓、新区大型地块应使用不同的地块与退让；
- 建筑不可用固定实体尺寸套所有地块；尺寸是规划输出，并须通过 OBB、
  入口、贴地、道路和工程审计。

### 4.4 道路没有车道/高速概念

优先级：P0。

- 至少区分高速/快速路、主干路、集散路、本地路和巷道；
- 每条道路发布 laneCount、laneWidth、median、sidewalk、speedLimit、
  roadClass 和 collisionHeight；
- 高速/快速路应有中央隔离、连续边线、较大曲线半径和互通/匝道逻辑；
- 本地路不应被强行画成高速断面，巷道也不应绘制完整双黄线；
- 绘制资源从道路等级选择，而不是“手上有什么材质就生成什么道路”。

### 4.5 建筑旋转、碰撞和绿植穿墙

优先级：P0。

- Rust 发布 yaw 是唯一 OBB 方向；
- Three.js root 必须直接使用同一 yaw，需要朝向道路时只旋转子节点；
- 建筑的侧立面、背立面和屋顶必须属于同一几何体合同；
- 屋内的绿色平面不是庭院，除非它是独立且通过 OBB 外检查的 Rust 实体；
- 建筑与道路、建筑、树、灌木、岩石、水体、台地边界必须做最终 footprint
  相交检查；
- 搬家或缩放建筑后，必须重新检查水体、坡度、填挖上限、入口和流式分块
  边界，不能沿用原地块的结论。

### 4.6 树和水体

优先级：P1。

- 树冠宽度必须与环境尺度一致，不能把树干直径当树冠；
- 树、灌木、草丛和岩石的地表许可与禁区必须分开；
- 水面应水平或有明确河床坡降，不能跟随每个地形格起伏；
- 岸线需要有湿边、浅水、护岸或自然滩涂过渡；
- 水体内部不得生成陆生实体，桥墩、码头和芦苇必须是明确例外。

## 5. 不建议直接采用的“网络参考”

- 不直接复制参考项目的地图、建筑轮廓、纹理或 UI；
- 不把单张概念图的透视比例当作米制数据；
- 不用未见正文的搜索摘要替代官方页面或 PDF；
- 不以 OSM 建筑数量作为硬编码生成目标；
- 不用游戏截图证明测绘精度；
- 不把 3D 模型包中的透视展示图反推为碰撞尺寸。

## 5.1 2026-10-23 补充网络核对

本轮重新核对了几类与本项目最接近的公开实现/资产来源。只采用“可验证页面
存在且用途明确”的部分；没有把展示截图当尺度证据。

| 来源 | 实测 | 可用于本项目 | 不能直接照搬 |
|---|---|---|---|
| Three.js Terrain Geometry<br>https://threejs.org/examples/webgl_geometry_terrain.html | HTTP 200 | 高度栅格 + 顶点法线的连续地表做法；说明渲染应消费同一高度场 | 该示例没有城市、道路横断面、碰撞或真实地貌语义 |
| Three.js Ocean Shader<br>https://threejs.org/examples/webgl_shaders_ocean.html | HTTP 200 | 水面法线、波幅、受光与水平基准的表现方式 | 不是河流/潮汐/岸线模型，不能替代 Rust 水位与河床 |
| Kenney City Kit Roads<br>https://kenney.nl/assets/city-kit-roads | HTTP 200 | 模块化道路、交叉口、路缘的造型参考 | 模块拼接不能决定世界中的车道数、转弯半径和碰撞 |
| Quaternius<br>https://quaternius.com/ | HTTP 200 | 风格化建筑与自然资产的比例/色块参考 | 展示图透视不能反推真实 width/depth/height |
| Khronos glTF Sample Assets<br>https://github.com/KhronosGroup/glTF-Sample-Assets | HTTP 200 | 模型坐标、材质和导入约定 | 通用示例资产不包含本项目资产合同与碰撞元数据 |
| HKSS Natural Terrain<br>https://hkss.cedd.gov.hk/hkss/eng/natural_terrain.aspx | HTTP 200 | 山体、边坡、地表水与城市接触面的工程关注点 | 不是地图生成算法 |
| Highways Department Structures Design Manual<br>https://www.hyd.gov.hk/en/technical_references/technical_document/structures_design_manual_2013/index.html | HTTP 200 | 桥梁/高架结构的设计语境 | 不把手册默认值直接复制到游戏碰撞 |
| Poly Haven Models<br>https://polyhaven.com/models | HTTP 200 | 自然岩石、植被、建筑表面参考 | 资产许可和最终模型仍要逐项登记 |

由此得到的实现约束：

1. 地形、路面、水岸和碰撞必须共享 Rust 设计数据，Three.js 示例只提供
   表现方法；
2. 道路模块和交叉口应先来自路线/车道/碰撞合同，再选择模型；
3. 山地城市优先考虑自然等高线、台地和高架/挡墙组合，不能把陡坡直接铺平；
4. 水面使用独立水平面或明确河床纵坡，不跟随每个地形格子起伏；
5. 建筑和自然资产按米制契约缩放，不能按截图或模型单位猜测。

## 6. 建议的下一轮取舍

1. 先修复道路设计面、交叉口和车道连续性；
2. 再建立中心—更新—新区—郊外的分带建筑形态和尺寸变化；
3. 补齐山体、水体、岸线、树群的显式语义合同；
4. 使用 48 城样本生成密度曲线和尺度区间，而不是生成固定建筑数；
5. 只有通过 `scale-audit`、最终实体审计、AccessGraph、市场辐射和运行态
   截图后，才允许把结果称为“高密度核心区”。

