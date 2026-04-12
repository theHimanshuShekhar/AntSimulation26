use bevy::prelude::*;
use rand::Rng;
use crate::config::*;
use crate::ant::{Ant, AntState};
use crate::terrain::WorldMap;

/// Food component: represents a food source with remaining units
#[derive(Component)]
pub struct Food {
    pub units: u32,
}

/// Marker component for the nest location
#[derive(Component)]
pub struct Nest;

/// Timer for respawning food sources after they've been depleted
#[derive(Component)]
pub struct FoodRespawnTimer {
    pub timer: Timer,
    pub slot: usize, // which food slot this is (0..FOOD_SOURCE_COUNT)
}

/// Resource tracking total food collected and brought to nest
#[derive(Resource, Default)]
pub struct FoodScore {
    pub collected: u32,
}

/// Ant-Food interaction: when a Searching ant is within FOOD_INTERACTION_RADIUS of a Food entity,
/// the ant picks up 1 unit of food and switches to Returning state. If the food source is depleted,
/// it despawns and a respawn timer is spawned.
pub fn food_interaction_system(
    mut commands: Commands,
    mut food_query: Query<(Entity, &mut Food, &Transform)>,
    mut ant_query: Query<(&Transform, &mut Ant)>,
) {
    for (ant_transform, mut ant) in ant_query.iter_mut() {
        // Only Searching ants interact with food
        if ant.state != AntState::Searching {
            continue;
        }

        let ant_pos = ant_transform.translation.truncate();

        for (food_entity, mut food, food_transform) in food_query.iter_mut() {
            let food_pos = food_transform.translation.truncate();

            // Check if ant is within interaction radius
            if ant_pos.distance(food_pos) < FOOD_INTERACTION_RADIUS {
                // Ant picks up 1 unit of food
                food.units = food.units.saturating_sub(1);

                // Ant switches to Returning state
                ant.state = AntState::Returning;

                // If food source is depleted, despawn it and spawn respawn timer
                if food.units == 0 {
                    commands.entity(food_entity).despawn();
                    commands.spawn(FoodRespawnTimer {
                        timer: Timer::from_seconds(FOOD_RESPAWN_DELAY, TimerMode::Once),
                        slot: 0, // slot index doesn't matter for single respawn
                    });
                }

                // Only one food pickup per ant per frame
                break;
            }
        }
    }
}

/// Ant-Nest interaction: when a Returning ant is within NEST_INTERACTION_RADIUS of the Nest,
/// the ant switches to Searching state and the FoodScore is incremented.
pub fn nest_interaction_system(
    nest_query: Query<&Transform, With<Nest>>,
    mut ant_query: Query<(&Transform, &mut Ant)>,
    mut score: ResMut<FoodScore>,
) {
    // Get the nest position; if no nest, return early
    let Ok(nest_transform) = nest_query.get_single() else {
        return;
    };
    let nest_pos = nest_transform.translation.truncate();

    for (ant_transform, mut ant) in ant_query.iter_mut() {
        // Only Returning ants interact with nest
        if ant.state != AntState::Returning {
            continue;
        }

        let ant_pos = ant_transform.translation.truncate();

        // Check if ant is within nest interaction radius
        if ant_pos.distance(nest_pos) < NEST_INTERACTION_RADIUS {
            // Ant switches to Searching state
            ant.state = AntState::Searching;

            // Increment food score
            score.collected += 1;
        }
    }
}

/// Food respawn system: when a FoodRespawnTimer expires, it despawns itself and spawns a new
/// Food entity at a random open location on the map.
pub fn food_respawn_system(
    mut commands: Commands,
    mut timer_query: Query<(Entity, &mut FoodRespawnTimer)>,
    world_map: Res<WorldMap>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    time: Res<Time>,
) {
    let mut rng = rand::thread_rng();

    for (entity, mut respawn) in timer_query.iter_mut() {
        respawn.timer.tick(time.delta());

        if respawn.timer.finished() {
            // Despawn the timer entity
            commands.entity(entity).despawn();

            // Pick a random open cell for the new food source
            if world_map.open_cells.is_empty() {
                continue;
            }

            let cell_idx = world_map.open_cells[rng.gen_range(0..world_map.open_cells.len())];
            let gx = cell_idx % GRID_W;
            let gy = cell_idx / GRID_W;
            let pos = crate::terrain::grid_to_world(gx, gy);

            spawn_food(&mut commands, &mut meshes, &mut materials, pos);
        }
    }
}

/// Helper function to spawn a Food entity at a given position
pub fn spawn_food(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    pos: Vec2,
) {
    commands.spawn((
        Food {
            units: FOOD_PER_SOURCE,
        },
        Mesh2d(meshes.add(Circle::new(8.0))),
        MeshMaterial2d(materials.add(ColorMaterial::from(Color::srgb(0.1, 0.9, 0.1)))),
        Transform::from_translation(pos.extend(3.0)), // z=3: above terrain, pheromone, ants
    ));
}

/// Helper function to spawn the Nest entity at a given position
pub fn spawn_nest(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    pos: Vec2,
) {
    commands.spawn((
        Nest,
        Mesh2d(meshes.add(Circle::new(18.0))),
        MeshMaterial2d(materials.add(ColorMaterial::from(Color::srgb(0.55, 0.27, 0.07)))),
        Transform::from_translation(pos.extend(3.0)), // z=3: above terrain, pheromone, ants
    ));
}

/// Plugin that registers all food-related systems and resources
pub struct FoodPlugin;

impl Plugin for FoodPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(FoodScore::default()).add_systems(
            Update,
            (
                food_interaction_system,
                nest_interaction_system,
                food_respawn_system,
            ),
        );
    }
}
