use bevy::prelude::*;
use crate::config::*;
use crate::world::WorldMap;

/// Two types of pheromones: home (to nest) and food (to food sources)
#[derive(Clone, Copy, Debug)]
pub enum PheromoneKind {
    Home,
    Food,
}

/// Pheromone grid resource holding home and food pheromone channels
#[derive(Resource)]
pub struct PheromoneGrid {
    pub home: Vec<f32>,   // GRID_W * GRID_H, values 0.0–1.0
    pub food: Vec<f32>,   // GRID_W * GRID_H, values 0.0–1.0
    pub dirty: bool,      // set to true whenever grid changes, texture update clears it
}

impl PheromoneGrid {
    pub fn new() -> Self {
        Self {
            home: vec![0.0; GRID_W * GRID_H],
            food: vec![0.0; GRID_W * GRID_H],
            dirty: false,
        }
    }

    /// Deposit amount into one channel at grid index. Clamps to 1.0. Marks dirty.
    pub fn deposit(&mut self, idx: usize, kind: PheromoneKind, amount: f32) {
        let grid = match kind {
            PheromoneKind::Home => &mut self.home,
            PheromoneKind::Food => &mut self.food,
        };
        if idx < grid.len() {
            grid[idx] = (grid[idx] + amount).min(1.0);
            self.dirty = true;
        }
    }

    /// Sample a grid channel at index, returns 0.0 if out of bounds
    pub fn sample(&self, idx: usize, kind: PheromoneKind) -> f32 {
        let grid = match kind {
            PheromoneKind::Home => &self.home,
            PheromoneKind::Food => &self.food,
        };
        grid.get(idx).copied().unwrap_or(0.0)
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

    // Decay all cells
    for i in 0..GRID_W * GRID_H {
        grid.home[i] *= DECAY_FACTOR;
        if grid.home[i] < 0.001 {
            grid.home[i] = 0.0;
        }
        grid.food[i] *= DECAY_FACTOR;
        if grid.food[i] < 0.001 {
            grid.food[i] = 0.0;
        }
    }

    // Optional diffusion: simple 1-step box blur (skips wall cells)
    if DIFFUSION_ENABLED {
        let home_copy = grid.home.clone();
        let food_copy = grid.food.clone();
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
                let sum_h: f32 = neighbors.iter().map(|&n| home_copy[n]).sum();
                let sum_f: f32 = neighbors.iter().map(|&n| food_copy[n]).sum();
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
    for i in 0..GRID_W * GRID_H {
        let base = i * 4;
        if world_map.walls[i] {
            // Wall: dark grey, fully opaque
            pixels[base] = 60;      // R
            pixels[base + 1] = 60;  // G
            pixels[base + 2] = 60;  // B
            pixels[base + 3] = 255; // A
        } else if overlay.visible {
            // Open cell with pheromone overlay
            let food_val = (grid.food[i] * 255.0) as u8;
            let home_val = (grid.home[i] * 255.0) as u8;
            pixels[base] = 0;         // R
            pixels[base + 1] = food_val;  // G = food (green)
            pixels[base + 2] = home_val;  // B = home (blue)
            pixels[base + 3] = food_val.max(home_val); // A = max intensity
        } else {
            // No pheromones shown, open cell: transparent
            pixels[base] = 0;
            pixels[base + 1] = 0;
            pixels[base + 2] = 0;
            pixels[base + 3] = 0;
        }
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
