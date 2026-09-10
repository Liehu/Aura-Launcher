# 170 — P3-UI.0 尾款：testkit crate + README/ARCHITECTURE 18-crate 对齐

> 日期：2026-09-10。范围：P3-UI.0 后置项第一部分。输入基线：169 号的
> 补丁（plugin-host dev-dep testkit + ARCHITECTURE/README testkit
> mention + 18-crate 修正）。

## 交付

- `crates/launcher-plugin-testkit`（新 crate，P3-UI.0-H）：契约测试套
  件——manifest（valid/wrong-version/missing-id）、tool definition
  （valid/wrong-version）、UI Schema（base64 布局/超节点/未知 kind）、
  UiEvent、Rich payload（valid/malformed）、Python 进程 fixture
  （toggler/rich）——纯 DATA 无 IO，消费者决定如何运行。
- README 18-crate claim + ARCHITECTURE 补 `launcher-plugin-testkit`
  mention（topology 一致性检查）。

## Gate 结果

- 924 tests / 0 failed / topology 18 crates + 9 apps ok
