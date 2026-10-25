use hexalife_lib::config::SimConfig;
use hexalife_lib::sim::tick::tick_day;
use hexalife_lib::world::World;
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
        tick_day(&mut world, &mut rng);
    }
    assert_eq!(world.day, 10);
    assert!(world.stats.alive > 0);
}
