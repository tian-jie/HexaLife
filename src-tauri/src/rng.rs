//! 每人、每天、每阶段一条独立随机流。同一种子重复运行，每天的判定相同。
//! 第 0 天的人口放置仍用 StdRng，不经过这里。

pub const PHASE_MOVE_KIND: u32 = 1;
pub const PHASE_MOVE_DISTANCE: u32 = 2;
pub const PHASE_MOVE_DEST: u32 = 3;
pub const PHASE_SPREAD: u32 = 4;
pub const PHASE_NATURAL_DEATH: u32 = 5;
pub const PHASE_VIRUS_DEATH: u32 = 6;
pub const PHASE_BIRTH: u32 = 7;

#[derive(Clone, Debug)]
pub struct WyRng {
    state: u64,
}

impl WyRng {
    pub fn from_phase(seed: u64, day: u32, person: u32, phase: u32) -> Self {
        let mixed = seed
            ^ (day as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (person as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
            ^ (phase as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
        let mut state = splitmix64(mixed);
        if state == 0 {
            state = 0xA076_1D64_78BD_642F;
        }
        Self { state }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0xA076_1D64_78BD_642F);
        let a = self.state as u128;
        let b = (self.state ^ 0xE703_7ED1_A0B4_28DB) as u128;
        let t = a.wrapping_mul(b);
        (t as u64) ^ (t >> 64) as u64
    }

    pub fn next_f64(&mut self) -> f64 {
        let bits = self.next_u64() >> 11;
        bits as f64 * (1.0 / ((1u64 << 53) as f64))
    }
}

fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_phase_is_stable_and_streams_differ() {
        let a = WyRng::from_phase(1, 3, 9, PHASE_MOVE_KIND);
        let b = WyRng::from_phase(1, 3, 9, PHASE_MOVE_KIND);
        let mut a = a;
        let mut b = b;
        let mut other = WyRng::from_phase(1, 3, 9, PHASE_BIRTH);
        assert_eq!(a.next_u64(), b.next_u64());
        assert_ne!(a.next_u64(), other.next_u64());
    }

    #[test]
    fn unit_interval() {
        let mut rng = WyRng::from_phase(7, 1, 0, PHASE_SPREAD);
        for _ in 0..1000 {
            let u = rng.next_f64();
            assert!(u >= 0.0 && u < 1.0);
        }
    }
}
