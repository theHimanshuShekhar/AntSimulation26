mod config;
mod world;
mod pheromone;
mod ant;
mod food;
mod render;

use bevy::prelude::*;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use rand::Rng;
use config::*;
use ant::{Ant, AntState};
use food::{spawn_food, spawn_nest, FoodScore};
use world::WorldMap;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Ant Simulation".into(),
                resolution: (WINDOW_W, WINDOW_H).into(),
                resizable: false,
                ..default()
            }),
            ..default()
        }))
        .add_plugins((
            world::WorldPlugin,
            pheromone::PheromonePlugin,
            ant::AntPlugin,
            food::FoodPlugin,
            render::RenderPlugin,
            FrameTimeDiagnosticsPlugin,
        ))
        .add_systems(
            Startup,
            (
                spawn_camera,
                spawn_nest_and_food.after(world::cave_startup_system),
                spawn_ants.after(spawn_nest_and_food),
                setup_score_ui,
                setup_fps_ui,
            ),
        )
        .add_systems(Update, (update_score_ui, update_fps_ui))
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d::default());
}

fn spawn_nest_and_food(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    world_map: Res<WorldMap>,
) {
    let mut rng = rand::thread_rng();

    // Pick a random open cell for the nest
    let nest_pos = if !world_map.open_cells.is_empty() {
        let idx = world_map.open_cells[rng.gen_range(0..world_map.open_cells.len())];
        let gx = idx % GRID_W;
        let gy = idx / GRID_W;
        world::grid_to_world(gx, gy)
    } else {
        Vec2::ZERO
    };

    spawn_nest(&mut commands, &mut meshes, &mut materials, nest_pos);

    // Spawn initial food sources at random open cells
    for _ in 0..FOOD_SOURCE_COUNT {
        if world_map.open_cells.is_empty() {
            break;
        }
        let idx = world_map.open_cells[rng.gen_range(0..world_map.open_cells.len())];
        let gx = idx % GRID_W;
        let gy = idx / GRID_W;
        let pos = world::grid_to_world(gx, gy);
        spawn_food(&mut commands, &mut meshes, &mut materials, pos);
    }

    // Store nest position as a resource so ants can find it
    commands.insert_resource(NestPosition(nest_pos));
}

#[derive(Resource)]
pub struct NestPosition(pub Vec2);

fn spawn_ants(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    nest_pos: Res<NestPosition>,
) {
    let mut rng = rand::thread_rng();

    // Ant mesh: small triangle (elongated)
    let ant_mesh = meshes.add(Triangle2d::new(
        Vec2::new(6.0, 0.0),   // tip (forward)
        Vec2::new(-4.0, 3.0),  // rear left
        Vec2::new(-4.0, -3.0), // rear right
    ));

    let searching_material = materials.add(ColorMaterial::from(Color::srgb(0.6, 0.3, 0.1)));

    for _ in 0..ANT_COUNT {
        let angle = rng.gen::<f32>() * std::f32::consts::TAU;
        let pos = nest_pos.0;

        commands.spawn((
            Ant {
                angle,
                state: AntState::Searching,
            },
            Mesh2d(ant_mesh.clone()),
            MeshMaterial2d(searching_material.clone()),
            Transform::from_translation(pos.extend(1.0))
                .with_rotation(Quat::from_rotation_z(angle)),
        ));
    }
}

fn setup_score_ui(mut commands: Commands) {
    commands.spawn((
        Text::new("Food collected: 0"),
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
    mut query: Query<&mut Text, With<ScoreText>>,
) {
    if score.is_changed() {
        for mut text in query.iter_mut() {
            *text = Text::new(format!("Food collected: {}", score.collected));
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
