use crate::world::{STATE_HEALTHY, World};
use rand::rngs::StdRng;
use rand::Rng;
use std::collections::HashSet;

pub fn natural_death(
    world: &mut World,
    rng: &mut StdRng,
    alive_snapshot: &[u32],
    alive: &mut HashSet<u32>,
) {
    let rate = world.cfg.natural_death_rate_per_day;
    if rate <= 0.0 {
        return;
    }
    for id in alive_snapshot {
        if !alive.contains(id) {
            continue;
        }
        if rng.gen::<f64>() < rate {
            world.kill_person(*id);
            alive.remove(id);
            world.stats.new_natural_deaths += 1;
            world.total_natural_deaths += 1;
        }
    }
}

pub fn births(
    world: &mut World,
    rng: &mut StdRng,
    alive_snapshot: &[u32],
    alive: &mut HashSet<u32>,
) {
    let rate = world.cfg.birth_rate_per_day;
    if rate <= 0.0 {
        return;
    }
    for id in alive_snapshot {
        if !alive.contains(id) {
            continue;
        }
        if rng.gen::<f64>() < rate {
            let cell = world.people[*id as usize].cell;
            let new_id = world.people.len() as u32;
            world.people.push(crate::world::Person {
                cell,
                home: cell,
                next_in_cell: crate::world::hex::LINK_NONE,
                prev_in_cell: crate::world::hex::LINK_NONE,
                infected_on: 0,
                immune_until_day: 0,
                state: STATE_HEALTHY,
                generation: 0,
            });
            world.insert_into_cell(new_id);
            world.alive_slot_insert(new_id);
            alive.insert(new_id);
            world.stats.new_births += 1;
            world.total_births += 1;
        }
    }
}
