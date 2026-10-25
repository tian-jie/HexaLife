use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct RawConfig {
    pub map: Option<MapSection>,
    pub sim: Option<SimSection>,
    pub initial: Option<InitialSection>,
    pub density: Option<DensitySection>,
    pub disease: Option<DiseaseSection>,
    pub vital: Option<VitalSection>,
    pub movement: Option<MovementSection>,
    pub ui: Option<UiSection>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MapSection {
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimSection {
    pub max_days: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InitialSection {
    pub seed: Option<u64>,
    pub initial_population: Option<u64>,
    pub initial_infected_count: Option<u32>,
    pub initial_infection_mode: Option<String>,
    pub initial_infection_cell: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DensityCenter {
    pub cell: u32,
    pub peak: f64,
    pub sigma: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DensitySection {
    pub lambda_bg: Option<f64>,
    pub truncation: Option<f64>,
    pub centers: Option<Vec<DensityCenter>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiseaseSection {
    pub incubation_days: Option<u32>,
    pub immune_days: Option<u32>,
    pub infection_rate: Option<Vec<f64>>,
    pub lethality_rate: Option<Vec<f64>>,
    pub infected_can_move: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VitalSection {
    pub birth_rate_per_day: Option<f64>,
    pub natural_death_rate_per_day: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MoveKind {
    pub probability: Option<f64>,
    pub distance_min: Option<u32>,
    pub distance_max: Option<u32>,
    pub tau: Option<f64>,
    pub beta: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MovementSection {
    pub commute: Option<MoveKind>,
    pub visit: Option<MoveKind>,
    pub travel: Option<MoveKind>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UiSection {
    pub c_bright: Option<u32>,
    pub refresh_interval_ms: Option<u64>,
    pub speed_min: Option<f32>,
    pub speed_max: Option<f32>,
    pub default_speed: Option<f32>,
    pub speed_fastest: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct SimConfig {
    pub width: u32,
    pub height: u32,
    pub max_days: u32,
    pub seed: u64,
    pub initial_population: u64,
    pub initial_infected_count: u32,
    pub initial_infection_mode: InfectionMode,
    pub initial_infection_cell: u32,
    pub lambda_bg: f64,
    pub truncation: f64,
    pub centers: Vec<DensityCenter>,
    pub incubation_days: u32,
    pub immune_days: u32,
    pub infection_rate: [f64; 4],
    pub lethality_rate: [f64; 4],
    pub infected_can_move: bool,
    pub birth_rate_per_day: f64,
    pub natural_death_rate_per_day: f64,
    pub movement: [MoveProfile; 3],
    pub c_bright: u32,
    pub refresh_interval_ms: u64,
    pub speed_min: f32,
    pub speed_max: f32,
    pub default_speed: f32,
    pub speed_fastest: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfectionMode {
    Random,
    Cell,
}

#[derive(Debug, Clone, Copy)]
pub struct MoveProfile {
    pub probability: f64,
    pub distance_min: u32,
    pub distance_max: u32,
    pub tau: f64,
    pub beta: f64,
}

impl SimConfig {
    pub fn defaults() -> Self {
        Self {
            width: 10_000,
            height: 10_000,
            max_days: 10_000,
            seed: 1,
            initial_population: 100_000_000,
            initial_infected_count: 1,
            initial_infection_mode: InfectionMode::Random,
            initial_infection_cell: 0,
            lambda_bg: 0.2,
            truncation: 4.0,
            centers: default_centers(),
            incubation_days: 7,
            immune_days: 180,
            infection_rate: [0.70, 0.55, 0.45, 0.25],
            lethality_rate: [0.40, 0.30, 0.20, 0.05],
            infected_can_move: true,
            birth_rate_per_day: 0.0001,
            natural_death_rate_per_day: 0.00005,
            movement: default_movement(),
            c_bright: 64,
            refresh_interval_ms: 100,
            speed_min: 1.0,
            speed_max: 60.0,
            default_speed: 10.0,
            speed_fastest: -1.0,
        }
    }

    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("无法读取配置 {}: {}", path.display(), e))?;
        let raw: RawConfig = toml::from_str(&text)
            .map_err(|e| format!("配置 TOML 解析失败: {}", e))?;
        let mut cfg = Self::defaults();
        cfg.apply_raw(raw);
        cfg.validate()?;
        Ok(cfg)
    }

    fn apply_raw(&mut self, raw: RawConfig) {
        if let Some(m) = raw.map {
            if let Some(w) = m.width {
                self.width = w;
            }
            if let Some(h) = m.height {
                self.height = h;
            }
        }
        if let Some(s) = raw.sim {
            if let Some(d) = s.max_days {
                self.max_days = d;
            }
        }
        if let Some(i) = raw.initial {
            if let Some(v) = i.seed {
                self.seed = v;
            }
            if let Some(v) = i.initial_population {
                self.initial_population = v;
            }
            if let Some(v) = i.initial_infected_count {
                self.initial_infected_count = v;
            }
            if let Some(v) = i.initial_infection_mode {
                self.initial_infection_mode = if v.eq_ignore_ascii_case("cell") {
                    InfectionMode::Cell
                } else {
                    InfectionMode::Random
                };
            }
            if let Some(v) = i.initial_infection_cell {
                self.initial_infection_cell = v;
            }
        }
        if let Some(d) = raw.density {
            if let Some(v) = d.lambda_bg {
                self.lambda_bg = v;
            }
            if let Some(v) = d.truncation {
                self.truncation = v;
            }
            if let Some(v) = d.centers {
                self.centers = v;
            }
        }
        if let Some(d) = raw.disease {
            if let Some(v) = d.incubation_days {
                self.incubation_days = v;
            }
            if let Some(v) = d.immune_days {
                self.immune_days = v;
            }
            if let Some(v) = d.infection_rate {
                copy_rates(&v, &mut self.infection_rate);
            }
            if let Some(v) = d.lethality_rate {
                copy_rates(&v, &mut self.lethality_rate);
            }
            if let Some(v) = d.infected_can_move {
                self.infected_can_move = v;
            }
        }
        if let Some(v) = raw.vital {
            if let Some(x) = v.birth_rate_per_day {
                self.birth_rate_per_day = x;
            }
            if let Some(x) = v.natural_death_rate_per_day {
                self.natural_death_rate_per_day = x;
            }
        }
        if let Some(m) = raw.movement {
            apply_move(&m.commute, &mut self.movement[0]);
            apply_move(&m.visit, &mut self.movement[1]);
            apply_move(&m.travel, &mut self.movement[2]);
        }
        if let Some(u) = raw.ui {
            if let Some(v) = u.c_bright {
                self.c_bright = v;
            }
            if let Some(v) = u.refresh_interval_ms {
                self.refresh_interval_ms = v;
            }
            if let Some(v) = u.speed_min {
                self.speed_min = v;
            }
            if let Some(v) = u.speed_max {
                self.speed_max = v;
            }
            if let Some(v) = u.default_speed {
                self.default_speed = v;
            }
            if let Some(v) = u.speed_fastest {
                self.speed_fastest = v;
            }
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let check_prob = |name: &str, p: f64| -> Result<(), String> {
            if !(0.0..=1.0).contains(&p) {
                return Err(format!("{} 必须在 0 到 1 之间", name));
            }
            Ok(())
        };
        if self.width < 1 {
            return Err("map.width 必须 >= 1".into());
        }
        if self.height < 1 {
            return Err("map.height 必须 >= 1".into());
        }
        if self.max_days < 1 {
            return Err("sim.max_days 必须 >= 1".into());
        }
        if self.initial_population < 1 {
            return Err("initial.initial_population 必须 >= 1".into());
        }
        if self.incubation_days < 1 {
            return Err("disease.incubation_days 必须 >= 1".into());
        }
        if self.c_bright < 1 {
            return Err("ui.c_bright 必须 >= 1".into());
        }
        if self.lambda_bg < 0.0 {
            return Err("density.lambda_bg 必须 >= 0".into());
        }
        if self.truncation <= 0.0 {
            return Err("density.truncation 必须 > 0".into());
        }
        for (i, c) in self.centers.iter().enumerate() {
            if c.peak < 0.0 {
                return Err(format!("density.centers[{}].peak 必须 >= 0", i));
            }
            if c.sigma <= 0.0 {
                return Err(format!("density.centers[{}].sigma 必须 > 0", i));
            }
            let max_cell = self.width * self.height;
            if c.cell >= max_cell {
                return Err(format!(
                    "density.centers[{}].cell = {} 超出地图（width×height = {}，有效编号 0..{})",
                    i,
                    c.cell,
                    max_cell,
                    max_cell
                ));
            }
        }
        for (g, p) in self.infection_rate.iter().enumerate() {
            check_prob(&format!("disease.infection_rate[{}]", g), *p)?;
        }
        for (g, p) in self.lethality_rate.iter().enumerate() {
            check_prob(&format!("disease.lethality_rate[{}]", g), *p)?;
        }
        check_prob("vital.birth_rate_per_day", self.birth_rate_per_day)?;
        check_prob("vital.natural_death_rate_per_day", self.natural_death_rate_per_day)?;

        let move_sum = self.movement[0].probability
            + self.movement[1].probability
            + self.movement[2].probability;
        if move_sum > 1.0 {
            return Err("三种移动日概率之和不能大于 1".into());
        }
        for (name, m) in [
            ("movement.commute", self.movement[0]),
            ("movement.visit", self.movement[1]),
            ("movement.travel", self.movement[2]),
        ] {
            check_prob(&format!("{}.probability", name), m.probability)?;
            if m.distance_min > m.distance_max {
                return Err(format!("{}.distance_min 不能大于 distance_max", name));
            }
            if m.tau <= 0.0 {
                return Err(format!("{}.tau 必须 > 0", name));
            }
        }
        if self.total_placement_weight() <= 0.0 {
            return Err("全部格子权重之和为 0".into());
        }
        Ok(())
    }

    pub fn total_placement_weight(&self) -> f64 {
        let bg = self.lambda_bg * self.width as f64 * self.height as f64;
        let centers = self
            .centers
            .iter()
            .map(|c| center_window_weight_sum(self, c))
            .sum::<f64>();
        bg + centers
    }

    pub fn daily_infection_prob(&self, gen: u8) -> f64 {
        let g = gen.min(3) as usize;
        let pg = self.infection_rate[g];
        let d = self.incubation_days as f64;
        if pg >= 1.0 {
            1.0
        } else {
            1.0 - (1.0 - pg).powf(1.0 / d)
        }
    }

    pub fn daily_lethality_prob(&self, gen: u8) -> f64 {
        let g = gen.min(3) as usize;
        let lg = self.lethality_rate[g];
        let d = self.incubation_days as f64;
        if lg >= 1.0 {
            1.0
        } else {
            1.0 - (1.0 - lg).powf(1.0 / d)
        }
    }
}

pub fn center_window_weight_sum(cfg: &SimConfig, center: &DensityCenter) -> f64 {
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
            let dist = hex::hex_distance(cq, cr, nq as u32, nr as u32);
            let limit = cfg.truncation * center.sigma;
            if dist as f64 > limit {
                continue;
            }
            let w = center.peak * exp_gaussian(dist, center.sigma);
            sum += w;
        }
    }
    sum
}

fn exp_gaussian(dist: u32, sigma: f64) -> f64 {
    let d = dist as f64;
    (-d * d / (2.0 * sigma * sigma)).exp()
}

fn copy_rates(src: &[f64], dst: &mut [f64; 4]) {
    for i in 0..4 {
        if i < src.len() {
            dst[i] = src[i];
        }
    }
}

fn apply_move(src: &Option<MoveKind>, dst: &mut MoveProfile) {
    if let Some(s) = src {
        if let Some(v) = s.probability {
            dst.probability = v;
        }
        if let Some(v) = s.distance_min {
            dst.distance_min = v;
        }
        if let Some(v) = s.distance_max {
            dst.distance_max = v;
        }
        if let Some(v) = s.tau {
            dst.tau = v;
        }
        if let Some(v) = s.beta {
            dst.beta = v;
        }
    }
}

fn default_centers() -> Vec<DensityCenter> {
    vec![
        DensityCenter {
            cell: 25_000_000,
            peak: 4.0,
            sigma: 20.0,
        },
        DensityCenter {
            cell: 75_000_000,
            peak: 4.0,
            sigma: 20.0,
        },
        DensityCenter {
            cell: 50_005_000,
            peak: 4.0,
            sigma: 20.0,
        },
        DensityCenter {
            cell: 25_005_000,
            peak: 4.0,
            sigma: 20.0,
        },
        DensityCenter {
            cell: 75_005_000,
            peak: 4.0,
            sigma: 20.0,
        },
        DensityCenter {
            cell: 50_000_000,
            peak: 4.0,
            sigma: 20.0,
        },
    ]
}

fn default_movement() -> [MoveProfile; 3] {
    [
        MoveProfile {
            probability: 0.30,
            distance_min: 1,
            distance_max: 5,
            tau: 1.0,
            beta: 1.0,
        },
        MoveProfile {
            probability: 0.05 / 7.0,
            distance_min: 6,
            distance_max: 50,
            tau: 10.0,
            beta: 0.1,
        },
        MoveProfile {
            probability: 0.01 / 365.0,
            distance_min: 51,
            distance_max: 500,
            tau: 80.0,
            beta: 0.0,
        },
    ]
}

use crate::world::hex;
