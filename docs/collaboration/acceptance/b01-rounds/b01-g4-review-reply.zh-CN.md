G4 六个登记状态的负结果可以保留并关闭，不重跑本轮。但报告须纠正三点，随下一轮一并处理：

1. 撤回“两个8m平台必然无法匹配9m交集”“只有修改路口合同才能推进”。当前证据只支持已登记六态失败，其中四态为当前生产profile匹配拒绝、两态为station104工程拒绝；没有高程域穷尽证明。8m是最低固定平台范围，不要求第9米必然变高。
2. abEndStation75Mm / bcStation9Mm 实际写入height_at的米值：2.92应为2920mm，2.64/2.56应为2640/2560mm。修正后续代码及单位小测试；旧run/receipt不覆盖，在新勘误中披露。
3. R3取x172..180漏了C端8m平台内侧x181..184；完整平台+padding为x172..184。检查员从冻结natural复算更紧必要上界5314mm，限制点(184,177)，与station104拒绝一致。5324只能称部分采样得到的较松必要界，不是完整平台界；不用补跑G4。调用计数注明candidate-stage，准备阶段另列。

下一轮仅做已有接口内的junctionLengthM=9参数对照，不改生产合同：固定A/B/C坐标、B=3000、12m/shoulder3/fill6000/rise80，只把两条新增段的平台从8设9，旧路保持不变。junctionRadius仍9，不扩大豁免、不改solver/profile/carve/审核或默认。

最多两态：新9m参数下生产infer_end得到的C，以及C=3000。自然harness只调用一次、总≤180s；先两段工程，通过后才测量并按需baseline realize≤1/candidate realize≤2；bake0。任一实际失败如实保留，不换坐标/B/宽度/平台凑通过。

严格按ds-v41f-b01-g5-junction9-control-task.zh-CN.md执行，完成停下等复验。B仍partial，C不推进，不提交Git、不启动agent、不动F/A。
