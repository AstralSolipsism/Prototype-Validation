# P7 多人兴趣域、区域单写与跨区权威移交人工验收记录

日期：2026-08-09

验收对象：`p7-region-authority-manual-validation-windows-x64`

自动门禁最终基线：

- Workflow run：`31311262357`
- Validated source：`e3372efc8fa400db92e94dc02c92a1f481d0d68a`
- Windows Artifact ID：`9037481094`
- Windows Artifact ZIP SHA-256：`78c806f1613506488725caecfbce32cef3dbef3fb1d5cbd713ccd9c7c7cd36b3`
- 自动证据 Artifact ID：`9037467989`
- 自动证据 SHA-256：`83a2e7c4e0e69b048b6a1ab827df2ce950db1f935793c43373f510227dab4504`

## 项目所有者提交的人工运行产物

- `p7-validation-report.json`
  - SHA-256：`eb61e77d495ec426ba9f555ac172f63c2effe1a7bd8672c498bfaf01f87cd2e3`
- `p7-client-interests.json`
  - SHA-256：`79d5901f4033851793f107a117e4bf5677e36998b6a3798af2bcf18b5bae3d00`
- `p7-transfer-evidence.json`
  - SHA-256：`27784446fd389d6c7db0c7e33c6be96ef55bb98f2b3471f25189723b3e22cff6`

三项人工产物与最终 Windows 自动烟测包中的同名语义产物逐字节一致。

未提交 `p7-machine-info.txt`，因此本轮没有单独归档项目所有者测试机器的硬件信息。P6 已归档同一测试环境的 i9-9900KF、16 逻辑处理器和 63.93 GiB 内存基线；P7 本轮不以性能容量为主要停止线，因此该缺口不阻塞权威语义验收。

## 交叉一致性核验

对三项产物执行了 49 项结构和语义交叉检查，全部通过：

- 主报告包含 15 项检查，编号 1～15 连续，全部 `passed = true`；
- `all_passed = true`，`fingerprints_match = true`；
- 单独的客户端兴趣文件与主报告 first_run、repeat_run 中的 `projections` 完全一致；
- 单独的迁移证据与主报告两次运行中的 writer、transfer、mobile、parallel、hotspot 数据完全一致；
- 四个客户端分别只有一个权威模拟区域，视觉和预取集合可以超出该区域；
- 四客户端模拟区域互不相同，证明不是向所有客户端广播同一高精度投影；
- 船舱客户端的 `simulation_regions` 与 `mobile_regions` 相同，同时预取前方 `(1,0)`、`(2,0)`；
- 远处人物以同一个 Cold 摘要存在于四个客户端中，且没有泄漏到任何客户端的高精度人物列表；
- 重平衡前后均只有四个唯一 RegionId 所有者；只有东部区域发生 writer/epoch 变化；
- 东部区域从 worker 3 / epoch 1 迁移到 worker 5 / epoch 2，旧 writer 写入被拒绝；
- 四个 transfer ID 唯一，4 次重复迁移操作均保持幂等；
- 两类故障注入均恢复为唯一权威位置；
- 静态跨区、登船、货物登船和下船过程保持持久事实；
- 船舶从 `(0,0)` 到 `(1,0)` 后，迁移计数保持 3→3，货物和剩余乘客继续属于同一 MobileRegion；
- 并行和顺序执行的区域结果、累加器和指纹完全一致，最大并行 worker 数为 2；
- 热点区域 4 个客户端提交 256 条命令，但唯一 writer 数为 1；revision 从 5 连续推进到 261；
- first_run 与 repeat_run 的投影、writer、迁移、移动区域、并行、热点及最终指纹全部一致。

## 人工验收结果

```text
checks = 15 / 15 passed
all_passed = true
fingerprints_match = true

clients = 4
static_regions = 3
mobile_regions = 1

stale_writer_rejected = true
duplicate_transfer_operations = 4
fault_recoveries = 2
unique_entity_locations = true
persistent_facts_preserved = true
ship_internal_authority_preserved = true

parallel_workers = 2
parallel_equals_sequential = true

hotspot_commands = 256
hotspot_unique_writers = 1
hotspot_first_revision = 5
hotspot_final_revision = 261
hotspot_revisions_contiguous = true

fingerprint = 81aed9ca4d8bf3bb75068b37f6f9a7e2
repeat_fingerprint = 81aed9ca4d8bf3bb75068b37f6f9a7e2
```

## 最终判定

**P7 通过。**

本轮证明了四客户端兴趣过滤、静态和移动区域单写、writer epoch 重平衡、跨区事务、重复操作幂等、故障恢复、移动区域内部权威保持、远处人物摘要、分散区域并行和热点单 writer 串行能够共同成立。

本轮不代表最终 MMO 集群、跨机容灾、公网网络、反作弊或正式网络压缩已经完成。P7 停止线可以关闭，PR #29 可以转入正常代码评审，P8 港镇—船舶完整垂直切片可以解除阻塞。
