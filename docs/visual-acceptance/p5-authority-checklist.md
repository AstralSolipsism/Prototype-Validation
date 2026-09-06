# P5 权威服务端、存档、恢复与回放人工验收

本轮不评价画面质量。它验证客户端只是权威服务端的投影，并且在重复、乱序、断线、重连、快照、日志和服务端重启后仍保持同一世界事实。

## 验收包

应包含：

```text
p5_manual_validation.exe
p5-authority-checklist.md
collect_p5_machine_info.ps1
p5-authority-trace.json
RUN.md
```

## 运行

完整解压后，在目录中打开 PowerShell：

```powershell
powershell -ExecutionPolicy Bypass -File .\collect_p5_machine_info.ps1
.\p5_manual_validation.exe
```

程序每完成一步会暂停。阅读服务端、客户端 A、客户端 B 的结果后按 Enter 继续。

## 必须逐项确认

### 1. 两个客户端连接同一个权威服务端

- A、B 都通过真实本机 TCP 连接；
- 两者初始 revision 和状态指纹一致；
- 客户端没有自行创建门、物品、建筑或载具事实。

### 2. 门状态同步

- A 开门后服务端 revision 增加一次；
- B 同步后看到同一扇门、同一版本和同一开关状态；
- 不能出现 A 开、B 关的分叉。

### 3. 物品所有权同步

- 物品从世界容器转移到 A；
- A 的库存、容器内容、ItemOwner 三处关系一致；
- B 同步后得到完全相同的所有权结果。

### 4. 重复命令幂等

- 相同 CommandId 再次发送时显示 `Duplicate`；
- 物品不会再次增加；
- 世界 revision 不会再次增加；
- 原成功结果被原样返回。

### 5. 乱序与延迟

- “进入建筑”请求先于“跨地区移动”到达时先显示 Buffered；
- 缺失的前序请求到达后，两项命令按 request_sequence 顺序提交；
- 玩家不会先进入远方建筑再瞬移到该地区。

### 6. 版本冲突与权限

- 旧 Building version 的修改请求返回 VersionConflict；
- B 在没有位置和编辑权限时修改建筑返回 PermissionDenied；
- 两次拒绝都不能改变世界 revision 或建筑内容。

### 7. 快照与快照后的日志

- revision 5 时生成 `region-snapshot.json`；
- 快照后 B 再关闭门，形成仅存在于 `command-journal.ndjson` 的新状态；
- 文件可读且不是客户端视觉缓存。

### 8. 断线重连

- A 断开 TCP 连接后，人物、物品和建筑编辑不会消失；
- A 使用新 SessionId 重连后获得当前 revision；
- A 能看到断线期间 B 完成的门状态变化。

### 9. 服务端重启恢复

- 第一服务端停止后，程序建立新的 AuthorityServer；
- 新服务端从旧快照和快照后的日志恢复；
- 恢复前后的 revision 和 state fingerprint 完全一致；
- 快照后关闭的门不能回滚为打开。

### 10. 两客户端重新收敛

- 重启后 A、B 使用新会话重新连接；
- 两者收到同一恢复状态；
- 物品仍归 A，A 仍位于目标地区和目标建筑内；
- 建筑开口仍然存在；
- 门保持快照后状态。

### 11. 客户端视觉缓存可删除

- 清除客户端 visual cache 后，权威状态不变化；
- 两个客户端都可以只凭服务端快照重新建立缓存；
- 网格、材质、HLOD 等派生缓存不出现在权威 snapshot 中。

## 文件检查

程序完成后检查 `p5-data`：

```text
region-snapshot.json
command-journal.ndjson
p5-validation-report.json
```

`p5-validation-report.json` 中必须满足：

```text
all_passed = true
pre_crash_fingerprint = recovered_fingerprint
duplicate_receipts >= 2
rejected_receipts >= 2
buffered_out_of_order_commands >= 1
```

## 结论

### 通过

全部步骤 PASS，恢复前后状态一致，未出现重复执行、回滚、对象复制或客户端分叉。

### 有条件通过

权威、持久化和恢复链路成立，仅有控制台文案、等待时机、包结构等非语义工程问题。

### 失败

出现以下任一情况：

- 同一 CommandId 执行两次；
- B 无法同步 A 的门或物品变化；
- 乱序请求以错误顺序提交；
- 无权限或过期版本仍修改成功；
- 服务端重启后 revision、指纹或对象状态回滚；
- 重连造成玩家、物品、开口或载具重复；
- 删除客户端缓存导致权威事实丢失。
