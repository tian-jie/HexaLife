use hexalife_lib::config::SimConfig;
use hexalife_lib::rng::{WyRng, PHASE_SPREAD};
use hexalife_lib::sim::disease::decide_infections;
use hexalife_lib::sim::movement::decide_moves;
use hexalife_lib::sim::tick::tick_day;
use hexalife_lib::world::hex::{self, LINK_NONE};
use hexalife_lib::sim::tick::immune_expiry_for_day;
use hexalife_lib::world::{STATE_INFECTED, World};
use rand::rngs::StdRng;
use rand::SeedableRng;

fn tiny_config() -> SimConfig {
    let mut cfg = SimConfig::defaults();
    cfg.width = 20;
    cfg.height = 20;
    cfg.initial_population = 200;
    cfg.initial_infected_count = 1;
    cfg.centers = vec![hexalife_lib::config::DensityCenter {
        cell: 200,
        peak: 4.0,
        sigma: 5.0,
    }];
    cfg.max_days = 30;
    cfg.seed = 42;
    cfg
}

#[test]
fn population_exact_and_reproducible() {
    let cfg = tiny_config();
    let mut rng1 = StdRng::seed_from_u64(cfg.seed);
    let w1 = World::new(cfg.clone(), &mut rng1).expect("world");
    let mut rng2 = StdRng::seed_from_u64(cfg.seed);
    let w2 = World::new(cfg, &mut rng2).expect("world");
    assert_eq!(w1.alive.len(), 200);
    assert_eq!(w2.alive.len(), 200);
    for i in 0..200 {
        assert_eq!(w1.people[i].cell, w2.people[i].cell);
    }
}

#[test]
fn runs_several_days_without_panic() {
    let cfg = tiny_config();
    let mut rng = StdRng::seed_from_u64(cfg.seed);
    let mut world = World::new(cfg, &mut rng).expect("world");
    for _ in 0..10 {
        tick_day(&mut world);
    }
    assert_eq!(world.day, 10);
    assert!(world.stats.alive > 0);
}

#[test]
fn same_seed_same_days() {
    let cfg = tiny_config();
    let mut a = World::new(cfg.clone(), &mut StdRng::seed_from_u64(cfg.seed)).unwrap();
    let mut b = World::new(cfg.clone(), &mut StdRng::seed_from_u64(cfg.seed)).unwrap();
    for _ in 0..15 {
        tick_day(&mut a);
        tick_day(&mut b);
    }
    assert_eq!(a.people.len(), b.people.len());
    for (p, q) in a.people.iter().zip(b.people.iter()) {
        assert_eq!(p.cell, q.cell);
        assert_eq!(p.state, q.state);
        assert_eq!(p.generation, q.generation);
        assert_eq!(p.infected_on, q.infected_on);
    }
    assert_eq!(a.stats.infected, b.stats.infected);
    assert_eq!(a.stats.alive, b.stats.alive);
    let mut scanned = [0u64; 4];
    for p in &a.people {
        if p.state == STATE_INFECTED {
            scanned[hexalife_lib::world::generation_index(p.generation)] += 1;
        }
    }
    assert_eq!(scanned, a.infected_by_gen);
}

fn replay_spread_parent(world: &World, day: u32, target: u32, pairs: &[(u32, u32)]) -> u32 {
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
        let lo = pairs.partition_point(|(cell_id, _)| *cell_id < c);
        let hi = pairs.partition_point(|(cell_id, _)| *cell_id <= c);
        for (_, inf_id) in &pairs[lo..hi] {
            infectors.push(*inf_id);
        }
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
    assert!(!hitters.is_empty());
    let sum: f64 = hitters.iter().map(|(_, p)| *p).sum();
    let u = rng.next_f64() * sum;
    let mut acc = 0.0;
    for (id, p) in &hitters {
        acc += *p;
        if acc > u {
            return *id;
        }
    }
    hitters.last().unwrap().0
}

fn spread_decisions_at_day(world: &mut World, day: u32) -> Vec<hexalife_lib::sim::disease::InfectionDecision> {
    immune_expiry_for_day(world, day);
    let n = world.people.len();
    let active: Vec<u32> = (0..n)
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
    decide_infections(world, day, &active)
}

#[test]
fn spread_parent_order_matches_wyrng_replay() {
    let mut cfg = SimConfig::defaults();
    cfg.width = 20;
    cfg.height = 20;
    cfg.initial_population = 80;
    cfg.initial_infected_count = 1;
    cfg.infection_rate = [1.0, 1.0, 1.0, 1.0];
    cfg.lethality_rate = [0.0, 0.0, 0.0, 0.0];
    cfg.infected_can_move = false;
    cfg.natural_death_rate_per_day = 0.0;
    cfg.birth_rate_per_day = 0.0;
    cfg.seed = 3;
    cfg.max_days = 60;

    let mut world = World::new(cfg.clone(), &mut StdRng::seed_from_u64(cfg.seed)).unwrap();

    for _ in 0..cfg.max_days {
        tick_day(&mut world);
        let day = world.day;
        let target = world
            .people
            .iter()
            .enumerate()
            .find(|(_, p)| p.state == STATE_INFECTED && p.generation == 1 && p.infected_on == day)
            .map(|(i, _)| i as u32);
        if let Some(target) = target {
            assert_eq!(world.people[target as usize].generation, 1);

            let mut probe = World::new(cfg.clone(), &mut StdRng::seed_from_u64(cfg.seed)).unwrap();
            for _ in 1..day {
                tick_day(&mut probe);
            }
            let decisions = spread_decisions_at_day(&mut probe, day);
            let sim_parent = decisions
                .iter()
                .find(|d| d.target == target)
                .map(|d| d.parent)
                .expect("spread decision for new infection");

            let active: Vec<u32> = (0..probe.people.len())
                .filter_map(|i| {
                    let id = i as u32;
                    probe.is_contagious_at_day_start(id, day).then_some(id)
                })
                .collect();
            let mut pairs: Vec<(u32, u32)> = active
                .iter()
                .map(|&id| (probe.people[id as usize].cell, id))
                .collect();
            pairs.sort_unstable();

            let replay_parent = replay_spread_parent(&probe, day, target, &pairs);
            assert_eq!(replay_parent, sim_parent, "day {day} target {target}");
            return;
        }
    }
    panic!(
        "no second-generation infection within {} days (last day {})",
        cfg.max_days,
        world.day
    );
}
