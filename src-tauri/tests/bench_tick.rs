use hexalife_lib::config::{DensityCenter, SimConfig};
use hexalife_lib::sim::tick::tick_day;
use hexalife_lib::world::World;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::time::Instant;

fn config_50k() -> SimConfig {
    let mut cfg = SimConfig::defaults();
    cfg.width = 1000;
    cfg.height = 1000;
    cfg.initial_population = 50_000;
    cfg.initial_infected_count = 1;
    cfg.centers = vec![
        DensityCenter { cell: 250000, peak: 4.0, sigma: 20.0 },
        DensityCenter { cell: 750000, peak: 4.0, sigma: 20.0 },
        DensityCenter { cell: 500500, peak: 4.0, sigma: 20.0 },
        DensityCenter { cell: 250500, peak: 4.0, sigma: 20.0 },
        DensityCenter { cell: 750500, peak: 4.0, sigma: 20.0 },
        DensityCenter { cell: 500000, peak: 4.0, sigma: 20.0 },
    ];
    cfg.seed = 1;
    cfg
}

#[test]
fn bench_world_and_one_day_50k() {
    let cfg = config_50k();
    let mut rng = StdRng::seed_from_u64(1);
    let t0 = Instant::now();
    let mut world = World::new(cfg, &mut rng).expect("world");
    eprintln!("world build: {:?}", t0.elapsed());
    let t1 = Instant::now();
    tick_day(&mut world, &mut rng);
    eprintln!("one tick: {:?}", t1.elapsed());
    assert_eq!(world.day, 1);
}
