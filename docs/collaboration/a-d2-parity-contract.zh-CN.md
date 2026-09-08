# A-D2 / A-D3 共享的紧凑独立parity合同

日期2026-09-08。本文件由总控冻结输入输出接口，不是已生成的真实证明。Luna D2消费，后续D3生产；D3不得仅调用projection writer然后拿自己输出互比充当独立验证。

## JSON（v1，示例字段不是实际证据）

顶层：format="glyphweave.render-parity"，version=1，status="pass"或"fail"，worldRevision:string，worldFile:{path:absolute,sha256:hex64}，scenes:[]。
每scene：sceneId、widthM、depthM、originX、originZ、chunkSizeM、source:{path:absolute,sha256,bytes}、header:{path:absolute,sha256,bytes}、uniquePlants、uniquePatches、chunks:[]。
每chunk：key="chunkX:chunkZ"、chunkX、chunkZ、worldX、worldZ、validWidthM、validDepthM、file（与header descriptor相同的相对路径）、sha256（chunk完整字节hash）、bytes、patches、plants、plantIdsSha256、patchIdsSha256。

IDs摘要算法统一为：按UTF-8字节字典序排序原字符串ID，JSON紧凑数组（不额外空格/换行）的UTF-8字节做SHA256；目前ASCII ID与JS默认sort一致，Unicode ID如实现未明确一致则不默默接受。重复ID不得排序后去重掩盖。

## 独立性

D3必须从权威scene解析得到期望patch/plant完整对象，然后逐chunk比较派生对象全部字段、覆盖、边界和hash；header中entities/chunks/accessSurfaces等被投影字段也同权威比较。源scene及header/world文件hash绑定，pass不代表semantic quality gate通过。
不得一次JSON.parse(readFile(full-scene,'utf8'))读超大源；采用Rust serde_json::from_reader等非整字符串路径或经验证的流式读取。基于Rust权威对象内存解析可接受但须记录峰值，不宣称常数内存。应拒绝截断/缺块/旧hash/错scene/缺漏/跨块重复植物/篡改投影；不放宽边界、不要用producer内部chunk_range原样充当独立预言。

## D2消费

本地受限读取parity，验证上述format/version/status及结构；校验目标HTTP world.json字节hash/worldRevision及目标scene preview-scene.json字节hash与parity相符，校验header descriptor集合与parity对应字段。完整场景维度、origin、partial chunk来自该绑定。旧r47无world/source绑定的parity不能标完整pass；可以明确legacy/incomplete用于调试但不正式验收。

浏览器实际实例ID、loaded patch ID/计数、缓存统计与parity逐块比对。不能只验证fetch的chunk自带hash而跳过独立parity；chunk请求自身hash与parity一致是传输完整性，不自动证明源对象正确。每次回访必须等当次navigation完成，旧feedback不能过关。

parity文件真实来源/producer runtime/命令退出码由主Agent在工具外确认并写验收收据，不凭status字符串认证。D2可用完整合成正向做单元测试；真实parity未生成时integration_pending，不伪造现存世界证据。

## 大文件脚本迁移队列

当前scripts/check-render-stream.mjs、check-maximum-world.mjs、check-frontage.mjs仍有整scene JSON.parse路径。A-D3需用安全读取接口替换或将超预算输入明确拒绝并委派Rust等价核对；不得只删验证项。原脚本不是当前最大图完成依据。

## 发布前补充（首轮主Agent复验）

每chunk新增nonMeshPatches与nonMeshPatchIdsSha256（仅源material=access_pavement），accessSurfaces与accessSurfaceIdsSha256（规范ID为字符串access:加JSON紧凑数组[sceneId,entityId,anchorId]，按保守vertices AABB与chunk相交规则分配），对应浏览器实际access mesh名称；未知豁免不得来自观测自报。缺字段incomplete。

后续完整A-07还需非生态实体表示覆盖与高程来源绑定，首轮D2未覆盖须显式incomplete/coverage_pending，不能把植物/patch通过说成所有实体/高度通过。
