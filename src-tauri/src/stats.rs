use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct DayStats {
    pub day: u32,
    pub alive: u64,
    pub healthy: u64,
    pub infected: u64,
    pub immune: u64,
    pub new_infections: u64,
    pub new_immune: u64,
    pub new_virus_deaths: u64,
    pub new_natural_deaths: u64,
    pub new_births: u64,
    pub total_virus_deaths: u64,
    pub total_natural_deaths: u64,
    pub total_births: u64,
    pub virus_deaths_by_gen: [u64; 4],
    pub infected_by_gen: [u64; 4],
    pub run_state: String,
    pub error: Option<String>,
    pub map_width: u32,
    pub map_height: u32,
    pub camera_center_q: f64,
    pub camera_center_r: f64,
    pub camera_cells_per_pixel: f64,
}

impl DayStats {
    pub fn zeroed() -> Self {
        Self {
            run_state: "idle".into(),
            ..Default::default()
        }
    }
}
