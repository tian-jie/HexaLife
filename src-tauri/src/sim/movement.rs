use crate::config::MoveProfile;
use crate::world::hex;
use crate::world::{Person, STATE_INFECTED, World};
use rand::rngs::StdRng;
use rand::Rng;

pub fn try_move_person(world: &World, id: u32, day: u32, rng: &mut StdRng) -> Option<u32> {
    let p = &world.people[id as usize];
    if p.state == STATE_INFECTED {
        let contagious = day > p.infected_on && day <= p.infected_on + world.cfg.incubation_days;
        if contagious && !world.cfg.infected_can_move {
            return None;
        }
        if day == p.infected_on {
            return None;
        }
    }
    let profiles = &world.cfg.movement;
    let u = rng.gen::<f64>();
    let pc = profiles[0].probability;
    let pv = profiles[1].probability;
    let pt = profiles[2].probability;
    let profile_idx = if u < pc {
        0
    } else if u < pc + pv {
        1
    } else if u < pc + pv + pt {
        2
    } else {
        return None;
    };
    pick_destination(world, p, profiles[profile_idx], rng)
}

fn pick_destination(world: &World, person: &Person, profile: MoveProfile, rng: &mut StdRng) -> Option<u32> {
    let (q, r) = hex::qr_from_index(world.cfg.width, person.cell);
    let max_d = profile
        .distance_max
        .min(world.cfg.width + world.cfg.height);
    let mut dist_weights = Vec::new();
    for d in profile.distance_min..=max_d {
        if !hex::has_cells_at_distance(world.cfg.width, world.cfg.height, q, r, d) {
            continue;
        }
        let w = 2.0_f64.powf(-((d - profile.distance_min) as f64) / profile.tau);
        dist_weights.push((d, w));
    }
    if dist_weights.is_empty() {
        return None;
    }
    let dist = weighted_pick_index(&dist_weights, rng);
    let d = dist_weights[dist].0;
    let candidates = hex::cells_at_distance(world.cfg.width, world.cfg.height, q, r, d);
    let (hq, hr) = hex::qr_from_index(world.cfg.width, person.home);
    let mut cell_weights: Vec<(u32, f64)> = candidates
        .into_iter()
        .map(|c| {
            let (cq, cr) = hex::qr_from_index(world.cfg.width, c);
            let dist_home = hex::hex_distance(cq, cr, hq, hr);
            let w = 2.0_f64.powf(-profile.beta * dist_home as f64);
            (c, w)
        })
        .collect();
    cell_weights.sort_by_key(|(c, _)| *c);
    let idx = weighted_pick_index_pairs(&cell_weights, rng);
    Some(cell_weights[idx].0)
}

fn weighted_pick_index(weights: &[(u32, f64)], rng: &mut StdRng) -> usize {
    let sum: f64 = weights.iter().map(|(_, w)| *w).sum();
    let u = rng.gen::<f64>() * sum;
    let mut acc = 0.0;
    for (i, (_, w)) in weights.iter().enumerate() {
        acc += *w;
        if acc > u {
            return i;
        }
    }
    weights.len() - 1
}

fn weighted_pick_index_pairs(weights: &[(u32, f64)], rng: &mut StdRng) -> usize {
    let sum: f64 = weights.iter().map(|(_, w)| *w).sum();
    let u = rng.gen::<f64>() * sum;
    let mut acc = 0.0;
    for (i, (_, w)) in weights.iter().enumerate() {
        acc += *w;
        if acc > u {
            return i;
        }
    }
    weights.len() - 1
}
