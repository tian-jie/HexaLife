pub const LINK_NONE: u32 = u32::MAX;

pub fn cell_index(width: u32, q: u32, r: u32) -> u32 {
    r * width + q
}

pub fn qr_from_index(width: u32, index: u32) -> (u32, u32) {
    (index % width, index / width)
}

pub fn in_bounds(width: u32, height: u32, q: i32, r: i32) -> bool {
    q >= 0 && r >= 0 && (q as u32) < width && (r as u32) < height
}

pub fn hex_distance(q1: u32, r1: u32, q2: u32, r2: u32) -> u32 {
    let dq = (q1 as i32 - q2 as i32).unsigned_abs();
    let dr = (r1 as i32 - r2 as i32).unsigned_abs();
    let ds = ((q1 as i32 + r1 as i32) - (q2 as i32 + r2 as i32)).unsigned_abs();
    (dq + dr + ds) / 2
}

const NEIGHBOR_DR: [i32; 6] = [0, 0, 1, -1, 1, -1];
const NEIGHBOR_DQ: [i32; 6] = [1, -1, 0, 0, 1, -1];

/// Axial step directions (same order as neighbor_indices).
const RING_DQ: [i32; 6] = [1, 1, 0, -1, -1, 0];
const RING_DR: [i32; 6] = [0, -1, -1, 0, 1, 1];

pub fn neighbor_indices(width: u32, height: u32, q: u32, r: u32) -> [u32; 6] {
    let mut out = [LINK_NONE; 6];
    for i in 0..6 {
        let nq = q as i32 + NEIGHBOR_DQ[i];
        let nr = r as i32 + NEIGHBOR_DR[i];
        if in_bounds(width, height, nq, nr) {
            out[i] = cell_index(width, nq as u32, nr as u32);
        }
    }
    out
}

/// In-bounds cells at exactly graph distance `dist` from (q, r). O(dist) on the ring.
/// Fast check: any in-bounds cell at exactly `dist` (without building the list).
pub fn has_cells_at_distance(width: u32, height: u32, q: u32, r: u32, dist: u32) -> bool {
    if dist == 0 {
        return true;
    }
    let d = dist as i32;
    let mut cq = q as i32 + RING_DQ[4] * d;
    let mut cr = r as i32 + RING_DR[4] * d;
    for side in 0..6 {
        for _ in 0..d {
            if in_bounds(width, height, cq, cr) {
                return true;
            }
            cq += RING_DQ[side];
            cr += RING_DR[side];
        }
    }
    false
}

pub fn cells_at_distance(width: u32, height: u32, q: u32, r: u32, dist: u32) -> Vec<u32> {
    if dist == 0 {
        return vec![cell_index(width, q, r)];
    }
    let d = dist as i32;
    let mut cq = q as i32 + RING_DQ[4] * d;
    let mut cr = r as i32 + RING_DR[4] * d;
    let mut cells = Vec::with_capacity((6 * d) as usize);
    for side in 0..6 {
        for _ in 0..d {
            if in_bounds(width, height, cq, cr) {
                cells.push(cell_index(width, cq as u32, cr as u32));
            }
            cq += RING_DQ[side];
            cr += RING_DR[side];
        }
    }
    cells.sort_unstable();
    cells.dedup();
    cells
}

pub fn tile_coords(tile_size: u32, q: u32, r: u32) -> (u32, u32) {
    (q / tile_size, r / tile_size)
}
