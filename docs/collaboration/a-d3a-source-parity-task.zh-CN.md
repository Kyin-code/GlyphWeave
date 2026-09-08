# A-D3a：Rust权威读取与独立投影parity（排队，收到派发才开工）

日期2026-09-08；Luna承担原v4f D3，主Agent持续验收到里程碑A。唯一仓库G:/difyllmwiki/GlyphWeave；F:/modenmapbox禁止写。Terra当前A-T3占worldgen.rs，D3a不得修改该文件。

## 目的与写集

解决A-06的Node整scene字符串风险，并为A-D2生成当前世界独立来源证据。不是再造地图，也不是拿producer输出互相比对冒充独立审核。

允许：bevy/crates/core/src/preview_stream.rs（仅引入audit模块/必要导出，不改现有writer分区规则）；新增bevy/crates/core/src/preview_stream_audit.rs；bevy/crates/cli/src/main.rs（只新命令接线与场景读取from_reader安全替换，不放宽gate）；新增bevy/crates/cli/tests/preview_parity.rs；scripts/check-render-stream.mjs、check-maximum-world.mjs、check-frontage.mjs（受限读取/消费经过绑定的紧凑audit输入）；可新增scripts/collaboration/audit-input.mjs及.test.mjs；docs/collaboration/a-d3a-result.zh-CN.md；bevy/work/collaboration/luna-d3a/。

禁止worldgen.rs、settlement、Luna D2 HTML、A-D1工具、manifest、资产/依赖/许可证（D3b另队列）、其他文件。新模块通过已有preview_stream #[path] mod引用，不需改lib.rs。Git写操作全部禁止。

## CLI接口

新增 glyphweave audit-preview-stream WORLD_DIR OUT_DIR。OUT_DIR必须全新、在只读world树外；canonical/realpath/父路径约束，拒覆盖。成功写OUT_DIR/parity.json及每scene一个紧凑audit-source文件；失败退出非零并只在OUT_DIR留下明确fail报告，不修world/投影、不得给失败数据生成status=pass。

parity精确遵循docs/collaboration/a-d2-parity-contract.zh-CN.md（包括首轮nonMesh/accessSurfaces补充），D2消费新版，不用旧r47格式。

## Rust独立验证

1. world/index/header小JSON有明确预算（64MiB）；大scene使用serde_json::from_reader(BufReader)，不能read_to_string或serde_json::to_string整scene。typed对象占用内存可以，但明确不是常数内存。推荐HashingReader使source bytes/hash对应实际解析字节；边读边hash并校验EOF/截断，严禁事后hash另一份内容充当本次输入。
2. 验证scene/root/manifest身份、scene/chunk尺寸和完整coverage、源entities/patch/plant重复ID、非法值、路径逃逸。源为原始SceneIndex，不从preview header反建源。
3. preview-scene导出字段逐项与源核对（entities/chunks/accessSurfaces/landUseZones/landUseBudget/必要roadNetworkPlan与landcover summaries等），并且worldRevision一致；检查所有已有导出字段，不把漏验字段默默标全通过。
4. 每个render chunk单独受限读取+hash。完整patch和plant对象与源逐字段相等；独立按chunk矩形与源patch AABB求预期集合，不直接调用writer chunk_range当预言；按half-open源植物位置求归属，跨块plant重复/遗漏拒绝。patch边界沿用实际producer关闭/半开组合精确规则，必须先读原check-render-stream脚本与writer解释，不臆改producer。
5. source/header/world/chunk hashes及每chunk sorted UTF8 ID SHA256产出合同字段。nonMesh仅source material=access_pavement，实际accessSurface名称按access:紧凑JSON[sceneId,entityId,anchorId]，期望与chunk保守verticesOverlapChunks重叠的源surface集合一致。
6. 每chunk追加heightFile、heightBytes、heightSha256，校验GWHM magic/version/dimensions/精确payload长度；naturalHeightFile及其hash/bytes同样记录。供后续D2绑定实际请求的高度来源。不要把高度传输校验冒充全几何物理审核。

## 紧凑audit-source

从已验证源仅导出旧Node几何脚本需要的数据：sceneId/width/depth/origin/chunkSize/chunks/entities/roadNetworkPlan/landUseZones/accessGraph/accessSurfaces，以及ecology plants/community/rejections和agriculture fields、patches的精确计数摘要。完整源hash/bytes、worldRevision绑定；parity中记录auditInput文件path/hash/bytes。不含全量生态plants或landcover patches数组，不用删原scene内容实现小文件。

check-frontage在显式传入audit-dir时读取经过parity绑定的auditInput，继续原pad/道路station/入口/高度采样几何核验。check-maximum-world继续完整height覆盖/长度检查与精确源计数。check-render-stream可调用新Rust独立核验或消费由总控受信执行的parity，但不能只stat旧parity便声称刚做独立审核。

兼容：旧2参数脚本对小source允许<=64MiB有限读取；超过预算明确拒绝并给出audit-preview-stream命令及--audit-dir用法。不默默跳过大图检查、不再调用JSON.parse(readFile(full-scene))无预算路径。新审核路径把源与派生绑定做清楚，工具外producer命令/runtime真实性仍由主Agent记录，不做伪签名。

## 测试与资源

小Rust fixture及CLI集成覆盖合法2块/partial chunk/负origin、源或chunk截断、header实体篡改、patch/plant内容改且重算chunkhash、跨块漏/重复plant、旧revision/hash、invalid height、路径/junction逃逸、out覆盖/在world树内。至少一项证明producer导出通过后独立validator能发现同步更新header descriptor的恶意投影修改。
Node helper测试预算拒绝、旧小图兼容、compact input绑定和源hash不匹配、错误缺少audit字段。

Rust RUN槽须等Terra通知释放且主Agent明确授予；初阶段可实现/Node静态，不自己竞争cargo。只精准core/CLI测试，不完整workspace/release/最大图，不启动server、不装依赖/新模型。真实r49的独立parity由主Agent使用验收后runtime串行生成；你不要擅跑443MB源来试错。

完成交候选hash/命令/覆盖/未验证项。A-D3b许可证完整库存与notice包装随后派发，不将npm已安装manifest列表当lockfile完整清单。main review/license-primary已有three r178官方LICENSE供后续只读；本轮不抢worldgen包装模块写锁。
