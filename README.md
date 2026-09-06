# Prototype Validation

本仓库用于生活世界模拟开放沙盒项目的**技术预生产与高风险技术验证**。它不是正式游戏仓库，也不以快速堆叠玩法为目标。

## 验证顺序

1. **P0 基础契约**：稳定身份、世界时间、命令与事件、确定性随机、参考系数学、回放基础。
2. **P1 复杂路线卷轴摄影机**：直线、弧线、九十度转弯、十字路口、纵深片段和镜头重构。
3. **P2 移动参考系与内外部统一**：船舶或空艇内部局部坐标、窗外真实世界、靠岸与上下船。
4. **P3 语义建筑编译器**。
5. **P4 确定性世界生成**。
6. **P5 权威服务端、存档与回放**。
7. **P6 模拟分级与百万身份压力测试**。
8. **P7 多人兴趣域与区域权威**。
9. **P8 港镇—船舶垂直切片**。

## 当前交付

- `world_ids`：固定宽度、可序列化的稳定领域身份；
- `world_time`：离散世界时间和固定步长时钟；
- `deterministic_rng`：按阶段、空间和特征隔离的确定性随机流；
- `world_math`：刚性参考系、局部/世界变换和摄影机相对坐标；
- `protocol`：类型化命令、结果、事件、版本检查和幂等账本；
- `replay_core`：连续命令日志和语义指纹；
- `scroll_camera_core`：与 Bevy 解耦的路线采样及卷轴摄影机状态；
- `p1_scenario`：P1 手工路线、双向镜头语法和固定测试地标；
- `camera_trace`：正向与反向无头轨迹报告；
- `camera_routes`：Bevy 0.19 可视化验证程序。

## 本地运行

需要 Rust 1.97.1：

```bash
./scripts/run_static_checks.sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo run -p camera_trace -- artifacts/p1-camera-trace.json
cargo run -p camera_routes
```

`camera_routes` 操作：

- `Space`：暂停或继续当前方向的自动行进；
- `A` / `Left`：沿路线反向移动；
- `D` / `Right`：沿路线正向移动；
- `R`：回到起点并正向自动运行；
- `E`：移到终点并反向自动运行。

## 架构红线

- 领域核心 crate 不得依赖 Bevy、窗口、渲染、具体网络库或数据库；
- Bevy `Entity`、`Transform`、资产句柄不得进入持久化协议；
- 世界生成语义结果必须可复现，线程调度顺序不得改变结果；
- 移动区域使用刚性参考系，不允许通过缩放伪造内部空间；
- 摄影机可以重新构图，但不得移动世界地标来配合画面；
- 原型只有通过对应停止线后，下一阶段才可扩大范围。

详见 [`docs/technical-preproduction.md`](docs/technical-preproduction.md)。
