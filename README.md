# Prototype Validation

本仓库用于生活世界模拟开放沙盒项目的**技术预生产与高风险技术验证**。它不是正式游戏仓库，也不以快速堆叠玩法为目标。

## 验证顺序与状态

1. **P0 基础契约：已通过**  
   稳定身份、世界时间、命令与事件、确定性随机、参考系数学和回放基础。

2. **P1 复杂路线卷轴摄影机：已通过，存在非阻塞工程后续**  
   直线、弧线、九十度转向、十字路口、坡道、双向路线和镜头重构。

3. **P2 移动参考系与内外部统一：已通过**  
   船舶内部局部坐标、真实窗外视差、统一主摄影机、上下船和大坐标稳定性。

4. **P3 语义建筑编译器：已通过**  
   建筑蓝图、空间约束、房间与门户图、碰撞、网格块、剖视、Massing 和局部重编译。

5. **P4 分层确定性世界生成：已通过**  
   Atlas 语义地图、地区级物化、自然地理、人文历史、建筑落位、地表路线、桥梁语义、跨地区卷轴旅行和跨尺度身份一致性。

6. **P5 权威服务端、存档、恢复与回放：已解除阻塞，尚未启动**  
   规划见 Issue #24。

7. **P6 模拟分级与百万身份压力测试：未启动**。
8. **P7 多人兴趣域与区域权威：未启动**。
9. **P8 港镇—船舶垂直切片：未启动**。

权威阶段状态见 [`prototype-status.json`](prototype-status.json)。

## 已验证的空间与世界架构

```text
WorldAtlasManifest
└── AtlasCell：约 4 km 对边距离的地区级语义容器
    ├── 独立 CellMaterializationId
    ├── 多个约 250 m TerrainTile
    ├── 当前地区 97×97 全精度物化
    ├── 最多六个邻区 17×17 代理
    ├── 自然地理、人文历史和土地利用
    ├── GroundingResult 建筑基础
    └── SurfaceConforming / Bridge 路线语义
```

### 空间单位保持分离

- `AtlasCell`：世界地图地区与宏观语义；
- `TerrainTile`：地形网格、碰撞和流式加载；
- `SimulationRegion`：服务端权威和并行调度；
- `PersistencePage`：存档、压缩和 I/O；
- `MobileRegion`：船舶、空艇和移动建筑内部。

这些单位共享同一套世界地址和稳定身份，但不强制一一对应。

## P4 最终结论

P4 经历两轮人工 GPU 验收。第一轮发现并阻断了三项设计偏差：

- AtlasCell 与 TerrainTile 尺度混同；
- 建筑没有正式地形落位和基础；
- 路线单独平滑 Y 值导致地底穿模。

修正版完成后，自动门禁与人工复验均通过：

- 单格对边距离约 4 km；
- 当前格约 278 个 TerrainTile；
- 共享边界最大高程误差约 `1.95e-12 m`；
- 8 栋建筑具有显式板式或台阶式基础；
- 普通路线最大贴地误差约 `2.84e-14 m`；
- 三条路线各跨越 3 个 AtlasCell；
- Linux 与 Windows x64 Bevy 构建通过；
- 项目所有者最终结论：**总体验收全部通过，内容均满足预期**。

证据：

- [`docs/evidence/p4-region-scale-manual-gpu-validation-2026-08-09.md`](docs/evidence/p4-region-scale-manual-gpu-validation-2026-08-09.md)
- Issue #11
- PR #18

## 当前主要组件

### 基础契约与回放

- `world_ids`
- `world_time`
- `deterministic_rng`
- `world_math`
- `protocol`
- `replay_core`

### 空间、摄影机与移动区域

- `scroll_camera_core`
- `mobile_region_core`
- `camera_routes`
- `mobile_region_visual`

### 建筑

- `building_core`
- `p3_building_scenario`
- `building_visual`

### 世界生成与地区物化

- `world_generation_core`
- `integrated_world_core`
- `region_scale_core`
- `p4_world_scenario`
- `p4_integrated_scenario`
- `p4_region_scale_scenario`
- `p4_region_scale_trace`
- `p4_region_scale_visual`

## 本地验证

需要 Rust 1.97.1：

```bash
./scripts/run_static_checks.sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace \
  --exclude camera_routes \
  --exclude mobile_region_visual \
  --exclude building_visual \
  --exclude world_visual \
  --exclude integrated_world_visual \
  --exclude p4_region_scale_visual \
  --all-targets --locked
cargo run --locked -p p4_region_scale_trace -- artifacts/p4-region-scale-trace.json
cargo run --locked -p p4_region_scale_visual
```

## 已知非阻塞事项

- Issue #5：横向输入按当前镜头屏幕方向映射；
- Issue #6：摄影机与主体之间的结构遮挡、淡出与剖视；
- Issue #7：P1 录像、固定截图与截图回归基线。

## 架构红线

- 领域核心 crate 不得依赖 Bevy、窗口、渲染、具体网络库或数据库；
- Bevy `Entity`、`Transform` 和资产句柄不得进入持久化协议；
- 世界生成语义必须可复现，线程调度和无关遍历顺序不得改变结果；
- AtlasCell、TerrainTile、SimulationRegion、PersistencePage 不得合并成一个万能 Chunk；
- 山脉、河流、道路、建筑和地标先作为世界事实成立，摄影机不得移动它们来配合构图；
- 相邻地区必须共享确定性的边界契约；
- 卷轴轨道必须来自真实地形、道路、桥梁、建筑和门户；
- 普通路线必须贴地，脱离地表必须具有桥梁、隧道、路基或其他明确工程语义；
- 建筑蓝图、落位结果和空间语义是权威数据，网格、碰撞、导航和 HLOD 是可重建派生数据；
- 地图、地区实景、LOD 和卷轴路线中的同一对象必须共享稳定身份；
- 移动区域使用刚性参考系，不允许通过缩放伪造内部空间；
- 原型只有通过对应停止线后，下一阶段才可扩大范围。

下一阶段：Issue #24，P5 权威服务端、存档、恢复与回放验证。
