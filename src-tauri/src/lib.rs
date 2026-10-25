pub mod config;
pub mod rng;
pub mod ipc;
pub mod sim;
mod state;
pub mod stats;
pub mod world;

use ipc::view::Camera;
use state::{Control, SharedSim};
use stats::DayStats;
use std::sync::Mutex;
use tauri::State;

struct AppState {
    sim: SharedSim,
}

#[tauri::command]
fn start(app: State<'_, Mutex<AppState>>) {
    let _ = app.lock().unwrap().sim.tx.send(Control::Start);
}

#[tauri::command]
fn pause(app: State<'_, Mutex<AppState>>) {
    let _ = app.lock().unwrap().sim.tx.send(Control::Pause);
}

#[tauri::command]
fn resume(app: State<'_, Mutex<AppState>>) {
    let _ = app.lock().unwrap().sim.tx.send(Control::Resume);
}

#[tauri::command]
fn stop(app: State<'_, Mutex<AppState>>) {
    let _ = app.lock().unwrap().sim.tx.send(Control::Stop);
}

#[tauri::command]
fn set_speed(speed: f32, app: State<'_, Mutex<AppState>>) {
    let _ = app.lock().unwrap().sim.tx.send(Control::SetSpeed(speed));
}

#[tauri::command]
fn set_camera(camera: Camera, app: State<'_, Mutex<AppState>>) {
    let _ = app.lock().unwrap().sim.tx.send(Control::SetCamera(camera));
}

#[tauri::command]
fn get_stats(app: State<'_, Mutex<AppState>>) -> DayStats {
    app.lock().unwrap().sim.latest_stats.lock().unwrap().clone()
}

#[tauri::command]
fn get_view(app: State<'_, Mutex<AppState>>) -> Vec<u8> {
    app.lock().unwrap().sim.latest_view.lock().unwrap().clone()
}

#[derive(serde::Serialize)]
struct CellCounts {
    healthy: u32,
    infected: u32,
    immune: u32,
}

#[tauri::command]
fn query_cell(q: u32, r: u32, app: State<'_, Mutex<AppState>>) -> CellCounts {
    let (h, i, m) = app.lock().unwrap().sim.query_cell(q, r);
    CellCounts {
        healthy: h,
        infected: i,
        immune: m,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let sim = SharedSim::spawn();
    tauri::Builder::default()
        .manage(Mutex::new(AppState { sim }))
        .invoke_handler(tauri::generate_handler![
            start,
            pause,
            resume,
            stop,
            set_speed,
            set_camera,
            get_stats,
            get_view,
            query_cell
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
