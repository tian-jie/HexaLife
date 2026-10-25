use hexalife_lib::config::{InfectionMode, SimConfig};
use hexalife_lib::world::{STATE_HEALTHY, STATE_INFECTED, World};
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::collections::HashMap;

#[test]
fn initial_cell_uses_gen0_infection_rate() {
    let mut cfg = SimConfig::defaults();
    cfg.width = 30;
    cfg.height = 30;
    cfg.initial_population = 500;
    cfg.initial_infected_count = 1;
    cfg.initial_infection_mode = InfectionMode::Random;
    cfg.infection_rate[0] = 0.7;
    cfg.seed = 99;
    cfg.centers = vec![];

    let mut rng = StdRng::seed_from_u64(cfg.seed);
    let world = World::new(cfg, &mut rng).expect("world");

    let mut infected_by_cell: HashMap<u32, u32> = HashMap::new();
    let mut infected_people = 0u32;
    for id in &world.alive {
        if world.people[*id as usize].state == STATE_INFECTED {
            infected_people += 1;
            let c = world.people[*id as usize].cell;
            *infected_by_cell.entry(c).or_insert(0) += 1;
        }
    }
    assert!(infected_people > 0);
    assert_eq!(infected_by_cell.len(), 1);
    let cell = *infected_by_cell.keys().next().unwrap();
    let (h, i, _) = world.cell_counts(cell);
    assert_eq!(i, infected_people);
    assert!(h > 0, "P_g=0.7 时不应整格全员感染");
}

#[test]
fn gen0_rate_one_infects_all_in_outbreak_cell_only() {
    let mut cfg = SimConfig::defaults();
    cfg.width = 30;
    cfg.height = 30;
    cfg.initial_population = 500;
    cfg.initial_infected_count = 1;
    cfg.initial_infection_mode = InfectionMode::Random;
    cfg.infection_rate[0] = 1.0;
    cfg.seed = 7;
    cfg.centers = vec![];

    let mut rng = StdRng::seed_from_u64(cfg.seed);
    let world = World::new(cfg, &mut rng).expect("world");

    let outbreak_cell = world
        .alive
        .iter()
        .find(|id| world.people[*id as usize].state == STATE_INFECTED)
        .map(|id| world.people[*id as usize].cell)
        .expect("outbreak");

    let mut cur = world.cell_head[outbreak_cell as usize];
    while cur != hexalife_lib::world::hex::LINK_NONE {
        assert_ne!(world.people[cur as usize].state, STATE_HEALTHY);
        cur = world.people[cur as usize].next_in_cell;
    }

    for id in &world.alive {
        if world.people[*id as usize].cell != outbreak_cell {
            assert_eq!(world.people[*id as usize].state, STATE_HEALTHY);
        }
    }
}
