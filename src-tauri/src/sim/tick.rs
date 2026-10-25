use crate::sim::disease::{spread, virus_death_and_recovery};
use crate::sim::movement::try_move_person;
use crate::sim::vital::{births, natural_death};
use crate::world::{STATE_HEALTHY, World};
use rand::rngs::StdRng;
use std::collections::HashSet;

pub fn tick_day(world: &mut World, rng: &mut StdRng) {
    let day = world.day + 1;
    world.day = day;
    world.stats = crate::stats::DayStats::zeroed();
    world.stats.day = day;

    if day < world.immune_expiring.len() as u32 {
        let expiring = world.immune_expiring[day as usize].clone();
        for id in expiring {
            if world.people[id as usize].state == crate::world::STATE_IMMUNE {
                let cell = world.people[id as usize].cell;
                world.adjust_tile(cell, crate::world::STATE_IMMUNE, -1);
                world.people[id as usize].state = STATE_HEALTHY;
                world.adjust_tile(cell, STATE_HEALTHY, 1);
            }
        }
        world.immune_expiring[day as usize].clear();
    }

    let alive_start = world.sorted_alive_snapshot();
    let mut alive: HashSet<u32> = alive_start.iter().copied().collect();

    let active_infected: Vec<u32> = alive_start
        .iter()
        .filter(|id| world.is_contagious_at_day_start(**id, day))
        .copied()
        .collect();

    for id in &alive_start {
        if let Some(dest) = try_move_person(world, *id, day, rng) {
            world.move_person(*id, dest);
        }
    }

    spread(world, day, &active_infected, rng);

    natural_death(world, rng, &alive_start, &mut alive);
    virus_death_and_recovery(world, day, &active_infected, &mut alive, rng);

    births(world, rng, &alive_start, &mut alive);

    world.refresh_aggregate_stats();
}
