# A-D3b许可证发布预检（未完成，不是法律合规认证）

检查日期2026-09-08。主Agent在worker实现期间做只读来源预检，后续Luna D3b补全清单/notice包装；当前未修改第三方文件或许可证。

> 最终状态（2026-09-08）：A-09 已按“盘点并明确未核实项”收尾。完整锁文件库存见
> `bevy/work/collaboration/review/release-license-inventory.json`，Three.js LICENSE 已随
> 源码和导出 preview 分发，建筑资产来源与 `workerd-windows-64` 许可证元数据仍明确标记
> 未解析。最终签收见 `docs/collaboration/a-final-signoff.zh-CN.md`。本文件后续“未完成”
> 文字保留为预检历史。

## 已发现必须收尾项

1. adapters/html/vendor/three/build/three.core.js声明REVISION=178及MIT SPDX、2010–2025 Three.js Authors版权。该vendor目录当前有源文件，但没有完整LICENSE文件；worldgen嵌入导出vendor文件列表也没有完整许可证。需按具体r178上游补全文本并确保源码包及导出preview包保留。已从官方原始地址获取LICENSE到review/license-primary/three-r178-LICENSE.txt，SHA256 bfe119ea4fd413f5f7ca3fcd63adb0c4a073ed39daa2fe7d3e6b769e21272601。不是声称所有vendored字节都已经对照官方，后续须逐文件hash核对或说明修改。
官方来源：https://raw.githubusercontent.com/mrdoob/three.js/r178/LICENSE 。上游条款要求分发副本或实质部分保留版权和许可通知。此处仅作具体文本履行项，不作全项目MIT兼容性结论。

2. stylized-nature/License_Standard.txt随文件明确CC0-1.0；官方对应pack页也声明CC0。来源：https://quaternius.com/packs/stylizednaturemegakit.html 。CC0说明：https://creativecommons.org/publicdomain/zero/1.0/ ，注意CC0不代替商标/专利等其他权利核验。

3. assets/third_party/quaternius/buildings四个OBJ/MTL的README却统一指向Stylized Nature pack，不能把自然pack声明自动推广到建筑文件。官方有Buildings Pack、Simple Buildings等CC0页面，但尚未通过实际原包/文件hash确认本地四建筑的具体来源。候选来源：https://quaternius.com/packs/buildings.html 和 https://quaternius.com/packs/simplebuildings.html 。仅为候选，不标来源已证实、不下载新模型替换现存数据。

4. pnpm licenses list --json退出1，ERR_PNPM_MISSING_PACKAGE_INDEX_FILE，涉及@cloudflare/workerd-windows-64@1.20260701.1；原输出bevy/work/collaboration/review/npm-license-preflight.json。不能把失败命令说成许可列表通过，也不自动pnpm install更改依赖。

5. 主Agent直接读取已安装package.json生成224包的预检清单，位于bevy/work/collaboration/review/npm-installed-license-preflight.json；每项都声明license，但不是锁文件完整依赖或发布分发清单。发现@img/sharp-win32-x64声明Apache-2.0 AND LGPL-3.0-or-later，lightningcss及其Windows包声明MPL-2.0。D3b必须区分构建工具/运行依赖/实际捆绑二进制，不应仅看到copyleft字符串便删依赖，也不能标全MIT。此处只是包元数据事实，不作链接/分发法律判断。

## A-09验收需要

- Cargo.lock与pnpm-lock实际锁定包库存及许可证表达式/原始文本位置，缺失明确待核验；分源码发布与二进制/HTML资产导出两种范围。
- 自有MIT、vendor、模型/纹理、字体/GIS分别记录；上游参考但未复制代码与实际vendoring严格区分。
- THREE完整许可复制及导出包装需等Terra释放worldgen.rs写锁后执行。
- 资产来源映射修正，确实无法证实的条目如实列发布阻塞/限制，不把整个资产树重新授权MIT。
- 可复现构建、版本/来源/hash清单与发布说明由最后候选再生成，不用此预检替代正式发布验收。
