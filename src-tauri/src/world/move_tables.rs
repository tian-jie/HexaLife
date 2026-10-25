use crate::config::MoveProfile;
use crate::world::hex;
use rand::rngs::StdRng;
use rand::Rng;

pub struct DistLut {
    pub distances: Vec<u32>,
    pub cum_weight: Vec<f64>,
    pub total: f64,
}

pub struct MoveTables {
    pub profiles: [DistLut; 3],
}

impl MoveTables {
    pub fn from_config(width: u32, height: u32, profiles: &[MoveProfile; 3]) -> Self {
        Self {
            profiles: [
                build_dist_lut(width, height, profiles[0]),
                build_dist_lut(width, height, profiles[1]),
                build_dist_lut(width, height, profiles[2]),
            ],
        }
    }

    pub fn pick_distance(lut: &DistLut, rng: &mut StdRng) -> Option<u32> {
        if lut.distances.is_empty() {
            return None;
        }
        let u = rng.gen::<f64>() * lut.total;
        for (i, &cw) in lut.cum_weight.iter().enumerate() {
            if u < cw {
                return Some(lut.distances[i]);
            }
        }
        lut.distances.last().copied()
    }
}

fn build_dist_lut(width: u32, height: u32, profile: MoveProfile) -> DistLut {
    let max_d = profile.distance_max.min(width + height);
    let mut distances = Vec::new();
    let mut weights = Vec::new();
    let (q, r) = (width / 2, height / 2);
    for d in profile.distance_min..=max_d {
        if hex::has_cells_at_distance(width, height, q, r, d) {
            distances.push(d);
            let w = 2.0_f64.powf(-((d - profile.distance_min) as f64) / profile.tau);
            weights.push(w);
        }
    }
    let total = weights.iter().sum();
    let mut cum = Vec::with_capacity(weights.len());
    let mut acc = 0.0;
    for w in weights {
        acc += w;
        cum.push(acc);
    }
    DistLut {
        distances,
        cum_weight: cum,
        total,
    }
}
