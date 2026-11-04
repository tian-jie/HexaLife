use crate::config::{center_window_weight_sum, SimConfig};
use crate::world::habit::HabitMask;
use crate::world::hex;
use rand::rngs::StdRng;
use rand::Rng;

pub struct AliasTable {
    cells: Vec<u32>,
    prob: Vec<f64>,
    alias: Vec<usize>,
}

impl AliasTable {
    pub fn sample(&self, rng: &mut StdRng) -> u32 {
        let i = rng.gen_range(0..self.cells.len());
        if rng.gen::<f64>() < self.prob[i] {
            self.cells[i]
        } else {
            self.cells[self.alias[i]]
        }
    }

    fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

pub struct PopulationPlanner {
    bg: f64,
    center_weights: Vec<f64>,
    total: f64,
    center_tables: Vec<AliasTable>,
    cell_count: u32,
    habitable_index: Option<Vec<u32>>,
}

impl PopulationPlanner {
    pub fn new(cfg: &SimConfig, habit: &HabitMask) -> Self {
        let cell_count = cfg.width * cfg.height;
        let habitable_index = habit.habitable_index().map(|s| s.to_vec());
        let bg = match &habitable_index {
            None => cfg.lambda_bg * cfg.width as f64 * cfg.height as f64,
            Some(_) => cfg.lambda_bg * habit.habitable_count() as f64,
        };
        let center_tables: Vec<AliasTable> = cfg
            .centers
            .iter()
            .map(|c| build_center_alias(cfg, c, habit))
            .collect();
        let center_weights: Vec<f64> = if habitable_index.is_none() {
            cfg.centers
                .iter()
                .map(|c| center_window_weight_sum(cfg, c))
                .collect()
        } else {
            cfg.centers
                .iter()
                .zip(center_tables.iter())
                .map(|(c, table)| {
                    if table.is_empty() {
                        0.0
                    } else {
                        center_window_weight_sum_habit(cfg, c, habit)
                    }
                })
                .collect()
        };
        let total = bg + center_weights.iter().sum::<f64>();
        Self {
            bg,
            center_weights,
            total,
            center_tables,
            cell_count,
            habitable_index,
        }
    }

    pub fn total_weight(&self) -> f64 {
        self.total
    }

    pub fn pick_cell(&self, rng: &mut StdRng) -> u32 {
        let u = rng.gen::<f64>() * self.total;
        let mut acc = self.bg;
        if u < acc {
            return match &self.habitable_index {
                None => rng.gen_range(0..self.cell_count),
                Some(indices) => {
                    let i = rng.gen_range(0..indices.len());
                    indices[i]
                }
            };
        }
        for (i, w) in self.center_weights.iter().enumerate() {
            acc += *w;
            if u < acc {
                return self.center_tables[i].sample(rng);
            }
        }
        match &self.habitable_index {
            None => self
                .center_tables
                .last()
                .map(|t| t.sample(rng))
                .unwrap_or(0),
            Some(indices) => {
                for table in self.center_tables.iter().rev() {
                    if !table.is_empty() {
                        return table.sample(rng);
                    }
                }
                indices[rng.gen_range(0..indices.len())]
            }
        }
    }
}

fn center_window_weight_sum_habit(
    cfg: &SimConfig,
    center: &crate::config::DensityCenter,
    habit: &HabitMask,
) -> f64 {
    let (cq, cr) = hex::qr_from_index(cfg.width, center.cell);
    let radius = (cfg.truncation * center.sigma).ceil() as u32;
    let mut sum = 0.0;
    let rad = radius as i32;
    for dr in -rad..=rad {
        for dq in -rad..=rad {
            let nq = cq as i32 + dq;
            let nr = cr as i32 + dr;
            if !hex::in_bounds(cfg.width, cfg.height, nq, nr) {
                continue;
            }
            let idx = hex::cell_index(cfg.width, nq as u32, nr as u32);
            if !habit.is_habitable(idx) {
                continue;
            }
            let dist = hex::hex_distance(cq, cr, nq as u32, nr as u32);
            let limit = cfg.truncation * center.sigma;
            if dist as f64 > limit {
                continue;
            }
            let w = center.peak
                * (-(dist as f64).powi(2) / (2.0 * center.sigma * center.sigma)).exp();
            sum += w;
        }
    }
    sum
}

pub fn build_center_alias(
    cfg: &SimConfig,
    center: &crate::config::DensityCenter,
    habit: &HabitMask,
) -> AliasTable {
    let (cq, cr) = hex::qr_from_index(cfg.width, center.cell);
    let radius = (cfg.truncation * center.sigma).ceil() as u32;
    let mut cells = Vec::new();
    let mut weights = Vec::new();
    let rad = radius as i32;
    for dr in -rad..=rad {
        for dq in -rad..=rad {
            let nq = cq as i32 + dq;
            let nr = cr as i32 + dr;
            if !hex::in_bounds(cfg.width, cfg.height, nq, nr) {
                continue;
            }
            let dist = hex::hex_distance(cq, cr, nq as u32, nr as u32);
            if dist as f64 > cfg.truncation * center.sigma {
                continue;
            }
            let idx = hex::cell_index(cfg.width, nq as u32, nr as u32);
            if !habit.is_habitable(idx) {
                continue;
            }
            let w = center.peak
                * (-(dist as f64).powi(2) / (2.0 * center.sigma * center.sigma)).exp();
            if w > 0.0 {
                cells.push(idx);
                weights.push(w);
            }
        }
    }
    build_alias(cells, weights)
}

fn build_alias(cells: Vec<u32>, weights: Vec<f64>) -> AliasTable {
    let n = cells.len();
    let sum = weights.iter().sum::<f64>();
    let mut prob = vec![0.0; n];
    let mut alias = vec![0usize; n];
    if n == 0 {
        return AliasTable {
            cells,
            prob,
            alias,
        };
    }
    let mut scaled: Vec<f64> = weights.iter().map(|w| w * n as f64 / sum).collect();
    let mut small = Vec::new();
    let mut large = Vec::new();
    for (i, p) in scaled.iter().enumerate() {
        if *p < 1.0 {
            small.push(i);
        } else {
            large.push(i);
        }
    }
    while let (Some(s), Some(l)) = (small.pop(), large.pop()) {
        prob[s] = scaled[s];
        alias[s] = l;
        let new_l = scaled[l] + scaled[s] - 1.0;
        scaled[l] = new_l;
        if new_l < 1.0 {
            small.push(l);
        } else {
            large.push(l);
        }
    }
    for i in large.into_iter().chain(small) {
        prob[i] = 1.0;
    }
    AliasTable { cells, prob, alias }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SimConfig;
    use rand::SeedableRng;
    use std::io::Write;

    fn write_mask_4x1(path: &std::path::Path) {
        let bits = [0b1010u8];
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(b"HLHB").unwrap();
        f.write_all(&4u32.to_le_bytes()).unwrap();
        f.write_all(&1u32.to_le_bytes()).unwrap();
        f.write_all(&bits).unwrap();
    }

    #[test]
    fn population_only_on_habitable_cells() {
        let dir = std::env::temp_dir().join("hexalife_pop_test");
        std::fs::create_dir_all(&dir).unwrap();
        let mask_path = dir.join("pop.bin");
        write_mask_4x1(&mask_path);

        let mut cfg = SimConfig::defaults();
        cfg.width = 4;
        cfg.height = 1;
        cfg.initial_population = 30;
        cfg.initial_infected_count = 0;
        cfg.lambda_bg = 1.0;
        cfg.centers = vec![];
        cfg.habitability = Some(mask_path.to_string_lossy().into_owned());

        let mut rng = StdRng::seed_from_u64(99);
        let world = crate::world::World::new(cfg, &mut rng).expect("world");
        for id in &world.alive {
            let cell = world.people[*id as usize].cell;
            assert!(cell == 1 || cell == 3, "cell {cell}");
        }
    }
}
