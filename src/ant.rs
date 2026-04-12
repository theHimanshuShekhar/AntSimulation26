use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use crate::config::*;
use crate::pheromone::{PheromoneGrid, PheromoneKind};
use crate::terrain::WorldMap;

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
}

/// Staging buffer for ant pheromone deposits (parallel-safe writes).
/// Tuple: (grid_idx, kind, amount, trail_direction)
#[derive(Resource, Default)]
pub struct AntDeposits(pub Vec<(usize, PheromoneKind, f32, Vec2)>);

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
    if intensity < 0.001 {
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
    // Map to [0.2, 1.0] so even opposing trails contribute 20% of intensity
    intensity * (0.2 + 0.8 * ((alignment + 1.0) * 0.5))
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
fn handle_collision(old_pos: Vec2, new_pos: Vec2, angle: &mut f32, world_map: &WorldMap) -> Vec2 {
    let half_w = WINDOW_W / 2.0;
    let half_h = WINDOW_H / 2.0;

    let mut pos = new_pos;

    // World boundary bounce
    if pos.x < -half_w + 5.0 || pos.x > half_w - 5.0 {
        *angle = std::f32::consts::PI - *angle;
        pos.x = pos.x.clamp(-half_w + 5.0, half_w - 5.0);
    }
    if pos.y < -half_h + 5.0 || pos.y > half_h - 5.0 {
        *angle = -*angle;
        pos.y = pos.y.clamp(-half_h + 5.0, half_h - 5.0);
    }

    // Check ant footprint: center + 4 cardinal probes at collision radius.
    // Grid cells are 5px wide; a 4px radius ensures the ant never visually
    // overlaps a wall cell even when its center is adjacent to one.
    let r = 4.0_f32;
    let hit = is_wall(pos, world_map)
        || is_wall(pos + Vec2::new( r,  0.0), world_map)
        || is_wall(pos + Vec2::new(-r,  0.0), world_map)
        || is_wall(pos + Vec2::new( 0.0,  r), world_map)
        || is_wall(pos + Vec2::new( 0.0, -r), world_map);

    if hit {
        // Reverse direction with some randomness so the ant escapes the wall
        *angle += std::f32::consts::PI + (rand::random::<f32>() - 0.5) * 1.0;
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

        // 3. Weighted follow: probability of following pheromone scales with total signal strength.
        //    When signal is absent, fall back to a pure random walk.
        let total_signal = left + ahead + right;
        let follow_prob = (total_signal * PHEROMONE_FOLLOW_WEIGHT).min(1.0);
        let follow_pheromone = rng.gen::<f32>() < follow_prob;

        // 4. Steer based on whether ant chose to follow pheromone or random walk
        let turn = if follow_pheromone {
            // Steer toward the strongest sensor direction
            if ahead >= left && ahead >= right {
                0.0 // go straight
            } else if left > right {
                -SENSOR_ANGLE * 0.5 // turn left
            } else {
                SENSOR_ANGLE * 0.5 // turn right
            }
        } else {
            // No signal (or chance miss): pure random walk
            gaussian_noise(rng) * ANT_TURN_NOISE
        };

        // 5. Small base noise always applied to prevent perfectly straight paths
        let base_noise = gaussian_noise(rng) * ANT_TURN_NOISE * 0.15;
        ant.angle += turn + base_noise;

        // 6. If returning and no home signal, add slight bias toward nest (0,0)
        if ant.state == AntState::Returning && left < 0.01 && ahead < 0.01 && right < 0.01 {
            let to_nest = Vec2::new(0.0, 0.0) - pos;
            if to_nest.length() > 1.0 {
                let target_angle = to_nest.y.atan2(to_nest.x);
                let angle_diff_val = angle_diff(target_angle, ant.angle);
                ant.angle += angle_diff_val * 0.05; // gentle pull
            }
        }

        // 7. Move forward
        let dx = ant.angle.cos() * ANT_SPEED * dt;
        let dy = ant.angle.sin() * ANT_SPEED * dt;
        let new_pos = pos + Vec2::new(dx, dy);

        // 8. Wall + boundary collision
        let new_pos = handle_collision(pos, new_pos, &mut ant.angle, &world_map);

        transform.translation = new_pos.extend(1.0); // z=1 so ants render above texture
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

/// Plugin that registers ant systems
pub struct AntPlugin;

impl Plugin for AntPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AntDeposits::default())
            .add_systems(
                Update,
                (
                    ant_behavior_system,
                    ant_deposit_flush_system.after(ant_behavior_system),
                ),
            );
    }
}
