use crate::config::MoveProfile;
use crate::world::move_tables::MoveTables;
use crate::world::hex;
use crate::world::{Person, STATE_INFECTED, World};
use rand::rngs::StdRng;
use rand::Rng;

const RING_STACK_CAP: usize = 512;

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
    let (profile_idx, profile) = if u < pc {
        (0, profiles[0])
    } else if u < pc + pv {
        (1, profiles[1])
    } else if u < pc + pv + pt {
        (2, profiles[2])
    } else {
        return None;
    };
    pick_destination(world, &world.move_tables, profile_idx, profile, p, rng)
}

fn pick_destination(
    world: &World,
    tables: &MoveTables,
    profile_idx: usize,
    profile: MoveProfile,
    person: &Person,
    rng: &mut StdRng,
) -> Option<u32> {
    let lut = &tables.profiles[profile_idx];
    let d = MoveTables::pick_distance(lut, rng)?;
    let (q, r) = hex::qr_from_index(world.cfg.width, person.cell);
    let (hq, hr) = hex::qr_from_index(world.cfg.width, person.home);

    let mut stack_cells = [0u32; RING_STACK_CAP];
    let candidates: &[u32] = if d <= 32 {
        let n = fill_ring(world.cfg.width, world.cfg.height, q, r, d, &mut stack_cells);
        &stack_cells[..n]
    } else {
        // 旅游等远距离仍走堆，但距离权重已查表
        let v = hex::cells_at_distance(world.cfg.width, world.cfg.height, q, r, d);
        return pick_weighted_cell(world, &v, profile, hq, hr, rng);
    };

    if candidates.is_empty() {
        return None;
    }
    pick_weighted_cell(world, candidates, profile, hq, hr, rng)
}

fn fill_ring(width: u32, height: u32, q: u32, r: u32, d: u32, out: &mut [u32; RING_STACK_CAP]) -> usize {
    hex::ring_cells_into(width, height, q, r, d, out)
}

fn pick_weighted_cell(
    world: &World,
    candidates: &[u32],
    profile: MoveProfile,
    hq: u32,
    hr: u32,
    rng: &mut StdRng,
) -> Option<u32> {
    if candidates.is_empty() {
        return None;
    }
    let mut stack_weights: [(u32, f64); RING_STACK_CAP] = [(0, 0.0); RING_STACK_CAP];
    let mut n = 0usize;
    for &c in candidates {
        if n >= RING_STACK_CAP {
            break;
        }
        let (cq, cr) = hex::qr_from_index(world.cfg.width, c);
        let dist_home = hex::hex_distance(cq, cr, hq, hr);
        let w = 2.0_f64.powf(-profile.beta * dist_home as f64);
        stack_weights[n] = (c, w);
        n += 1;
    }
    stack_weights[..n].sort_by_key(|(c, _)| *c);
    let sum: f64 = stack_weights[..n].iter().map(|(_, w)| *w).sum();
    let u = rng.gen::<f64>() * sum;
    let mut acc = 0.0;
    for i in 0..n {
        acc += stack_weights[i].1;
        if acc > u {
            return Some(stack_weights[i].0);
        }
    }
    Some(stack_weights[n - 1].0)
}
