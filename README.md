# Virus Spreading Simulator / 病原世界 (HexaLife)

平面六边形格子上的病毒传播模拟。规则见 `docs/specs.md`，实现见 `docs/design.md`。

## 开发

```bash
npm install
npm run tauri:dev
```

开始和重置时同时读两个文件：

- `HEXALIFE_ENV`：环境。地图大小、人口、密度、出行、出生和自然死亡。
- `HEXALIFE_VIRUS`：病毒。发病、感染率、致死率、免疫、发病者能否移动，以及初始暴发方式。

病毒文件里出现的字段会盖过环境文件。都未设置时使用 `config/env/1000.toml` 和 `config/viruses/baseline.toml`。

`HEXALIFE_VIRUS` 可以是名字，也可以是文件路径。`HEXALIFE_ENV` 未设置、但设置了 `HEXALIFE_CONFIG` 时，把后者当作环境文件。

```bash
HEXALIFE_ENV=config/env/dev-small.toml HEXALIFE_VIRUS=covid npm run tauri:dev
```

推荐组合：

```bash
npm run tauri:covid
npm run tauri:norovirus
npm run tauri:bubonic
npm run tauri:pneumonic
```

| 环境文件 | 规模 |
|---|---|
| `config/env/1000.toml` | 1000×1000，1000 万人 |
| `config/env/world-1000.toml` | 1000×1000 世界地图（WSF2019 宜居掩码） |
| `config/env/world-10000.toml` | 10000×10000 世界地图（均匀人口 + GeoTIFF 掩码） |
| `config/env/10000-world.toml` | 同 `10000.toml` 城市密度 + `wsf2019-10000x10000.bin` |
| `config/env/1000-mobile.toml` | 同上，城市更集中，远行更多 |
| `config/env/cities.toml` | 1000×1000，人口集中在城市 |
| `config/env/dev-small.toml` | 200×200，5 万人 |
| `config/env/10000.toml` | 10000×10000，1 亿人 |

| `HEXALIFE_VIRUS` | 文件 |
|---|---|
| `baseline` | `config/viruses/baseline.toml` |
| `covid` | `config/viruses/covid.toml` |
| `norovirus` | `config/viruses/norovirus.toml` |
| `bubonic` | `config/viruses/bubonic.toml` |
| `pneumonic` | `config/viruses/pneumonic.toml` |

## WSF2019 宜居掩码（世界地图）

高精度掩码从 DLR 全分辨率 GeoTIFF 瓦片生成（非 512px 缩略图）。需 Python 依赖：

```bash
pip install -r tools/requirements-wsf.txt
```

生成 `1000×1000`、`10000×10000`、`100000×100000` 三档（约数小时，100k 掩码约 1.25 GiB，已 gitignore）：

```bash
npm run build:wsf-masks
```

单档示例：

```bash
python3 tools/build_wsf_mask.py --source tif --width 1000 --height 1000 \
  --out assets/masks/wsf2019-1000x1000.bin
```

瓦片缓存：`assets/wsf2019/tiles/`（已忽略）。快速预览可用 `--source thumbnail`。

## 测试

```bash
cd src-tauri && cargo test
```

小地图集成测试在 `src-tauri/tests/sim_small.rs`。`config/env/10000.toml` 需要较大内存，请谨慎直接开始。
