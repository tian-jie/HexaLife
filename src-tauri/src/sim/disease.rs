use crate::world::hex::{self, LINK_NONE};
use crate::world::{generation_index, STATE_HEALTHY, STATE_IMMUNE, World};
use rand::rngs::StdRng;
use rand::Rng;
use std::collections::HashMap;

pub fn spread(world: &mut World, day: u32, active: &[u32], rng: &mut StdRng) {
    let width = world.cfg.width;
    let height = world.cfg.height;

    let mut infectors_by_cell: HashMap<u32, Vec<u32>> = HashMap::new();
    for id in active {
        let cell = world.people[*id as usize].cell;
        infectors_by_cell.entry(cell).or_default().push(*id);
    }

    let healthy_targets: Vec<u32> = world
        .sorted_alive_snapshot()
        .into_iter()
        .filter(|id| world.people[*id as usize].state == STATE_HEALTHY)
        .collect();

    for target in healthy_targets {
        let cell = world.people[target as usize].cell;
        let (q, r) = hex::qr_from_index(width, cell);
        let mut contact_cells = vec![cell];
        for n in hex::neighbor_indices(width, height, q, r) {
            if n != LINK_NONE {
                contact_cells.push(n);
            }
        }
        let mut hitters: Vec<(u32, f64)> = Vec::new();
        for c in contact_cells {
            if let Some(ids) = infectors_by_cell.get(&c) {
                for inf_id in ids {
                    let gen = world.people[*inf_id as usize].generation;
                    let p = world.cfg.daily_infection_prob(gen);
                    if rng.gen::<f64>() < p {
                        hitters.push((*inf_id, p));
                    }
                }
            }
        }
        if hitters.is_empty() {
            continue;
        }
        hitters.sort_by_key(|(id, _)| *id);
        let parent = pick_parent(&hitters, rng);
        let parent_gen = world.people[parent as usize].generation;
        world.infect_new(target, day, parent_gen);
    }
}

fn pick_parent(hitters: &[(u32, f64)], rng: &mut StdRng) -> u32 {
    let sum: f64 = hitters.iter().map(|(_, p)| *p).sum();
    let u = rng.gen::<f64>() * sum;
    let mut acc = 0.0;
    for (id, p) in hitters {
        acc += *p;
        if acc > u {
            return *id;
        }
    }
    hitters.last().unwrap().0
}

pub fn virus_death_and_recovery(
    world: &mut World,
    day: u32,
    active: &[u32],
    alive: &mut std::collections::HashSet<u32>,
    rng: &mut StdRng,
) {
    let d = world.cfg.incubation_days;
    let n = world.cfg.immune_days;
    for id in active {
        if !alive.contains(id) {
            continue;
        }
        let gen = world.people[*id as usize].generation;
        let delta = world.cfg.daily_lethality_prob(gen);
        if rng.gen::<f64>() < delta {
            world.kill_person(*id);
            alive.remove(id);
            world.stats.new_virus_deaths += 1;
            world.total_virus_deaths += 1;
            let gi = generation_index(gen);
            world.virus_deaths_by_gen[gi] += 1;
            continue;
        }
        if day == world.people[*id as usize].infected_on + d {
            let cell = world.people[*id as usize].cell;
            world.adjust_tile(cell, crate::world::STATE_INFECTED, -1);
            world.people[*id as usize].state = STATE_IMMUNE;
            world.adjust_tile(cell, STATE_IMMUNE, 1);
            let until = world.people[*id as usize].infected_on + d + n + 1;
            world.people[*id as usize].immune_until_day = until;
            let bucket = until.min(world.immune_expiring.len() as u32 - 1) as usize;
            world.immune_expiring[bucket].push(*id);
            world.stats.new_immune += 1;
        }
    }
}
