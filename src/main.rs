mod config;
mod noise;
mod terrain;
mod pheromone;
mod ant;
mod food;
mod render;

use bevy::prelude::*;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use rand::Rng;
use std::collections::HashSet;
use config::*;
use ant::{Ant, AntState, Colony};
use food::{spawn_food, spawn_nest, FoodAssets, FoodScore};
use terrain::WorldMap;

fn main() {
    build_and_run();
}

fn build_and_run() {
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

    App::new()
        .add_plugins(default_plugins)
        .add_plugins((
            terrain::WorldPlugin,
            pheromone::PheromonePlugin,
            ant::AntPlugin,
            food::FoodPlugin,
            render::RenderPlugin,
            FrameTimeDiagnosticsPlugin,
        ))
        .insert_resource(FoodPositions::default())
        .add_systems(PreUpdate, update_food_positions)
        .add_systems(
            Startup,
            (
                spawn_camera,
                spawn_nest_and_food
                    .after(terrain::terrain_startup_system)
                    .after(food::setup_food_assets),
                spawn_ants.after(spawn_nest_and_food),
                setup_score_ui,
                setup_fps_ui,
            ),
        )
        .add_systems(Update, (update_score_ui, update_fps_ui, ant_respawn_system))
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d::default());
}

/// Cached food positions updated each PreUpdate tick for ant seek-force queries.
#[derive(Resource, Default)]
pub struct FoodPositions(pub Vec<Vec2>);

fn update_food_positions(
    mut positions: ResMut<FoodPositions>,
    food_query: Query<&Transform, With<food::Food>>,
) {
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
) {
    let mut rng = rand::thread_rng();

    // Nest always spawns at world center — the center exclusion zone in terrain generation
    // guarantees this area is always wall-free.
    let nest_pos = Vec2::ZERO;
    let nest_grid_x = GRID_W / 2;
    let nest_grid_y = GRID_H / 2;

    spawn_nest(&mut commands, &mut meshes, &mut materials, nest_pos);

    // Spawn initial food sources at open cells that are far enough from the nest
    let food_candidates: Vec<usize> = world_map
        .open_cells
        .iter()
        .copied()
        .filter(|&cell_idx| {
            let gx = cell_idx % GRID_W;
            let gy = cell_idx / GRID_W;
            let dx = gx as i32 - nest_grid_x as i32;
            let dy = gy as i32 - nest_grid_y as i32;
            let dist = ((dx * dx + dy * dy) as f32).sqrt() as usize;
            dist >= FOOD_MIN_NEST_DIST_CELLS
        })
        .collect();

    let source = if food_candidates.is_empty() { &world_map.open_cells } else { &food_candidates };

    let mut occupied: HashSet<usize> = HashSet::new();

    for _ in 0..FOOD_SOURCE_COUNT {
        if source.is_empty() {
            break;
        }
        // Pick a cluster center from valid (far-from-nest) open cells
        let center_idx = source[rng.gen_range(0..source.len())];
        let center_gx = center_idx % GRID_W;
        let center_gy = center_idx / GRID_W;
        let center_world = terrain::grid_to_world(center_gx, center_gy);

        // Scatter FOOD_CLUSTER_SIZE items around the center
        for _ in 0..FOOD_CLUSTER_SIZE {
            let mut placed = false;
            for _ in 0..10 {
                let angle = rng.gen::<f32>() * std::f32::consts::TAU;
                let radius = rng.gen::<f32>() * FOOD_CLUSTER_RADIUS;
                let offset_world = Vec2::new(angle.cos() * radius, angle.sin() * radius);
                let candidate = center_world + offset_world;

                if let Some((gx, gy)) = terrain::world_to_grid(candidate) {
                    let cell_idx = terrain::idx(gx, gy);
                    if !world_map.walls[cell_idx] && !occupied.contains(&cell_idx) {
                        occupied.insert(cell_idx);
                        let pos = terrain::grid_to_world(gx, gy);
                        spawn_food(&mut commands, &food_assets, pos);
                        placed = true;
                        break;
                    }
                }
            }
            let _ = placed;
        }
    }

    // Store nest position as a resource so ants can find it
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

fn spawn_ants(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    nest_pos: Res<NestPosition>,
) {
    let mut rng = rand::thread_rng();

    let ant_mesh = meshes.add(Triangle2d::new(
        Vec2::new(6.0, 0.0),   // tip (forward)
        Vec2::new(-4.0, 3.0),  // rear left
        Vec2::new(-4.0, -3.0), // rear right
    ));
    let searching_material = materials.add(ColorMaterial::from(Color::srgb(0.6, 0.3, 0.1)));

    for _ in 0..ANT_COUNT {
        let angle = rng.gen::<f32>() * std::f32::consts::TAU;
        let lifetime = rng.gen_range(ANT_LIFETIME_MIN..ANT_LIFETIME_MAX);
        commands.spawn((
            Ant {
                angle,
                state: AntState::Searching,
                age: 0.0,
                lifetime,
            },
            Mesh2d(ant_mesh.clone()),
            MeshMaterial2d(searching_material.clone()),
            Transform::from_translation(nest_pos.0.extend(2.0))
                .with_rotation(Quat::from_rotation_z(angle)),
        ));
    }

    commands.insert_resource(AntAssets {
        mesh: ant_mesh,
        material: searching_material,
    });
    commands.insert_resource(Colony::new(ANT_COUNT));
}

fn ant_respawn_system(
    mut commands: Commands,
    mut colony: ResMut<Colony>,
    nest_pos: Res<NestPosition>,
    ant_assets: Res<AntAssets>,
    time: Res<Time>,
) {
    colony.respawn_timer += time.delta_secs();
    if colony.respawn_timer < ANT_RESPAWN_INTERVAL {
        return;
    }
    colony.respawn_timer -= ANT_RESPAWN_INTERVAL;

    let to_spawn = ANT_RESPAWN_BATCH.min(colony.pending_respawn);
    if to_spawn == 0 {
        return;
    }

    let mut rng = rand::thread_rng();

    for _ in 0..to_spawn {
        let angle = rng.gen::<f32>() * std::f32::consts::TAU;
        let lifetime = rng.gen_range(ANT_LIFETIME_MIN..ANT_LIFETIME_MAX);
        commands.spawn((
            Ant {
                angle,
                state: AntState::Searching,
                age: 0.0,
                lifetime,
            },
            Mesh2d(ant_assets.mesh.clone()),
            MeshMaterial2d(ant_assets.material.clone()),
            Transform::from_translation(nest_pos.0.extend(2.0))
                .with_rotation(Quat::from_rotation_z(angle)),
        ));
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
            left: Val::Px(10.0),
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
    mut query: Query<&mut Text, With<ScoreText>>,
) {
    if score.is_changed() || colony.is_changed() {
        for mut text in query.iter_mut() {
            *text = Text::new(format!(
                "Food: {}  Ants: {} / {}",
                score.collected, colony.active, ANT_COUNT
            ));
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
            *text = Text::new(format!("FPS: {:.0}", fps));
        }
    }
}
