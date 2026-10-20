# C-L1：Quaternius 建筑资产来源核对（2026-10-20）

状态：**来源包已解析，许可历史有可记录边界**。

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

## 3. 精确来源

通过 Sourcegraph 找到公开镜像 `awplanets/awplanet` 的
`src/data/quaterniusAssets.js`，其将同族 `houses/1Story_GableRoof_Mat.obj`、
`2Story_Wide_Mat.obj` 明确标为 **Ultimate Buildings Pack**。随后核对官方
页面和下载目录：

- 官方页：`https://quaternius.com/packs/ultimatetexturedbuildings.html`
- 页面标题：`Quaternius • Ultimate Buildings Pack`（December 2019）
- 官方 Drive：
  `https://drive.google.com/drive/folders/1RE3qXhbE5yGS3t-xGFJ8GmOtTgCUF3LQ`
- OBJ 子目录：
  `https://drive.google.com/drive/folders/1MfEng5EbtFU0-L9SHqcU6Mm3gKlUk37k`

OBJ 目录确实列出四组 `_Mat.obj/.mtl`：

- `1Story_GableRoof_Mat`
- `2Story_GableRoof_Mat`
- `2Story_Wide_Mat`
- `2Story_RoundRoof_Mat`

本地文件是派生导出：移除 `_Mat` 后缀，并把多材质简化为单一材质。
公开镜像的几何计数与本地 1Story 完全一致（1227 vertices / 831 faces /
2852 vt）；2Story_Wide 的顶点和面数一致，vt 仅 7678 vs 7680。

## 4. 许可

- 官方 Ultimate Buildings Pack 页面仍声明 CC0-1.0；
- 当前全局 Quaternius Asset License (QAL v1.0) 允许个人、教育和商业产品使用，
  无需署名，但禁止把原始资产单独作为资产包再分发；
- QAL §7 明确说明新版本不追溯适用于已获得资产；
- 仓库历史仍不能证明精确下载日期，因此不能把单一许可快照冒充历史事实。

`NOTICE.txt` 已同时记录发布日期 CC0 声明、当前 QAL 条款和派生关系。

## 5. 结论

- 精确来源包：**Ultimate Buildings Pack (Dec 2019)**；
- 本地资产性质：派生的命名/材质简化版本，不是逐字节原包；
- 许可可审计，但下载日期未知仍是发布记录中的历史边界；
- C-L1 不再因“来源包未知”阻塞。
