# Prototype Validation

本仓库用于生活世界模拟开放沙盒项目的**技术预生产与高风险技术验证**。它不是正式游戏仓库，也不以快速堆叠玩法为目标。

## 验证顺序与状态

1. **P0 基础契约：已通过**。稳定身份、世界时间、命令与事件、确定性随机、参考系数学、回放基础。
2. **P1 复杂路线卷轴摄影机：已通过，存在非阻塞工程后续**。直线、九十度转向、十字路口、坡道、双向路线和镜头重构。
3. **P2 移动参考系与内外部统一：已通过**。船舶内部局部坐标、真实窗外视差、统一主摄影机、上下船和大坐标稳定性。
4. **P3 语义建筑编译器：已通过**。建筑蓝图、空间约束、房间与门户图、碰撞、网格块、剖视、Massing 和局部重编译。
5. **P4 分层确定性世界生成：进行中**。
   - **P4A 宏观世界骨架与连续三维投影：已通过**；
   - **P4B Atlas 语义世界地图：未启动，Issue #13**；
   - **P4C 地块内部高分辨率自然地理物化：未启动，Issue #14**；
   - **P4D 人文历史、聚落形态与土地利用生成：未启动，Issue #15**；
   - **P4E 从详细世界生成通行网络与卷轴轨道：未启动，Issue #16**；
   - **P4F 地图—地块—局部场景—路线跨尺度一致性：未启动，Issue #17**。
6. **P5 权威服务端、存档与回放：未启动，受 P4B～P4F 阻塞**。
7. **P6 模拟分级与百万身份压力测试：未启动**。
8. **P7 多人兴趣域与区域权威：未启动**。
9. **P8 港镇—船舶垂直切片：未启动**。

## P4 的关键范围纠正

P4A 已经证明：一份确定性的宏观世界清单可以维持六边形拓扑、跨界门户、河流、道路、港镇、建筑、卷轴路线和地标的统一身份，并派生出连续三维场景。

P4A **没有**证明完整的分层世界架构。当前 19 格原型仍然把低采样地块直接投影为三维面片，因此视觉上接近一片带颜色的连续斜坡。

项目要求的正确关系是：

```text
WorldAtlasManifest
世界地图上的区域语义、地貌骨架、边界契约和历史摘要
        ↓
AtlasCellSpec
一个六边形区域的详细生成输入，而不是一块三维面片
        ↓
Cell Materialization
高分辨率山脊、谷地、河网、海岸、生态、聚落和历史痕迹
        ↓
Traversal Compilation
从已经成立的地形、道路、建筑和门户生成通行网络
        ↓
Scroll Route / Camera Corridor
从真实通行网络中筛选卷轴轨道和摄影机语法
        ↓
Bevy Client Projection
近景、外壳、Massing、远景和地图表现共享同一世界身份
```

因此：

> 六边形是承载宏观地理、人文历史、共享边界契约和按需物化输入的**区域容器**，不是局部三维地形的一块多边形面。

## 当前交付

### P0/P1

- `world_ids`：固定宽度、可序列化的稳定领域身份；
- `world_time`：离散世界时间和固定步长时钟；
- `deterministic_rng`：按阶段、空间和特征隔离的确定性随机流；
- `world_math`：刚性参考系、局部/世界变换和摄影机相对坐标；
- `protocol`：类型化命令、结果、事件、版本检查和幂等账本；
- `replay_core`：连续命令日志和语义指纹；
- `scroll_camera_core`：与 Bevy 解耦的路线采样及卷轴摄影机状态；
- `camera_trace` 与 `camera_routes`：P1 无头及 Bevy 验证。

### P2

- `mobile_region_core`：参考系归属、保持世界姿态的迁移、权威/表现姿态分离和对接锚点；
- `p2_reference_frame_trace`：大坐标、内部刚性、上下船迁移和摄影机相对精度报告；
- `mobile_region_visual`：一台主摄影机下同时显示船舱、真实窗户、船体和外部世界。

### P3

- `building_core`：引擎无关建筑蓝图、约束验证、房间与门户图、碰撞、导航、网格块、外壳、剖视组、Massing 和 DirtySet；
- `p3_building_scenario` 与 `p3_building_trace`：确定性建筑样板和无头报告；
- `building_visual`：完整结构、外壳、Massing、剖视和局部加窗验证。

### P4A

- `world_generation_core`：宏观六边形地址、版本化世界清单、地块级地形摘要、气候、水系、海岸、道路、港镇、地标、建筑实例和路线骨架；
- 世界约束验证：拓扑、边界、水流、海岸、道路坡度、聚落选址、建筑、门户和统一地标；
- `p4_world_scenario`：半径 2、19 格河谷港镇确定性样板；
- `p4_world_trace`：遍历顺序不变性、跨地块连续性和结构约束报告；
- `world_visual`：P4A 连续三维投影和卷轴路线 GPU 验证。

## 本地运行

需要 Rust 1.97.1：

```bash
./scripts/run_static_checks.sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --exclude camera_routes --exclude mobile_region_visual --exclude building_visual --exclude world_visual --all-targets --locked
cargo run --locked -p camera_trace -- artifacts/p1-camera-trace.json
cargo run --locked -p p2_reference_frame_trace -- artifacts/p2-reference-frame-trace.json
cargo run --locked -p p3_building_trace -- artifacts/p3-building-trace.json
cargo run --locked -p p4_world_trace -- artifacts/p4-world-trace.json
cargo run --locked -p camera_routes
cargo run --locked -p mobile_region_visual
cargo run --locked -p building_visual
cargo run --locked -p world_visual
```

## 已知非阻塞事项

- Issue #5：横向输入应按当前镜头屏幕方向映射；
- Issue #6：摄影机与主体之间的结构遮挡、淡出与剖视；
- Issue #7：P1 录像、固定截图与截图回归基线。

## 架构红线

- 领域核心 crate 不得依赖 Bevy、窗口、渲染、具体网络库或数据库；
- Bevy `Entity`、`Transform`、资产句柄不得进入持久化协议；
- 世界生成语义结果必须可复现，线程调度和无关遍历顺序不得改变结果；
- Atlas 地图、地块详细世界和局部三维场景必须分层，不允许把一块六边形网格面当作地块全部地理内容；
- 山脉、河流、道路、建筑和地标先作为世界事实成立，摄影机不得移动它们来配合构图；
- 相邻地块共享同一边界契约和跨界门户，不允许各自生成互相矛盾的边缘；
- 卷轴轨道必须从真实地形、道路、建筑和门户中生成，不能先画轨道再围绕轨道伪造世界；
- 地图、局部场景、LOD 和卷轴路线中的同一对象必须共享稳定身份；
- 移动区域使用刚性参考系，不允许通过缩放伪造内部空间；
- 建筑蓝图和空间语义是权威数据，网格、碰撞、导航和 HLOD 是可重建派生数据；
- 原型只有通过对应停止线后，下一阶段才可扩大范围。

详见 [`docs/technical-preproduction.md`](docs/technical-preproduction.md) 和 P4 umbrella Issue #11。
