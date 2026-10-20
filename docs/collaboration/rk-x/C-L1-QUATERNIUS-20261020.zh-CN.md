# C-L1：Quaternius 建筑资产来源核对（2026-10-20）

状态：**仍未解析**。本轮把候选范围进一步排除，而不是猜测通过。

## 1. 本地文件

`assets/third_party/quaternius/buildings/`：

- `1Story_GableRoof.obj`
- `2Story_GableRoof.obj`
- `2Story_Wide.obj`
- `2Story_RoundRoof.obj`

## 2. 官方候选包核对

官网候选：

- Buildings Pack（Jan 2019）：`https://quaternius.com/packs/buildings.html`
- Simple Buildings Pack（Aug 2017）：`https://quaternius.com/packs/simplebuildings.html`

两个页面的公开 Google Drive OBJ 文件夹列表：

- Buildings Pack：`Building1_Large`、`Building1_Small`、`Building2_Large`、
  `Building2_Small`、`Building3_Big`、`Building3_Small`、`Building4`、
  `House1`、`House2`。
- Simple Buildings：`Bank`、`Flat`、`Flat2`、`Hospital`、`House`、`House2`、
  `House3`、`House4`、`House5`、`Shop`。

两套都没有本地四个 `GableRoof`/`Wide` 文件名。因此不能把本地资产归到这两个
候选包，也不能把它们的 CC0 声明直接套到本地文件上。

## 3. 结论

- 本地四个建筑 OBJ/MTL 的精确 Quaternius pack/archive 仍未知；
- 发布级 C-L1 不能把这些建筑资产标为已解析；
- 下一步需要从原始下载记录、文件哈希或作者提供的来源清单定位精确包。
