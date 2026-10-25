use crate::config::{center_window_weight_sum, SimConfig};
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
}

pub struct PopulationPlanner {
    bg: f64,
    center_weights: Vec<f64>,
    total: f64,
    center_tables: Vec<AliasTable>,
    cell_count: u32,
}

impl PopulationPlanner {
    pub fn new(cfg: &SimConfig) -> Self {
        let bg = cfg.lambda_bg * cfg.width as f64 * cfg.height as f64;
        let center_weights: Vec<f64> = cfg
            .centers
            .iter()
            .map(|c| center_window_weight_sum(cfg, c))
            .collect();
        let center_tables: Vec<AliasTable> = cfg
            .centers
            .iter()
            .map(|c| build_center_alias(cfg, c))
            .collect();
        let total = bg + center_weights.iter().sum::<f64>();
        Self {
            bg,
            center_weights,
            total,
            center_tables,
            cell_count: cfg.width * cfg.height,
        }
    }

    pub fn pick_cell(&self, rng: &mut StdRng) -> u32 {
        let u = rng.gen::<f64>() * self.total;
        let mut acc = self.bg;
        if u < acc {
            return rng.gen_range(0..self.cell_count);
        }
        for (i, w) in self.center_weights.iter().enumerate() {
            acc += *w;
            if u < acc {
                return self.center_tables[i].sample(rng);
            }
        }
        self.center_tables
            .last()
            .map(|t| t.sample(rng))
            .unwrap_or(0)
    }
}

pub fn build_center_alias(cfg: &SimConfig, center: &crate::config::DensityCenter) -> AliasTable {
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
            let w = center.peak * (-(dist as f64).powi(2) / (2.0 * center.sigma * center.sigma)).exp();
            if w > 0.0 {
                cells.push(hex::cell_index(cfg.width, nq as u32, nr as u32));
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
