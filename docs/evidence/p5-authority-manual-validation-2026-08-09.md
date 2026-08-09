# P5 权威服务端、存档、恢复与回放人工验收记录

日期：2026-08-09

验收对象：`p5-authority-manual-validation-windows-x64`

自动门禁基线：

- Workflow run：`31301005730`
- Validated source：`5a37ac6a3928601a2a87dfa556eb8ed9fc5fb674`
- Artifact ID：`9034536316`
- ZIP SHA-256：`66521d5eb06b66e0e18217c26d2c21cb5d8b84ec9178c6647db2f24e4d177388`

## 项目所有者提交的人工运行产物

项目所有者运行交互式 Windows x64 验收程序后，提交了默认 `p5-data` 目录中的三项产物：

- `p5-validation-report.json`
  - SHA-256：`ce5aba2822c8ab43ec3d497760985ba3f4086f187847397bce5f5b3fcafe2297`
- `region-snapshot.json`
  - SHA-256：`18e144ee752e94350417f2bd6882177f29dee9020bbd0903aa05cc515fb48ce4`
- `command-journal.ndjson`
  - SHA-256：`47e67096827e4a348b8fc3cc790dbe7592449ac92062c97c5a651606a125f075`

报告中的输出路径为 `p5-data\region-snapshot.json` 和 `p5-data\command-journal.ndjson`，对应人工交互模式的默认输出目录，而不是自动烟测目录。

## 产物一致性核验

三项产物经逐项解析和交叉核对，结果如下：

- 报告包含 14 项检查，编号 1～14 连续，全部为 `passed = true`；
- 报告总结果为 `all_passed = true`；
- 命令日志共 18 条，`sequence` 为 0～17，连续且无重复；
- 18 个日志命令 ID 唯一，命令信封与存储回执中的 `command_id` 一致；
- 每个 Session 内的 `request_sequence` 从 0 连续递增；
- 快照位于 world revision 5，`next_journal_sequence = 13`；
- 快照中的幂等账本与日志 0～12 的命令集合完全一致，存储回执逐项一致；
- 快照指纹 `e45cfef3dfc65c844f4705845213ea16` 与 revision 5 检查点一致；
- 快照后日志保留了断线、关门、新会话重连和恢复后双客户端连接；
- 快照后唯一改变世界事实的操作将门从打开改为关闭，世界推进到 revision 6；
- 过期 Building version 返回 `VersionConflict`，无编辑权限返回 `PermissionDenied`，两次拒绝均未推进 revision；
- 最终日志和恢复后 A、B 两个快照均使用指纹 `2c0436e8121afd5377673a3de7028807`；
- 恢复前与恢复后指纹完全一致；
- 最终物品仍归 A，A 仍位于目标建筑内，建筑开口仍存在，门保持快照后的关闭状态。

## 人工验收结果

```text
all_passed = true
final_revision = 6
journal_records = 18
duplicate_receipts = 2
rejected_receipts = 2
buffered_out_of_order_commands = 1
pre_crash_fingerprint = 2c0436e8121afd5377673a3de7028807
recovered_fingerprint = 2c0436e8121afd5377673a3de7028807
```

提交产物满足人工验收清单中的全部通过条件，没有发现重复执行、客户端分叉、拒绝失效、快照回滚、日志断裂、对象复制或恢复后状态不一致。

## 最终判定

**P5 通过。**

P5 停止线可以关闭。PR #25 可以转为正常代码评审状态，P6 可以解除阻塞。