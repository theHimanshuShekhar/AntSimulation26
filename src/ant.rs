use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use crate::config::*;
use crate::pheromone::{PheromoneGrid, PheromoneKind};
use crate::world::WorldMap;

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

/// Staging buffer for ant pheromone deposits (parallel-safe writes)
#[derive(Resource, Default)]
pub struct AntDeposits(pub Vec<(usize, PheromoneKind, f32)>);

/// Sample pheromone sensors at three positions: left, ahead, right
fn sample_sensors(
    pos: Vec2,
    angle: f32,
    kind: PheromoneKind,
    grid: &PheromoneGrid,
) -> (f32, f32, f32) {
    let left_angle = angle - SENSOR_ANGLE;
    let ahead_angle = angle;
    let right_angle = angle + SENSOR_ANGLE;

    let sample_at = |a: f32| -> f32 {
        let sensor_pos = pos + Vec2::new(a.cos(), a.sin()) * SENSOR_DIST;
        if let Some((gx, gy)) = crate::world::world_to_grid(sensor_pos) {
            grid.sample(crate::world::idx(gx, gy), kind)
        } else {
            0.0
        }
    };

    (sample_at(left_angle), sample_at(ahead_angle), sample_at(right_angle))
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

    // Wall collision: if new position is in a wall, stay at old position and bounce
    if let Some((gx, gy)) = crate::world::world_to_grid(pos) {
        let wall_idx = crate::world::idx(gx, gy);
        if world_map.walls[wall_idx] {
            // Bounce: flip angle and return old position
            *angle += std::f32::consts::PI + (rand::random::<f32>() - 0.5) * 0.5;
            return old_pos;
        }
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

        // 2. Sample 3 sensors ahead
        let (left, ahead, right) = sample_sensors(pos, ant.angle, follow_kind, &pheromone_grid);

        // 3. Add noise to sensor readings (~10% random weight)
        let noise_l = rng.gen::<f32>() * 0.1;
        let noise_a = rng.gen::<f32>() * 0.1;
        let noise_r = rng.gen::<f32>() * 0.1;
        let left = left + noise_l;
        let ahead = ahead + noise_a;
        let right = right + noise_r;

        // 4. Steer based on sensors
        let turn = if ahead > left && ahead > right {
            0.0 // go straight
        } else if left > right {
            -SENSOR_ANGLE * 0.5 // turn left
        } else if right > left {
            SENSOR_ANGLE * 0.5 // turn right
        } else {
            // Equal or no signal: random walk
            let gaussian_noise = gaussian_noise(rng) * ANT_TURN_NOISE;
            gaussian_noise
        };

        // 5. Apply gaussian angle noise always (even when following trail)
        let base_noise = gaussian_noise(rng) * ANT_TURN_NOISE * 0.3;
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

        // 9. Deposit pheromone at current position
        if let Some((gx, gy)) = crate::world::world_to_grid(new_pos) {
            let grid_idx = crate::world::idx(gx, gy);
            if !world_map.walls[grid_idx] {
                let deposit_kind = match ant.state {
                    AntState::Searching => PheromoneKind::Home,
                    AntState::Returning => PheromoneKind::Food,
                };
                deposits.0.push((grid_idx, deposit_kind, DEPOSIT_STRENGTH));
            }
        }
    }
}

/// Flush all collected ant pheromone deposits to the pheromone grid
pub fn ant_deposit_flush_system(
    mut grid: ResMut<PheromoneGrid>,
    deposits: Res<AntDeposits>,
) {
    for &(idx, kind, amount) in &deposits.0 {
        grid.deposit(idx, kind, amount);
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
