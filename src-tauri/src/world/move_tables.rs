use crate::config::MoveProfile;
use crate::rng::WyRng;
use crate::world::hex;

pub struct DistLut {
    pub distances: Vec<u32>,
    pub cum_weight: Vec<f64>,
    pub total: f64,
    pub home_weight: Vec<f64>,
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

    pub fn pick_distance(lut: &DistLut, rng: &mut WyRng) -> Option<u32> {
        if lut.distances.is_empty() || lut.total <= 0.0 {
            return None;
        }
        let u = rng.next_f64() * lut.total;
        let i = lut.cum_weight.partition_point(|&cw| cw <= u);
        lut.distances
            .get(i)
            .copied()
            .or_else(|| lut.distances.last().copied())
    }
}

pub fn home_weight_at(lut: &DistLut, dist: u32) -> f64 {
    lut.home_weight.get(dist as usize).copied().unwrap_or(0.0)
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
    let max_home = width.saturating_add(height);
    let home_weight = if profile.beta == 0.0 {
        vec![1.0; max_home as usize + 1]
    } else {
        (0..=max_home)
            .map(|d| 2.0_f64.powf(-profile.beta * d as f64))
            .collect()
    };
    DistLut {
        distances,
        cum_weight: cum,
        total,
        home_weight,
    }
}

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
