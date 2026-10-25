use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::time::Instant;

/// Very cheap refill when statistical quality is enough.
struct FastFill(u64);
impl FastFill {
    fn next_f64(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.0 >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

const N: u64 = 20_000_000;

#[test]
fn rng_pool_compare() {
    let mut rng = StdRng::seed_from_u64(42);
    let t0 = Instant::now();
    let mut hits = 0u64;
    for _ in 0..N {
        if rng.gen::<f64>() < 0.0001 {
            hits += 1;
        }
    }
    eprintln!("StdRng inline {} draws: {:?}", N, t0.elapsed());

    let mut rng = StdRng::seed_from_u64(42);
    let t2 = Instant::now();
    let pool: Vec<f64> = (0..N).map(|_| rng.gen()).collect();
    eprintln!("prefill {} f64 (StdRng): {:?}", N, t2.elapsed());

    let t3 = Instant::now();
    hits = 0;
    for u in &pool {
        let v = std::hint::black_box(*u);
        if v < 0.0001 {
            hits += 1;
        }
    }
    eprintln!("sequential read {} f64: {:?}", N, t3.elapsed());

    let mut rng = StdRng::seed_from_u64(42);
    let mut pool1m: Vec<f64> = (0..1_000_000).map(|_| rng.gen()).collect();
    let mut idx = 0usize;
    let t4 = Instant::now();
    hits = 0;
    for _ in 0..N {
        if pool1m[idx] < 0.0001 {
            hits += 1;
        }
        idx += 1;
        if idx >= pool1m.len() {
            idx = 0;
            for i in 0..pool1m.len() {
                pool1m[i] = rng.gen();
            }
        }
    }
    eprintln!("1M pool + StdRng refill per 1M for {} draws: {:?}", N, t4.elapsed());

    let mut ff = FastFill(42);
    let t6 = Instant::now();
    let pool_fast: Vec<f64> = (0..N).map(|_| ff.next_f64()).collect();
    eprintln!("prefill {} with LCG: {:?}", N, t6.elapsed());

    let t7 = Instant::now();
    hits = 0;
    for u in &pool_fast {
        let v = std::hint::black_box(*u);
        if v < 0.0001 {
            hits += 1;
        }
    }
    eprintln!("read {} from LCG pool: {:?}", N, t7.elapsed());
}
