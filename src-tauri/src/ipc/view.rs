use crate::world::World;
use crate::world::TILE_SIZE;

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Camera {
    pub center_q: f64,
    pub center_r: f64,
    pub cells_per_pixel: f64,
}

/// 世界地图默认对准经纬网格中心，缩放以约 512 像素宽看全图。
pub fn default_camera_for_map(width: u32, height: u32) -> Camera {
    let w = width as f64;
    let h = height as f64;
    let view = 512.0;
    Camera {
        center_q: w / 2.0,
        center_r: h / 2.0,
        cells_per_pixel: (w / view).max(h / view).max(0.25),
    }
}

pub fn cell_rgba(count: u32, infected: u32, c_bright: u32, habitable: bool) -> [u8; 4] {
    if count == 0 {
        if habitable {
            return [70, 78, 48, 255];
        }
        return [12, 28, 56, 255];
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

fn sum_tiles_in_rect(world: &World, q0: i32, r0: i32, q1: i32, r1: i32) -> (u32, u32, bool) {
    let mut total = 0u32;
    let mut infected = 0u32;
    let mut any_habitable = false;
    if q1 < q0 || r1 < r0 {
        return (0, 0, false);
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
            if t.habitable_cells > 0 {
                any_habitable = true;
            }
        }
    }
    (total, infected, any_habitable)
}

fn sum_cell_pop_in_rect(world: &World, q0: i32, r0: i32, span: u32) -> (u32, u32, bool) {
    let mut total = 0u32;
    let mut infected = 0u32;
    let mut any_habitable = false;
    for dr in 0..span {
        for dq in 0..span {
            let q = q0 + dq as i32;
            let r = r0 + dr as i32;
            if !crate::world::hex::in_bounds(world.cfg.width, world.cfg.height, q, r) {
                continue;
            }
            let idx = crate::world::hex::cell_index(world.cfg.width, q as u32, r as u32);
            let p = &world.cell_pop[idx as usize];
            total += p.healthy + p.infected + p.immune;
            infected += p.infected;
            if world.habit.is_habitable(idx) {
                any_habitable = true;
            }
        }
    }
    (total, infected, any_habitable)
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
    let use_tiles = cpp >= TILE_SIZE as f64;
    let mask_active = world.habit.habitable_index().is_some();

    for py in 0..h {
        for px in 0..w {
            let fq = camera.center_q + (px as f64 - w as f64 / 2.0) * cpp;
            let fr = camera.center_r + (py as f64 - h as f64 / 2.0) * cpp;
            let q0 = fq.floor() as i32;
            let r0 = fr.floor() as i32;
            let span = cpp.ceil().max(1.0) as u32;
            let q1 = q0 + span as i32 - 1;
            let r1 = r0 + span as i32 - 1;

            let (total, infected, any_habitable) = if use_tiles {
                sum_tiles_in_rect(world, q0, r0, q1, r1)
            } else {
                sum_cell_pop_in_rect(world, q0, r0, span)
            };

            let rgba = if total == 0 && !mask_active {
                [0, 0, 0, 255]
            } else {
                let habitable = any_habitable || !mask_active;
                cell_rgba(total, infected, c_bright, habitable)
            };
            let i = (py * w + px) * 4;
            pixels[i..i + 4].copy_from_slice(&rgba);
        }
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_cell_colors_by_habitability() {
        assert_eq!(cell_rgba(0, 0, 64, false), [12, 28, 56, 255]);
        assert_eq!(cell_rgba(0, 0, 64, true), [70, 78, 48, 255]);
    }
}
