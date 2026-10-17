# B-05 结果：集成回归与最终候选

状态：`partial` / 大图与浏览器 `not_run`。

## 准入判断

B-01～B-04 的小场景仍有 frontage、道路供给和用途缺额；不满足“至少一个 ≤1km 可行组合的必需需求实际实现”的最大图准入条件。因此未运行 6000×10000m 生成/gate，也未启动 240 块浏览器巡检。

## 已执行验证

- `cargo test --workspace -- --test-threads=1`：402 passed / 3 ignored / 0 failed。
- `node --test scripts/collaboration/*.test.mjs`：252 passed / 0 failed。
- `pnpm lint` exit 0（既有 warning）；`pnpm typecheck` exit 0；`pnpm test` 170 passed。
- `pnpm doc-tree:check` PASS；`git diff --check` exit 0。
- 日志：`bevy/work/collaboration/ds-b-run/b05/`。

## 生态尾差

新 B ≤1km 输入的 ecologyUnfilledPlants=0；没有在真实新输入中复现 A 的 1 株尾差，因此未做无依据的硬编码补树。A 原始 342379/342380 事实保留。

## 未执行

- 最大图生成/独立 gate：not_run。
- 240 块浏览器完整巡检：not_run。
- 最终 release runtime/parity/source snapshot：not_run。

## 结论

工程回归通过，但最终最大图候选未形成，B-05 不能 ready_for_review；状态 partial。
