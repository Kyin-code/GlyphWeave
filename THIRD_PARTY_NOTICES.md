# GlyphWeave third-party notices

本文件记录当前仓库中已确认的第三方代码和资源状态。GlyphWeave 自有代码继续使用 MIT 许可证；本文件不把第三方资产、算法参考或未来候选依赖重新许可为 MIT。

## 当前状态

截至 2026-09-06：

- `Mapgen4`、`symbios-ground`、`WorldEngine` 和 `FastNoiseLite` **没有作为 Cargo 依赖集成**。
- 当前 Rust 自然地形代码是 GlyphWeave 自己的实现，其中部分设计受到这些项目启发。
- 仅有“参考思想”不等于复制源码，也不触发将上游代码放入本仓库的再发布义务；如果未来复制、改写或 vendoring，必须为具体版本补充版权、许可证、NOTICE 和修改说明。
- 任何新增第三方依赖必须锁定版本/commit，并在 CI 和发布清单中复核其完整依赖树。

## 项目和候选来源

| 名称 | 当前关系 | 许可证处理 |
|---|---|---|
| Red Blob Games Mapgen4 | 算法思想参考；未集成源码 | 如未来复制或改写，保留 Apache-2.0、版权、NOTICE 和修改说明 |
| TheJanusStream symbios-ground | terrain carving 思想参考；未集成源码 | 在确认具体仓库版本许可证后再决定；不得仅凭博客或注释复制 |
| Gapi34 WorldEngine | 研究参考；未集成源码 | 未来引入前核对仓库版本、许可证和依赖 |
| Auburn FastNoiseLite | 候选噪声依赖；当前未引入 | 引入时保留其许可证和版权文件，并限制其职责为噪声 |

## 已存在资源

`assets/third_party/quaternius/stylized-nature/License_Standard.txt` 是随自然资产一同分发的上游许可证文件。模型、纹理、字体和代码必须分别按各自许可证处理，不能因为 GlyphWeave 代码是 MIT 就改变这些资源的授权条件。

## 引入第三方代码的最低要求

1. 记录上游仓库、版本或 commit、下载日期和原始许可证路径；
2. 保留版权、许可证和 NOTICE 文件；
3. 对改写文件注明修改；
4. 不把上游类型直接暴露给 GlyphWeave 的世界合同；
5. 添加 wrapper、许可证扫描和最小适配测试；
6. 在发布包和源码包中都能找到对应通知；
7. 如果许可证或依赖关系不清楚，停止引入并改为只参考算法思想。

## r44 HTML 派生读取（2026-09-07）

本轮没有新增第三方代码/资产包，也没有集成候选地形仓库源码。现有 `serde_json` 1.0.150 启用 `float_roundtrip` feature，以保持 Rust authored f64 数值的 JSON 往返；锁定版本未变，Cargo.lock 未变。该锁定包自身 Cargo.toml 声明 `MIT OR Apache-2.0`，feature 为不追加依赖的空列表。发布仍须保留所选授权要求的上游许可与版权通知，本条不是全项目发布合规认证。

实现与精度回归证据见 `G:\difyllmwiki\GlyphWeave\docs\r44-authoritative-render-stream.zh-CN.md`。
