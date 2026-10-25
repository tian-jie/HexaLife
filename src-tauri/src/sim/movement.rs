use crate::config::MoveProfile;
use crate::rng::{WyRng, PHASE_MOVE_DEST, PHASE_MOVE_DISTANCE, PHASE_MOVE_KIND};
use crate::world::hex::{self, LINK_NONE};
use crate::world::move_tables::{home_weight_at, MoveTables};
use crate::world::{Person, STATE_DEAD, STATE_INFECTED, World};
use rayon::prelude::*;
use std::cell::RefCell;

thread_local! {
    static RING_BUF: RefCell<Vec<u32>> = RefCell::new(Vec::with_capacity(1024));
}

pub fn decide_moves(world: &World, day: u32, n: usize) -> Vec<u32> {
    (0..n)
        .into_par_iter()
        .map(|id| decide_one(world, day, id as u32))
        .collect()
}

fn decide_one(world: &World, day: u32, id: u32) -> u32 {
    let p = &world.people[id as usize];
    if p.state == STATE_DEAD {
        return LINK_NONE;
    }
    if p.state == STATE_INFECTED {
        let contagious =
            day > p.infected_on && day <= p.infected_on + world.cfg.incubation_days;
        if contagious && !world.cfg.infected_can_move {
            return LINK_NONE;
        }
        if day == p.infected_on {
            return LINK_NONE;
        }
    }
    let profiles = &world.cfg.movement;
    let seed = world.cfg.seed;
    let mut rng_kind = WyRng::from_phase(seed, day, id, PHASE_MOVE_KIND);
    let u = rng_kind.next_f64();
    let pc = profiles[0].probability;
    let pv = profiles[1].probability;
    let pt = profiles[2].probability;
    let (profile_idx, profile) = if u < pc {
        (0, profiles[0])
    } else if u < pc + pv {
        (1, profiles[1])
    } else if u < pc + pv + pt {
        (2, profiles[2])
    } else {
        return LINK_NONE;
    };
    pick_destination(world, &world.move_tables, profile_idx, profile, p, day, id) // profile used via lut
}

fn pick_destination(
    world: &World,
    tables: &MoveTables,
    profile_idx: usize,
    _profile: MoveProfile,
    person: &Person,
    day: u32,
    id: u32,
) -> u32 {
    let lut = &tables.profiles[profile_idx];
    let mut rng_dist = WyRng::from_phase(world.cfg.seed, day, id, PHASE_MOVE_DISTANCE);
    let d = match MoveTables::pick_distance(lut, &mut rng_dist) {
        Some(d) => d,
        None => return LINK_NONE,
    };
    let (q, r) = hex::qr_from_index(world.cfg.width, person.cell);
    let (hq, hr) = hex::qr_from_index(world.cfg.width, person.home);

    if d <= 8 {
        let mut stack_cells = [0u32; 48];
        let n = hex::ring_cells_into(world.cfg.width, world.cfg.height, q, r, d, &mut stack_cells);
        if n == 0 {
            return LINK_NONE;
        }
        return pick_weighted_cell(world, &stack_cells[..n], lut, hq, hr, day, id);
    }

    RING_BUF.with(|b| {
        let mut v = b.borrow_mut();
        v.clear();
        let need = (6 * d) as usize;
        if v.len() < need {
            v.resize(need, 0);
        }
        let n = hex::ring_cells_into(
            world.cfg.width,
            world.cfg.height,
            q,
            r,
            d,
            &mut v[..need],
        );
        if n == 0 {
            return LINK_NONE;
        }
        pick_weighted_cell(world, &v[..n], lut, hq, hr, day, id)
    })
}

fn pick_weighted_cell(
    world: &World,
    candidates: &[u32],
    lut: &crate::world::move_tables::DistLut,
    hq: u32,
    hr: u32,
    day: u32,
    id: u32,
) -> u32 {
    if candidates.is_empty() {
        return LINK_NONE;
    }
    let mut cells: Vec<(u32, f64)> = Vec::with_capacity(candidates.len());
    for &c in candidates {
        let (cq, cr) = hex::qr_from_index(world.cfg.width, c);
        let dist_home = hex::hex_distance(cq, cr, hq, hr);
        let w = home_weight_at(lut, dist_home);
        cells.push((c, w));
    }
    cells.sort_unstable_by_key(|(c, _)| *c);
    let sum: f64 = cells.iter().map(|(_, w)| *w).sum();
    if sum <= 0.0 {
        return LINK_NONE;
    }
    let mut rng_dest = WyRng::from_phase(world.cfg.seed, day, id, PHASE_MOVE_DEST);
    let u = rng_dest.next_f64() * sum;
    let mut acc = 0.0;
    for &(cell, w) in &cells {
        acc += w;
        if acc > u {
            return cell;
        }
    }
    cells.last().map(|(c, _)| *c).unwrap_or(LINK_NONE)
}
