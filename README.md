# Virus Spreading Simulator / 病原世界 (HexaLife)

平面六边形格子上的病毒传播模拟。规则见 `docs/specs.md`，实现见 `docs/design.md`。

## 开发

```bash
npm install
npm run tauri:dev
```

模拟较重时可用 Release 编译跑开发窗口（更快）：

```bash
npm run tauri:dev:fast
```

配置：`config/default.toml`（仅在「开始」「重置」时读取）。

## 测试

```bash
cd src-tauri && cargo test
```

小地图集成测试在 `src-tauri/tests/sim_small.rs`。默认 10000×10000 / 1 亿人口需要较大内存，请谨慎在本机直接「开始」。
