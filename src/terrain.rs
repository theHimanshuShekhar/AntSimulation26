use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use rand::Rng;
use std::collections::VecDeque;
use crate::config::*;
use crate::noise::generate_fbm;

/// WorldMap resource — same public API as old world.rs so other modules need minimal changes.
#[derive(Resource)]
pub struct WorldMap {
    /// Raw FBM density field — kept for future terrain editing.
    pub density: Vec<f32>,
    /// density > TERRAIN_ISO_LEVEL → wall. Flat row-major, length GRID_W * GRID_H.
    pub walls: Vec<bool>,
    /// Open cells with ≥2-cell clearance from walls (valid spawn positions).
    pub open_cells: Vec<usize>,
}

/// Convert (x, y) grid coordinates to flat index.
#[inline]
pub fn idx(x: usize, y: usize) -> usize {
    y * GRID_W + x
}

/// Convert grid coordinates to world coordinates (centered at origin).
pub fn grid_to_world(gx: usize, gy: usize) -> Vec2 {
    let cell_w = WINDOW_W / GRID_W as f32;
    let cell_h = WINDOW_H / GRID_H as f32;
    Vec2::new(
        gx as f32 * cell_w - WINDOW_W / 2.0,
        gy as f32 * cell_h - WINDOW_H / 2.0,
    )
}

/// Convert world coordinates to grid coordinates. Returns None if out of bounds.
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

// ---------------------------------------------------------------------------
// Marching squares
// ---------------------------------------------------------------------------

/// Triangle index lookup for all 16 marching squares cases.
///
/// Bit layout: bit0=BL, bit1=BR, bit2=TR, bit3=TL  (1 = solid/wall).
/// Cell vertex indices: 0=BL corner, 1=BR corner, 2=TR corner, 3=TL corner,
///   4=bottom-midpoint, 5=right-midpoint, 6=top-midpoint, 7=left-midpoint.
/// Winding: counter-clockwise (Bevy front-face default for Camera2d).
const TRIANGLES: [&[u32]; 16] = [
    &[],                                         // 0: none
    &[0, 4, 7],                                  // 1: BL
    &[4, 1, 5],                                  // 2: BR
    &[0, 1, 5, 0, 5, 7],                         // 3: BL+BR
    &[5, 2, 6],                                  // 4: TR
    &[0, 4, 7, 5, 2, 6],                         // 5: BL+TR (saddle)
    &[4, 1, 2, 4, 2, 6],                         // 6: BR+TR
    &[0, 1, 2, 0, 2, 6, 0, 6, 7],               // 7: BL+BR+TR
    &[7, 6, 3],                                  // 8: TL
    &[0, 4, 6, 0, 6, 3],                         // 9: BL+TL
    &[4, 1, 5, 7, 6, 3],                         // 10: BR+TL (saddle)
    &[0, 1, 5, 0, 5, 6, 0, 6, 3],               // 11: BL+BR+TL
    &[7, 5, 2, 7, 2, 3],                         // 12: TR+TL
    &[0, 4, 5, 0, 5, 2, 0, 2, 3],               // 13: BL+TR+TL
    &[4, 1, 2, 4, 2, 3, 4, 3, 7],               // 14: BR+TR+TL
    &[0, 1, 2, 0, 2, 3],                         // 15: all solid
];

/// Linear interpolation parameter along an edge to reach the iso-level.
fn iso_t(a: f32, b: f32, iso: f32) -> f32 {
    if (b - a).abs() < 1e-6 {
        0.5
    } else {
        ((iso - a) / (b - a)).clamp(0.0, 1.0)
    }
}

fn mix2(p0: [f32; 2], p1: [f32; 2], t: f32) -> [f32; 2] {
    [p0[0] + t * (p1[0] - p0[0]), p0[1] + t * (p1[1] - p0[1])]
}

/// Build a Bevy `Mesh` from the FBM density field using the marching squares algorithm.
///
/// Vertices on wall/open edges are linearly interpolated to sit on the iso-contour,
/// producing smooth terrain edges instead of stairstepped grid boundaries.
pub fn marching_squares_mesh(density: &[f32]) -> Mesh {
    let cell_w = WINDOW_W / GRID_W as f32;
    let cell_h = WINDOW_H / GRID_H as f32;
    let iso = TERRAIN_ISO_LEVEL;

    let corner_pos = |gx: usize, gy: usize| -> [f32; 2] {
        [
            gx as f32 * cell_w - WINDOW_W / 2.0,
            gy as f32 * cell_h - WINDOW_H / 2.0,
        ]
    };

    let mut positions: Vec<[f32; 3]> = Vec::new();

    for y in 0..GRID_H - 1 {
        for x in 0..GRID_W - 1 {
            let d_bl = density[y * GRID_W + x];
            let d_br = density[y * GRID_W + (x + 1)];
            let d_tr = density[(y + 1) * GRID_W + (x + 1)];
            let d_tl = density[(y + 1) * GRID_W + x];

            let config = ((d_bl > iso) as u8)
                | (((d_br > iso) as u8) << 1)
                | (((d_tr > iso) as u8) << 2)
                | (((d_tl > iso) as u8) << 3);

            if config == 0 {
                continue;
            }

            let p_bl = corner_pos(x, y);
            let p_br = corner_pos(x + 1, y);
            let p_tr = corner_pos(x + 1, y + 1);
            let p_tl = corner_pos(x, y + 1);

            // 8 cell vertices: 4 corners + 4 iso-interpolated midpoints
            let verts: [[f32; 2]; 8] = [
                p_bl,                                        // 0: BL corner
                p_br,                                        // 1: BR corner
                p_tr,                                        // 2: TR corner
                p_tl,                                        // 3: TL corner
                mix2(p_bl, p_br, iso_t(d_bl, d_br, iso)),   // 4: bottom-mid
                mix2(p_br, p_tr, iso_t(d_br, d_tr, iso)),   // 5: right-mid
                mix2(p_tl, p_tr, iso_t(d_tl, d_tr, iso)),   // 6: top-mid
                mix2(p_bl, p_tl, iso_t(d_bl, d_tl, iso)),   // 7: left-mid
            ];

            for &vi in TRIANGLES[config as usize] {
                let v = verts[vi as usize];
                positions.push([v[0], v[1], 0.0]);
            }
        }
    }

    let n = positions.len();
    let indices: Vec<u32> = (0..n as u32).collect();
    let normals: Vec<[f32; 3]> = vec![[0.0, 0.0, 1.0]; n];
    let uvs: Vec<[f32; 2]> = vec![[0.0, 0.0]; n];

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

// ---------------------------------------------------------------------------
// Terrain generation
// ---------------------------------------------------------------------------

/// Generate the FBM terrain, logical wall grid, and valid spawn cells.
pub fn generate_terrain(rng: &mut impl Rng) -> WorldMap {
    let seed = rng.gen::<f32>() * 1000.0;

    // Step 1: Generate FBM density field
    let mut density = generate_fbm(
        GRID_W,
        GRID_H,
        FBM_SCALE,
        FBM_LAYERS,
        FBM_LACUNARITY,
        FBM_PERSISTENCE,
        seed,
    );

    // Step 2: Force border band to solid (density = 1.0)
    let b = CAVE_BORDER_THICKNESS;
    for y in 0..GRID_H {
        for x in 0..GRID_W {
            if x < b || x >= GRID_W - b || y < b || y >= GRID_H - b {
                density[y * GRID_W + x] = 1.0;
            }
        }
    }

    // Step 3: Zero density within center exclusion radius (nest area always open)
    let cx = GRID_W / 2;
    let cy = GRID_H / 2;
    let excl_sq = (CAVE_CENTER_EXCLUSION * CAVE_CENTER_EXCLUSION) as i32;
    for y in 0..GRID_H {
        for x in 0..GRID_W {
            let dx = x as i32 - cx as i32;
            let dy = y as i32 - cy as i32;
            if dx * dx + dy * dy < excl_sq {
                density[y * GRID_W + x] = 0.0;
            }
        }
    }

    // Step 4: Threshold to logical wall grid
    let mut walls: Vec<bool> = density.iter().map(|&d| d > TERRAIN_ISO_LEVEL).collect();

    // Step 5: BFS flood-fill from center — unreachable open cells become walls
    let start_idx = cy * GRID_W + cx;
    let mut reachable = vec![false; GRID_W * GRID_H];
    let mut queue = VecDeque::new();
    queue.push_back(start_idx);
    reachable[start_idx] = true;

    while let Some(i) = queue.pop_front() {
        let iy = i / GRID_W;
        let ix = i % GRID_W;
        // Explicit bounds checks avoid re-pushing the boundary cell as its own neighbor
        // (which saturating_sub/min would do when ix==0 or iy==0).
        if ix > 0 {
            let ni = iy * GRID_W + (ix - 1);
            if !walls[ni] && !reachable[ni] { reachable[ni] = true; queue.push_back(ni); }
        }
        if ix + 1 < GRID_W {
            let ni = iy * GRID_W + (ix + 1);
            if !walls[ni] && !reachable[ni] { reachable[ni] = true; queue.push_back(ni); }
        }
        if iy > 0 {
            let ni = (iy - 1) * GRID_W + ix;
            if !walls[ni] && !reachable[ni] { reachable[ni] = true; queue.push_back(ni); }
        }
        if iy + 1 < GRID_H {
            let ni = (iy + 1) * GRID_W + ix;
            if !walls[ni] && !reachable[ni] { reachable[ni] = true; queue.push_back(ni); }
        }
    }

    for i in 0..walls.len() {
        if !walls[i] && !reachable[i] {
            walls[i] = true;
        }
    }

    // Step 6: Collect open cells with ≥2-cell clearance
    let inner = CAVE_BORDER_THICKNESS + 2;
    let mut open_cells: Vec<usize> = Vec::new();
    for y in inner..GRID_H - inner {
        for x in inner..GRID_W - inner {
            let i = y * GRID_W + x;
            if !walls[i]
                && !walls[(y - 2) * GRID_W + x]
                && !walls[(y + 2) * GRID_W + x]
                && !walls[y * GRID_W + (x - 2)]
                && !walls[y * GRID_W + (x + 2)]
            {
                open_cells.push(i);
            }
        }
    }

    // Fallback: 1-cell clearance
    if open_cells.is_empty() {
        for y in 1..GRID_H - 1 {
            for x in 1..GRID_W - 1 {
                let i = y * GRID_W + x;
                if !walls[i]
                    && !walls[(y - 1) * GRID_W + x]
                    && !walls[(y + 1) * GRID_W + x]
                    && !walls[y * GRID_W + (x - 1)]
                    && !walls[y * GRID_W + (x + 1)]
                {
                    open_cells.push(i);
                }
            }
        }
    }

    WorldMap { density, walls, open_cells }
}

/// Bevy startup system: generate terrain and insert WorldMap resource.
pub fn terrain_startup_system(mut commands: Commands) {
    let mut rng = rand::thread_rng();
    let world_map = generate_terrain(&mut rng);
    commands.insert_resource(world_map);
}

/// Plugin that registers the terrain generation startup system.
pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, terrain_startup_system);
    }
}
