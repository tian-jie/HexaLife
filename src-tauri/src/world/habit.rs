use std::path::{Path, PathBuf};

const MAGIC: &[u8; 4] = b"HLHB";

pub struct HabitMask {
    pub width: u32,
    pub height: u32,
    /// None：每一格都宜居，不分配位图。
    bits: Option<Vec<u8>>,
    /// None：背景抽样仍走 0..cell_count。Some：仅这些格子，升序。
    habitable_index: Option<Vec<u32>>,
}

impl HabitMask {
    pub fn all(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            bits: None,
            habitable_index: None,
        }
    }

    pub fn from_bits(width: u32, height: u32, bits: Vec<u8>) -> Result<Self, String> {
        let cell_count = width as u64 * height as u64;
        if cell_count == 0 {
            return Err("地图尺寸无效".into());
        }
        let need = ((cell_count + 7) / 8) as usize;
        if bits.len() < need {
            return Err(format!(
                "宜居掩码数据过短：需要 {} 字节，实际 {}",
                need,
                bits.len()
            ));
        }
        let mut habitable_index = Vec::new();
        for index in 0..cell_count {
            if bit_set(&bits, index) {
                habitable_index.push(index as u32);
            }
        }
        Ok(Self {
            width,
            height,
            bits: Some(bits),
            habitable_index: Some(habitable_index),
        })
    }

    pub fn load(path: &Path, width: u32, height: u32) -> Result<Self, String> {
        let path = resolve_habit_path(path);
        let data = std::fs::read(&path)
            .map_err(|e| format!("无法读取宜居掩码 {}: {}", path.display(), e))?;
        if data.len() < 12 {
            return Err(format!("宜居掩码文件过短: {}", path.display()));
        }
        if data[0..4] != *MAGIC {
            return Err(format!("宜居掩码 magic 无效: {}", path.display()));
        }
        let file_w = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let file_h = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        if file_w != width || file_h != height {
            return Err(format!(
                "宜居掩码尺寸 {}×{} 与地图 {}×{} 不一致",
                file_w,
                file_h,
                width,
                height
            ));
        }
        Self::from_bits(width, height, data[12..].to_vec())
    }

    pub fn is_habitable(&self, index: u32) -> bool {
        match &self.bits {
            None => true,
            Some(bits) => bit_set(bits, index as u64),
        }
    }

    pub fn habitable_count(&self) -> u32 {
        match &self.habitable_index {
            None => self.width * self.height,
            Some(v) => v.len() as u32,
        }
    }

    pub fn habitable_index(&self) -> Option<&[u32]> {
        self.habitable_index.as_deref()
    }
}

/// 相对路径从当前目录和可执行文件目录向上找。Tauri 的工作目录是 `src-tauri`，掩码在仓库根的 `assets/`。
pub(crate) fn resolve_habit_path(raw: &Path) -> PathBuf {
    if raw.is_absolute() {
        return raw.to_path_buf();
    }
    for root in habit_search_roots() {
        if let Some(found) = walk_for_file(&root, raw) {
            return found;
        }
    }
    raw.to_path_buf()
}

fn habit_search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            roots.push(parent.to_path_buf());
        }
    }
    roots
}

fn walk_for_file(start: &Path, rel: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    for _ in 0..8 {
        let candidate = dir.join(rel);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

fn bit_set(bits: &[u8], index: u64) -> bool {
    let byte = (index / 8) as usize;
    let bit = (index % 8) as u8;
    bits.get(byte).is_some_and(|b| (b >> bit) & 1 == 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn mask_4x1_indices_1_3() -> HabitMask {
        // indices 1 and 3 habitable in 4×1
        let bits = vec![0b1010];
        HabitMask::from_bits(4, 1, bits).expect("mask")
    }

    #[test]
    fn from_bits_habitable_indices() {
        let m = mask_4x1_indices_1_3();
        assert!(!m.is_habitable(0));
        assert!(m.is_habitable(1));
        assert!(!m.is_habitable(2));
        assert!(m.is_habitable(3));
        assert_eq!(m.habitable_count(), 2);
    }

    #[test]
    fn all_cells_habitable() {
        let m = HabitMask::all(10, 10);
        assert!(m.is_habitable(55));
        assert_eq!(m.habitable_count(), 100);
        assert!(m.habitable_index().is_none());
    }

    #[test]
    fn load_round_trip() {
        let dir = std::env::temp_dir().join("hexalife_habit_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mask.bin");
        let bits = vec![0b1010u8];
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(MAGIC).unwrap();
        f.write_all(&4u32.to_le_bytes()).unwrap();
        f.write_all(&1u32.to_le_bytes()).unwrap();
        f.write_all(&bits).unwrap();
        let m = HabitMask::load(&path, 4, 1).unwrap();
        assert!(m.is_habitable(1));
        assert!(!m.is_habitable(0));
    }

    #[test]
    fn load_bad_magic_fails() {
        let dir = std::env::temp_dir().join("hexalife_habit_bad");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.bin");
        std::fs::write(&path, b"XXXX\x04\x00\x00\x00\x01\x00\x00\x00\x00").unwrap();
        assert!(HabitMask::load(&path, 4, 1).is_err());
    }

    #[test]
    fn load_size_mismatch_fails() {
        let dir = std::env::temp_dir().join("hexalife_habit_size");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("size.bin");
        let mut data = Vec::new();
        data.extend_from_slice(MAGIC);
        data.extend_from_slice(&8u32.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.push(0);
        std::fs::write(&path, &data).unwrap();
        assert!(HabitMask::load(&path, 4, 1).is_err());
    }

    #[test]
    fn relative_mask_found_above_nested_workdir() {
        let root = std::env::temp_dir().join(format!("hexalife_habit_walk_{}", std::process::id()));
        let nested = root.join("src-tauri");
        let mask_dir = root.join("assets/masks");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir_all(&mask_dir).unwrap();
        let mask = mask_dir.join("wsf2019-1000x1000.bin");
        std::fs::write(&mask, b"HLHB").unwrap();
        let rel = Path::new("assets/masks/wsf2019-1000x1000.bin");
        let found = walk_for_file(&nested, rel).expect("mask above nested dir");
        assert_eq!(found, mask);
        let _ = std::fs::remove_dir_all(&root);
    }
}
