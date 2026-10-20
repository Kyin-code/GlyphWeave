# GlyphWeave third-party notices

本文件记录当前仓库中已确认的第三方代码和资源状态。GlyphWeave 自有代码继续使用 MIT 许可证；本文件不把第三方资产、算法参考或未来候选依赖重新许可为 MIT。本文件是发布工程盘点，不是法律意见，也不代表已完成法律合规认证。

## 2026-09-08 锁文件库存

机器可读证据：

- `bevy/work/collaboration/review/release-license-inventory.json`
- 生成命令：`node scripts/collaboration/license-inventory.mjs --out bevy/work/collaboration/review/release-license-inventory.json`
- `bevy/Cargo.lock` SHA256：`D6A21933A42ABD3E99842FACC7AAF4120FA386E5657C340C63227C5ABFF9C804`
- `pnpm-lock.yaml` SHA256：`A3AE8F0235DA396A09DEEC199308965EA22BB3FE3F106697797C3EFBB51388F6`

### Rust

- `bevy/Cargo.lock` 共 518 个包；`cargo metadata --locked` 解析到 518 个包，缺失 0。
- 主要许可证表达式：`MIT OR Apache-2.0` 287、`MIT` 97、`Apache-2.0 OR MIT` 34、`MIT/Apache-2.0` 20、`Apache-2.0` 13，其余为 Zlib/BSD/Unlicense/MIT-0/CC0/ISC/OFL/LLVM-exception 等。
- 含 GPL/LGPL 选项的 3 个包均为双/多许可，可使用非 copyleft 选项：`r-efi@5.3.0`、`r-efi@6.0.0`（`MIT OR Apache-2.0 OR LGPL-2.1-or-later`）、`self_cell@1.2.2`（`Apache-2.0 OR GPL-2.0-only`）。
- 未发现只有强制 copyleft 单一许可的 Rust 锁定依赖。

### npm / pnpm

- `pnpm-lock.yaml` 的 `packages:` 段共 347 个锁定包。
- 当前平台已安装并可解析许可证元数据的包为 224 个；`pnpm licenses list --prod --json` 与 `pnpm licenses list --dev --json` 分两次执行，避免整表因缺少可选包索引而失败。
- 122 个锁定包是当前平台未安装的可选原生包（esbuild/sharp/lightningcss/oxlint/rolldown/tailwindcss 等非 Windows x64 变体或 fsevents）。
- `@cloudflare/workerd-windows-64@1.20260701.1` 已解析为 **Apache-2.0**：
  `npm view ... license` 返回 `Apache-2.0`；下载的
  `cloudflare-workerd-windows-64-1.20260701.1.tgz` 的 `package/package.json`
  也声明 `"license": "Apache-2.0"`，npm shasum
  `5bb96eae407ec4dcf927656c63dadaf99eaf3e9c`。tarball 内没有单独 LICENSE
  文件，因此发布时必须以 `https://github.com/cloudflare/workerd` 的 Apache-2.0
  文本作为上游许可依据（2026-10-20 读取 `main/LICENSE`，长度 10,174 字节）。
  证据见 `docs/collaboration/rk-x/C-L1-WORKERD-20261020.zh-CN.md`。
- 含 copyleft 表达式的已解析包：`lightningcss@1.32.0` 与 `lightningcss-win32-x64-msvc@1.32.0` 为 `MPL-2.0`（构建工具链）；`@img/sharp-win32-x64@0.34.5` 为 `Apache-2.0 AND LGPL-3.0-or-later`（可选原生图像处理包）。是否随最终分发取决于发布包实际包含的内容，不能仅凭字符串删除依赖或宣称全 MIT。

## Vendored 代码与资产

| 项目 | 仓库路径 | 许可证 / 状态 | 证据 |
|---|---|---|---|
| Three.js r178 | `adapters/html/vendor/three/**` | MIT，完整文本已随源码和导出的 preview 分发 | `adapters/html/vendor/three/LICENSE`，SHA256 `bfe119ea4fd413f5f7ca3fcd63adb0c4a073ed39daa2fe7d3e6b769e21272601`；上游 `https://raw.githubusercontent.com/mrdoob/three.js/r178/LICENSE` |
| Quaternius Stylized Nature | `assets/third_party/quaternius/stylized-nature/**` | CC0-1.0 | `stylized-nature/License_Standard.txt`，SHA256 `120710e542c3ebaf83856c4eb55b4a1e680593b39302eba7fdc5d942dfe78c58`；上游 `https://quaternius.com/packs/stylizednaturemegakit.html` |
| Quaternius building OBJ/MTL | `assets/third_party/quaternius/buildings/**` | **Ultimate Buildings Pack (Dec 2019)**；页面声明 CC0-1.0，当前 QAL v1.0 禁止单独再分发原始资产；本地为改名/单材质派生 | `buildings/NOTICE.txt`；精确下载日期仍未由仓库历史证明 |

导出 preview 现在包含：

- `preview/vendor/three/LICENSE`
- `assets/THIRD_PARTY_NOTICES.md`
- `assets/licenses/quaternius-stylized-nature-CC0.txt`
- `assets/licenses/quaternius-buildings-NOTICE.txt`

## 复现

```powershell
cargo metadata --manifest-path bevy/Cargo.toml --locked --format-version 1
pnpm licenses list --prod --json
pnpm licenses list --dev --json
node scripts/collaboration/license-inventory.mjs --out bevy/work/collaboration/review/release-license-inventory.json
```

发布前必须重新执行上述命令，并把 runtime、manifest、world 与导出 preview 的 hash 写入发布记录。锁文件变化后不得复用本文件的旧计数。

## 当前状态

截至 2026-09-08：

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
