# GlyphWeave 发布复现与第三方材料

日期：2026-09-08。适用范围：里程碑 A 的稳定底座发布材料，不是法律合规认证，也不代表 B/C 内容质量已经完成。

## 1. 锁文件与依赖

- Rust：`bevy/Cargo.lock`，518 个锁定包；`cargo metadata --locked` 解析 518 个，缺失 0。
- Node：`pnpm-lock.yaml`，`packages:` 段 347 个锁定包。
- 当前 Windows x64 平台已安装并解析许可证元数据 224 个包。
- 122 个锁定包为当前平台未安装的可选原生包；1 个锁定包
  `@cloudflare/workerd-windows-64@1.20260701.1` 缺少可用许可证元数据，
  原因是 pnpm 包索引缺失，已在机器可读清单中标记未解析。

生成命令：

```powershell
node scripts/collaboration/license-inventory.mjs `
  --out bevy/work/collaboration/review/release-license-inventory.json
```

完整说明见仓库根目录 `THIRD_PARTY_NOTICES.md`。

## 2. 源码与资产通知

- 自有代码：根目录 `LICENSE`（MIT）。
- Three.js r178：`adapters/html/vendor/three/LICENSE`，并随
  `preview/vendor/three/LICENSE` 导出。
- Quaternius Stylized Nature：`assets/third_party/quaternius/stylized-nature/License_Standard.txt`（CC0-1.0）。
- Quaternius building OBJ/MTL：`assets/third_party/quaternius/buildings/NOTICE.txt`。
  精确上游 pack 来源尚未证实，该限制不得被隐藏或改写成已确认。

导出 preview 必须同时包含：

```text
preview/vendor/three/LICENSE
assets/THIRD_PARTY_NOTICES.md
assets/licenses/quaternius-stylized-nature-CC0.txt
assets/licenses/quaternius-buildings-NOTICE.txt
```

## 3. 构建与测试

```powershell
cargo check --manifest-path bevy/Cargo.toml --workspace
cargo test  --manifest-path bevy/Cargo.toml --workspace
pnpm lint
pnpm typecheck
pnpm test
pnpm doc-tree:check
git diff --check
```

发布 runtime：

```powershell
cargo build --release --manifest-path bevy/Cargo.toml -p glyphweave-cli
Get-FileHash bevy/target/release/glyphweave.exe -Algorithm SHA256
```

最终 runtime 和输入 manifest 的 SHA256 必须写入当次验收记录；不得复用旧候选的 hash。

### 2026-09-08 最终候选

- Branch：`refactor/authoritative-baked-audit`
- HEAD：`6020fbe2713e8c79c1b2934b244402dc93e1df14`
- Runtime：`bevy/target-r-final/release/glyphweave.exe`
- Runtime SHA256：`99374af17e471e46e5c09185e28775fcd6661e71e025d6a45aa49f2e521b3bc8`
- 输入 manifest SHA256：`d4fe4d8c72783067f6996621f2836fd5bbc8a154f3fb81c6d2e220fba9154749`
- 源码清单：`bevy/work/collaboration/review/final-source-manifest.json`
- Runtime 绑定：`bevy/work/collaboration/review/final-runtime.json`

## 4. 世界生成与独立门禁

```powershell
& bevy/target/release/glyphweave.exe generate-world `
  examples/maximum-local-height-frontage.manifest.json `
  bevy/work/<candidate>
& bevy/target/release/glyphweave.exe world-quality-gate `
  bevy/work/<candidate>
& bevy/target/release/glyphweave.exe audit-preview-stream `
  bevy/work/<candidate> bevy/work/<parity>
```

生成与门禁预算保持 600 秒 / 8192 MiB。质量门禁允许保留逐条批准的 B/C 内容缺口，
但不得把工程错误、伪造数据、保护层改写或静默需求下降写成内容例外。

最终候选：

- 世界：`bevy/work/milestone-a-final-r51-20260908-v2-abs2`
- 生成：267.164s，峰值约 980 MiB，`worldRevision=bcc8328c0618ac78756a37bd7eaf013db8098a4f9a73aec19e583822242e52c0`
- 独立 gate：39.200s，峰值约 1298 MiB，退出 1，六类失败全部为已批准内容缺口
- 确定性重跑：`bevy/work/milestone-a-final-r51-20260908-v2-repeat`，267.552s，峰值约 983 MiB；共有文件 1778 个，1777 个逐字节相同，唯一差异为 `generation-report.json.output`
- 独立 parity：`bevy/work/milestone-a-final-r51-v2-abs2-parity-20260908/parity.json`
- 机器验收：`bevy/work/collaboration/review/final-evidence-check-final.json`（status pass，worldQualityStatus fail）

## 5. HTML 全域巡检

```powershell
node scripts/collaboration/check-preview-world.mjs `
  --url http://HOST:PORT/preview/index.html `
  --parity ABS_CURRENT_PARITY.json `
  --out ABS_NEW_RESULT.json `
  --browser
```

巡检必须覆盖全部 chunk、关键块 warm + 三轮回访、资源生命周期、网络重试和过期响应拒绝。

最终结果：`bevy/work/milestone-a-final-r51-v2-abs2-preview-check.json`，
240 块 coverage、3 个 warm、9 个 revisit、networkRetry/lateResponse 均 pass，
`entityCoverage=pass`、`heightCoverage=pass`。原始同页浏览器观测保存在
`bevy/work/milestone-a-final-r51-v2-preview-check-browser.json`；最终候选与其
world/header/scene/parity 语义 hash 完全一致后按 hash 重放。

## 6. 当前限制

- Quaternius building 资产的精确来源和适用许可版本仍未证实。
- `@cloudflare/workerd-windows-64@1.20260701.1` 的许可证元数据仍缺失。
- 本文件不声称 whole-world quality pass，也不声称 milestone C 完成。
