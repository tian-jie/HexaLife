use crate::rng::{WyRng, PHASE_BIRTH, PHASE_NATURAL_DEATH};
use crate::world::{STATE_DEAD, World};
use rayon::prelude::*;

pub fn decide_natural_deaths(world: &World, day: u32, n: usize) -> Vec<bool> {
    let rate = world.cfg.natural_death_rate_per_day;
    if rate <= 0.0 {
        return vec![false; n];
    }
    let seed = world.cfg.seed;
    (0..n)
        .into_par_iter()
        .map(|id| {
            let id = id as u32;
            if world.people[id as usize].state == STATE_DEAD {
                return false;
            }
            let mut rng = WyRng::from_phase(seed, day, id, PHASE_NATURAL_DEATH);
            rng.next_f64() < rate
        })
        .collect()
}

pub fn decide_births(world: &World, day: u32, n: usize) -> Vec<bool> {
    let rate = world.cfg.birth_rate_per_day;
    if rate <= 0.0 {
        return vec![false; n];
    }
    let seed = world.cfg.seed;
    (0..n)
        .into_par_iter()
        .map(|id| {
            let id = id as u32;
            if world.people[id as usize].state == STATE_DEAD {
                return false;
            }
            let mut rng = WyRng::from_phase(seed, day, id, PHASE_BIRTH);
            rng.next_f64() < rate
        })
        .collect()
}
