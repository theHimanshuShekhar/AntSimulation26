use bevy::prelude::*;
use rand::Rng;
use crate::config::*;
use std::collections::VecDeque;

/// WorldMap resource containing wall layout and valid spawn locations
#[derive(Resource)]
pub struct WorldMap {
    /// Flat grid: true = wall, false = open. Length = GRID_W * GRID_H
    pub walls: Vec<bool>,
    /// Indices of open cells with ≥2-cell clearance from walls (valid for spawning)
    pub open_cells: Vec<usize>,
}

/// Convert (x, y) grid coordinates to flat index
#[inline]
pub fn idx(x: usize, y: usize) -> usize {
    y * GRID_W + x
}

/// Convert grid coordinates to world coordinates (centered at origin)
pub fn grid_to_world(gx: usize, gy: usize) -> Vec2 {
    let cell_w = WINDOW_W / GRID_W as f32;
    let cell_h = WINDOW_H / GRID_H as f32;

    let x = gx as f32 * cell_w - WINDOW_W / 2.0;
    let y = gy as f32 * cell_h - WINDOW_H / 2.0;

    Vec2::new(x, y)
}

/// Convert world coordinates to grid coordinates, returns None if out of bounds
pub fn world_to_grid(pos: Vec2) -> Option<(usize, usize)> {
    let cell_w = WINDOW_W / GRID_W as f32;
    let cell_h = WINDOW_H / GRID_H as f32;

    let gx = ((pos.x + WINDOW_W / 2.0) / cell_w) as i32;
    let gy = ((pos.y + WINDOW_H / 2.0) / cell_h) as i32;

    if gx >= 0 && gx < GRID_W as i32 && gy >= 0 && gy < GRID_H as i32 {
        Some((gx as usize, gy as usize))
    } else {
        None
    }
}

/// Generate a cave using cellular automata
pub fn generate_cave(rng: &mut impl Rng) -> WorldMap {
    // Step 1: Random initialization
    let mut walls = vec![false; GRID_W * GRID_H];

    for y in 0..GRID_H {
        for x in 0..GRID_W {
            let i = idx(x, y);
            // Set borders to walls
            if x == 0 || x == GRID_W - 1 || y == 0 || y == GRID_H - 1 {
                walls[i] = true;
            } else {
                walls[i] = rng.gen::<f32>() < CAVE_INITIAL_WALL_CHANCE;
            }
        }
    }

    // Step 2: Smoothing iterations using cellular automata
    for _ in 0..CAVE_SMOOTH_ITERATIONS {
        let mut new_walls = walls.clone();

        for y in 1..GRID_H - 1 {
            for x in 1..GRID_W - 1 {
                let i = idx(x, y);

                // Count wall neighbors (8-neighbor Moore neighborhood)
                let mut wall_count = 0;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let nx = (x as i32 + dx) as usize;
                        let ny = (y as i32 + dy) as usize;
                        if walls[idx(nx, ny)] {
                            wall_count += 1;
                        }
                    }
                }

                // Apply cellular automata rules
                if wall_count >= CAVE_BIRTH_LIMIT {
                    new_walls[i] = true;
                } else if wall_count < CAVE_DEATH_LIMIT {
                    new_walls[i] = false;
                }
                // else: cell stays the same
            }
        }

        walls = new_walls;
    }

    // Step 3: Flood-fill connectivity
    let center_x = GRID_W / 2;
    let center_y = GRID_H / 2;
    let mut start_idx = idx(center_x, center_y);

    // If center is a wall, find nearest open cell
    if walls[start_idx] {
        let mut found = false;
        'outer: for radius in 1..=(GRID_W.max(GRID_H)) {
            for y in (center_y.saturating_sub(radius))..=(center_y + radius).min(GRID_H - 1) {
                for x in (center_x.saturating_sub(radius))..=(center_x + radius).min(GRID_W - 1) {
                    let i = idx(x, y);
                    if !walls[i] {
                        start_idx = i;
                        found = true;
                        break 'outer;
                    }
                }
            }
        }
        if !found {
            // Fallback: no open cells at all, return mostly walls
            return WorldMap {
                walls,
                open_cells: vec![],
            };
        }
    }

    // BFS from start cell to mark reachable cells
    let mut reachable = vec![false; GRID_W * GRID_H];
    let mut queue = VecDeque::new();
    queue.push_back(start_idx);
    reachable[start_idx] = true;

    while let Some(i) = queue.pop_front() {
        let y = i / GRID_W;
        let x = i % GRID_W;

        // 4-neighbor connectivity (Von Neumann neighborhood)
        let neighbors = [
            (x.saturating_sub(1), y),
            ((x + 1).min(GRID_W - 1), y),
            (x, y.saturating_sub(1)),
            (x, (y + 1).min(GRID_H - 1)),
        ];

        for (nx, ny) in neighbors.iter() {
            let ni = idx(*nx, *ny);
            if !walls[ni] && !reachable[ni] {
                reachable[ni] = true;
                queue.push_back(ni);
            }
        }
    }

    // Mark unreachable open cells as walls
    for i in 0..walls.len() {
        if !walls[i] && !reachable[i] {
            walls[i] = true;
        }
    }

    // Step 4: Collect open cells with ≥2-cell clearance from walls
    let mut open_cells = Vec::new();

    for y in 2..GRID_H - 2 {
        for x in 2..GRID_W - 2 {
            let i = idx(x, y);
            if !walls[i] {
                // Check 2-cell clearance in 4 cardinal directions
                let has_clearance = !walls[idx(x - 2, y)] && // left 2
                                    !walls[idx(x + 2, y)] && // right 2
                                    !walls[idx(x, y - 2)] && // down 2
                                    !walls[idx(x, y + 2)];   // up 2

                if has_clearance {
                    open_cells.push(i);
                }
            }
        }
    }

    // Fallback: if no cells with strict clearance, relax to 1-cell clearance
    if open_cells.is_empty() {
        for y in 1..GRID_H - 1 {
            for x in 1..GRID_W - 1 {
                let i = idx(x, y);
                if !walls[i] {
                    let has_clearance = !walls[idx(x - 1, y)] && // left 1
                                        !walls[idx(x + 1, y)] && // right 1
                                        !walls[idx(x, y - 1)] && // down 1
                                        !walls[idx(x, y + 1)];   // up 1

                    if has_clearance {
                        open_cells.push(i);
                    }
                }
            }
        }
    }

    // Final fallback: accept all open cells even with 0 clearance
    if open_cells.is_empty() {
        for i in 0..walls.len() {
            if !walls[i] {
                open_cells.push(i);
            }
        }
    }

    WorldMap { walls, open_cells }
}

/// Bevy startup system to generate the cave on initialization
pub fn cave_startup_system(mut commands: Commands) {
    let mut rng = rand::thread_rng();
    let world_map = generate_cave(&mut rng);
    commands.insert_resource(world_map);
}

/// Plugin that registers the world generation system
pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, cave_startup_system);
    }
}
