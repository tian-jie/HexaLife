use crate::world::World;
use crate::world::TILE_SIZE;

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Camera {
    pub center_q: f64,
    pub center_r: f64,
    pub cells_per_pixel: f64,
}

pub fn cell_rgba(count: u32, infected: u32, c_bright: u32) -> [u8; 4] {
    if count == 0 {
        return [0, 0, 0, 255];
    }
    let ratio = infected as f64 / count as f64;
    let light = (f64::ln(1.0 + count as f64) / f64::ln(1.0 + c_bright as f64)).clamp(0.0, 1.0);
    let hue = if infected == 0 {
        120.0
    } else {
        60.0 * (1.0 - ratio)
    };
    let (r, g, b) = hsv_to_rgb(hue, 1.0, light);
    [r, g, b, 255]
}

fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (u8, u8, u8) {
    let c = v * s;
    let hh = (h / 60.0) % 6.0;
    let x = c * (1.0 - ((hh % 2.0) - 1.0).abs());
    let m = v - c;
    let (rp, gp, bp) = match hh as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((rp + m) * 255.0) as u8,
        ((gp + m) * 255.0) as u8,
        ((bp + m) * 255.0) as u8,
    )
}

fn sum_tiles_in_rect(
    world: &World,
    q0: i32,
    r0: i32,
    q1: i32,
    r1: i32,
) -> (u32, u32) {
    let mut total = 0u32;
    let mut infected = 0u32;
    if q1 < q0 || r1 < r0 {
        return (0, 0);
    }
    let tq0 = (q0.max(0) as u32) / TILE_SIZE;
    let tq1 = (q1.max(0) as u32) / TILE_SIZE;
    let tr0 = (r0.max(0) as u32) / TILE_SIZE;
    let tr1 = (r1.max(0) as u32) / TILE_SIZE;
    let tq1 = tq1.min(world.tiles_w - 1);
    let tr1 = tr1.min(world.tiles_h - 1);
    for ty in tr0..=tr1 {
        for tx in tq0..=tq1 {
            let t = &world.tiles[(ty * world.tiles_w + tx) as usize];
            total += t.healthy + t.infected + t.immune;
            infected += t.infected;
        }
    }
    (total, infected)
}

fn sum_cells_in_rect(world: &World, q0: i32, r0: i32, span: u32) -> (u32, u32) {
    let mut total = 0u32;
    let mut infected = 0u32;
    for dr in 0..span {
        for dq in 0..span {
            let q = q0 + dq as i32;
            let r = r0 + dr as i32;
            if !crate::world::hex::in_bounds(world.cfg.width, world.cfg.height, q, r) {
                continue;
            }
            let cell = crate::world::hex::cell_index(world.cfg.width, q as u32, r as u32);
            let (h, i, m) = world.cell_counts(cell);
            total += h + i + m;
            infected += i;
        }
    }
    (total, infected)
}

pub fn render_view(world: &World, camera: Camera, out_w: u16, out_h: u16) -> Vec<u8> {
    let w = out_w as usize;
    let h = out_h as usize;
    let mut buf = vec![0u8; 4 + w * h * 4];
    buf[0..2].copy_from_slice(&out_w.to_le_bytes());
    buf[2..4].copy_from_slice(&out_h.to_le_bytes());
    let pixels = &mut buf[4..];
    let cpp = camera.cells_per_pixel.max(0.25);
    let c_bright = world.cfg.c_bright;
    let use_tile_blocks = cpp >= TILE_SIZE as f64;

    for py in 0..h {
        for px in 0..w {
            let fq = camera.center_q + (px as f64 - w as f64 / 2.0) * cpp;
            let fr = camera.center_r + (py as f64 - h as f64 / 2.0) * cpp;
            let q0 = fq.floor() as i32;
            let r0 = fr.floor() as i32;
            let span = cpp.ceil().max(1.0) as u32;
            let q1 = q0 + span as i32 - 1;
            let r1 = r0 + span as i32 - 1;

            let (total, infected) = if use_tile_blocks {
                sum_tiles_in_rect(world, q0, r0, q1, r1)
            } else if span <= 1 {
                let (t, i) = sum_cells_in_rect(world, q0, r0, 1);
                (t, i)
            } else {
                sum_cells_in_rect(world, q0, r0, span)
            };

            let rgba = cell_rgba(total, infected, c_bright);
            let i = (py * w + px) * 4;
            pixels[i..i + 4].copy_from_slice(&rgba);
        }
    }
    buf
}
