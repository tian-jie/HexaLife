use crate::config::SimConfig;
use crate::ipc::view::Camera;
use crate::sim::tick::tick_day;
use crate::stats::DayStats;
use crate::world::World;
use crossbeam_channel::{Receiver, Sender};
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub enum Control {
    Start,
    Pause,
    Resume,
    Reset,
    SetSpeed(f32),
    SetCamera(Camera),
    QueryCell {
        q: u32,
        r: u32,
        reply: std::sync::mpsc::Sender<(u32, u32, u32)>,
    },
    Shutdown,
}

struct Session {
    world: World,
    rng: StdRng,
}

pub struct SharedSim {
    pub tx: Sender<Control>,
    pub latest_stats: Arc<Mutex<DayStats>>,
    pub latest_view: Arc<Mutex<Vec<u8>>>,
    pub camera: Arc<Mutex<Camera>>,
}

impl SharedSim {
    pub fn spawn() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let latest_stats = Arc::new(Mutex::new(DayStats::zeroed()));
        let latest_view = Arc::new(Mutex::new(Vec::new()));
        let camera = Arc::new(Mutex::new(Camera {
            center_q: 500.0,
            center_r: 500.0,
            cells_per_pixel: 2.0,
        }));
        let stats_clone = latest_stats.clone();
        let view_clone = latest_view.clone();
        let camera_clone = camera.clone();
        thread::spawn(move || sim_loop(rx, stats_clone, view_clone, camera_clone));
        SharedSim {
            tx,
            latest_stats,
            latest_view,
            camera,
        }
    }

    pub fn query_cell(&self, q: u32, r: u32) -> (u32, u32, u32) {
        let (reply_tx, reply_rx) = std::sync::mpsc::channel();
        if self
            .tx
            .send(Control::QueryCell {
                q,
                r,
                reply: reply_tx,
            })
            .is_err()
        {
            return (0, 0, 0);
        }
        reply_rx
            .recv_timeout(Duration::from_millis(30))
            .unwrap_or((0, 0, 0))
    }
}

fn config_path() -> PathBuf {
    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.to_path_buf());
        }
    }
    for mut dir in candidates {
        for _ in 0..6 {
            let p = dir.join("config/default.toml");
            if p.exists() {
                return p;
            }
            if !dir.pop() {
                break;
            }
        }
    }
    PathBuf::from("config/default.toml")
}

fn try_build_session() -> Result<Session, String> {
    let path = config_path();
    let cfg = SimConfig::load_from_file(&path)?;
    let mut rng = StdRng::seed_from_u64(cfg.seed);
    let world = World::new(cfg, &mut rng)?;
    Ok(Session { world, rng })
}

fn set_status(stats_out: &Arc<Mutex<DayStats>>, run_state: &str, error: Option<String>) {
    let mut st = stats_out.lock().unwrap();
    st.run_state = run_state.into();
    st.error = error;
}

fn sim_loop(
    rx: Receiver<Control>,
    stats_out: Arc<Mutex<DayStats>>,
    view_out: Arc<Mutex<Vec<u8>>>,
    camera: Arc<Mutex<Camera>>,
) {
    let mut running = false;
    let mut session: Option<Session> = None;
    let mut speed = 10.0f32;
    let mut refresh_ms = 100u64;
    let mut last_push = Instant::now();

    loop {
        if !running {
            match rx.recv() {
                Ok(Control::Shutdown) => break,
                Ok(cmd) => {
                    handle_idle_cmd(
                        cmd,
                        &mut session,
                        &mut running,
                        &mut speed,
                        &mut refresh_ms,
                        &stats_out,
                        &view_out,
                        &camera,
                    );
                }
                Err(_) => break,
            }
            continue;
        }

        drain_pending(
            &rx,
            &mut session,
            &mut running,
            &mut speed,
            &mut refresh_ms,
            &stats_out,
            &view_out,
            &camera,
        );

        if !running {
            continue;
        }

        let started = Instant::now();
        if let Some(sess) = &mut session {
            tick_day(&mut sess.world, &mut sess.rng);
            if sess.world.day >= sess.world.cfg.max_days {
                running = false;
                push_frame(sess, &stats_out, &view_out, &camera, "paused", None);
                continue;
            }
            push_stats_light(sess, &stats_out, "running", None);
            if last_push.elapsed() >= Duration::from_millis(refresh_ms) {
                push_frame(sess, &stats_out, &view_out, &camera, "running", None);
                last_push = Instant::now();
            }
        }

        drain_pending(
            &rx,
            &mut session,
            &mut running,
            &mut speed,
            &mut refresh_ms,
            &stats_out,
            &view_out,
            &camera,
        );

        let elapsed = started.elapsed();
        if speed > 0.0 {
            let min = Duration::from_secs_f32(1.0 / speed);
            if elapsed < min {
                thread::sleep(min - elapsed);
            }
        }
    }
}

fn drain_pending(
    rx: &Receiver<Control>,
    session: &mut Option<Session>,
    running: &mut bool,
    speed: &mut f32,
    refresh_ms: &mut u64,
    stats_out: &Arc<Mutex<DayStats>>,
    view_out: &Arc<Mutex<Vec<u8>>>,
    camera: &Arc<Mutex<Camera>>,
) {
    while let Ok(cmd) = rx.try_recv() {
        apply_cmd(
            cmd,
            session,
            running,
            speed,
            refresh_ms,
            stats_out,
            view_out,
            camera,
        );
    }
}

fn begin_world_build(stats_out: &Arc<Mutex<DayStats>>, label: &str) {
    set_status(stats_out, label, None);
}

fn handle_idle_cmd(
    cmd: Control,
    session: &mut Option<Session>,
    running: &mut bool,
    speed: &mut f32,
    refresh_ms: &mut u64,
    stats_out: &Arc<Mutex<DayStats>>,
    view_out: &Arc<Mutex<Vec<u8>>>,
    camera: &Arc<Mutex<Camera>>,
) {
    match cmd {
        Control::Shutdown => {}
        Control::QueryCell { q, r, reply } => {
            let counts = session
                .as_ref()
                .map(|s| cell_counts_at(&s.world, q, r))
                .unwrap_or((0, 0, 0));
            let _ = reply.send(counts);
        }
        Control::SetSpeed(s) => *speed = s,
        Control::SetCamera(c) => {
            if let Ok(mut cam) = camera.lock() {
                *cam = c;
            }
        }
        Control::Start => {
            begin_world_build(stats_out, "generating");
            match try_build_session() {
                Ok(sess) => {
                    *refresh_ms = sess.world.cfg.refresh_interval_ms;
                    *speed = sess.world.cfg.default_speed;
                    *session = Some(sess);
                    *running = true;
                    if let Some(s) = session.as_ref() {
                        push_frame(s, stats_out, view_out, camera, "running", None);
                    }
                }
                Err(e) => {
                    set_status(stats_out, "idle", Some(e));
                }
            }
        }
        Control::Reset => {
            let had = session.is_some();
            begin_world_build(stats_out, "generating");
            match try_build_session() {
                Ok(sess) => {
                    *refresh_ms = sess.world.cfg.refresh_interval_ms;
                    *speed = sess.world.cfg.default_speed;
                    *session = Some(sess);
                    *running = true;
                    if let Some(s) = session.as_ref() {
                        push_frame(s, stats_out, view_out, camera, "running", None);
                    }
                }
                Err(e) => {
                    set_status(stats_out, if had { "paused" } else { "idle" }, Some(e));
                }
            }
        }
        Control::Pause | Control::Resume => {}
    }
}

fn apply_cmd(
    cmd: Control,
    session: &mut Option<Session>,
    running: &mut bool,
    speed: &mut f32,
    refresh_ms: &mut u64,
    stats_out: &Arc<Mutex<DayStats>>,
    view_out: &Arc<Mutex<Vec<u8>>>,
    camera: &Arc<Mutex<Camera>>,
) {
    match cmd {
        Control::Shutdown => *running = false,
        Control::QueryCell { q, r, reply } => {
            let counts = session
                .as_ref()
                .map(|s| cell_counts_at(&s.world, q, r))
                .unwrap_or((0, 0, 0));
            let _ = reply.send(counts);
        }
        Control::Pause => {
            *running = false;
            if let Some(s) = session.as_ref() {
                push_frame(s, stats_out, view_out, camera, "paused", None);
            }
        }
        Control::Resume => {
            if let Some(s) = session.as_ref() {
                if s.world.day < s.world.cfg.max_days {
                    *running = true;
                }
            }
        }
        Control::SetSpeed(s) => *speed = s,
        Control::SetCamera(c) => {
            if let Ok(mut cam) = camera.lock() {
                *cam = c;
            }
        }
        Control::Start | Control::Reset => {
            handle_idle_cmd(
                cmd,
                session,
                running,
                speed,
                refresh_ms,
                stats_out,
                view_out,
                camera,
            );
        }
    }
}

fn cell_counts_at(world: &World, q: u32, r: u32) -> (u32, u32, u32) {
    if !crate::world::hex::in_bounds(world.cfg.width, world.cfg.height, q as i32, r as i32) {
        return (0, 0, 0);
    }
    let cell = crate::world::hex::cell_index(world.cfg.width, q, r);
    world.cell_counts(cell)
}

fn push_stats_light(
    sess: &Session,
    stats_out: &Arc<Mutex<DayStats>>,
    run_state: &str,
    error: Option<String>,
) {
    let mut st = sess.world.stats.clone();
    st.run_state = run_state.into();
    st.error = error;
    *stats_out.lock().unwrap() = st;
}

fn push_frame(
    sess: &Session,
    stats_out: &Arc<Mutex<DayStats>>,
    view_out: &Arc<Mutex<Vec<u8>>>,
    camera: &Arc<Mutex<Camera>>,
    run_state: &str,
    error: Option<String>,
) {
    let cam = camera.lock().unwrap();
    let view = crate::ipc::view::render_view(&sess.world, *cam, 512, 512);
    *view_out.lock().unwrap() = view;
    let mut st = sess.world.stats.clone();
    st.run_state = run_state.into();
    st.error = error;
    if sess.world.day == 0 && st.alive == 0 {
        st.alive = sess.world.alive.len() as u64;
    }
    *stats_out.lock().unwrap() = st;
}
