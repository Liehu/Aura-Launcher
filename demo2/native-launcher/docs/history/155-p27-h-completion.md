# 155 — P2.7 Batch 22：H 线非证书项收口 + P2.7 完成宣告

> 日期：2026-09-10。范围：P2.7 第二十二批（§39-H 非证书项；154 号交接）。
> 输入基线：history/154（863 tests）。

## 交付

- **H03 AI Quality Corpus**：
  - `crates/launcher-ai/tests/ai_quality_corpus.json`（新）：20 条冻结
    意图语料——search/open/execute/workflow 路由、大小写与空白归一、
    兜底 Search（含中文输入）、注入风味输入保持为 DATA（search query）、
    前缀边界（"opennotepad" 不路由为 Open）；
  - `tests/ai_corpus.rs`（新）：逐条断言 intent + 三类实体投影，另附
    确定性双跑稳定断言（防顺序/大小写相关的实现回归）。语料即验收
    基线：后续改动 parser 必须先过语料。
- **H01 CI AI Tests**：确认 release_gate 的 `cargo test --workspace`
  已覆盖 G 线套件（g_line_suite 10 条）与本批语料测试——AI 测试随
  标准 gate 全量运行，无独立 CI 通道需求。
- **H05 文档冻结**：PROJECT-HANDBOOK P2.7 状态矩阵全面刷新（A–H 各线
  批次号溯源），并修正 P26-F/G 遗留的 ⏳ 标记（137 号已收口）。
- **H06 完成宣告**：见下。

## P2.7 完成宣告

**P2.7 AI / Agent Productization = 完成**（证书依赖项除外）：

```text
P27-000/001/002/003  契约/基线/安全模型            ✅ 101/138 号
P27-A01–A06          Provider/Context/Intent/Prompt/校验/澄清 ✅ 117-121/138/141 号
P27-B01–B06          Catalog/Plan/Validator/Proposal/Risk   ✅ 133/139/140/148 号
P27-C01–C07          Session/Loop/Replan/取消/宿主接线/恢复 ✅ 121/136/142/147 号
P27-D01–D05          Approval 全套（含 §23 接线 + D02 UI）  ✅ 151 号
P27-E01–E06          Memory/Privacy/注入防御               ✅ 152 号
P27-F01–F05          Product UX + §37/§38 可观测性         ✅ 153 号
P27-G01–G05          QA 补强矩阵（抓出并修复 2 缺陷）       ✅ 154 号
P27-H01/H03/H05      CI 载体/质量语料/文档冻结             ✅ 本批
P27-H02/H04/H06      Security/Release Gate/签名发布         ◐ 随全仓 gate；
                     （MSIX 签名等外部证书）                  证书后置
```

**最终基线**：865 tests 全绿 / zero warnings / topology 17 crates +
9 apps / 全仓 gate（G01–G17）PASS。

## 待续（非 P2.7）

1. Pinyin 完整拼音表（优化项）
2. MSIX 签名 + 发布流程（等外部证书）
3. Mimosa 安全扫描缓冲不足问题（scanner_enobufs，hook 提示）——重跑
   deep 审计确认基线
