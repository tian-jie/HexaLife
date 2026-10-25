use crate::sim::disease::{
    apply_recovery, decide_infections, decide_virus_fates, VirusFate,
};
use crate::sim::movement::decide_moves;
use crate::sim::vital::{decide_births, decide_natural_deaths};
use crate::world::hex::LINK_NONE;
use crate::world::{
    generation_index, Person, STATE_HEALTHY, STATE_IMMUNE, World,
};
pub fn immune_expiry_for_day(world: &mut World, day: u32) {
    if day < world.immune_expiring.len() as u32 {
        let expiring = std::mem::take(&mut world.immune_expiring[day as usize]);
        for id in expiring {
            if world.people[id as usize].state == STATE_IMMUNE {
                let cell = world.people[id as usize].cell;
                world.adjust_pop(cell, STATE_IMMUNE, -1);
                world.people[id as usize].state = STATE_HEALTHY;
                world.adjust_pop(cell, STATE_HEALTHY, 1);
            }
        }
    }
}

pub fn tick_day(world: &mut World) {
    let day = world.day + 1;
    world.day = day;
    world.stats = crate::stats::DayStats::zeroed();
    world.stats.day = day;

    immune_expiry_for_day(world, day);

    let n = world.people.len();
    let active_infected: Vec<u32> = (0..n)
        .filter_map(|i| {
            let id = i as u32;
            world.is_contagious_at_day_start(id, day).then_some(id)
        })
        .collect();

    let moves = decide_moves(world, day, n);
    for id in 0..n {
        let dest = moves[id];
        if dest != LINK_NONE {
            world.move_person(id as u32, dest);
        }
    }

    let infections = decide_infections(world, day, &active_infected);
    for hit in infections {
        if world.people[hit.target as usize].state != STATE_HEALTHY {
            continue;
        }
        let parent_gen = world.people[hit.parent as usize].generation;
        world.infect_new(hit.target, day, parent_gen);
    }

    let die = decide_natural_deaths(world, day, n);
    for id in 0..n {
        if die[id] && world.is_alive(id as u32) {
            world.kill_person(id as u32);
            world.stats.new_natural_deaths += 1;
            world.total_natural_deaths += 1;
        }
    }

    let fates = decide_virus_fates(world, day, &active_infected);
    for (id, fate) in active_infected.iter().copied().zip(fates) {
        if !world.is_alive(id) {
            continue;
        }
        match fate {
            VirusFate::Died => {
                let gi = generation_index(world.people[id as usize].generation);
                world.kill_person(id);
                world.stats.new_virus_deaths += 1;
                world.total_virus_deaths += 1;
                world.virus_deaths_by_gen[gi] += 1;
            }
            VirusFate::Recovered => {
                apply_recovery(world, id);
            }
            VirusFate::Unchanged => {}
        }
    }

    let n = world.people.len();
    let born = decide_births(world, day, n);
    for id in 0..n {
        if !born[id] || !world.is_alive(id as u32) {
            continue;
        }
        let cell = world.people[id].cell;
        let new_id = world.people.len() as u32;
        world.people.push(Person {
            cell,
            home: cell,
            next_in_cell: LINK_NONE,
            prev_in_cell: LINK_NONE,
            infected_on: 0,
            immune_until_day: 0,
            state: STATE_HEALTHY,
            generation: 0,
            alive_index: 0,
        });
        world.insert_into_cell(new_id);
        world.push_alive(new_id);
        world.stats.new_births += 1;
        world.total_births += 1;
    }

    world.refresh_aggregate_stats();
}
