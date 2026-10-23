# 三城独立切面 r249–r252

三城都使用独立 512m Manifest 和工作目录，一次只烘焙/检查一座城，避免同时
加载三个大世界。

## 旧金山：金门大桥南岸

- Manifest：`examples/san-francisco-golden-gate-south-512.manifest.json`
- crop：Fort Point / Presidio 南岸桥头 512m 窗口；
- 公开来源：Golden Gate Bridge District、NPS Golden Gate National
  Recreation Area、OpenStreetMap；
- 比例：金门大桥主跨约 1280m，不能塞进 512m，因此只表现南塔与引桥局部；
- 水：400→240m 局部海峡切面，深水、强风浪；
- 结果：232 实体，2 地标，建筑碰撞 0，入口断连 0，`scale-audit=pass`；
- 视觉：天空视角能辨认海峡、桥面、双塔、南岸道路与 Presidio 绿地；
- 预览：`http://127.0.0.1:8250/preview/index.html?view=skyline&build=sf-r254`。

`quality-report=warn`，原因是 structured-water 场景尚未执行完整 land-use
surface realization；不影响硬碰撞/水体/入口审计，但不能冒充 fully-realized
B/C 景观。

## 上海：外滩剖面

- Manifest：`examples/shanghai-bund-512.manifest.json`
- crop：外滩历史街墙、滨水步道和黄浦江西缘的 512m 窗口；
- 公开来源：Britannica The Bund、Shanghai municipal portal、OpenStreetMap；
- 水：300m 黄浦江切面，河流方向 `world_z`；
- 结果：455 实体，2 地标，建筑碰撞 0，入口断连 0，
  `scale-audit=pass`、`quality-report=pass`；
- 规划：6,000 户、16,000 岗位、120,000m² 零售；旧城 34%、更新 35%、
  新区 25%、外围 6%；
- 视觉：天空视角可见西岸聚落、弯曲主路、河岸和局部跨水桥；
  街景视角可见连续道路、两侧建筑和路口；
- 预览：
  - 天空：`http://127.0.0.1:8251/preview/index.html?view=skyline&build=sh-r254`
  - 街景：`http://127.0.0.1:8251/preview/index.html?view=street&build=sh-r254`

## 贵阳：阿哈湖湿地广场走廊

- Manifest：`examples/guiyang-ahha-wetland-512.manifest.json`
- crop：阿哈湖边缘湿地、广场与低密度聚落的 512m 窗口；
- 公开来源：Guiyang municipal portal、Baidu Baike 阿哈湖、OpenStreetMap；
- 地形：`plateau-basin`、regional relief 520m、karst hint；
- 水：220×168m 湖湾；道路严格走 route-evolution 跨水合同；
- 结果：906 实体，1 地标，建筑碰撞 0，入口断连 0，
  `scale-audit=pass`、`quality-report=pass`；
- 视觉：天空视角可见湖湾、环湖道路、坡地聚落、树林和岩丘；
- 预览：`http://127.0.0.1:8252/preview/index.html?view=skyline&build=gy-r254`。

## 后续

- 三城目前是核心机制切片，不是完整城市复刻；
- 旧金山需要继续完成 structured-water 的 land-use realization；
- 上海需要从当前 455 实体继续补充历史街墙、里弄和滨江连续公共界面；
- 贵阳需要把程序化建筑立面替换为更明确的湿地服务设施和低层旅游建筑；
- 三城下一轮都必须先过 `scale-audit`，再逐城截图，不能一次并行烘焙。
