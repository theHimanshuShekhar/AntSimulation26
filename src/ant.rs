use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use crate::config::*;
use crate::pheromone::{PheromoneGrid, PheromoneKind};
use crate::terrain::WorldMap;
use crate::FoodPositions;

/// Ant state: either searching for food or returning to nest
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AntState {
    Searching,
    Returning,
}

/// Ant component representing a single ant's behavior
#[derive(Component)]
pub struct Ant {
    pub angle: f32,       // current direction in radians
    pub state: AntState,
    pub age: f32,         // seconds this ant has been alive
    pub lifetime: f32,    // seconds until death (randomized at spawn)
}

/// Staging buffer for ant pheromone deposits (parallel-safe writes).
/// Tuple: (grid_idx, kind, amount, trail_direction)
#[derive(Resource, Default)]
pub struct AntDeposits(pub Vec<(usize, PheromoneKind, f32, Vec2)>);

/// Colony-level population tracker. Written by ant_age_system, read by ant_respawn_system.
#[derive(Resource)]
pub struct Colony {
    pub active: usize,          // currently alive ants
    pub total_died: usize,      // cumulative deaths since startup
    pub pending_respawn: usize, // ants queued for respawning
    pub respawn_timer: f32,     // seconds since last respawn tick
}

impl Colony {
    pub fn new(initial_count: usize) -> Self {
        Self {
            active: initial_count,
            total_died: 0,
            pending_respawn: 0,
            respawn_timer: 0.0,
        }
    }
}

/// Score a single pheromone sensor position using both intensity and trail direction.
///
/// The score combines:
///   - base intensity (20% weight even with zero alignment)
///   - directional alignment: how well the stored trail direction points toward this sensor
///
/// This means an ant approaching a trail from the correct end scores high; approaching
/// from the wrong end scores near the baseline, preventing sharp 180° reversals while
/// still preferring correctly-oriented trail segments.
fn score_sensor(ant_pos: Vec2, sensor_pos: Vec2, kind: PheromoneKind, grid: &PheromoneGrid) -> f32 {
    let Some((gx, gy)) = crate::terrain::world_to_grid(sensor_pos) else {
        return 0.0;
    };
    let idx = crate::terrain::idx(gx, gy);
    let intensity = grid.sample(idx, kind);
    if intensity < PHEROMONE_ZERO_THRESHOLD {
        return 0.0;
    }
    let trail_dir = grid.sample_dir(idx, kind);
    if trail_dir.length_squared() < 1e-6 {
        // No direction recorded yet — fall back to raw intensity
        return intensity;
    }
    // Vector from ant toward this sensor position
    let to_sensor = (sensor_pos - ant_pos).normalize_or_zero();
    // alignment ∈ [-1, 1]: +1 = trail direction perfectly matches heading toward sensor
    let alignment = trail_dir.dot(to_sensor);
    // Map to [SENSOR_MIN_ALIGNMENT, 1.0] so even opposing trails contribute a floor of intensity
    intensity * (SENSOR_MIN_ALIGNMENT + (1.0 - SENSOR_MIN_ALIGNMENT) * ((alignment + 1.0) * 0.5))
}

/// Score pheromone sensors at three positions: (left, ahead, right)
fn score_sensors(pos: Vec2, angle: f32, kind: PheromoneKind, grid: &PheromoneGrid) -> (f32, f32, f32) {
    let sensor_pos = |a: f32| pos + Vec2::new(a.cos(), a.sin()) * SENSOR_DIST;
    (
        score_sensor(pos, sensor_pos(angle - SENSOR_ANGLE), kind, grid),
        score_sensor(pos, sensor_pos(angle),                kind, grid),
        score_sensor(pos, sensor_pos(angle + SENSOR_ANGLE), kind, grid),
    )
}

/// Returns true if the given world position is inside a wall or out of bounds
fn is_wall(pos: Vec2, world_map: &WorldMap) -> bool {
    match crate::terrain::world_to_grid(pos) {
        Some((gx, gy)) => world_map.walls[crate::terrain::idx(gx, gy)],
        None => true, // out of bounds counts as wall
    }
}

/// Handle wall and boundary collisions
fn handle_collision(old_pos: Vec2, new_pos: Vec2, angle: &mut f32, world_map: &WorldMap, rng: &mut impl Rng) -> Vec2 {
    let half_w = WINDOW_W / 2.0;
    let half_h = WINDOW_H / 2.0;

    let mut pos = new_pos;

    // World boundary bounce
    if pos.x < -half_w + ANT_BOUNDARY_MARGIN || pos.x > half_w - ANT_BOUNDARY_MARGIN {
        *angle = std::f32::consts::PI - *angle;
        pos.x = pos.x.clamp(-half_w + ANT_BOUNDARY_MARGIN, half_w - ANT_BOUNDARY_MARGIN);
    }
    if pos.y < -half_h + ANT_BOUNDARY_MARGIN || pos.y > half_h - ANT_BOUNDARY_MARGIN {
        *angle = -*angle;
        pos.y = pos.y.clamp(-half_h + ANT_BOUNDARY_MARGIN, half_h - ANT_BOUNDARY_MARGIN);
    }

    // Check ant footprint: center + 4 cardinal probes at collision radius.
    // Grid cells are 5px wide; ANT_COLLISION_RADIUS ensures the ant never visually
    // overlaps a wall cell even when its center is adjacent to one.
    let r = ANT_COLLISION_RADIUS;
    let hit = is_wall(pos, world_map)
        || is_wall(pos + Vec2::new( r,  0.0), world_map)
        || is_wall(pos + Vec2::new(-r,  0.0), world_map)
        || is_wall(pos + Vec2::new( 0.0,  r), world_map)
        || is_wall(pos + Vec2::new( 0.0, -r), world_map);

    if hit {
        use std::f32::consts::PI;
        let probe_dist = r * 3.0; // 3x radius — enough to clear one grid cell

        // Try the standard PI-reverse + noise first, then random fallbacks.
        // This ensures we never commit to an angle that immediately re-hits a wall,
        // which is what causes ants to appear frozen with no pheromone nearby.
        for attempt in 0..8u32 {
            let try_angle = if attempt == 0 {
                *angle + PI + (rng.gen::<f32>() - 0.5) * 1.0
            } else {
                rng.gen::<f32>() * 2.0 * PI
            };
            let probe = old_pos + Vec2::new(try_angle.cos(), try_angle.sin()) * probe_dist;
            let clear = !is_wall(probe, world_map)
                && !is_wall(probe + Vec2::new( r,  0.0), world_map)
                && !is_wall(probe + Vec2::new(-r,  0.0), world_map)
                && !is_wall(probe + Vec2::new( 0.0,  r), world_map)
                && !is_wall(probe + Vec2::new( 0.0, -r), world_map);
            if clear {
                *angle = try_angle;
                return old_pos;
            }
        }
        // Last resort: plain reverse (only in extremely tight corners)
        *angle += PI;
        return old_pos;
    }

    pos
}

/// Generate Gaussian-distributed random value using Box-Muller transform
fn gaussian_noise(rng: &mut impl Rng) -> f32 {
    use std::f32::consts::PI;
    let u1: f32 = rng.gen::<f32>().max(1e-10);
    let u2: f32 = rng.gen::<f32>();
    (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
}

/// Compute shortest signed angle difference (handles wrapping around 2π)
fn angle_diff(target: f32, current: f32) -> f32 {
    use std::f32::consts::PI;
    let mut diff = target - current;
    while diff > PI {
        diff -= 2.0 * PI;
    }
    while diff < -PI {
        diff += 2.0 * PI;
    }
    diff
}

/// Core ant behavior system: sensors, steering, movement, pheromone deposits
pub fn ant_behavior_system(
    mut ant_query: Query<(&mut Transform, &mut Ant)>,
    pheromone_grid: Res<PheromoneGrid>,
    world_map: Res<WorldMap>,
    mut deposits: ResMut<AntDeposits>,
    time: Res<Time>,
    food_positions: Res<FoodPositions>,
    mut rng: Local<Option<rand::rngs::SmallRng>>,
) {
    // Initialize RNG on first call
    let rng = rng.get_or_insert_with(|| rand::rngs::SmallRng::from_entropy());

    let dt = time.delta_secs();
    deposits.0.clear();

    for (mut transform, mut ant) in ant_query.iter_mut() {
        let pos = transform.translation.truncate(); // Vec2

        // 1. Determine which pheromone to follow based on state
        let follow_kind = match ant.state {
            AntState::Searching => PheromoneKind::Food,
            AntState::Returning => PheromoneKind::Home,
        };

        // 2. Score 3 sensors — combines intensity with directional alignment
        let (left, ahead, right) = score_sensors(pos, ant.angle, follow_kind, &pheromone_grid);

        // 3. Wander force: Gaussian angle noise, attenuated when pheromone is strong
        let total_signal = left + ahead + right;
        let wander_w = 1.0 - (total_signal * PHEROMONE_FOLLOW_WEIGHT).min(PHEROMONE_FOLLOW_MAX);
        let wander = wander_w * WANDER_WEIGHT * gaussian_noise(rng) * ANT_TURN_NOISE;

        // 4. Pheromone force: continuous angle delta toward strongest sensor
        let phero_delta: f32 = if ahead >= left && ahead >= right {
            0.0
        } else if left > right {
            -SENSOR_ANGLE
        } else {
            SENSOR_ANGLE
        };
        let phero = total_signal.min(1.0) * PHEROMONE_WEIGHT * phero_delta;

        // 5. Seek force: steer toward nearest food (Searching) or nest (Returning, no signal)
        let seek: f32 = match ant.state {
            AntState::Searching => {
                let seek_radius_sq = SEEK_RADIUS * SEEK_RADIUS;
                let nearest = food_positions.0.iter()
                    .filter(|&&fp| fp.distance_squared(pos) < seek_radius_sq)
                    .min_by(|&&a, &&b| {
                        a.distance_squared(pos)
                            .partial_cmp(&b.distance_squared(pos))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                if let Some(&food_pos) = nearest {
                    let target = (food_pos - pos).y.atan2((food_pos - pos).x);
                    angle_diff(target, ant.angle) * SEEK_WEIGHT
                } else {
                    0.0
                }
            }
            AntState::Returning => {
                if total_signal < 0.01 {
                    let to_nest = Vec2::ZERO - pos;
                    if to_nest.length() > 1.0 {
                        angle_diff(to_nest.y.atan2(to_nest.x), ant.angle) * SEEK_WEIGHT
                    } else {
                        0.0
                    }
                } else {
                    0.0
                }
            }
        };

        // 6. Combine forces + small base noise to prevent perfectly straight paths
        let base_noise = gaussian_noise(rng) * ANT_TURN_NOISE * BASE_NOISE_FRACTION;
        ant.angle += wander + phero + seek + base_noise;

        // 7. Move forward
        let dx = ant.angle.cos() * ANT_SPEED * dt;
        let dy = ant.angle.sin() * ANT_SPEED * dt;
        let new_pos = pos + Vec2::new(dx, dy);

        // 8. Wall + boundary collision
        let new_pos = handle_collision(pos, new_pos, &mut ant.angle, &world_map, rng);

        transform.translation = new_pos.extend(2.0); // z=2: above terrain and pheromone overlay
        transform.rotation = Quat::from_rotation_z(ant.angle);

        // 9. Deposit pheromone at current position.
        //    Trail direction = reverse of ant's movement so followers travel back along the path.
        //    e.g. ant moving east → deposit points west (back toward where it came from).
        if let Some((gx, gy)) = crate::terrain::world_to_grid(new_pos) {
            let grid_idx = crate::terrain::idx(gx, gy);
            if !world_map.walls[grid_idx] {
                let deposit_kind = match ant.state {
                    AntState::Searching => PheromoneKind::Home,
                    AntState::Returning => PheromoneKind::Food,
                };
                // Reverse of movement direction = direction back along the trail
                let trail_dir = Vec2::new(-ant.angle.cos(), -ant.angle.sin());
                deposits.0.push((grid_idx, deposit_kind, DEPOSIT_STRENGTH, trail_dir));
            }
        }
    }
}

/// Flush all collected ant pheromone deposits to the pheromone grid
pub fn ant_deposit_flush_system(
    mut grid: ResMut<PheromoneGrid>,
    deposits: Res<AntDeposits>,
) {
    for &(idx, kind, amount, dir) in &deposits.0 {
        grid.deposit(idx, kind, amount, dir);
    }
}

/// Age all ants each frame; despawn those that exceed their lifetime.
pub fn ant_age_system(
    mut commands: Commands,
    mut ant_query: Query<(Entity, &mut Ant)>,
    mut colony: ResMut<Colony>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (entity, mut ant) in ant_query.iter_mut() {
        ant.age += dt;
        if ant.age >= ant.lifetime {
            commands.entity(entity).despawn();
            colony.active = colony.active.saturating_sub(1);
            colony.pending_respawn += 1;
            colony.total_died += 1;
        }
    }
}

/// Plugin that registers ant systems
pub struct AntPlugin;

impl Plugin for AntPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AntDeposits::default())
            .add_systems(
                Update,
                (
                    ant_age_system,
                    ant_behavior_system.after(ant_age_system),
                    ant_deposit_flush_system.after(ant_behavior_system),
                ),
            );
    }
}
