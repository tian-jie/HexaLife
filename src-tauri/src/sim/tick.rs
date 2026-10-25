use crate::sim::disease::{spread, virus_death_and_recovery};
use crate::sim::movement::try_move_person;
use crate::sim::vital::{births, natural_death};
use crate::world::{STATE_HEALTHY, World};
use rand::rngs::StdRng;

pub fn tick_day(world: &mut World, rng: &mut StdRng) {
    let day = world.day + 1;
    world.day = day;
    world.stats = crate::stats::DayStats::zeroed();
    world.stats.day = day;

    if day < world.immune_expiring.len() as u32 {
        let expiring = std::mem::take(&mut world.immune_expiring[day as usize]);
        for id in expiring {
            if world.people[id as usize].state == crate::world::STATE_IMMUNE {
                let cell = world.people[id as usize].cell;
                world.adjust_pop(cell, crate::world::STATE_IMMUNE, -1);
                world.people[id as usize].state = STATE_HEALTHY;
                world.adjust_pop(cell, STATE_HEALTHY, 1);
            }
        }
    }

    world.day_sorted.clear();
    world.day_sorted.extend_from_slice(&world.alive);
    world.day_sorted.sort_unstable();

    let active_infected: Vec<u32> = world
        .day_sorted
        .iter()
        .filter(|&id| world.is_contagious_at_day_start(*id, day))
        .copied()
        .collect();

    let n_alive = world.day_sorted.len();
    for i in 0..n_alive {
        let id = world.day_sorted[i];
        if let Some(dest) = try_move_person(world, id, day, rng) {
            world.move_person(id, dest);
        }
    }

    spread(world, day, &active_infected, rng);

    natural_death(world, rng);
    virus_death_and_recovery(world, day, &active_infected, rng);

    births(world, rng);

    world.refresh_aggregate_stats();
}
