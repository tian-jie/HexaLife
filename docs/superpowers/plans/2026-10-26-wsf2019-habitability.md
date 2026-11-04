# WSF2019 宜居掩码 Implementation Plan

> **For agentic workers:** Implement this plan in the current workspace. Do not commit. Do not modify `config/viruses/covid.toml` or any other unrelated dirty file.

**Goal:** 用 DLR World Settlement Footprint 2019 把矩形格子映射成世界地图。格子带宜居标记；不宜居格子没有人，人的移动也不会进入。

**Architecture:** 离线脚本把 WSF2019 的 2°×2° 缩略图重采样成与 `width×height` 对齐的位图。运行时加载该位图。人口抽样、出生所在格、移动候选都跳过不宜居格。画面上不宜居是深蓝，宜居但无人是暗橄榄，有人仍用现有绿/黄/红。

**Tech Stack:** Rust（现有 `hexalife` crate）、Python 3 + Pillow（仅生成掩码）、Tauri 前端图例与悬停文案。

## Global Constraints

- 数据源目录：`https://download.geoservice.dlr.de/WSF2019/files/`
- WSF2019：EPSG:4326，定居点像素为 255，其余为 0。瓦片 `WSF2019_v1_{lon}_{lat}.tif` 覆盖 `[lon, lon+2] × [lat, lat+2]`，文件名是左下角。使用同名 `*_thumbnail.png`（512×512 RGB，北朝上，只有 `(0,0,0)` 与 `(255,255,255)`）。
- 瓦片有 0.1° 缓冲；缩略图按名义 2°×2° 对齐。由此产生的亚格误差可以接受。
- 地图不环绕。经度 −180 与 180 在矩形左右两边，移动不能绕过去。
- 未配置 `habitability` 时，每一格都宜居，并且 `PopulationPlanner::pick_cell` 的随机数消耗与现在完全一致（背景仍是 `rng.gen_range(0..cell_count)`）。现有测试必须继续通过。
- 不宜居格权重为 0。初始人口、常住地、出生、移动目的地都只能落在宜居格。出生已经落在父母当前格，父母不会站在不宜居格，所以出生逻辑不用另选格子。
- 不要下载 19GB 的 `WSF2019_cog.tif`，也不要把缩略图提交进 git。
- 不要 `git commit`。不要改 `config/viruses/covid.toml`。

## 坐标

格子 `(q, r)`，`index = r * width + q`。`r = 0` 在北，和屏幕向下增加 `r` 一致。

- `lon0 = -180 + q * 360 / width`
- `lon1 = -180 + (q + 1) * 360 / width`
- `lat_north = 90 - r * 180 / height`
- `lat_south = 90 - (r + 1) * 180 / height`

格子宜居，当且仅当有缩略图像素中心落在该经纬矩形内，且该像素任一通道为 255。目录里没有覆盖到的地方（海洋、无瓦片）不宜居。

## 掩码文件

路径例：`assets/masks/wsf2019-1000x1000.bin`

```
magic: 4 bytes b"HLHB"
u32 le width
u32 le height
bits: ceil(width * height / 8) bytes, row-major
bit (index % 8) of byte index/8 is 1 when habitable
```

## 文件

- Create: `src-tauri/src/world/habit.rs` — 加载、查询、全宜居快路径
- Create: `tools/build_wsf_mask.py` — 下载缩略图并写出 bin
- Create: `assets/masks/wsf2019-1000x1000.bin` — 跑脚本得到，提交用的掩码（实现者生成，本计划不内嵌字节）
- Create: `config/env/world-1000.toml`
- Modify: `src-tauri/src/world/mod.rs` — `World` 持有 `HabitMask`，建世界时加载，填充 tile 宜居计数
- Modify: `src-tauri/src/world/populate.rs` — 背景与中心只抽宜居格
- Modify: `src-tauri/src/sim/movement.rs` — 候选权重跳过不宜居格
- Modify: `src-tauri/src/ipc/view.rs` — 空格子颜色区分宜居
- Modify: `src-tauri/src/config.rs` — `[map] habitability`
- Modify: `src-tauri/src/state.rs` 与 `src-tauri/src/lib.rs` — `query_cell` 多返回 `habitable`
- Modify: `src/main.ts`、`src/ui/legend.ts`、`src/styles.css`
- Modify: `package.json`、`README.md`、`docs/specs.md`（只加一小节，不要重写）
- Modify: `.gitignore` — 忽略 `assets/wsf2019/`

## 运行时类型

```rust
pub struct HabitMask {
    pub width: u32,
    pub height: u32,
    /// None：每一格都宜居，不分配位图。
    bits: Option<Vec<u8>>,
    /// None：背景抽样仍走 0..cell_count。Some：仅这些格子，升序。
    habitable_index: Option<Vec<u32>>,
}

impl HabitMask {
    pub fn all(width: u32, height: u32) -> Self;
    pub fn from_bits(width: u32, height: u32, bits: Vec<u8>) -> Result<Self, String>;
    pub fn load(path: &Path, width: u32, height: u32) -> Result<Self, String>;
    pub fn is_habitable(&self, index: u32) -> bool;
    pub fn habitable_count(&self) -> u32;
}
```

`SimConfig` 增加 `pub habitability: Option<String>`。TOML：

```toml
[map]
habitability = "assets/masks/wsf2019-1000x1000.bin"
```

相对路径相对进程当前工作目录。文件缺失、magic 不对、宽高与地图不一致：`World::new` 返回中文错误，拒绝开始。

`World` 增加 `pub habit: HabitMask`。`TileCounts` 增加 `pub habitable_cells: u32`，只在 `World::new` 里按掩码数一遍，之后不随人口变化。

## 人口

`PopulationPlanner::new(cfg, habit: &HabitMask)`。

- `habit.habitable_index` 为 `None`：背景总权重仍是 `lambda_bg * width * height`，`pick_cell` 背景分支仍是 `rng.gen_range(0..cell_count)`。中心别名表不过滤。这是为了保住现有种子。
- 否则：背景总权重是 `lambda_bg * habitable_count`。背景分支在 `habitable_index` 里均匀抽。中心别名表跳过不宜居格；窗口内没有宜居格则该中心权重为 0，且没有表。`pick_cell` 不得抽到不宜居格。
- 有效总权重为 0：`World::new` 返回 `没有宜居格子，无法放置人口`。

## 移动

`pick_weighted_cell` 里，不宜居候选不进入加权列表。列表空或权重和 ≤ 0 时返回 `LINK_NONE`（留在原地）。不要改环上格子的生成函数。

## 颜色

`cell_rgba(count, infected, c_bright, habitable: bool) -> [u8; 4]`

- `count == 0 && !habitable` → `[12, 28, 56, 255]` 不宜居
- `count == 0 && habitable` → `[70, 78, 48, 255]` 宜居标记
- `count > 0` → 现有 HSV，与 `habitable` 无关

一个像素盖住多格时：人数仍按现在的方式相加。人数为 0 时，范围内任一格宜居就用宜居色，否则用不宜居色。`cpp >= TILE_SIZE` 的分块路径用 `TileCounts.habitable_cells > 0` 判断，允许块边界上的粗略误差。

悬停：`(${q}, ${r}) 宜居|不宜居 · 健康 … · 感染 … · 免疫 …`

`query_cell` 的回复从 `(u32, u32, u32)` 改为 `(u32, u32, u32, bool)`。越界或没有世界时宜居为 false。`CellCounts` 增加 `habitable: bool`。

图例两条：不宜居（`#0c1c38`）、宜居无人（`#464e30`）。删掉原来的「无人 = 纯黑」说明，或改成只在未启用掩码时仍可能看到黑；世界地图模式下空宜居格不是黑。未启用掩码时无人仍是黑色，图例写：不宜居 / 宜居无人 / 无感染者 / 感染占比。

## 世界环境

`config/env/world-1000.toml`：复制 `config/env/1000.toml` 的模拟、初始人口、生命、移动、UI。地图加上面的 `habitability`。`[density] lambda_bg = 1.0`，`centers = []`，这样人口均匀落在定居点上，不把原来的 6 个假城市堆进海洋。`initial_infection_mode = "random"`。

`package.json` 增加：

```json
"tauri:world": "HEXALIFE_ENV=config/env/world-1000.toml HEXALIFE_VIRUS=baseline tauri dev --release"
```

README 的环境表加一行 `config/env/world-1000.toml`。

`docs/specs.md` 在第 3 节末尾加「3.4 宜居」。写明：可选掩码；不宜居无人且不可进入；未配置则全部宜居；颜色两条。不要改第 3.2 节里「背景在全部格子中均匀抽取」的原文，改为加一句「启用宜居掩码后，背景只在宜居格中均匀抽取，不宜居格权重为 0」。

## 生成脚本

`tools/build_wsf_mask.py`，标准库 + Pillow。

```bash
python3 tools/build_wsf_mask.py --width 1000 --height 1000 --out assets/masks/wsf2019-1000x1000.bin
```

- 拉取目录页，正则收集 `WSF2019_v1_(-?\d+)_(-?\d+)_thumbnail.png`。
- 并发下载（约 16 线程）到 `assets/wsf2019/thumbnails/`，已存在且能被 Pillow 打开则跳过。
- 每个缩略图：左上为 `(lon, lat+2)`，右下为 `(lon+2, lat)`。像素中心映射到格子。任一通道等于 255 则把对应格子置位。同一格被多张瓦片碰到只记一次。
- 写出 HLHB 文件。打印宜居格数量。宜居格必须大于 0，且小于总格数（海洋必须存在）。若为 0 或等于全部格子，脚本以非 0 退出。

`.gitignore` 增加 `assets/wsf2019/`。

## 测试

在 `habit.rs`、`populate.rs`、`movement.rs`、`ipc/view.rs` 加 `#[cfg(test)]`。运行：

```bash
cd src-tauri && cargo test
```

必须通过，包括原有测试。

1. `from_bits` / `load`：4×1，只有 index 1 和 3。magic、宽高错误要失败。
2. 人口：4×1，掩码只有 1 和 3，`lambda_bg > 0`，无中心，放 30 人，格子只可能是 1 或 3。
3. 全不宜居：`World::new` 失败。
4. 移动：3×1，只有中间格宜居，一个人在中间，三种移动里通勤概率 1、距离 1..1。调用 `decide_moves` 后目的地是 `LINK_NONE`。
5. `cell_rgba(0, 0, 64, false) == [12, 28, 56, 255]`，`cell_rgba(0, 0, 64, true) == [70, 78, 48, 255]`，`cell_rgba(0, 0, 64, true)` 在「全部宜居、count 0」时仍是宜居色。未传掩码的旧调用要改成显式 `habitable` 参数，不要留默认值让调用点沉默。
6. 无 `habitability` 的现有 `cargo test` 全绿。

生成 `assets/masks/wsf2019-1000x1000.bin` 并确认文件头是 `HLHB`，宽高 1000，宜居格数量在总格数的 1% 到 80% 之间。

## 不做

- 不改六边形距离、感染接触范围、出生率公式。
- 不把 10000×10000 掩码作为本次产物。
- 不把缩略图或 GeoTIFF 放进仓库。
