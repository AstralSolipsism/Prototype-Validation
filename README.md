# Prototype Validation

本仓库用于生活世界模拟开放沙盒项目的**技术预生产与高风险技术验证**。它不是正式游戏仓库，也不以快速堆叠玩法为目标。

## 验证顺序与状态

1. **P0 基础契约：已通过**。稳定身份、世界时间、命令与事件、确定性随机、参考系数学、回放基础。
2. **P1 复杂路线卷轴摄影机：已通过，存在非阻塞工程后续**。直线、九十度转向、十字路口、坡道、双向路线和镜头重构。
3. **P2 移动参考系与内外部统一：已通过**。船舶内部局部坐标、真实窗外视差、统一主摄影机、上下船和大坐标稳定性。
4. **P3 语义建筑编译器：已通过**。建筑蓝图、空间约束、房间与门户图、碰撞、网格块、剖视、Massing 和局部重编译。
5. **P4 确定性世界生成：未启动**。
6. **P5 权威服务端、存档与回放：未启动**。
7. **P6 模拟分级与百万身份压力测试：未启动**。
8. **P7 多人兴趣域与区域权威：未启动**。
9. **P8 港镇—船舶垂直切片：未启动**。

## 当前交付

### P0/P1

- `world_ids`：固定宽度、可序列化的稳定领域身份；
- `world_time`：离散世界时间和固定步长时钟；
- `deterministic_rng`：按阶段、空间和特征隔离的确定性随机流；
- `world_math`：刚性参考系、局部/世界变换和摄影机相对坐标；
- `protocol`：类型化命令、结果、事件、版本检查和幂等账本；
- `replay_core`：连续命令日志和语义指纹；
- `scroll_camera_core`：与 Bevy 解耦的路线采样及卷轴摄影机状态；
- `p1_scenario`：P1 手工路线和双向镜头语法；
- `camera_trace`：P1 正向与反向无头轨迹报告；
- `camera_routes`：P1 Bevy 0.19 可视化程序。

### P2

- `mobile_region_core`：参考系归属、保持世界姿态的迁移、权威/表现姿态分离和对接锚点；
- `p2_reference_frame_trace`：大坐标、内部刚性、上下船迁移和摄影机相对精度报告；
- `mobile_region_visual`：一台主摄影机下同时显示船舱、真实窗户、船体、岛屿、港口和高塔；
- `p2-windows-visual-package`：Windows x64 人工验收包；
- `docs/evidence/p2-manual-gpu-validation-2026-08-08.md`：P2 人工通过记录。

### P3

- `building_core`：引擎无关建筑蓝图、约束验证、房间与门户图、碰撞、导航、网格块、外壳、剖视组、Massing 和 DirtySet；
- `p3_building_scenario`：两层商住建筑、楼梯、门窗、静态/移动参考系实例和不可达反例；
- `p3_building_trace`：确定性、可达性、局部修改和同源包围范围无头报告；
- `building_visual`：同屏固定建筑与移动平台建筑，支持完整结构、外壳、Massing、剖视和局部加窗；
- `p3-windows-visual-package`：Windows x64 人工验收包工作流；
- `docs/evidence/p3-manual-gpu-validation-2026-08-08.md`：P3 人工通过记录。

## 本地运行

需要 Rust 1.97.1：

```bash
./scripts/run_static_checks.sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --exclude camera_routes --exclude mobile_region_visual --exclude building_visual --all-targets --locked
cargo run --locked -p camera_trace -- artifacts/p1-camera-trace.json
cargo run --locked -p p2_reference_frame_trace -- artifacts/p2-reference-frame-trace.json
cargo run --locked -p p3_building_trace -- artifacts/p3-building-trace.json
cargo run --locked -p camera_routes
cargo run --locked -p mobile_region_visual
cargo run --locked -p building_visual
```

`mobile_region_visual` 操作：

- `Space`：暂停或继续船舶运动；
- `C`：切换舱内窗口与外部剖视镜头；
- `B`：在世界参考系与船舶参考系之间迁移红色主体；
- `P`：开启或关闭客户端表现摇摆；
- `R`：重置航行时间。

`building_visual` 操作：

- `M`：完整结构 / 外部壳体 / Massing；
- `X`：切换北、东、南、西、屋顶剖视和无剖视；
- `W`：新增或移除指定展示窗；
- `O`：暂停或继续右侧移动平台；
- `A` / `D` 或左右方向键：环绕观察；
- 上下方向键：调整摄影机高度；
- `R`：重置。

## 已知非阻塞事项

- Issue #5：横向输入应按当前镜头屏幕方向映射；
- Issue #6：摄影机与主体之间的结构遮挡、淡出与剖视；
- Issue #7：P1 录像、固定截图与截图回归基线。

## 架构红线

- 领域核心 crate 不得依赖 Bevy、窗口、渲染、具体网络库或数据库；
- Bevy `Entity`、`Transform`、资产句柄不得进入持久化协议；
- 世界生成语义结果必须可复现，线程调度顺序不得改变结果；
- 移动区域使用刚性参考系，不允许通过缩放伪造内部空间；
- 权威导航姿态与客户端表现摇摆必须分离；
- 窗外世界必须是真实几何，不使用第二摄影机贴图冒充普通窗口；
- 建筑蓝图和空间语义是权威数据，网格、碰撞、导航和 HLOD 是可重建派生数据；
- 摄影机可以重新构图，但不得移动世界地标来配合画面；
- 原型只有通过对应停止线后，下一阶段才可扩大范围。

详见 [`docs/technical-preproduction.md`](docs/technical-preproduction.md)。
