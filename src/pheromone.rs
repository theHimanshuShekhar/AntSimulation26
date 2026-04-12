use bevy::prelude::*;
use crate::config::*;
use crate::terrain::WorldMap;

/// Two types of pheromones: home (to nest) and food (to food sources)
#[derive(Clone, Copy, Debug)]
pub enum PheromoneKind {
    Home,
    Food,
}

/// Pheromone grid resource holding home and food pheromone channels.
/// Each cell stores intensity plus an accumulated direction vector (dir_x, dir_y).
/// The direction is the sum of (deposit_amount * trail_direction) for all deposits,
/// so normalizing it gives the dominant travel direction at that cell.
#[derive(Resource)]
pub struct PheromoneGrid {
    pub home: Vec<f32>,       // intensity, GRID_W * GRID_H, values 0.0–1.0
    pub home_dir_x: Vec<f32>, // accumulated direction x component
    pub home_dir_y: Vec<f32>,
    pub food: Vec<f32>,
    pub food_dir_x: Vec<f32>,
    pub food_dir_y: Vec<f32>,
    pub dirty: bool,
    // Pre-allocated diffusion scratch buffers — avoids per-tick Vec allocation
    scratch_home: Vec<f32>,
    scratch_food: Vec<f32>,
}

impl PheromoneGrid {
    pub fn new() -> Self {
        let n = GRID_W * GRID_H;
        Self {
            home: vec![0.0; n],
            home_dir_x: vec![0.0; n],
            home_dir_y: vec![0.0; n],
            food: vec![0.0; n],
            food_dir_x: vec![0.0; n],
            food_dir_y: vec![0.0; n],
            dirty: false,
            scratch_home: vec![0.0; n],
            scratch_food: vec![0.0; n],
        }
    }

    /// Deposit amount + trail direction into one channel at grid index.
    /// `dir` is the direction other ants should travel here (reverse of depositor's motion).
    pub fn deposit(&mut self, idx: usize, kind: PheromoneKind, amount: f32, dir: Vec2) {
        let (intensity, dx, dy) = match kind {
            PheromoneKind::Home => (
                &mut self.home,
                &mut self.home_dir_x,
                &mut self.home_dir_y,
            ),
            PheromoneKind::Food => (
                &mut self.food,
                &mut self.food_dir_x,
                &mut self.food_dir_y,
            ),
        };
        if idx >= intensity.len() {
            return;
        }
        intensity[idx] = (intensity[idx] + amount).min(1.0);
        // Accumulate direction weighted by deposit amount so busier trails dominate
        dx[idx] += dir.x * amount;
        dy[idx] += dir.y * amount;
        self.dirty = true;
    }

    /// Sample intensity at index, returns 0.0 if out of bounds
    pub fn sample(&self, idx: usize, kind: PheromoneKind) -> f32 {
        let grid = match kind {
            PheromoneKind::Home => &self.home,
            PheromoneKind::Food => &self.food,
        };
        grid.get(idx).copied().unwrap_or(0.0)
    }

    /// Sample normalized trail direction at index; returns Vec2::ZERO if no direction stored
    pub fn sample_dir(&self, idx: usize, kind: PheromoneKind) -> Vec2 {
        let (dx_vec, dy_vec) = match kind {
            PheromoneKind::Home => (&self.home_dir_x, &self.home_dir_y),
            PheromoneKind::Food => (&self.food_dir_x, &self.food_dir_y),
        };
        if idx >= dx_vec.len() {
            return Vec2::ZERO;
        }
        Vec2::new(dx_vec[idx], dy_vec[idx]).normalize_or_zero()
    }
}

impl Default for PheromoneGrid {
    fn default() -> Self {
        Self::new()
    }
}

/// Overlay texture and visibility control for pheromone visualization
#[derive(Resource)]
pub struct PheromoneOverlay {
    pub texture: Handle<Image>,
    pub visible: bool,
}

/// Decay and diffuse pheromones at regular intervals
pub fn pheromone_decay_system(
    mut grid: ResMut<PheromoneGrid>,
    world_map: Res<WorldMap>,
    time: Res<Time>,
    mut timer: Local<f32>,
) {
    *timer += time.delta_secs();
    if *timer < DECAY_INTERVAL {
        return;
    }
    *timer -= DECAY_INTERVAL;

    // Decay all cells (intensity and direction components together)
    for i in 0..GRID_W * GRID_H {
        grid.home[i] *= DECAY_FACTOR;
        grid.home_dir_x[i] *= DECAY_FACTOR;
        grid.home_dir_y[i] *= DECAY_FACTOR;
        if grid.home[i] < 0.001 {
            grid.home[i] = 0.0;
            grid.home_dir_x[i] = 0.0;
            grid.home_dir_y[i] = 0.0;
        }

        grid.food[i] *= DECAY_FACTOR;
        grid.food_dir_x[i] *= DECAY_FACTOR;
        grid.food_dir_y[i] *= DECAY_FACTOR;
        if grid.food[i] < 0.001 {
            grid.food[i] = 0.0;
            grid.food_dir_x[i] = 0.0;
            grid.food_dir_y[i] = 0.0;
        }
    }

    // Optional diffusion: simple 1-step box blur (skips wall cells)
    if DIFFUSION_ENABLED {
        // Copy current values into scratch buffers (no alloc — buffers pre-allocated in PheromoneGrid::new).
        // Use indexed loop so the borrow checker can see scratch and live fields are disjoint.
        for i in 0..GRID_W * GRID_H {
            grid.scratch_home[i] = grid.home[i];
            grid.scratch_food[i] = grid.food[i];
        }
        for y in 1..GRID_H - 1 {
            for x in 1..GRID_W - 1 {
                let i = y * GRID_W + x;
                if world_map.walls[i] {
                    continue;
                }
                let neighbors = [
                    (y - 1) * GRID_W + x,
                    (y + 1) * GRID_W + x,
                    y * GRID_W + (x - 1),
                    y * GRID_W + (x + 1),
                ];
                let sum_h: f32 = neighbors.iter().map(|&n| grid.scratch_home[n]).sum();
                let sum_f: f32 = neighbors.iter().map(|&n| grid.scratch_food[n]).sum();
                // Weighted average: 60% current, 40% neighbors average
                grid.home[i] = (grid.home[i] * 0.6 + (sum_h / 4.0) * 0.4).min(1.0);
                grid.food[i] = (grid.food[i] * 0.6 + (sum_f / 4.0) * 0.4).min(1.0);
            }
        }
    }

    grid.dirty = true;
}

/// Update pheromone texture based on grid state and visibility
pub fn pheromone_texture_update_system(
    mut grid: ResMut<PheromoneGrid>,
    overlay: Option<Res<PheromoneOverlay>>,
    world_map: Option<Res<WorldMap>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(overlay) = overlay else { return };
    let Some(world_map) = world_map else { return };

    if !grid.dirty {
        return;
    }
    grid.dirty = false;

    let Some(image) = images.get_mut(&overlay.texture) else {
        return;
    };

    let pixels = &mut image.data;
    // Bevy/wgpu texture row 0 is at the TOP of the screen (world y = +WINDOW_H/2),
    // but grid gy=0 maps to world y = -WINDOW_H/2 (bottom). Flip Y when writing
    // so the texture aligns with ant world positions.
    if overlay.visible {
        for gy in 0..GRID_H {
            for gx in 0..GRID_W {
                let grid_i = gy * GRID_W + gx;
                let tex_row = GRID_H - 1 - gy; // Y-flip
                let base = (tex_row * GRID_W + gx) * 4;
                if world_map.walls[grid_i] {
                    pixels[base] = 0;
                    pixels[base + 1] = 0;
                    pixels[base + 2] = 0;
                    pixels[base + 3] = 0;
                } else {
                    let food_val = (grid.food[grid_i] * 255.0) as u8;
                    let home_val = (grid.home[grid_i] * 255.0) as u8;
                    pixels[base] = 0;
                    pixels[base + 1] = food_val;  // G = food (green)
                    pixels[base + 2] = home_val;  // B = home (blue)
                    pixels[base + 3] = food_val.max(home_val);
                }
            }
        }
    } else {
        // Overlay hidden: zero out entire texture in one pass
        pixels.fill(0);
    }
}

/// Plugin that registers pheromone systems
pub struct PheromonePlugin;

impl Plugin for PheromonePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PheromoneGrid::new()).add_systems(
            Update,
            (pheromone_decay_system, pheromone_texture_update_system),
        );
    }
}
