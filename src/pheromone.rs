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
    scratch_home_dx: Vec<f32>,
    scratch_home_dy: Vec<f32>,
    scratch_food_dx: Vec<f32>,
    scratch_food_dy: Vec<f32>,
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
            scratch_home_dx: vec![0.0; n],
            scratch_home_dy: vec![0.0; n],
            scratch_food_dx: vec![0.0; n],
            scratch_food_dy: vec![0.0; n],
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
    /// True when wall pixels are currently written into the texture.
    /// Toggling visibility off calls `fill(0)`, clearing them; this flag tracks whether
    /// they need to be re-drawn before the next pheromone update pass.
    pub walls_drawn: bool,
}

/// Decay one pheromone channel at cell `i` in-place.
/// Takes slices so the caller can deref once and let the borrow checker split struct fields.
#[inline]
fn decay_channel(intensity: &mut [f32], dx: &mut [f32], dy: &mut [f32], i: usize) {
    intensity[i] *= DECAY_FACTOR;
    dx[i] *= DECAY_FACTOR;
    dy[i] *= DECAY_FACTOR;
    if intensity[i] < PHEROMONE_ZERO_THRESHOLD {
        intensity[i] = 0.0;
        dx[i] = 0.0;
        dy[i] = 0.0;
    }
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

    // Decay open cells only — walls never receive deposits so can be skipped.
    // Deref ResMut once so the borrow checker can split struct fields across the two calls.
    {
        let g = &mut *grid;
        for i in 0..GRID_W * GRID_H {
            if world_map.walls[i] {
                continue;
            }
            decay_channel(&mut g.home, &mut g.home_dir_x, &mut g.home_dir_y, i);
            decay_channel(&mut g.food, &mut g.food_dir_x, &mut g.food_dir_y, i);
        }
    }

    // Optional diffusion: simple 1-step box blur (skips wall cells)
    if DIFFUSION_ENABLED {
        // Copy current values into scratch buffers (no alloc — buffers pre-allocated in PheromoneGrid::new).
        // Use indexed loop so the borrow checker can see scratch and live fields are disjoint.
        for i in 0..GRID_W * GRID_H {
            grid.scratch_home[i] = grid.home[i];
            grid.scratch_food[i] = grid.food[i];
            grid.scratch_home_dx[i] = grid.home_dir_x[i];
            grid.scratch_home_dy[i] = grid.home_dir_y[i];
            grid.scratch_food_dx[i] = grid.food_dir_x[i];
            grid.scratch_food_dy[i] = grid.food_dir_y[i];
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
                let sum_hdx: f32 = neighbors.iter().map(|&n| grid.scratch_home_dx[n]).sum();
                let sum_hdy: f32 = neighbors.iter().map(|&n| grid.scratch_home_dy[n]).sum();
                let sum_fdx: f32 = neighbors.iter().map(|&n| grid.scratch_food_dx[n]).sum();
                let sum_fdy: f32 = neighbors.iter().map(|&n| grid.scratch_food_dy[n]).sum();
                // Weighted average: DIFFUSION_SELF_WEIGHT current, DIFFUSION_NEIGHBOR_WEIGHT neighbors
                grid.home[i] = (grid.home[i] * DIFFUSION_SELF_WEIGHT + (sum_h / 4.0) * DIFFUSION_NEIGHBOR_WEIGHT).min(1.0);
                grid.food[i] = (grid.food[i] * DIFFUSION_SELF_WEIGHT + (sum_f / 4.0) * DIFFUSION_NEIGHBOR_WEIGHT).min(1.0);
                grid.home_dir_x[i] = grid.home_dir_x[i] * DIFFUSION_SELF_WEIGHT + (sum_hdx / 4.0) * DIFFUSION_NEIGHBOR_WEIGHT;
                grid.home_dir_y[i] = grid.home_dir_y[i] * DIFFUSION_SELF_WEIGHT + (sum_hdy / 4.0) * DIFFUSION_NEIGHBOR_WEIGHT;
                grid.food_dir_x[i] = grid.food_dir_x[i] * DIFFUSION_SELF_WEIGHT + (sum_fdx / 4.0) * DIFFUSION_NEIGHBOR_WEIGHT;
                grid.food_dir_y[i] = grid.food_dir_y[i] * DIFFUSION_SELF_WEIGHT + (sum_fdy / 4.0) * DIFFUSION_NEIGHBOR_WEIGHT;
            }
        }
    }

    grid.dirty = true;
}

/// Update pheromone texture based on grid state and visibility
pub fn pheromone_texture_update_system(
    mut grid: ResMut<PheromoneGrid>,
    mut overlay: Option<ResMut<PheromoneOverlay>>,
    world_map: Option<Res<WorldMap>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(overlay) = overlay.as_mut() else { return };
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
        // If wall pixels were cleared (e.g. after a hide→show toggle), re-draw them.
        // Wall pixels are transparent (0,0,0,0) — same as the cleared state — so
        // we only need to ensure non-wall cells get the correct pheromone values.
        // Because `fill(0)` already makes walls transparent, we just mark them drawn.
        if !overlay.walls_drawn {
            overlay.walls_drawn = true;
        }
        for gy in 0..GRID_H {
            for gx in 0..GRID_W {
                let grid_i = gy * GRID_W + gx;
                // Wall pixels are pre-filled at startup and transparent — skip them
                if world_map.walls[grid_i] {
                    continue;
                }
                let tex_row = GRID_H - 1 - gy; // Y-flip
                let base = (tex_row * GRID_W + gx) * 4;
                let food_val = (grid.food[grid_i] * 255.0) as u8;
                let home_val = (grid.home[grid_i] * 255.0) as u8;
                pixels[base] = 0;
                pixels[base + 1] = food_val;  // G = food (green)
                pixels[base + 2] = home_val;  // B = home (blue)
                pixels[base + 3] = food_val.max(home_val);
            }
        }
    } else {
        // Overlay hidden: zero out entire texture in one pass
        pixels.fill(0);
        overlay.walls_drawn = false;
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
