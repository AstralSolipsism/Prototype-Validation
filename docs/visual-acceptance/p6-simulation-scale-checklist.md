# P6 模拟分级、事件日历与百万持久人口人工验收清单

## 验收目标

本阶段不评价画面，也不验证完整心理学。需要确认：

> 世界可以保存百万级人物，但只有少量人物运行高频模拟；远处人物通过到期事件和聚合结算继续生活，运行成本不随总人口逐帧增长。

## 运行前

1. 完整解压 Windows 验收包。
2. 在解压目录打开 PowerShell。
3. 运行机器信息采集：

```powershell
powershell -ExecutionPolicy Bypass -File .\collect_p6_machine_info.ps1
```

4. 打开任务管理器的“详细信息”或“性能”页，可选记录程序峰值内存。
5. 启动：

```powershell
.\p6_manual_validation.exe
```

程序每完成一项检查会暂停。阅读输出后按 Enter。

## 逐项检查

### 1. 三个模拟层数量

必须显示：

```text
Cold = 1,000,000
Warm = 10,000
Hot = 300
```

这些是实际创建的数据，不是把小样本乘以比例后的估算。

### 2. 内存预算

程序应显示模型自有内存总量及分层占用。自动门禁要求模型自有数据不超过 128 MiB。

可选在任务管理器记录整个进程的峰值工作集。进程工作集会高于模型自有内存，因为还包括 Rust 运行库、分配器、程序代码和 JSON 输出缓冲。

### 3. Cold 到期桶

必须显示：

- 30 天内到期结算数量；
- `full_population_scans = 0`；
- 实际结算量远小于“每秒更新一百万人”的假设量。

这证明冷状态不是每帧遍历一百万人。

### 4. Warm 事件日历

必须显示温状态事件处理数量，并且：

- 只处理已到期事件；
- `full_population_scans = 0`；
- 事件数与 10,000 人、30 天的生活节奏相符，而不是固定帧数乘人口。

### 5. Hot 固定步长

必须显示：

```text
fixed_step_updates = 300 × 1,200 = 360,000
```

只有 Hot 人物运行这种高频需求和运行缓存更新。

### 6. 时间加速

30 个游戏日应在一次运行中完成。程序不能出现按 30 天真实秒数机械等待，也不能长时间无响应。

记录：

- 第一次完整运行耗时；
- 第二次完整运行耗时；
- 是否出现异常长时间卡死或内存持续上涨。

自动预算为每次运行不超过 120 秒；正常桌面 CPU 应明显低于该上限。

### 7. Cold → Warm → Hot → Warm → Cold

至少 64 人完成完整升降级。必须显示：

- 稳定 PersonId 不变；
- 家庭、财富、职业、长期承诺和位置不丢失；
- 需求摘要确实发生变化；
- 最终重新回到 Cold；
- 没有人被复制或消失。

`p6-tier-transition.json` 中应有示例人物的前后状态。

### 8. 热状态运行缓存可删除

必须显示 300 个 Hot 运行缓存被清除并重新构建，且重建后全部有效。

这里的运行缓存可以包括短期路径游标、动画相位和当前目标。删除这些内容不应删除人物身份和持久事实。

### 9. 历史事件稀疏

永久历史事件数量必须远少于内部更新数量。不能把每次饥饿增加、每个热状态 Tick 都写入历史。

### 10. 两次独立运行确定性一致

程序会从相同种子重新创建第二个完整世界。两次最终 fingerprint 必须完全相同。

如果两次人数和事件数相同但 fingerprint 不同，仍判失败。

## 输出文件

运行成功后，默认目录中应存在：

```text
p6-data\p6-validation-report.json
p6-data\p6-tier-transition.json
```

报告至少满足：

```text
all_passed = true
fingerprints_match = true
first_run.config.cold_population = 1000000
first_run.config.warm_population = 10000
first_run.config.hot_population = 300
first_run.cold_metrics.full_population_scans = 0
first_run.warm_metrics.full_population_scans = 0
first_run.transition.facts_preserved = true
first_run.hot_caches_valid_after_rebuild = true
```

## 结论

### 通过

所有自动项为 PASS，程序正常结束，两个输出文件存在，人工没有发现数量伪造、长时间卡死、内存失控、升降级丢失事实或两次结果不一致。

### 有条件通过

架构和数据连续性成立，仅存在控制台说明、输出格式、机器信息采集或预算展示方面的工程问题。

### 失败

出现任一情况：

- 没有实际创建要求数量的人物；
- 冷或温状态需要逐帧扫描全部人口；
- 时间加速表现为按秒遍历全部人物；
- 升降级后身份、家庭、财产、承诺或位置丢失；
- 出现重复人物；
- 清除运行缓存后无法恢复；
- 两次 fingerprint 不一致；
- 自动报告包含失败项；
- 程序崩溃、持续内存增长或无法在预算内完成。
