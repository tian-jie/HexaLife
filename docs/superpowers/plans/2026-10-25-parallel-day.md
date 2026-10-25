# 并行单日模拟 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 在 1000×1000、1000 万人口下把一天的计算从数秒降到可交互（目标是疫情中前期大约每秒十几天），规则不变。

**Architecture:** 第 0 天的人口放置和初始感染仍用一条 `StdRng`，种子是 `cfg.seed`。从第 1 天起，每个阶段里每个人用 `WyRng::from_phase(seed, day, person_id, phase)` 自己取随机数。判定在 rayon 里并行，格子链表和计数仍由模拟线程按生命编号升序提交。去掉每天全员排序和 `alive` 的 `HashMap`。

**Tech Stack:** Rust，现有 Tauri/hexalife crate，新增 `rayon = "1"`。不新增别的随机数 crate。

## Global Constraints

- 不执行 git commit，不改 git config。
- 阶段顺序不变：免疫到期 → 移动 → 传播 → 自然死亡 → 病毒死亡/康复 → 出生 → 统计。
- 同一阶段内，状态修改按生命编号升序提交。
- 传播时，每个目标的命中判定按传染源编号升序，全部命中判定完成后再抽一次父代。
- 新生儿当天不移动、不传播、不被感染、不生育；编号只追加、不回收。
- 自然死亡先于病毒死亡。自然死亡的感染者不计入病毒死亡。
- 移动每天至多一种，区间顺序仍是通勤、探亲、旅游。目的地权重仍是 \(2^{-\beta \cdot dist(C,H)}\)，按格子编号升序累加抽样。
- 暂停、继续、速度、刷新不消耗随机数。
- 不引入格子计数模型（问题 2 不做）。
- 不要为了并行给 `World` 加锁。并行段只读 `World`。

---

### Task 1: 按阶段播种的 WyRng

**Files:**
- Create: `src-tauri/src/rng.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/rng.rs`（模块内单元测试）

**Interfaces:**
- Produces:
  - `pub struct WyRng`
  - `pub fn WyRng::from_phase(seed: u64, day: u32, person: u32, phase: u32) -> WyRng`
  - `pub fn WyRng::next_u64(&mut self) -> u64`
  - `pub fn WyRng::next_f64(&mut self) -> f64`，值域 \([0, 1)\)
  - `pub const PHASE_MOVE_KIND: u32 = 1`
  - `pub const PHASE_MOVE_DISTANCE: u32 = 2`
  - `pub const PHASE_MOVE_DEST: u32 = 3`
  - `pub const PHASE_SPREAD: u32 = 4`
  - `pub const PHASE_NATURAL_DEATH: u32 = 5`
  - `pub const PHASE_VIRUS_DEATH: u32 = 6`
  - `pub const PHASE_BIRTH: u32 = 7`

- [x] **Step 1: 写 `src-tauri/src/rng.rs`**

```rust
//! 每人、每天、每阶段一条独立随机流。同一种子重复运行，每天的判定相同。
//! 第 0 天的人口放置仍用 StdRng，不经过这里。

pub const PHASE_MOVE_KIND: u32 = 1;
pub const PHASE_MOVE_DISTANCE: u32 = 2;
pub const PHASE_MOVE_DEST: u32 = 3;
pub const PHASE_SPREAD: u32 = 4;
pub const PHASE_NATURAL_DEATH: u32 = 5;
pub const PHASE_VIRUS_DEATH: u32 = 6;
pub const PHASE_BIRTH: u32 = 7;

#[derive(Clone, Debug)]
pub struct WyRng {
    state: u64,
}

impl WyRng {
    pub fn from_phase(seed: u64, day: u32, person: u32, phase: u32) -> Self {
        let mixed = seed
            ^ (day as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (person as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
            ^ (phase as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
        let mut state = splitmix64(mixed);
        if state == 0 {
            state = 0xA076_1D64_78BD_642F;
        }
        Self { state }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0xA076_1D64_78BD_642F);
        let a = self.state as u128;
        let b = (self.state ^ 0xE703_7ED1_A0B4_28DB) as u128;
        let t = a.wrapping_mul(b);
        (t as u64) ^ (t >> 64) as u64
    }

    pub fn next_f64(&mut self) -> f64 {
        let bits = self.next_u64() >> 11;
        bits as f64 * (1.0 / ((1u64 << 53) as f64))
    }
}

fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_phase_is_stable_and_streams_differ() {
        let a = WyRng::from_phase(1, 3, 9, PHASE_MOVE_KIND);
        let b = WyRng::from_phase(1, 3, 9, PHASE_MOVE_KIND);
        let mut a = a;
        let mut b = b;
        let mut other = WyRng::from_phase(1, 3, 9, PHASE_BIRTH);
        assert_eq!(a.next_u64(), b.next_u64());
        assert_ne!(a.next_u64(), other.next_u64());
    }

    #[test]
    fn unit_interval() {
        let mut rng = WyRng::from_phase(7, 1, 0, PHASE_SPREAD);
        for _ in 0..1000 {
            let u = rng.next_f64();
            assert!(u >= 0.0 && u < 1.0);
        }
    }
}
```

- [x] **Step 2: 在 `src-tauri/src/lib.rs` 顶部加 `pub mod rng;`**

- [x] **Step 3: 运行**

Run: `cargo test --manifest-path src-tauri/Cargo.toml rng:: -- --test-threads=8`

Expected: `same_phase_is_stable_and_streams_differ` 与 `unit_interval` 通过。

---

### Task 2: 拆掉全员排序和 alive HashMap，分代人数改为增量

**Files:**
- Modify: `src-tauri/src/world/mod.rs`

**Interfaces:**
- Consumes: 现有 `Person`、`kill_person`、`infect_new`、`infect_initial`、`insert_into_cell`、`refresh_aggregate_stats`
- Produces:
  - `Person` 增加 `pub alive_index: u32`
  - `World` 删除 `alive_slot` 和 `day_sorted`
  - `World` 增加 `pub infected_by_gen: [u64; 4]`
  - `World::push_alive(&mut self, id: u32)`
  - `World::is_alive` 改为看 `state != STATE_DEAD`
  - `World::kill_person` 在改状态前，若原状态是感染，则 `infected_by_gen[generation]` 减 1；用 `alive_index` 做交换删除
  - `infect_initial` / `infect_new` 在写入感染状态后，把对应代际加 1
  - `refresh_aggregate_stats` 和 `rebuild_day_zero_stats` 直接复制 `self.infected_by_gen`，不再扫描 `alive`

- [x] **Step 1: 改 `Person` 和 `World` 字段**

`Person` 在 `generation` 后增加 `pub alive_index: u32`。所有构造 `Person` 的地方（`populate`、`sim/vital.rs` 的出生）都写 `alive_index: 0`，真正的下标由 `push_alive` 覆盖。

删除 `alive_slot: HashMap<u32, usize>` 和 `day_sorted: Vec<u32>`，以及只为它们服务的 import。`HashSet` 仍留给 `occupied_cells`。

`World::new` 里初始化 `infected_by_gen: [0; 4]`，不再创建 `alive_slot` 和 `day_sorted`。

- [x] **Step 2: 用 `push_alive` 替换 HashMap 插入**

```rust
pub(crate) fn push_alive(&mut self, id: u32) {
    let idx = self.alive.len() as u32;
    self.people[id as usize].alive_index = idx;
    self.alive.push(id);
}
```

`populate` 末尾的 `alive_slot.insert` + `alive.push` 改成 `self.push_alive(uid)`。删除 `alive_slot_insert`。`vital::births` 在本任务先改成调用 `push_alive`（出生随机数留到 Task 5）。

- [x] **Step 3: 改 `kill_person` 和 `is_alive`**

```rust
pub fn is_alive(&self, id: u32) -> bool {
    self.people
        .get(id as usize)
        .is_some_and(|p| p.state != STATE_DEAD)
}

pub fn kill_person(&mut self, id: u32) {
    let state = self.people[id as usize].state;
    if state == STATE_INFECTED {
        let g = generation_index(self.people[id as usize].generation);
        self.infected_by_gen[g] = self.infected_by_gen[g].saturating_sub(1);
    }
    self.remove_from_cell(id);
    self.people[id as usize].state = STATE_DEAD;
    let pos = self.people[id as usize].alive_index as usize;
    let last = self.alive.len() - 1;
    if pos != last {
        let moved = self.alive[last];
        self.alive[pos] = moved;
        self.people[moved as usize].alive_index = pos as u32;
    }
    self.alive.pop();
}
```

- [x] **Step 4: 感染计数**

`infect_initial` 和 `infect_new` 在设置 `generation` 之后：

```rust
let g = generation_index(self.people[id as usize].generation);
self.infected_by_gen[g] += 1;
```

`count_infected` 改为 `self.infected_by_gen.iter().sum::<u64>() as u32`。

`refresh_aggregate_stats` 与 `rebuild_day_zero_stats` 删除「扫描 alive 累加 infected_by_gen」的循环，改为 `self.stats.infected_by_gen = self.infected_by_gen`。健康/感染/免疫总数仍由 `tiles` 相加。

- [x] **Step 5: 编译**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib --offline 2>&1 | head` 先不要求全绿；本任务结束后至少 `cargo check --manifest-path src-tauri/Cargo.toml --tests` 里，`world` 不再引用 `alive_slot` / `day_sorted`。若 `tick.rs` 仍使用 `day_sorted`，Task 6 会删掉，本任务允许先留着编译失败的调用点，但优先在 Task 6 之前把 `tick.rs` 对 `day_sorted` 的使用改成直接扫 `0..people.len()` 里 `state != DEAD` 的编号，以便本任务能 `cargo check` 通过。免疫到期、移动、死亡、出生的随机数行为留到后面的任务，这里只保证字段和计数能编译。

---

### Task 3: 常住点权重查表，距离用二分，环上格子不截断

**Files:**
- Modify: `src-tauri/src/world/move_tables.rs`
- Modify: `src-tauri/src/world/hex.rs`（只在需要时加一个不截断的环填充）
- Test: `src-tauri/src/world/move_tables.rs` 单元测试

**Interfaces:**
- Produces:
  - `DistLut` 增加 `pub home_weight: Vec<f64>`，下标是到常住点的距离 \(d\)，值是 \(2^{-\beta d}\)。长度至少是 `width + height`。
  - `pub fn home_weight_at(lut: &DistLut, dist: u32) -> f64`，超出表长时返回 `0.0`
  - `MoveTables::pick_distance(lut, rng: &mut crate::rng::WyRng) -> Option<u32>` 用 `cum_weight.partition_point(|&cw| cw <= u)`，语义与原来的 `u < cw` 一致

- [x] **Step 1: 建表**

在 `build_dist_lut` 里：

```rust
let max_home = width.saturating_add(height);
let home_weight = if profile.beta == 0.0 {
    vec![1.0; max_home as usize + 1]
} else {
    (0..=max_home)
        .map(|d| 2.0_f64.powf(-profile.beta * d as f64))
        .collect()
};
```

把 `home_weight` 放进 `DistLut`。

- [x] **Step 2: 二分抽距离**

```rust
pub fn pick_distance(lut: &DistLut, rng: &mut crate::rng::WyRng) -> Option<u32> {
    if lut.distances.is_empty() || lut.total <= 0.0 {
        return None;
    }
    let u = rng.next_f64() * lut.total;
    let i = lut.cum_weight.partition_point(|&cw| cw <= u);
    lut.distances.get(i).copied().or_else(|| lut.distances.last().copied())
}

pub fn home_weight_at(lut: &DistLut, dist: u32) -> f64 {
    lut.home_weight.get(dist as usize).copied().unwrap_or(0.0)
}
```

- [x] **Step 3: 单元测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MoveProfile;

    #[test]
    fn home_weight_matches_formula() {
        let profile = MoveProfile {
            probability: 0.3,
            distance_min: 1,
            distance_max: 5,
            tau: 1.0,
            beta: 1.0,
        };
        let lut = build_dist_lut(100, 100, profile);
        for d in 0..40 {
            let expect = 2.0_f64.powf(-(d as f64));
            let got = home_weight_at(&lut, d);
            assert!((got - expect).abs() < 1e-12, "d={d} got={got} expect={expect}");
        }
    }
}
```

`build_dist_lut` 若是私有，测试放在同一模块即可调用。

- [x] **Step 4: 运行** `cargo test --manifest-path src-tauri/Cargo.toml home_weight_matches_formula -- --test-threads=8`

Expected: PASS。

---

### Task 4: 并行决定移动，按编号提交

**Files:**
- Modify: `src-tauri/Cargo.toml`（加入 `rayon = "1"`）
- Modify: `src-tauri/src/sim/movement.rs`
- Modify: `src-tauri/src/world/hex.rs`

**Interfaces:**
- Consumes: `WyRng`、`PHASE_MOVE_*`、`MoveTables::pick_distance`、`home_weight_at`、`hex::ring_cells_into`
- Produces: `pub fn decide_moves(world: &World, day: u32, n: usize) -> Vec<u32>`
  - 长度 `n`，下标是生命编号
  - 不移动时为 `hex::LINK_NONE`
  - 只读 `world`

- [x] **Step 1: `hex::ring_cells_into` 不得截断**

调用方传入的缓冲区长度必须 `>= 6 * d`。函数在写满时可以停，但 `decide_moves` 不许用 512 的上限把远距离环截断。距离环上的格子个数最多是 `6 * d`。

通勤（`d <= 8`，环最多 48 格）用栈数组 `[u32; 48]`。更远的距离用线程局部复用缓冲，避免每天数百万次堆分配，也避免多线程抢同一条 `Vec`：

```rust
use std::cell::RefCell;

thread_local! {
    static RING_BUF: RefCell<Vec<u32>> = RefCell::new(Vec::with_capacity(1024));
}
```

在 worker 里 `RING_BUF.with(|b| { let mut v = b.borrow_mut(); v.clear(); ... fill ... })`。填充函数把结果写入调用者给的 slice 或 `Vec`，返回实际个数。边界外的格子不写入。个数为 0 则留在原地。

- [x] **Step 2: 单人判定，随机流独立**

对编号 `id`，若 `id >= n` 或 `state == DEAD`，结果是 `LINK_NONE`，不取随机数。

感染者：传染日且 `infected_can_move == false`，或 `day == infected_on`，结果是 `LINK_NONE`，不取随机数。传染日定义与现在的 `try_move_person` 相同：`day > infected_on && day <= infected_on + incubation_days`。

否则：

1. `WyRng::from_phase(cfg.seed, day, id, PHASE_MOVE_KIND)` 取一个 `u`。按 `p_c`、`p_v`、`p_t` 选类型；都未命中则 `LINK_NONE`。
2. 新的 `WyRng::from_phase(..., PHASE_MOVE_DISTANCE)` 调用 `pick_distance`。`None` 则留在原地。
3. 收集该距离的环，用 `home_weight_at` 得到权重，**按格子编号升序排序**后累加抽样。抽样用 `WyRng::from_phase(..., PHASE_MOVE_DEST)` 的一个 `next_f64`。累加和为 0 则留在原地。抽样写法和现在的 `pick_weighted_cell` 相同：`u * sum`，第一个使累加和大于它的格子。

不要调用 `2.0_f64.powf`。

- [x] **Step 3: 并行**

```rust
use rayon::prelude::*;

pub fn decide_moves(world: &World, day: u32, n: usize) -> Vec<u32> {
    (0..n)
        .into_par_iter()
        .map(|id| decide_one(world, day, id as u32))
        .collect()
}
```

`World` 必须是 `Sync`。不要在这段里改 `World`。

---

### Task 5: 并行决定传播、病毒命运、自然死亡和出生

**Files:**
- Modify: `src-tauri/src/sim/disease.rs`
- Modify: `src-tauri/src/sim/vital.rs`

**Interfaces:**
- Produces:
  - `pub struct InfectionDecision { pub target: u32, pub parent: u32 }`
  - `pub fn decide_infections(world: &World, day: u32, active: &[u32]) -> Vec<InfectionDecision>`
    返回值按 `target` 升序
  - `pub enum VirusFate { Unchanged, Died, Recovered }`
  - `pub fn decide_virus_fates(world: &World, day: u32, active: &[u32]) -> Vec<VirusFate>`
    与 `active` 等长、对齐
  - `pub fn decide_natural_deaths(world: &World, day: u32, n: usize) -> Vec<bool>`
  - `pub fn decide_births(world: &World, day: u32, n: usize) -> Vec<bool>`
- 删除 `MAX_HITTERS`。命中列表不截断。
- 这些函数只读 `World`，不调用 `infect_new` / `kill_person`。

- [x] **Step 1: 传染源索引**

由 `active` 建成 `Vec<(cell, id)>`，按 `(cell, id)` 排序。查某格的传染源用两次 `partition_point`，格内编号已经有序。

目标集合：所有「自身或六邻居里有传染源」的格子上的健康者。收集后 `sort_unstable` 并去重。这与现在先收格子再收健康者的范围相同。

- [x] **Step 2: 每个目标一条 `PHASE_SPREAD` 流**

对目标 `id`，把 7 个格子里的传染源编号收进一个 `Vec`，再按编号升序排序（规格要求命中判定按传染源编号，不是按格子方位）。

按这个顺序，每个传染源取一次 `next_f64()`，小于 `world.cfg.daily_infection_prob(generation)` 则记入命中者 `(id, p)`。

没有命中者则这个目标没有决定。有命中者时再取一次 `next_f64()`，用现在的 `pick_parent` 累加规则选父代（把 `StdRng` 换成 `WyRng`）。

并行：

```rust
let mut slots: Vec<Option<InfectionDecision>> = vec![None; targets.len()];
slots.par_iter_mut().enumerate().for_each(|(i, slot)| {
    *slot = decide_target(world, day, targets[i], &pairs);
});
slots.into_iter().flatten().collect()
```

`targets` 已升序，所以展平后的决定也是升序。

- [x] **Step 3: 病毒命运**

对 `active` 里每个仍可能处理的人（调用时他们还活着），`PHASE_VIRUS_DEATH` 取一次 `next_f64()`。小于 `daily_lethality_prob` 则 `Died`。否则若 `day == infected_on + incubation_days` 则 `Recovered`，否则 `Unchanged`。`par_iter` 收集，顺序与 `active` 一致。

- [x] **Step 4: 自然死亡和出生**

长度 `n` 的 `Vec<bool>`，下标是编号。`state == DEAD` 的位置为 `false`，不取随机数。存活者用 `PHASE_NATURAL_DEATH` 或 `PHASE_BIRTH` 各取一次 `next_f64()`，小于对应日概率则为 `true`。概率为 0 时返回全 `false`，不取随机数。用 `into_par_iter`。

---

### Task 6: 把一天接起来

**Files:**
- Modify: `src-tauri/src/sim/tick.rs`
- Modify: `src-tauri/src/sim/disease.rs`（`virus_death_and_recovery` 改为按 `VirusFate` 提交，或把提交写在 `tick.rs`）
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/tests/sim_small.rs`
- Modify: `src-tauri/tests/bench_tick.rs`

**Interfaces:**
- Consumes: Task 4 和 Task 5 的 `decide_*`
- Produces: `pub fn tick_day(world: &mut World)`
  不再接收 `StdRng`。随机种子只用 `world.cfg.seed` 和 `world.day`。

- [x] **Step 1: `tick_day` 主体**

```rust
pub fn tick_day(world: &mut World) {
    let day = world.day + 1;
    world.day = day;
    world.stats = crate::stats::DayStats::zeroed();
    world.stats.day = day;

    // 免疫到期：保持现有按桶取出的顺序，无随机数。

    let n = world.people.len();
    let active_infected: Vec<u32> = (0..n)
        .filter_map(|i| {
            let id = i as u32;
            world.is_contagious_at_day_start(id, day).then_some(id)
        })
        .collect();

    let moves = decide_moves(world, day, n);
    for id in 0..n {
        let dest = moves[id];
        if dest != LINK_NONE {
            world.move_person(id as u32, dest);
        }
    }

    let infections = decide_infections(world, day, &active_infected);
    for hit in infections {
        if world.people[hit.target as usize].state != STATE_HEALTHY {
            continue;
        }
        let parent_gen = world.people[hit.parent as usize].generation;
        world.infect_new(hit.target, day, parent_gen);
    }

    let die = decide_natural_deaths(world, day, n);
    for id in 0..n {
        if die[id] && world.is_alive(id as u32) {
            world.kill_person(id as u32);
            world.stats.new_natural_deaths += 1;
            world.total_natural_deaths += 1;
        }
    }

    let fates = decide_virus_fates(world, day, &active_infected);
    for (id, fate) in active_infected.iter().copied().zip(fates) {
        if !world.is_alive(id) {
            continue;
        }
        match fate {
            VirusFate::Died => {
                let gi = generation_index(world.people[id as usize].generation);
                world.kill_person(id);
                world.stats.new_virus_deaths += 1;
                world.total_virus_deaths += 1;
                world.virus_deaths_by_gen[gi] += 1;
            }
            VirusFate::Recovered => {
                // 与现有 virus_death_and_recovery 的康复分支相同：
                // 感染计数改免疫，immune_until_day = infected_on + D + N + 1，
                // 推进 immune_expiring[bucket]，stats.new_immune += 1。
                // kill_person 已经减过 infected_by_gen 的人不会走到这里。
                // 康复必须自己把 infected_by_gen 减 1。
            }
            VirusFate::Unchanged => {}
        }
    }

    let n = world.people.len();
    let born = decide_births(world, day, n);
    for id in 0..n {
        if !born[id] || !world.is_alive(id as u32) {
            continue;
        }
        let cell = world.people[id].cell;
        let new_id = world.people.len() as u32;
        world.people.push(Person { /* 与现有 births 相同，state 健康，home = cell，alive_index: 0 */ });
        world.insert_into_cell(new_id);
        world.push_alive(new_id);
        world.stats.new_births += 1;
        world.total_births += 1;
    }

    world.refresh_aggregate_stats();
}
```

康复分支要先读 generation，把 `infected_by_gen` 减 1，再把状态改成免疫。不要再调用会扫描全员的逻辑。

删除 `sim/vital.rs` 里对 `StdRng` 的逐人循环；出生和自然死亡的提交放在 `tick.rs`。`vital.rs` 只留 `decide_*`。若这样更清楚，提交函数可以留在 `vital.rs`，但不得再接收 `StdRng`。

- [x] **Step 2: 模拟线程**

`state.rs` 里 `tick_day(&mut sess.world, &mut sess.rng)` 改为 `tick_day(&mut sess.world)`。

`Session` 可以不再保存 `rng`。`try_build_session` 仍用 `StdRng::seed_from_u64(cfg.seed)` 调用 `World::new`，建完世界就丢掉这个 rng。

- [x] **Step 3: 改现有测试的调用**

`tests/sim_small.rs` 和 `tests/bench_tick.rs` 改为 `tick_day(&mut world)`。人口数量和初始格子的断言不动，它们仍由 `StdRng` 决定。

- [x] **Step 4: 复现测试**

在 `tests/sim_small.rs` 增加：

```rust
#[test]
fn same_seed_same_days() {
    let cfg = tiny_config();
    let mut a = World::new(cfg.clone(), &mut StdRng::seed_from_u64(cfg.seed)).unwrap();
    let mut b = World::new(cfg.clone(), &mut StdRng::seed_from_u64(cfg.seed)).unwrap();
    for _ in 0..15 {
        tick_day(&mut a);
        tick_day(&mut b);
    }
    assert_eq!(a.people.len(), b.people.len());
    for (p, q) in a.people.iter().zip(b.people.iter()) {
        assert_eq!(p.cell, q.cell);
        assert_eq!(p.state, q.state);
        assert_eq!(p.generation, q.generation);
        assert_eq!(p.infected_on, q.infected_on);
    }
    assert_eq!(a.stats.infected, b.stats.infected);
    assert_eq!(a.stats.alive, b.stats.alive);
    let mut scanned = [0u64; 4];
    for p in &a.people {
        if p.state == hexalife_lib::world::STATE_INFECTED {
            scanned[hexalife_lib::world::generation_index(p.generation)] += 1;
        }
    }
    assert_eq!(scanned, a.infected_by_gen);
}
```

再增加一个父代顺序测试：20×20、人口 80、`infection_rate = [1.0, 1.0, 1.0, 1.0]`、`lethality_rate` 全 0、`infected_can_move = false`、出生率和自然死亡率 0、种子 3、初始 1 名感染者。跑到出现第二名感染者的那一天，断言新感染者的 `generation == 1`。用测试里单独的 `WyRng::from_phase(cfg.seed, day, target, PHASE_SPREAD)` 按编号升序重放命中和父代抽样，断言父代编号与模拟结果一致。若这一天没有新感染者，测试失败并打印天数，不要放宽概率去绕过。

- [x] **Step 5: 运行正确性测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=8`

Expected: 全部通过。`rng_pool_compare` 仍可保留，它不进入 `tick_day`。

---

### Task 7: 规格文字与速度观察

**Files:**
- Modify: `docs/specs.md` 第 10 节
- Modify: `docs/design.md` 第 1、2、4.2、7 节里关于「一条 StdRng、模拟逻辑单线程、每天排序 alive」的句子
- Modify: `src-tauri/tests/bench_tick.rs`

- [x] **Step 1: 替换规格第 10 节的随机流描述**

写成下面的规则，删掉「只使用一条随机流」：

- 人口放置和初始感染仍是一条 `StdRng`，种子来自当次开始或重置的 `seed`，顺序仍是规格原来的第 1、2 步。
- 第 1 天及以后，每个阶段的每个生命使用独立随机流，由 `(seed, 天数, 生命编号, 阶段)` 确定。阶段编号固定为：移动类型 1、移动距离 2、移动目的地 3、传播 4、自然死亡 5、病毒死亡 6、出生 7。
- 阶段顺序和「按生命编号升序提交结果」不变。传播阶段对每个目标仍先按传染源编号升序做命中判定，再抽父代；这些随机数都来自该目标的传播流。
- 同一种子、同一配置，重复开始得到相同的每日统计。速度、暂停、继续、刷新不改变该统计。
- 判定可以并行。并行不得改变上面的随机流和提交顺序。

同步改 `docs/design.md` 里矛盾的句子：模拟线程仍是唯一改世界的线程；一天内部的判定用多核；人的存活下标存在 `Person.alive_index`，不再用 `HashMap`，也不再每天排序。

- [x] **Step 2: 速度探针**

把 `bench_tick.rs` 扩成：1000×1000、人口 1_000_000、种子 1、初始感染 1，建世界后连续 `tick_day` 5 天，每天 `eprintln` 耗时。测试只断言 `world.day == 5` 且 `stats.alive > 0`，不断言具体毫秒数。

另写一个 `#[ignore]` 测试：人口 `10_000_000`，同样的地图，跑 2 天并打印耗时。不放进默认 `cargo test`。

- [x] **Step 3: 跑探针**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --release bench_world -- --nocapture --test-threads=1`

Expected: 测试通过，并打印每天毫秒数。1_000_000 人口时，单天应明显低于 1 秒。若高于 1 秒，在测试输出里保留数字，不要用放宽断言蒙混；回到移动环的分配和传播索引上查，不要改随机流定义。

有余力时再跑：

`cargo test --manifest-path src-tauri/Cargo.toml --release -- --ignored --nocapture bench_10m --test-threads=1`

10_000_000 人口、疫情最初两天，单天目标是大约 1 秒以内。做不到就在完成说明里写实测数字，不要改规则去凑时间。

---

## 完成标准

- `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=8` 通过。
- 同一种子两份世界跑 15 天，人的格子、状态、代际、感染日一致，分代感染计数与实扫一致。
- 第 0 天的放置仍只使用 `StdRng`。
- 没有 git commit。
