# r41：显式田块长宽与农村分区尺寸程序

日期：2026-09-07。权威仓库：G:\difyllmwiki\GlyphWeave。

## 1. 要解决的问题

r40所有田块均固定24×24m，耕作面16×16m。这既限制了不同农村形态，也让面积审计绑定到“田块数×256”。本轮增加明确的尺寸意图，并将面积、候选间距和入口计算推广到矩形；不是为了提高gate通过率而自动把田块缩小。

## 2. 输入合同

示例文件：examples/multiscale-farms.manifest.json。保留原样例seed42的人文需求，新增：

    "agricultureProgram": {
      "version": 1,
      "scenes": {
        "scene-0": {
          "defaultField": { "widthM": 32, "depthM": 24 },
          "zoneFields": {
            "scene-0.zone.rural.6": { "widthM": 48, "depthM": 32 }
          }
        }
      }
    }

该对象位于style内。zoneFields只引用Rust实际生成的农村zone ID：Agent/用户先查看分区，再表达尺寸偏好，不能靠伪造zone替代核心所有权规划。

- 长宽分别为16..128m，4m的整数倍；只是当前算法范围，不是农业/建筑法规。
- 未提供agricultureProgram时维持24×24m旧行为。
- 显式程序必须version1，1..64个scene；每scene最多256个分区覆盖。
- 拒绝未知字段、未知scene、未知/非农村zone、非法长宽、naturalOnly冲突和缺少landUseProfile。
- 不支持road-first规划的场景不得静默忽略程序。
- 不尝试从48×32自动降级24×24；若来源余地容不下尺寸及边界缓冲，写source_too_small_for_requested_field_and_margin拒绝记录。

## 3. Rust-first数据流与重构

新增 settlement/farm_program.rs，负责反序列化、尺寸/场景/分区校验和源绑定。

    manifest.style.agricultureProgram
      -> FarmProgram / FarmPattern / FieldTemplate
      -> LandcoverPlan.farmPattern
      -> agriculture::realize
      -> SurfaceRealization / SurfaceAudit

LandcoverPlan旧API保留默认None；新增plan_landcover_with_program用于实际bake。farmPattern是可回放的规划输入，不是产出已经实现的声明。

CLI在独立重建之前，先将plan.farmPattern与权威manifest对比。即便同时修改scene与landcover-plan.json，也不能用两份相等的假程序绕过原输入。

低层audit_landcover检查所有权派生与程序结构；它本身不持有manifest，不能替代CLI的源绑定。调用者提供不同合法程序时必须视为不同输入，而不是修改结果文件。

生成器revision：worldgen-2026-09-07-r41-explicit-field-dimensions；农业model：explicit-rectangular-field-access-v3。旧输出需对应旧CLI验证，新输出fresh bake。

## 4. 矩形生成几何

- 候选网格间距为宽+4m、深+4m；外边界预留2m。
- 农业田块固定保留4m环状地头，因此耕作矩形为(width-8)×(depth-8)。
- 四个候选入口由四边中点向内2m计算，不再用固定12/22偏移。
- 仍使用田面全1m单元12%坡度上限、4m宽接入/8%坡度/160m长度限制，以及r40有界绕行。
- 路径、地头、耕作内部、已有农地/保护地/水体/实体约束均保留。
- 不执行新增挖填土、不修改自然或最终高度。

例如48×32田块：总面积1536m²、耕作40×24=960m²、地头576m²。SurfaceAudit独立重建mesh并对照这些实际矩形面积，不再使用每块固定256/320常数。

较大田块在坡地和碎片化所有权中可能实现更少面积。这是用户指定尺寸的真实代价，不应通过暗中换成小田块或改变门槛来隐藏。

## 5. 测试

- 非法尺寸、未知字段、缺版本、null程序、未知scene/zone、naturalOnly等拒绝。
- 默认24×24兼容；32×24与48×32混合分区尺寸精准执行。
- 核心fixture确认48×32真实网格面积960/576、所有道路长度仍受限。
- 128×128在小所有权片区放不下时无田块/网格且存在拒绝，不自动缩小。
- CLI真实多尺度样例验证两种尺寸都存在，面积不再等于田块数×256。
- 同时伪造两份计划的默认长宽、删掉程序，或改田块结果尺寸，分别被manifest源绑定/独立几何重建拒绝；恢复后通过。
- 旧CLI回归中原独立重建错误文字合同保持不变，不修改测试预期来消除失败。

## 6. 尚未完成与来源

本轮没有实现自动优化经营面积、根据地形自动选择多尺度尺寸、坡地整地、梯田、灌溉/排水、土壤适宜性或地块经营需求。显式长宽可控是这些工作的一步，不是完整农业系统。自然地貌深化、城市剩余开发、最大60km²性能和最终视觉仍属于原始目标。

无新增第三方代码、依赖或资产；新增领域合同与几何参数化均为项目自有实现。未改变Mapgen4/symbios-ground等候选依赖的许可证状态。

## 7. 实测结果

中断后复核最终文件与日志，以下结果来自r41最终输出，不是初始实验。

- 多尺度输出：bevy/work/farm-r41-final-multiscale；同seed42默认尺寸输出：bevy/work/farm-r41-final-legacy42。
- 17个32×24m田块、1个48×32m田块；耕作面7488m²、地头7104m²、农道2504m²；6条绕行。
- 默认无程序的29个田块与r40的田块记录和全部mesh逐项相同；显式程序没有改变既有实体、生态记录；自然和最终高度逐字节相同。
- 同seed42默认小田块的农村未实现144600m²；指定较大田块后147788m²。增加3188m²未实现面积，是尺寸选择的真实代价，不称为覆盖率改善。
- 最终Rust workspace：327 Core、44 App、6 CLI单元、20 CLI集成，共397通过。日志bevy/work/r41-workspace-final-tests.log。Node34、Vitest170通过；lint退出0但有既有warning，typecheck和doc-tree通过。
- 两组fresh scale/rules审计通过；设施4/4、住宅18/18；完整world-quality-gate均实际退出1、fail。多尺度总未实现163920m²，完整目标未完成。
- 浏览器检查18田块、96个农业patch、6条绕行；顶点/索引核对通过、页面错误0，截图multiscale-farm.png已实际查看，视觉仍未最终验收。
- 证据为多尺度输出目录r41-evidence.json和multiscale-farm.png.json。
- 预览端口8861，独立runtime：bevy/work/r41-preview-runtime/glyphweave.exe。
