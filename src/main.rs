mod config;
mod noise;
mod terrain;
mod pheromone;
mod ant;
mod food;
mod render;
pub mod sim_config;
mod ui;

use bevy::prelude::*;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use rand::Rng;
use config::*;
use sim_config::SimConfig;
use ant::{Ant, AntState, Colony};
use food::{spawn_food, spawn_nest, FoodAssets, FoodScore};
use terrain::WorldMap;
use ui::UiPlugin;

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

static LOG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

fn log(msg: &str) {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let line = format!("[{}] {}\n", timestamp, msg);
    if let Ok(mut guard) = LOG_FILE.lock() {
        if let Some(f) = guard.as_mut() {
            let _ = f.write_all(line.as_bytes());
            let _ = f.flush();
        }
    }
    eprintln!("{}", line.trim());
}

fn init_log() {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let log_path = exe_dir.join("ant_simulation.log");
    match OpenOptions::new().create(true).append(true).open(&log_path) {
        Ok(f) => {
            if let Ok(mut guard) = LOG_FILE.lock() {
                *guard = Some(f);
            }
            log(&format!("=== Ant Simulation started === log: {}", log_path.display()));
        }
        Err(e) => eprintln!("Could not open log file {}: {}", log_path.display(), e),
    }
}

fn main() {
    init_log();
    log("main: setting panic hook");
    std::panic::set_hook(Box::new(|info| {
        let msg = format!("PANIC: {}", info);
        log(&msg);
        // Give the OS time to flush before the process dies
        std::thread::sleep(std::time::Duration::from_millis(200));
    }));
    log("main: calling build_and_run");
    build_and_run();
    log("main: build_and_run returned (clean exit)");
}

/// Marker component for all simulation entities that should be despawned on restart.
/// Camera and HUD text are NOT marked — they persist across restarts.
#[derive(Component)]
pub struct SimEntity;

/// App state: Running is normal simulation; Restarting tears down and reinitialises.
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Running,
    Restarting,
}

fn build_and_run() {
    log("build_and_run: configuring DefaultPlugins");
    let default_plugins = DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Ant Simulation".into(),
            resolution: (WINDOW_W, WINDOW_H).into(),
            resizable: false,
            ..default()
        }),
        ..default()
    });

    #[cfg(feature = "dev")]
    let default_plugins = default_plugins.set(AssetPlugin {
        watch_for_changes_override: Some(true),
        ..default()
    });

    log("build_and_run: building App");
    App::new()
        .add_plugins(default_plugins)
        .init_resource::<SimConfig>()
        .init_state::<AppState>()
        .add_plugins((
            terrain::WorldPlugin,
            pheromone::PheromonePlugin,
            ant::AntPlugin,
            food::FoodPlugin,
            render::RenderPlugin,
            FrameTimeDiagnosticsPlugin,
            UiPlugin,
        ))
        .insert_resource(FoodPositions::default())
        // ── Persistent one-time startup (camera, HUD) ──────────────────────
        .add_systems(
            Startup,
            (spawn_camera, setup_score_ui, setup_fps_ui),
        )
        // ── Simulation init/reinit on every RunningState entry ─────────────
        .add_systems(
            OnEnter(AppState::Running),
            (
                reset_simulation_resources,
                food::setup_food_assets,
                terrain::terrain_startup_system,
                render::setup_terrain_mesh,
                render::setup_pheromone_overlay,
                spawn_nest_and_food,
                spawn_ants,
            )
                .chain(),
        )
        // ── Teardown on Restarting entry, then flip back to Running ─────────
        .add_systems(OnEnter(AppState::Restarting), teardown_system)
        // ── Per-frame update systems ────────────────────────────────────────
        .add_systems(
            Update,
            (
                update_score_ui,
                update_fps_ui,
                ant_respawn_system.after(ant::ant_age_system),
                update_food_positions.after(food::food_respawn_system),
            ),
        )
        .run();
    log("build_and_run: App::run() returned");
}

/// Despawn all simulation entities and immediately queue transition back to Running.
fn teardown_system(
    mut commands: Commands,
    query: Query<Entity, With<SimEntity>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
    next_state.set(AppState::Running);
}

/// Reset stateful resources so a fresh simulation starts clean.
fn reset_simulation_resources(mut commands: Commands) {
    log("system: reset_simulation_resources");
    commands.insert_resource(pheromone::PheromoneGrid::new());
    commands.insert_resource(FoodPositions::default());
    commands.insert_resource(FoodScore::default());
}

fn spawn_camera(mut commands: Commands) {
    log("system: spawn_camera");
    commands.spawn(Camera2d::default());
}

/// Cached food positions updated each frame for ant seek-force queries.
#[derive(Resource, Default)]
pub struct FoodPositions(pub Vec<Vec2>);

fn update_food_positions(
    mut positions: ResMut<FoodPositions>,
    food_query: Query<&Transform, With<food::Food>>,
    added_food: Query<(), Added<food::Food>>,
    mut removed_food: RemovedComponents<food::Food>,
) {
    let has_additions = !added_food.is_empty();
    let has_removals = removed_food.read().next().is_some();
    if !has_additions && !has_removals {
        return;
    }
    positions.0.clear();
    for transform in food_query.iter() {
        positions.0.push(transform.translation.truncate());
    }
}

fn spawn_nest_and_food(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    world_map: Res<WorldMap>,
    food_assets: Res<FoodAssets>,
    config: Res<SimConfig>,
) {
    log("system: spawn_nest_and_food");
    let mut rng = rand::thread_rng();

    let nest_pos = Vec2::ZERO;
    let nest_grid_x = GRID_W / 2;
    let nest_grid_y = GRID_H / 2;

    spawn_nest(&mut commands, &mut meshes, &mut materials, nest_pos);

    let food_candidates: Vec<usize> = world_map
        .open_cells
        .iter()
        .copied()
        .filter(|&cell_idx| {
            let gx = cell_idx % GRID_W;
            let gy = cell_idx / GRID_W;
            let dx = gx as i32 - nest_grid_x as i32;
            let dy = gy as i32 - nest_grid_y as i32;
            let dist_sq = (dx * dx + dy * dy) as usize;
            dist_sq >= FOOD_MIN_NEST_DIST_CELLS * FOOD_MIN_NEST_DIST_CELLS
        })
        .collect();

    let source = if food_candidates.is_empty() { &world_map.open_cells } else { &food_candidates };

    let mut occupied = vec![false; GRID_W * GRID_H];

    for _ in 0..config.food_source_count {
        if source.is_empty() {
            break;
        }
        let center_idx = source[rng.gen_range(0..source.len())];
        let center_gx = center_idx % GRID_W;
        let center_gy = center_idx / GRID_W;
        let center_world = terrain::grid_to_world(center_gx, center_gy);

        for _ in 0..config.food_cluster_size {
            for _ in 0..10 {
                let angle = rng.gen::<f32>() * std::f32::consts::TAU;
                let radius = rng.gen::<f32>() * config.food_cluster_radius;
                let offset_world = Vec2::new(angle.cos() * radius, angle.sin() * radius);
                let candidate = center_world + offset_world;

                if let Some((gx, gy)) = terrain::world_to_grid(candidate) {
                    let cell_idx = terrain::idx(gx, gy);
                    if !world_map.walls[cell_idx] && !occupied[cell_idx] {
                        occupied[cell_idx] = true;
                        let pos = terrain::grid_to_world(gx, gy);
                        spawn_food(&mut commands, &food_assets, pos, config.food_per_source);
                        break;
                    }
                }
            }
        }
    }

    commands.insert_resource(NestPosition(nest_pos));
}

#[derive(Resource)]
pub struct NestPosition(pub Vec2);

/// Cached handles for ant mesh and material — populated once at startup.
#[derive(Resource)]
pub struct AntAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<ColorMaterial>,
}

fn spawn_single_ant(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    material: Handle<ColorMaterial>,
    pos: Vec2,
    lifetime_min: f32,
    lifetime_max: f32,
    rng: &mut impl rand::Rng,
) {
    let angle = rng.gen::<f32>() * std::f32::consts::TAU;
    let lifetime = rng.gen_range(lifetime_min..lifetime_max);
    commands.spawn((
        SimEntity,
        Ant {
            angle,
            state: AntState::Searching,
            age: 0.0,
            lifetime,
            stuck_frames: 0,
        },
        Mesh2d(mesh),
        MeshMaterial2d(material),
        Transform::from_translation(pos.extend(2.0))
            .with_rotation(Quat::from_rotation_z(angle)),
    ));
}

fn spawn_ants(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    nest_pos: Res<NestPosition>,
    config: Res<SimConfig>,
) {
    log("system: spawn_ants");
    let mut rng = rand::thread_rng();

    let ant_mesh = meshes.add(Triangle2d::new(
        Vec2::new(6.0, 0.0),
        Vec2::new(-4.0, 3.0),
        Vec2::new(-4.0, -3.0),
    ));
    let searching_material = materials.add(ColorMaterial::from(Color::srgb(0.6, 0.3, 0.1)));

    for _ in 0..config.ant_count {
        spawn_single_ant(
            &mut commands,
            ant_mesh.clone(),
            searching_material.clone(),
            nest_pos.0,
            config.ant_lifetime_min,
            config.ant_lifetime_max,
            &mut rng,
        );
    }

    commands.insert_resource(AntAssets {
        mesh: ant_mesh,
        material: searching_material,
    });
    commands.insert_resource(Colony::new(config.ant_count));
}

fn ant_respawn_system(
    mut commands: Commands,
    mut colony: ResMut<Colony>,
    nest_pos: Res<NestPosition>,
    ant_assets: Res<AntAssets>,
    config: Res<SimConfig>,
    time: Res<Time>,
) {
    colony.respawn_timer += time.delta_secs();
    if colony.respawn_timer < config.ant_respawn_interval {
        return;
    }
    colony.respawn_timer -= config.ant_respawn_interval;

    let to_spawn = config.ant_respawn_batch.min(colony.pending_respawn);
    if to_spawn == 0 {
        return;
    }

    let mut rng = rand::thread_rng();

    for _ in 0..to_spawn {
        spawn_single_ant(
            &mut commands,
            ant_assets.mesh.clone(),
            ant_assets.material.clone(),
            nest_pos.0,
            config.ant_lifetime_min,
            config.ant_lifetime_max,
            &mut rng,
        );
    }

    colony.pending_respawn -= to_spawn;
    colony.active += to_spawn;
}

fn setup_score_ui(mut commands: Commands) {
    commands.spawn((
        Text::new("Food: 0  Ants: 0 / 0"),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(295.0), // offset past the 280px sidebar
            ..default()
        },
        ScoreText,
    ));
}

#[derive(Component)]
struct ScoreText;

fn update_score_ui(
    score: Res<FoodScore>,
    colony: Res<Colony>,
    config: Res<SimConfig>,
    mut query: Query<&mut Text, With<ScoreText>>,
) {
    if score.is_changed() || colony.is_changed() {
        for mut text in query.iter_mut() {
            text.0 = format!(
                "Food: {}  Ants: {} / {}",
                score.collected, colony.active, config.ant_count
            );
        }
    }
}

#[derive(Component)]
struct FpsText;

fn setup_fps_ui(mut commands: Commands) {
    commands.spawn((
        Text::new("FPS: --"),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(10.0),
            ..default()
        },
        FpsText,
    ));
}

fn update_fps_ui(
    diagnostics: Res<DiagnosticsStore>,
    mut query: Query<&mut Text, With<FpsText>>,
) {
    if let Some(fps) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
    {
        for mut text in query.iter_mut() {
            text.0 = format!("FPS: {:.0}", fps);
        }
    }
}
