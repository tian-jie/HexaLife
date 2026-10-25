use crate::rng::{WyRng, PHASE_SPREAD, PHASE_VIRUS_DEATH};
use crate::world::hex::{self, LINK_NONE};
use crate::world::{STATE_HEALTHY, STATE_IMMUNE, World};
use rayon::prelude::*;
use std::collections::HashMap;

#[derive(Clone)]
pub struct InfectionDecision {
    pub target: u32,
    pub parent: u32,
}

pub enum VirusFate {
    Unchanged,
    Died,
    Recovered,
}

pub fn decide_infections(world: &World, day: u32, active: &[u32]) -> Vec<InfectionDecision> {
    if active.is_empty() {
        return Vec::new();
    }
    let pairs = build_infector_pairs(world, active);
    let targets = collect_targets(world, active);
    if targets.is_empty() {
        return Vec::new();
    }
    let mut slots: Vec<Option<InfectionDecision>> = vec![None; targets.len()];
    slots
        .par_iter_mut()
        .enumerate()
        .for_each(|(i, slot)| {
            *slot = decide_target(world, day, targets[i], &pairs);
        });
    slots.into_iter().flatten().collect()
}

fn build_infector_pairs(world: &World, active: &[u32]) -> Vec<(u32, u32)> {
    let mut pairs: Vec<(u32, u32)> = active
        .iter()
        .map(|&id| (world.people[id as usize].cell, id))
        .collect();
    pairs.sort_unstable();
    pairs
}

fn collect_targets(world: &World, active: &[u32]) -> Vec<u32> {
    let width = world.cfg.width;
    let height = world.cfg.height;

    let mut contact_cells: Vec<u32> = Vec::new();
    let mut seen = HashMap::new();
    for &id in active {
        let cell = world.people[id as usize].cell;
        let (q, r) = hex::qr_from_index(width, cell);
        let mut batch = [cell, LINK_NONE, LINK_NONE, LINK_NONE, LINK_NONE, LINK_NONE, LINK_NONE];
        let mut n = 1usize;
        for nb in hex::neighbor_indices(width, height, q, r) {
            if nb != LINK_NONE {
                batch[n] = nb;
                n += 1;
            }
        }
        for i in 0..n {
            let c = batch[i];
            if seen.insert(c, ()).is_none() {
                contact_cells.push(c);
            }
        }
    }
    contact_cells.sort_unstable();

    let mut targets: Vec<u32> = Vec::new();
    for cell in contact_cells {
        let mut cur = world.cell_head[cell as usize];
        while cur != LINK_NONE {
            if world.people[cur as usize].state == STATE_HEALTHY {
                targets.push(cur);
            }
            cur = world.people[cur as usize].next_in_cell;
        }
    }
    targets.sort_unstable();
    targets
}

fn infector_ids_at_cell(pairs: &[(u32, u32)], cell: u32) -> Vec<u32> {
    let lo = pairs.partition_point(|(c, _)| *c < cell);
    let hi = pairs.partition_point(|(c, _)| *c <= cell);
    pairs[lo..hi].iter().map(|(_, id)| *id).collect()
}

fn decide_target(
    world: &World,
    day: u32,
    target: u32,
    pairs: &[(u32, u32)],
) -> Option<InfectionDecision> {
    if world.people[target as usize].state != STATE_HEALTHY {
        return None;
    }
    let width = world.cfg.width;
    let height = world.cfg.height;
    let cell = world.people[target as usize].cell;
    let (q, r) = hex::qr_from_index(width, cell);

    let mut contact = [LINK_NONE; 7];
    contact[0] = cell;
    let mut nc = 1usize;
    for nb in hex::neighbor_indices(width, height, q, r) {
        if nb != LINK_NONE {
            contact[nc] = nb;
            nc += 1;
        }
    }

    let mut infectors: Vec<u32> = Vec::new();
    for i in 0..nc {
        let c = contact[i];
        infectors.extend(infector_ids_at_cell(pairs, c));
    }
    infectors.sort_unstable();

    let mut rng = WyRng::from_phase(world.cfg.seed, day, target, PHASE_SPREAD);
    let mut hitters: Vec<(u32, f64)> = Vec::new();
    for inf_id in infectors {
        let gen = world.people[inf_id as usize].generation;
        let p = world.cfg.daily_infection_prob(gen);
        if rng.next_f64() < p {
            hitters.push((inf_id, p));
        }
    }
    if hitters.is_empty() {
        return None;
    }
    let parent = pick_parent(&hitters, &mut rng);
    Some(InfectionDecision {
        target,
        parent,
    })
}

fn pick_parent(hitters: &[(u32, f64)], rng: &mut WyRng) -> u32 {
    let sum: f64 = hitters.iter().map(|(_, p)| *p).sum();
    let u = rng.next_f64() * sum;
    let mut acc = 0.0;
    for (id, p) in hitters {
        acc += *p;
        if acc > u {
            return *id;
        }
    }
    hitters.last().unwrap().0
}

pub fn decide_virus_fates(world: &World, day: u32, active: &[u32]) -> Vec<VirusFate> {
    let d = world.cfg.incubation_days;
    active
        .par_iter()
        .map(|&id| {
            let gen = world.people[id as usize].generation;
            let mut rng = WyRng::from_phase(world.cfg.seed, day, id, PHASE_VIRUS_DEATH);
            let delta = world.cfg.daily_lethality_prob(gen);
            if rng.next_f64() < delta {
                return VirusFate::Died;
            }
            if day == world.people[id as usize].infected_on + d {
                VirusFate::Recovered
            } else {
                VirusFate::Unchanged
            }
        })
        .collect()
}

pub fn apply_recovery(world: &mut World, id: u32) {
    let g = crate::world::generation_index(world.people[id as usize].generation);
    world.infected_by_gen[g] = world.infected_by_gen[g].saturating_sub(1);
    let cell = world.people[id as usize].cell;
    world.adjust_pop(cell, crate::world::STATE_INFECTED, -1);
    world.people[id as usize].state = STATE_IMMUNE;
    world.adjust_pop(cell, STATE_IMMUNE, 1);
    let d = world.cfg.incubation_days;
    let n = world.cfg.immune_days;
    let until = world.people[id as usize].infected_on + d + n + 1;
    world.people[id as usize].immune_until_day = until;
    let bucket = until.min(world.immune_expiring.len() as u32 - 1) as usize;
    world.immune_expiring[bucket].push(id);
    world.stats.new_immune += 1;
}
