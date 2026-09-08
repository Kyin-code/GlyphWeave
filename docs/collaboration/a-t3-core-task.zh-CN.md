# A-T3-core：稳定候选ID与显式存量审计

日期2026-09-08，里程碑A持续授权。唯一根G:/difyllmwiki/GlyphWeave。主Agent已审查：当前多个optional生成路径在过滤/碰撞之后用serial分配ID（worldgen.rs约5874/5980/6357/6906/7275/7334/7373/7500/7584/7657等），新增前面的无关对象会产生身份风险。WorldPatch目前仅显式操作landmarks；不能把从零重生成冒充保存已有世界。

## 写集

Terra仅可改bevy/crates/core/src/worldgen.rs中的候选ID创建点/窄集成与对应测试；新增bevy/crates/core/src/worldgen/entity_identity.rs并在worldgen通过path mod引入；docs/collaboration/a-t3-result.zh-CN.md；bevy/work/collaboration/terra-t3/。access.rs和原CLI/HTML全部只读，Luna正在D2，未来D3可能写CLI；不要抢写。需要新pub re-export放worldgen已有边界，不改lib.rs。

附带先补T2的一个ignored真实世界等价test，允许仅在ecology.rs测试模块新增，保留已验收生产不动：环境变量指定WORLD_DIR，BufReader读SceneIndex/manifest，逐chunk读高度重建BakedTerrain；在同一输入源patch/entity上独立linear/indexed生成+verify，完整序列化相等与原已发布ecology一致（实际输入相同才断言）；报告counts/hash/计数/时间，缺参数显式失败，#[ignore]防默认测试跑大图。不运行它，不改变原T2算法。明确调用命令给总控。

## 设计冻结与实现要求

先用简短方案说明哪些分配点、候选key和迁移影响，然后实际实现，不只写报告。
1. 显式authored实体、规划request/site ID保持；不能以generated前缀辨认authored身份。可选生成ID来自固定候选来源key（scene命名空间、planner namespace、候选网格/slot及kind），不依赖输出Vec顺序/成功计数、全manifest hash或无关需求数量。key在约束过滤/工程挪位之前确定，不能在最终坐标上统一事后改名导致引用错配。
2. 重复候选key/重复ID fail closed，不用追加serial掩盖冲突。默认新生成采用明确stable-v1方案并更新WORLD_GENERATOR_REVISION以标明语义变更；不能改natural height算法、密度/预算/候选位置/失败原因来配合ID。旧已保存场景不自动改写ID。
3. 保留显式实体、道路/site/访问引用。若ID在原逻辑被用于几何/seed而迁移会改变生成结果，必须报告并先局部处理，不盲改全局然后降低既有测试。旧依赖确切serial-ID的测试可按已文档化新方案迁移，但几何/需求/失败期望不弱化。
4. 在新模块实现Rust存量审计API，输入old/new最终实体和明确允许的修改列表；逐ID列preserved/added/moved/updated/removed（分类规则固定），重复ID、未授权移除/移动/修改必须失败。allow列表精确ID+change kind，不wildcard、不默认全部允许；记录具体原因，不仅计数。不将字段全hash相同当成几何/规则审核。函数不改输入，不删除对象。
5. 区分fresh generation（无旧存量）与preserving编辑/重生成。当前CLI apply-patch只有manifest无法证明既有生成对象保留，必须在文档明示未集成，提出BASE_WORLD+explicit change approval的接口。CLI wiring与门禁挂接待总控后续单独授锁，不能只交库API便宣称A-03全部通过。

## 验证

小fixture：同输入相同ID/实体；新增空间上无关的authored或optional请求不重排已有候选身份；前序候选被拒不重编号后续；两个不同场景namespace；身份审计对未授权删/移/改/duplicate拒绝、精确授权通过并保留diff；serialize roundtrip；原patch显式操作语义不回退。

报告新增ID与旧ID的迁移边界：旧世界读取可继续，旧世界与新scheme不可静默当同一存量；必须明确unsupported/migration-required或精确迁移表，不假装继承全部旧身份。无需实现完整编辑器。

## 资源和停止

你获独占窄Rust测试槽，只执行对应core测试，test-threads=1；禁止完整workspace/release/最大图/new server。不运行ignored真实T2测试。Git禁止任何写操作，不碰F:/modenmapbox，不改Luna/旧A-D1/原world数据，不新增依赖/模型。review目录只读，不执行会写review的脚本。

遇跨文件冲突、某legacy生成路径key无法保持正确引用时交最小复现与方案给总控，不无限扩大重构。完成返回ready_for_review及hash/命令/精确覆盖/未集成CLI清单，主Agent会继续安排直到A完整，不需你自行派新任务。
