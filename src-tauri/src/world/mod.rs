pub mod hex;
pub mod move_tables;
pub mod populate;

use crate::config::{InfectionMode, SimConfig};
use crate::world::move_tables::MoveTables;
use crate::stats::DayStats;
use crate::world::hex::LINK_NONE;
use crate::world::populate::PopulationPlanner;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::Rng;
use std::collections::HashSet;
pub const TILE_SIZE: u32 = 100;

pub const STATE_HEALTHY: u8 = 0;
pub const STATE_INFECTED: u8 = 1;
pub const STATE_IMMUNE: u8 = 2;
pub const STATE_DEAD: u8 = 3;

#[derive(Clone, Copy)]
pub struct Person {
    pub cell: u32,
    pub home: u32,
    pub next_in_cell: u32,
    pub prev_in_cell: u32,
    pub infected_on: u32,
    pub immune_until_day: u32,
    pub state: u8,
    pub generation: u8,
    pub alive_index: u32,
}

#[derive(Clone)]
pub struct TileCounts {
    pub healthy: u32,
    pub infected: u32,
    pub immune: u32,
}

#[derive(Clone, Copy, Default)]
pub struct CellPop {
    pub healthy: u32,
    pub infected: u32,
    pub immune: u32,
}

pub struct World {
    pub cfg: SimConfig,
    pub day: u32,
    pub people: Vec<Person>,
    pub alive: Vec<u32>,
    pub infected_by_gen: [u64; 4],
    pub cell_head: Vec<u32>,
    pub cell_pop: Vec<CellPop>,
    pub tiles: Vec<TileCounts>,
    pub tiles_w: u32,
    pub tiles_h: u32,
    pub active_infected: Vec<u32>,
    pub immune_expiring: Vec<Vec<u32>>,
    pub stats: DayStats,
    pub virus_deaths_by_gen: [u64; 4],
    pub total_virus_deaths: u64,
    pub total_natural_deaths: u64,
    pub total_births: u64,
    pub move_tables: MoveTables,
}

impl World {
    pub fn new(cfg: SimConfig, rng: &mut StdRng) -> Result<Self, String> {
        let cell_count = cfg.width * cfg.height;
        let tiles_w = (cfg.width + TILE_SIZE - 1) / TILE_SIZE;
        let tiles_h = (cfg.height + TILE_SIZE - 1) / TILE_SIZE;
        let immune_buckets = cfg.max_days as usize + cfg.immune_days as usize + 32;
        let move_tables = MoveTables::from_config(cfg.width, cfg.height, &cfg.movement);
        let mut world = World {
            cfg,
            day: 0,
            people: Vec::new(),
            alive: Vec::new(),
            infected_by_gen: [0; 4],
            cell_head: vec![LINK_NONE; cell_count as usize],
            cell_pop: vec![CellPop::default(); cell_count as usize],
            tiles: vec![
                TileCounts {
                    healthy: 0,
                    infected: 0,
                    immune: 0,
                };
                (tiles_w * tiles_h) as usize
            ],
            tiles_w,
            tiles_h,
            active_infected: Vec::new(),
            immune_expiring: vec![Vec::new(); immune_buckets],
            stats: DayStats::zeroed(),
            virus_deaths_by_gen: [0; 4],
            total_virus_deaths: 0,
            total_natural_deaths: 0,
            total_births: 0,
            move_tables,
        };
        world.populate(rng)?;
        world.place_initial_infections(rng)?;
        world.rebuild_day_zero_stats();
        Ok(world)
    }

    fn populate(&mut self, rng: &mut StdRng) -> Result<(), String> {
        let n = self.cfg.initial_population as usize;
        let planner = PopulationPlanner::new(&self.cfg);
        self.people.reserve(n);
        self.alive.reserve(n);
        for id in 0..n {
            let cell = planner.pick_cell(rng);
            let person = Person {
                cell,
                home: cell,
                next_in_cell: LINK_NONE,
                prev_in_cell: LINK_NONE,
                infected_on: 0,
                immune_until_day: 0,
                state: STATE_HEALTHY,
                generation: 0,
                alive_index: 0,
            };
            self.people.push(person);
            self.insert_into_cell(id as u32);
            let uid = id as u32;
            self.push_alive(uid);
        }
        if self.alive.len() as u64 != self.cfg.initial_population {
            return Err("初始人口数量不匹配".into());
        }
        Ok(())
    }

    fn place_initial_infections(&mut self, rng: &mut StdRng) -> Result<(), String> {
        let cell_count = self.cfg.initial_infected_count;
        if cell_count == 0 {
            return Ok(());
        }
        match self.cfg.initial_infection_mode {
            InfectionMode::Random => {
                let mut cells: Vec<u32> = self.occupied_cells();
                cells.shuffle(rng);
                if cells.len() < cell_count as usize {
                    return Err(format!(
                        "initial.initial_infected_count = {} 大于有人的格子数 {}",
                        cell_count,
                        cells.len()
                    ));
                }
                for cell in cells.into_iter().take(cell_count as usize) {
                    self.infect_cell_by_gen0_rate(cell, rng)?;
                }
            }
            InfectionMode::Cell => {
                let cell = self.cfg.initial_infection_cell;
                if cell >= self.cfg.width * self.cfg.height {
                    return Err("initial.initial_infection_cell 超出地图".into());
                }
                if self.cell_head[cell as usize] == LINK_NONE {
                    return Err("指定初始感染格子没有生命".into());
                }
                if cell_count != 1 {
                    return Err(
                        "initial_infection_mode = cell 时 initial_infected_count 必须为 1（感染一格）"
                            .into(),
                    );
                }
                self.infect_cell_by_gen0_rate(cell, rng)?;
            }
        }
        if self.count_infected() == 0 {
            return Err(
                "初始感染格内未产生感染者（0 代总感染率过低或该格人数过少，可提高 disease.infection_rate[0]）"
                    .into(),
            );
        }
        Ok(())
    }

    fn count_infected(&self) -> u32 {
        self.infected_by_gen.iter().sum::<u64>() as u32
    }

    fn occupied_cells(&self) -> Vec<u32> {
        let mut set = HashSet::new();
        for id in &self.alive {
            set.insert(self.people[*id as usize].cell);
        }
        let mut cells: Vec<u32> = set.into_iter().collect();
        cells.sort_unstable();
        cells
    }

    /// 在指定格内，每名健康者以 0 代总感染率 P_g 独立判定是否成为第 0 天感染者。
    fn infect_cell_by_gen0_rate(&mut self, cell: u32, rng: &mut StdRng) -> Result<(), String> {
        let p_g = self.cfg.infection_rate[0];
        if !(0.0..=1.0).contains(&p_g) {
            return Err("disease.infection_rate[0] 必须在 0 到 1 之间".into());
        }
        let mut healthy_in_cell: Vec<u32> = Vec::new();
        let mut cur = self.cell_head[cell as usize];
        while cur != LINK_NONE {
            let id = cur;
            cur = self.people[cur as usize].next_in_cell;
            if self.people[id as usize].state == STATE_HEALTHY {
                healthy_in_cell.push(id);
            }
        }
        if healthy_in_cell.is_empty() {
            return Err("初始感染格内没有健康者".into());
        }
        for id in healthy_in_cell {
            if p_g >= 1.0 || rng.gen::<f64>() < p_g {
                self.infect_initial(id);
            }
        }
        Ok(())
    }

    fn infect_initial(&mut self, id: u32) {
        let cell = self.people[id as usize].cell;
        let prev = self.people[id as usize].state;
        self.adjust_pop(cell, prev, -1);
        let p = &mut self.people[id as usize];
        p.state = STATE_INFECTED;
        p.infected_on = 0;
        p.generation = 0;
        self.adjust_pop(cell, STATE_INFECTED, 1);
        let g = generation_index(self.people[id as usize].generation);
        self.infected_by_gen[g] += 1;
    }

    pub fn infect_new(&mut self, id: u32, day: u32, parent_gen: u8) {
        let cell = self.people[id as usize].cell;
        let prev = self.people[id as usize].state;
        self.adjust_pop(cell, prev, -1);
        let ng = parent_gen.saturating_add(1);
        let generation = if ng >= 3 { 3 } else { ng };
        let p = &mut self.people[id as usize];
        p.state = STATE_INFECTED;
        p.infected_on = day;
        p.generation = generation;
        self.adjust_pop(cell, STATE_INFECTED, 1);
        let g = generation_index(self.people[id as usize].generation);
        self.infected_by_gen[g] += 1;
        self.stats.new_infections += 1;
    }

    pub(crate) fn insert_into_cell(&mut self, id: u32) {
        let cell = self.people[id as usize].cell;
        let head = self.cell_head[cell as usize];
        self.people[id as usize].next_in_cell = head;
        self.people[id as usize].prev_in_cell = LINK_NONE;
        if head != LINK_NONE {
            self.people[head as usize].prev_in_cell = id;
        }
        self.cell_head[cell as usize] = id;
        self.adjust_pop(cell, STATE_HEALTHY, 1);
    }

    fn remove_from_cell(&mut self, id: u32) {
        let cell = self.people[id as usize].cell;
        let state = self.people[id as usize].state;
        self.adjust_pop(cell, state, -1);
        let prev = self.people[id as usize].prev_in_cell;
        let next = self.people[id as usize].next_in_cell;
        if prev == LINK_NONE {
            self.cell_head[cell as usize] = next;
        } else {
            self.people[prev as usize].next_in_cell = next;
        }
        if next != LINK_NONE {
            self.people[next as usize].prev_in_cell = prev;
        }
        self.people[id as usize].next_in_cell = LINK_NONE;
        self.people[id as usize].prev_in_cell = LINK_NONE;
    }

    fn link_into_cell(&mut self, id: u32, new_cell: u32) {
        let state = self.people[id as usize].state;
        let head = self.cell_head[new_cell as usize];
        self.people[id as usize].cell = new_cell;
        self.people[id as usize].next_in_cell = head;
        self.people[id as usize].prev_in_cell = LINK_NONE;
        if head != LINK_NONE {
            self.people[head as usize].prev_in_cell = id;
        }
        self.cell_head[new_cell as usize] = id;
        self.adjust_pop(new_cell, state, 1);
    }

    pub fn is_alive(&self, id: u32) -> bool {
        self.people
            .get(id as usize)
            .is_some_and(|p| p.state != STATE_DEAD)
    }

    pub(crate) fn push_alive(&mut self, id: u32) {
        let idx = self.alive.len() as u32;
        self.people[id as usize].alive_index = idx;
        self.alive.push(id);
    }

    pub fn move_person(&mut self, id: u32, new_cell: u32) {
        if new_cell == self.people[id as usize].cell {
            return;
        }
        self.remove_from_cell(id);
        self.link_into_cell(id, new_cell);
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

    pub(crate) fn adjust_pop(&mut self, cell: u32, state: u8, delta: i32) {
        let pop = &mut self.cell_pop[cell as usize];
        match state {
            STATE_HEALTHY => pop.healthy = (pop.healthy as i32 + delta).max(0) as u32,
            STATE_INFECTED => pop.infected = (pop.infected as i32 + delta).max(0) as u32,
            STATE_IMMUNE => pop.immune = (pop.immune as i32 + delta).max(0) as u32,
            _ => {}
        }
        let (q, r) = hex::qr_from_index(self.cfg.width, cell);
        let (tx, ty) = hex::tile_coords(TILE_SIZE, q, r);
        let idx = (ty * self.tiles_w + tx) as usize;
        let tile = &mut self.tiles[idx];
        match state {
            STATE_HEALTHY => tile.healthy = (tile.healthy as i32 + delta).max(0) as u32,
            STATE_INFECTED => tile.infected = (tile.infected as i32 + delta).max(0) as u32,
            STATE_IMMUNE => tile.immune = (tile.immune as i32 + delta).max(0) as u32,
            _ => {}
        }
    }

    pub fn tile_index_for_cell(&self, cell: u32) -> usize {
        let (q, r) = hex::qr_from_index(self.cfg.width, cell);
        let (tx, ty) = hex::tile_coords(TILE_SIZE, q, r);
        (ty * self.tiles_w + tx) as usize
    }

    pub fn is_contagious_at_day_start(&self, id: u32, day: u32) -> bool {
        let p = &self.people[id as usize];
        p.state == STATE_INFECTED
            && day > p.infected_on
            && day <= p.infected_on + self.cfg.incubation_days
    }

    pub fn cell_counts(&self, cell: u32) -> (u32, u32, u32) {
        let p = &self.cell_pop[cell as usize];
        (p.healthy, p.infected, p.immune)
    }

    fn rebuild_day_zero_stats(&mut self) {
        self.stats = DayStats::zeroed();
        self.stats.day = 0;
        self.stats.alive = self.alive.len() as u64;
        for t in &self.tiles {
            self.stats.healthy += t.healthy as u64;
            self.stats.infected += t.infected as u64;
            self.stats.immune += t.immune as u64;
        }
        self.stats.infected_by_gen = self.infected_by_gen;
    }

    pub fn refresh_aggregate_stats(&mut self) {
        self.stats.alive = self.alive.len() as u64;
        self.stats.healthy = 0;
        self.stats.infected = 0;
        self.stats.immune = 0;
        for t in &self.tiles {
            self.stats.healthy += t.healthy as u64;
            self.stats.infected += t.infected as u64;
            self.stats.immune += t.immune as u64;
        }
        self.stats.infected_by_gen = self.infected_by_gen;
        self.stats.total_virus_deaths = self.total_virus_deaths;
        self.stats.total_natural_deaths = self.total_natural_deaths;
        self.stats.total_births = self.total_births;
        self.stats.virus_deaths_by_gen = self.virus_deaths_by_gen;
    }
}

pub fn generation_index(gen: u8) -> usize {
    gen.min(3) as usize
}
