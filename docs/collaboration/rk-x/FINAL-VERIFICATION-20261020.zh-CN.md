# 2026-10-20 final verification snapshot

本文件记录本轮 ABC 收口后的实际回归，不把历史结果拼成新结果。

## 1. Rust workspace

第二次完整运行：

```powershell
cargo test --manifest-path bevy/Cargo.toml --workspace --no-fail-fast --quiet
```

- exit **0**；
- 全部 target 通过；
- core lib：**805 passed / 0 failed / 34 ignored**；
- 日志：`bevy/work/rk-x/final-workspace-test-2-20261020.log`。

第一次完整运行的失败仅为复制进产物的 preview/licenses 静态文件改变整树摘要；
对应 4 个 target 已逐项复验：

- `assigned_road_access`：4/4 pass；
- `derived_earthwork_blend`：10/10 pass；
- `development_provenance`：6/6 pass；
- `hillside_cut_city`：4/4 pass。

本次完整运行未复现已知偶发 `0xC0000005`，但该缺陷仍按
`KNOWN-ISSUE-intermittent-crash.zh-CN.md` 保留，不宣称已修复。

## 2. B-02 / B-04 当前控制

`service_coverage` 的 compact 供应控制最终断言：

- 3 条自动路；
- 实际 collector + local；
- frontage 请求 24、planned 24、realized-with-access **24/24**；
- road audit / tier audit pass；
- access disconnected **0**；
- coverage 使用作者尺寸 18×10m，24/24 地块在 compact profile
  `0.45..0.75` 范围内，status **pass**。

当前测试：1/1 pass。

## 3. 前端

- `pnpm lint`：exit 0（仓库既有 warnings，无 error）；
- `pnpm typecheck`：exit 0；
- `pnpm build`：exit 0。

## 4. HTML 最大图

预览服务读取 `c02-maximum-engineering-20261020/world/`：

- 战略图：12×20 chunks 全部完成绘制，console error/warn 为空；
- 2.5D：非港场景进入 Rust 内容 focus，单 focus chunk 加载；
- 最终截图有连续草地纹理、道路、地块/建筑与地块材质，不再暴露大块
  黑洞或粗块错位边界；
- 截图：
  `bevy/work/rk-x/c02-maximum-engineering-20261020/html-max-focus-55s.png`。

## 5. Godot

官方 Godot **4.7.2 stable**，最大图 headless smoke：

```text
GLYPHWEAVE_SMOKE scene=scene-0  chunk=0,1  loaded=6  X=500 Z=850 Y=0.00
```

exit 0，无 SCRIPT ERROR / ERROR。详情见
`C03-GODOT-SMOKE-20261020.zh-CN.md`。

## 6. 剩余边界

- 偶发进程访问违例仍存在，不能因为本次全绿而删除风险记录；
- 1km/120 户是输入与供给压力，不是 B-02 接口未闭合；
- Quaternius 来源包已解析，但精确历史下载日期仍未知；
- 未来设计 FUT-01..06 未纳入本轮。
