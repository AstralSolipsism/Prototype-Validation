# P6 模拟分级、事件日历与百万持久人口人工验收记录

日期：2026-08-09

验收对象：`p6-simulation-scale-manual-validation-windows-x64`

自动门禁基线：

- Workflow run：`31305508078`
- Validated source：`eca2cc8de2710d1190efad447f8ef3a7db8bc85a`
- Windows Artifact ID：`9035885890`
- Windows Artifact ZIP SHA-256：`ae683e1d1ab06fab07b2906a99e0ecc1ccbcd23725248f27c391d175c86c4169`

## 项目所有者提交的人工运行产物

- `p6-validation-report.json`
  - SHA-256：`aa755caad153a73498b4bc2fba1e950aad41d98f19d93199e81003b3100e8767`
- `p6-tier-transition.json`
  - SHA-256：`f19b57dce00bfc427f3539ef9cad69b39138d87375ffcd3b667d38ab65953585`

未提交 `p6-machine-info.txt`，因此项目所有者测试机器的 CPU、内存和操作系统版本没有归档。该缺口不影响本轮语义、规模和调度停止线判定，但后续做跨硬件容量比较时需重新采集。

## 产物一致性核验

- 人工报告包含 12 项检查，编号 1～12 连续，全部为 `passed = true`；
- 总结果为 `all_passed = true`；
- Cold、Warm、Hot 实际人数分别为 1,000,000、10,000、300；
- 模型自有内存为 56,105,856 字节（53.51 MiB），小于 128 MiB 门限；
- 30 天内 Cold 到期结算 30,000,000 次，`full_population_scans = 0`；
- Warm 到期事件 800,162 次，`full_population_scans = 0`；
- Hot 固定步长更新 360,000 次；
- 64 人完成 Cold → Warm → Hot → Warm → Cold，全部保持稳定身份和持久事实；
- 64 人的需求摘要均发生变化，证明升降级过程实际执行而非空操作；
- 300 个 Hot 运行缓存全部清除并全部重建，重建后有效；
- 31,160,162 次内部更新只形成 228 条显著历史，历史/内部更新比例约为 0.0007317%；
- 第一次完整运行 1,529 ms，重复运行 1,479 ms，均低于 120,000 ms 原型门限；
- 两次完整运行的人数、内存、事件数量、升降级结果、缓存结果和最终语义指标完全一致；
- 两次最终指纹均为 `1cde2c215bbb4b38ca89b8eadf1bd668`；
- 单独提交的 `p6-tier-transition.json` 与报告中 first_run 和 repeat_run 的 transition 对象完全一致；
- 8 个样例人物 ID 唯一，`final_tier = Cold`、`facts_preserved = true`，且 before 与 after 需求摘要不同；
- 人工 transition 文件与 Windows 自动烟测 transition 文件逐字节一致；
- 人工报告与 Windows 自动烟测报告除运行耗时和输出目录外，全部语义数据一致。

## 人工验收结果

```text
all_passed = true
fingerprints_match = true

cold_population = 1,000,000
warm_population = 10,000
hot_population = 300
model_owned_bytes = 56,105,856

cold_due_events = 30,000,000
warm_due_events = 800,162
hot_fixed_step_updates = 360,000
cold_full_population_scans = 0
warm_full_population_scans = 0

transition_people = 64
facts_preserved = true
hot_cache_entries_cleared = 300
hot_cache_entries_rebuilt = 300
history_events = 228

first_run_ms = 1,529
repeat_run_ms = 1,479
fingerprint = 1cde2c215bbb4b38ca89b8eadf1bd668
repeat_fingerprint = 1cde2c215bbb4b38ca89b8eadf1bd668
```

## 最终判定

**P6 通过。**

本轮证明的是分级模拟的数据结构、调度和升降级骨架具有可行性，不代表完整心理、经济、社会和国家系统已经具备同样性能。P6 停止线可以关闭，PR #27 可以转入正常代码评审，P7 多人兴趣域与区域权威可以解除阻塞。
