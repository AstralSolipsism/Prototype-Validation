# Prototype evidence

本目录记录停止线所需的可复核证据，而不是把自动生成结果误当作世界事实。

- GitHub Actions 生成的 `Cargo.lock` 和 P1 无头轨迹报告先作为 workflow artifact 保存；
- 通过审查后，将稳定的锁文件提交到仓库；
- GPU 截图、录像、硬件信息和人工验收结论按阶段建立独立子目录；
- `prototype-status.json` 只有在对应证据完整时才允许把 `passed` 改为 `true`。
