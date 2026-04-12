# Comprehensive Upgrade: SebLague Port Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the blob-CA cave with FBM + marching squares terrain; improve ant steering with composable angle forces; add ant lifetime and population management; switch to food blob clusters.

**Architecture:** `world.rs` is deleted and replaced by `terrain.rs` (FBM generation + marching squares mesh + same `WorldMap` API) and `noise.rs` (standalone FBM Perlin noise). Ant steering is rewritten with three additive angle forces (wander, pheromone, seek). A `Colony` resource and age system manage lifetimes and respawning. Food spawns in spatial clusters. All other systems (pheromone grid, sensors, collision, food interaction) are unchanged.

**Tech Stack:** Rust, Bevy 0.15, rand 0.8. No new crates required.

**Spec:** `docs/superpowers/specs/2026-04-12-seblague-comprehensive-upgrade-design.md`

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `src/config.rs` | Modify | Add FBM, terrain, steering, population, food cluster constants; remove old blob/CA constants |
| `src/noise.rs` | Create | FBM Perlin noise generator |
| `src/terrain.rs` | Create | WorldMap struct, FBM generation, logical wall grid, marching squares mesh, startup system, WorldPlugin |
| `src/world.rs` | Delete | Replaced by terrain.rs |
| `src/render.rs` | Modify | Add terrain mesh startup system; move pheromone overlay to z=1 |
| `src/pheromone.rs` | Modify | Remove wall-pixel branch from texture update (walls now rendered by terrain mesh) |
| `src/ant.rs` | Modify | Composable angle-force steering; ant age/lifetime component; Colony resource; ant_age_system |
| `src/food.rs` | Modify | Update world→terrain import; z=3 for food/nest |
| `src/main.rs` | Modify | Use terrain module; FoodPositions resource + update system; Colony resource; ant_respawn_system; updated UI; food cluster spawning; ant spawn z=2 |

---

## Task 1: Add new constants to config.rs

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Add FBM terrain constants after the Cave generation block**

In `src/config.rs`, replace the Cave generation block (lines 30–43) with:

```rust
// Cave / terrain shared
pub const CAVE_BORDER_THICKNESS: usize = 4;
pub const CAVE_CENTER_EXCLUSION: usize = 35;

// FBM terrain generation
pub const FBM_LAYERS: usize = 6;
pub const FBM_SCALE: f32 = 3.5;
pub const FBM_LACUNARITY: f32 = 2.0;
pub const FBM_PERSISTENCE: f32 = 0.5;
pub const TERRAIN_ISO_LEVEL: f32 = 0.52;

// Ant steering forces
pub const WANDER_WEIGHT: f32 = 1.0;
pub const PHEROMONE_WEIGHT: f32 = 2.5;
pub const SEEK_WEIGHT: f32 = 1.2;
pub const SEEK_RADIUS: f32 = 60.0;

// Ant lifetime / population
pub const ANT_LIFETIME_MIN: f32 = 30.0;
pub const ANT_LIFETIME_MAX: f32 = 90.0;
pub const ANT_RESPAWN_INTERVAL: f32 = 1.0;
pub const ANT_RESPAWN_BATCH: usize = 20;

// Food clustering
pub const FOOD_CLUSTER_SIZE: usize = 8;
pub const FOOD_CLUSTER_RADIUS: f32 = 20.0;
```

Keep all other constants (`WINDOW_W/H`, `GRID_W/H`, `ANT_*` existing, `PHEROMONE_*`, `FOOD_*`, `SENSOR_*`) unchanged. The old `CAVE_BLOB_*`, `CAVE_SMOOTH_*`, `CAVE_BIRTH_LIMIT`, `CAVE_DEATH_LIMIT` constants will be removed in Task 12 once terrain.rs no longer references them.

- [ ] **Step 2: Verify it compiles**

```bash
cargo build 2>&1 | head -30
```

Expected: compiles with possible unused-constant warnings (old cave constants still exist and are used by world.rs). Zero errors.

- [ ] **Step 3: Commit**

```bash
git add src/config.rs
git commit -m "config: add FBM terrain, steering force, population, and food cluster constants"
```

---

## Task 2: Create src/noise.rs

**Files:**
- Create: `src/noise.rs`

- [ ] **Step 1: Write the complete FBM Perlin noise module**

Create `src/noise.rs` with the following content:

```rust
/// FBM Perlin noise generator.
/// Produces values in [0.0, 1.0] over a GRID_W × GRID_H field.

// Ken Perlin's reference permutation table
const PERM: [u8; 256] = [
    151,160,137, 91, 90, 15,131, 13,201, 95, 96, 53,194,233,  7,225,
    140, 36,103, 30, 69,142,  8, 99, 37,240, 21, 10, 23,190,  6,148,
    247,120,234, 75,  0, 26,197, 62, 94,252,219,203,117, 35, 11, 32,
     57,177, 33, 88,237,149, 56, 87,174, 20,125,136,171,168, 68,175,
     74,165, 71,134,139, 48, 27,166, 77,146,158,231, 83,111,229,122,
     60,211,133,230,220,105, 92, 41, 55, 46,245, 40,244,102,143, 54,
     65, 25, 63,161,  1,216, 80, 73,209, 76,132,187,208, 89, 18,169,
    200,196,135,130,116,188,159, 86,164,100,109,198,173,186,  3, 64,
     52,217,226,250,124,123,  5,202, 38,147,118,126,255, 82, 85,212,
    207,206, 59,227, 47, 16, 58, 17,182,189, 28, 42,223,183,170,213,
    119,248,152,  2, 44,154,163, 70,221,153,101,155,167, 43,172,  9,
    129, 22, 39,253, 19, 98,108,110, 79,113,224,232,178,185,112,104,
    218,246, 97,228,251, 34,242,193,238,210,144, 12,191,179,162,241,
     81, 51,145,235,249, 14,239,107, 49,192,214, 31,181,199,106,157,
    184, 84,204,176,115,121, 50, 45,127,  4,150,254,138,236,205, 93,
    222,114, 67, 29, 24, 72,243,141,128,195, 78, 66,215, 61,156,180,
];

fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}

fn grad2(hash: u8, x: f32, y: f32) -> f32 {
    match hash & 3 {
        0 =>  x + y,
        1 => -x + y,
        2 =>  x - y,
        _ => -x - y,
    }
}

fn perlin2(x: f32, y: f32) -> f32 {
    let xi = (x.floor() as i32).rem_euclid(256) as usize;
    let yi = (y.floor() as i32).rem_euclid(256) as usize;
    let xf = x - x.floor();
    let yf = y - y.floor();
    let u = fade(xf);
    let v = fade(yf);

    let p = |i: usize| PERM[i & 255] as usize;
    let aa = p(p(xi)     + yi);
    let ab = p(p(xi)     + yi + 1);
    let ba = p(p(xi + 1) + yi);
    let bb = p(p(xi + 1) + yi + 1);

    lerp(
        lerp(
            grad2(PERM[aa], xf,       yf),
            grad2(PERM[ba], xf - 1.0, yf),
            u,
        ),
        lerp(
            grad2(PERM[ab], xf,       yf - 1.0),
            grad2(PERM[bb], xf - 1.0, yf - 1.0),
            u,
        ),
        v,
    )
}

/// Generate an FBM (Fractal Brownian Motion) noise field.
///
/// Returns a `Vec<f32>` of length `grid_w * grid_h` with values in `[0.0, 1.0]`.
/// `seed` offsets the sampling position so different seeds produce different terrain.
pub fn generate_fbm(
    grid_w: usize,
    grid_h: usize,
    scale: f32,
    layers: usize,
    lacunarity: f32,
    persistence: f32,
    seed: f32,
) -> Vec<f32> {
    let mut field = vec![0.0f32; grid_w * grid_h];

    // Use distinct axis offsets to avoid diagonal symmetry
    let sx = seed;
    let sy = seed * 1.7;

    for gy in 0..grid_h {
        for gx in 0..grid_w {
            // Normalized coordinates in [0, 1]
            let nx = gx as f32 / grid_w as f32;
            let ny = gy as f32 / grid_h as f32;

            let mut value = 0.0f32;
            let mut amplitude = 1.0f32;
            let mut frequency = scale;
            let mut max_val = 0.0f32;

            for _ in 0..layers {
                value += perlin2(nx * frequency + sx, ny * frequency + sy) * amplitude;
                max_val += amplitude;
                frequency *= lacunarity;
                amplitude *= persistence;
            }

            // Map from [-max_val, max_val] to [0, 1]
            field[gy * grid_w + gx] = (value / max_val) * 0.5 + 0.5;
        }
    }

    field
}
```

- [ ] **Step 2: Verify it compiles (not yet in mod list — that's fine)**

```bash
cargo build 2>&1 | head -20
```

Expected: success (noise.rs not yet in `main.rs` so it won't be compiled yet — that's intentional).

- [ ] **Step 3: Commit**

```bash
git add src/noise.rs
git commit -m "noise: add FBM Perlin noise generator"
```

---

## Task 3: Create src/terrain.rs

**Files:**
- Create: `src/terrain.rs`

- [ ] **Step 1: Write the complete terrain module**

Create `src/terrain.rs`:

```rust
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
/// Vertices are not deduplicated across cells — acceptable for this grid size.
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
    for y in 0..GRID_H {
        for x in 0..GRID_W {
            let dx = x as i32 - cx as i32;
            let dy = y as i32 - cy as i32;
            if ((dx * dx + dy * dy) as f32).sqrt() < CAVE_CENTER_EXCLUSION as f32 {
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
        for (nx, ny) in [
            (ix.saturating_sub(1), iy),
            ((ix + 1).min(GRID_W - 1), iy),
            (ix, iy.saturating_sub(1)),
            (ix, (iy + 1).min(GRID_H - 1)),
        ] {
            let ni = ny * GRID_W + nx;
            if !walls[ni] && !reachable[ni] {
                reachable[ni] = true;
                queue.push_back(ni);
            }
        }
    }

    for i in 0..walls.len() {
        if !walls[i] && !reachable[i] {
            walls[i] = true;
        }
    }

    // Step 6: Collect open cells with ≥2-cell clearance (same logic as old world.rs)
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
```

- [ ] **Step 2: Verify it compiles (not yet in mod list — that's fine)**

```bash
cargo build 2>&1 | head -20
```

Expected: success (terrain.rs not yet included in main.rs).

- [ ] **Step 3: Commit**

```bash
git add src/terrain.rs
git commit -m "terrain: add FBM + marching squares terrain generation"
```

---

## Task 4: Migrate world.rs → terrain.rs across the codebase

**Files:**
- Modify: `src/main.rs`
- Modify: `src/ant.rs`
- Modify: `src/food.rs`
- Delete: `src/world.rs`

- [ ] **Step 1: Update main.rs — swap mod declaration and all references**

In `src/main.rs`:

Replace `mod world;` with `mod noise;\nmod terrain;`

Replace all occurrences of `world::` with `terrain::`:
- `use world::WorldMap;` → `use terrain::WorldMap;`
- `world::WorldPlugin` → `terrain::WorldPlugin`
- `world::cave_startup_system` → `terrain::terrain_startup_system`
- `world::grid_to_world(gx, gy)` → `terrain::grid_to_world(gx, gy)`

In `spawn_nest_and_food`, update the system ordering attribute:

```rust
fn spawn_nest_and_food(
    // ... same parameters ...
)
```

And update `.after(world::cave_startup_system)` → `.after(terrain::terrain_startup_system)`:

```rust
.add_systems(
    Startup,
    (
        spawn_camera,
        spawn_nest_and_food.after(terrain::terrain_startup_system),
        spawn_ants.after(spawn_nest_and_food),
        setup_score_ui,
        setup_fps_ui,
    ),
)
```

- [ ] **Step 2: Update ant.rs — swap world references to terrain**

In `src/ant.rs`, replace:
```rust
use crate::world::WorldMap;
```
with:
```rust
use crate::terrain::WorldMap;
```

Replace all `crate::world::world_to_grid` → `crate::terrain::world_to_grid`
Replace all `crate::world::idx` → `crate::terrain::idx`

- [ ] **Step 3: Update food.rs — swap world references to terrain**

In `src/food.rs`, replace:
```rust
use crate::world::WorldMap;
```
with:
```rust
use crate::terrain::WorldMap;
```

Replace `crate::world::grid_to_world` → `crate::terrain::grid_to_world`

- [ ] **Step 4: Delete world.rs**

```bash
rm src/world.rs
```

- [ ] **Step 5: Verify it compiles**

```bash
cargo build 2>&1 | head -40
```

Expected: success. Possible unused import warnings — ignore them.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "migrate: replace world.rs with terrain.rs across all modules"
```

---

## Task 5: Update render.rs — add terrain mesh, fix z-layers

**Files:**
- Modify: `src/render.rs`

The current z-layers are: pheromone overlay z=0, ants z=1, food/nest z=2.
New z-layers: terrain mesh z=0, pheromone overlay z=1, ants z=2, food/nest z=3.

- [ ] **Step 1: Add terrain mesh startup system and update pheromone overlay z**

Replace the entire `src/render.rs` with:

```rust
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use crate::config::*;
use crate::pheromone::{PheromoneGrid, PheromoneOverlay};
use crate::terrain::{WorldMap, marching_squares_mesh};

/// Startup system: build the marching squares terrain mesh and spawn it at z=0.
/// Must run after terrain_startup_system so WorldMap is available.
pub fn setup_terrain_mesh(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    world_map: Res<WorldMap>,
) {
    let mesh = marching_squares_mesh(&world_map.density);
    commands.spawn((
        Mesh2d(meshes.add(mesh)),
        MeshMaterial2d(materials.add(ColorMaterial::from(Color::srgb(0.25, 0.22, 0.18)))),
        Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)), // z=0: terrain is bottommost
    ));
}

/// Startup system: create the pheromone overlay texture and spawn it at z=1 (above terrain).
pub fn setup_pheromone_overlay(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
) {
    let size = Extent3d {
        width: GRID_W as u32,
        height: GRID_H as u32,
        depth_or_array_layers: 1,
    };
    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();

    let texture_handle = images.add(image);

    commands.spawn((
        Sprite {
            image: texture_handle.clone(),
            custom_size: Some(Vec2::new(WINDOW_W, WINDOW_H)),
            ..default()
        },
        Transform::from_translation(Vec3::new(0.0, 0.0, 1.0)), // z=1: above terrain mesh
    ));

    commands.insert_resource(PheromoneOverlay {
        texture: texture_handle,
        visible: true,
    });
}

/// Input system: toggle pheromone overlay on P key press.
pub fn pheromone_overlay_toggle_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut overlay: ResMut<PheromoneOverlay>,
    mut grid: ResMut<PheromoneGrid>,
) {
    if keyboard.just_pressed(KeyCode::KeyP) {
        overlay.visible = !overlay.visible;
        grid.dirty = true;
    }
}

/// Plugin that registers rendering systems.
pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(
                Startup,
                (
                    setup_terrain_mesh.after(crate::terrain::terrain_startup_system),
                    setup_pheromone_overlay,
                ),
            )
            .add_systems(Update, pheromone_overlay_toggle_system);
    }
}
```

- [ ] **Step 2: Update food/nest z-layers in food.rs**

In `src/food.rs`, in `spawn_food`, change:
```rust
Transform::from_translation(pos.extend(2.0)), // z=2 above pheromone texture
```
to:
```rust
Transform::from_translation(pos.extend(3.0)), // z=3: above terrain, pheromone, ants
```

In `spawn_nest`, change:
```rust
Transform::from_translation(pos.extend(2.0)), // z=2 above pheromone texture
```
to:
```rust
Transform::from_translation(pos.extend(3.0)), // z=3: above terrain, pheromone, ants
```

- [ ] **Step 3: Update ant spawn z in main.rs**

In `src/main.rs`, in `spawn_ants`, change:
```rust
Transform::from_translation(pos.extend(1.0))
```
to:
```rust
Transform::from_translation(pos.extend(2.0)) // z=2: above terrain and pheromone overlay
```

- [ ] **Step 4: Verify it compiles**

```bash
cargo build 2>&1 | head -40
```

Expected: success.

- [ ] **Step 5: Commit**

```bash
git add src/render.rs src/food.rs src/main.rs
git commit -m "render: add terrain mesh at z=0, update z-layers for all entities"
```

---

## Task 6: Update pheromone.rs — remove wall-pixel branch

**Files:**
- Modify: `src/pheromone.rs`

The terrain mesh now renders walls. The pheromone texture no longer needs to paint walls as grey pixels — wall cells should be transparent (alpha=0) so the terrain mesh shows through.

- [ ] **Step 1: Remove the wall-pixel branch from pheromone_texture_update_system**

In `src/pheromone.rs`, locate `pheromone_texture_update_system`. The inner loop currently has:

```rust
if world_map.walls[grid_i] {
    // Wall: dark grey, fully opaque
    pixels[base] = 60;
    pixels[base + 1] = 60;
    pixels[base + 2] = 60;
    pixels[base + 3] = 255;
} else if overlay.visible {
    // ... pheromone rendering ...
} else {
    // ... transparent ...
}
```

Replace with:

```rust
if overlay.visible && !world_map.walls[grid_i] {
    let food_val = (grid.food[grid_i] * 255.0) as u8;
    let home_val = (grid.home[grid_i] * 255.0) as u8;
    pixels[base] = 0;
    pixels[base + 1] = food_val;  // G = food (green)
    pixels[base + 2] = home_val;  // B = home (blue)
    pixels[base + 3] = food_val.max(home_val);
} else {
    // Wall cells and hidden overlay: fully transparent
    pixels[base] = 0;
    pixels[base + 1] = 0;
    pixels[base + 2] = 0;
    pixels[base + 3] = 0;
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo build 2>&1 | head -20
```

Expected: success.

- [ ] **Step 3: Commit**

```bash
git add src/pheromone.rs
git commit -m "pheromone: remove wall-pixel rendering (terrain mesh now draws walls)"
```

---

## Task 7: Add FoodPositions resource

**Files:**
- Modify: `src/main.rs`

`FoodPositions` is a `Vec<Vec2>` resource updated each frame in `PreUpdate` schedule so `ant_behavior_system` (in `Update`) can read current food locations for the seek force without conflicting Transform queries.

- [ ] **Step 1: Add FoodPositions resource and update system to main.rs**

Add the following to `src/main.rs` (after the existing `NestPosition` resource):

```rust
/// Cached food positions updated each PreUpdate tick for ant seek-force queries.
#[derive(Resource, Default)]
pub struct FoodPositions(pub Vec<Vec2>);

fn update_food_positions(
    mut positions: ResMut<FoodPositions>,
    food_query: Query<&Transform, With<food::Food>>,
) {
    positions.0.clear();
    for transform in food_query.iter() {
        positions.0.push(transform.translation.truncate());
    }
}
```

Register it in `build_and_run`:

```rust
App::new()
    // ... existing plugins ...
    .insert_resource(FoodPositions::default())
    .add_systems(PreUpdate, update_food_positions)
    // ... existing systems ...
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo build 2>&1 | head -20
```

Expected: success.

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "main: add FoodPositions resource updated each PreUpdate for ant seek force"
```

---

## Task 8: Rewrite ant steering with composable angle forces

**Files:**
- Modify: `src/ant.rs`

Replace the binary follow/random-walk switch with three additive angle forces: wander (Gaussian noise, attenuated by signal strength), pheromone (continuous, scales with total signal), and seek (toward nearest food when Searching; toward nest when Returning with no home signal).

- [ ] **Step 1: Update ant_behavior_system signature and imports**

At the top of `src/ant.rs`, add:

```rust
use crate::FoodPositions;
```

Update `ant_behavior_system` signature to:

```rust
pub fn ant_behavior_system(
    mut ant_query: Query<(&mut Transform, &mut Ant)>,
    pheromone_grid: Res<PheromoneGrid>,
    world_map: Res<WorldMap>,
    mut deposits: ResMut<AntDeposits>,
    time: Res<Time>,
    food_positions: Res<FoodPositions>,
    mut rng: Local<Option<rand::rngs::SmallRng>>,
)
```

- [ ] **Step 2: Replace the steering body (steps 3–6 of the old function) with composable forces**

Replace everything between the sensor scoring (step 2) and the movement step (step 7) with:

```rust
// 3. Wander force: Gaussian angle noise, attenuated when pheromone is strong
let total_signal = left + ahead + right;
let wander_w = 1.0 - (total_signal * PHEROMONE_FOLLOW_WEIGHT).min(0.85);
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
        // Find nearest food within SEEK_RADIUS
        let nearest = food_positions.0.iter()
            .filter(|&&fp| fp.distance(pos) < SEEK_RADIUS)
            .min_by(|&&a, &&b| {
                a.distance(pos).partial_cmp(&b.distance(pos)).unwrap_or(std::cmp::Ordering::Equal)
            });
        if let Some(&food_pos) = nearest {
            let target = (food_pos - pos).y.atan2((food_pos - pos).x);
            angle_diff(target, ant.angle) * SEEK_WEIGHT
        } else {
            0.0
        }
    }
    AntState::Returning => {
        // Nest bias when no home pheromone detected
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
let base_noise = gaussian_noise(rng) * ANT_TURN_NOISE * 0.15;
ant.angle += wander + phero + seek + base_noise;
```

- [ ] **Step 3: Update ant z-layer in movement step**

In the movement step of `ant_behavior_system`, change:

```rust
transform.translation = new_pos.extend(1.0); // z=1 so ants render above texture
```

to:

```rust
transform.translation = new_pos.extend(2.0); // z=2: above terrain and pheromone overlay
```

- [ ] **Step 4: Verify it compiles**

```bash
cargo build 2>&1 | head -40
```

Expected: success.

- [ ] **Step 5: Commit**

```bash
git add src/ant.rs
git commit -m "ant: replace binary pheromone switch with composable wander/pheromone/seek angle forces"
```

---

## Task 9: Add ant lifetime, Colony resource, and ant_age_system

**Files:**
- Modify: `src/ant.rs`

- [ ] **Step 1: Add lifetime fields to the Ant component**

In `src/ant.rs`, update the `Ant` struct:

```rust
#[derive(Component)]
pub struct Ant {
    pub angle: f32,
    pub state: AntState,
    pub age: f32,      // seconds this ant has been alive
    pub lifetime: f32, // seconds until death (randomized at spawn)
}
```

- [ ] **Step 2: Add the Colony resource**

In `src/ant.rs`, add after the `AntDeposits` resource:

```rust
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
```

- [ ] **Step 3: Add ant_age_system**

In `src/ant.rs`, add the age system after `ant_deposit_flush_system`:

```rust
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
        }
    }
}
```

- [ ] **Step 4: Register ant_age_system in AntPlugin**

In `AntPlugin::build`, add `ant_age_system` to the Update schedule (after `ant_deposit_flush_system`):

```rust
impl Plugin for AntPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AntDeposits::default())
            .add_systems(
                Update,
                (
                    ant_behavior_system,
                    ant_deposit_flush_system.after(ant_behavior_system),
                    ant_age_system,
                ),
            );
    }
}
```

- [ ] **Step 5: Verify it compiles**

```bash
cargo build 2>&1 | head -40
```

Expected: success (Colony resource not yet inserted — will be fixed in Task 10).

- [ ] **Step 6: Commit**

```bash
git add src/ant.rs
git commit -m "ant: add lifetime/age component, Colony resource, and ant_age_system"
```

---

## Task 10: Colony respawn system and updated UI in main.rs

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Import Colony from ant.rs and insert the resource at startup**

In `src/main.rs`, add to the imports:

```rust
use ant::Colony;
```

In `spawn_ants`, after spawning all ants, insert the Colony resource:

```rust
fn spawn_ants(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    nest_pos: Res<NestPosition>,
) {
    let mut rng = rand::thread_rng();

    let ant_mesh = meshes.add(Triangle2d::new(
        Vec2::new(6.0, 0.0),
        Vec2::new(-4.0, 3.0),
        Vec2::new(-4.0, -3.0),
    ));
    let searching_material = materials.add(ColorMaterial::from(Color::srgb(0.6, 0.3, 0.1)));

    for _ in 0..ANT_COUNT {
        let angle = rng.gen::<f32>() * std::f32::consts::TAU;
        let lifetime = rng.gen_range(ANT_LIFETIME_MIN..ANT_LIFETIME_MAX);
        commands.spawn((
            Ant {
                angle,
                state: AntState::Searching,
                age: 0.0,
                lifetime,
            },
            Mesh2d(ant_mesh.clone()),
            MeshMaterial2d(searching_material.clone()),
            Transform::from_translation(nest_pos.0.extend(2.0))
                .with_rotation(Quat::from_rotation_z(angle)),
        ));
    }

    commands.insert_resource(Colony::new(ANT_COUNT));
}
```

- [ ] **Step 2: Add ant_respawn_system**

Add this system to `src/main.rs`:

```rust
fn ant_respawn_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut colony: ResMut<Colony>,
    nest_pos: Res<NestPosition>,
    time: Res<Time>,
) {
    colony.respawn_timer += time.delta_secs();
    if colony.respawn_timer < ANT_RESPAWN_INTERVAL {
        return;
    }
    colony.respawn_timer -= ANT_RESPAWN_INTERVAL;

    let to_spawn = ANT_RESPAWN_BATCH.min(colony.pending_respawn);
    if to_spawn == 0 {
        return;
    }

    let mut rng = rand::thread_rng();
    let ant_mesh = meshes.add(Triangle2d::new(
        Vec2::new(6.0, 0.0),
        Vec2::new(-4.0, 3.0),
        Vec2::new(-4.0, -3.0),
    ));
    let mat = materials.add(ColorMaterial::from(Color::srgb(0.6, 0.3, 0.1)));

    for _ in 0..to_spawn {
        let angle = rng.gen::<f32>() * std::f32::consts::TAU;
        let lifetime = rng.gen_range(ANT_LIFETIME_MIN..ANT_LIFETIME_MAX);
        commands.spawn((
            Ant {
                angle,
                state: AntState::Searching,
                age: 0.0,
                lifetime,
            },
            Mesh2d(ant_mesh.clone()),
            MeshMaterial2d(mat.clone()),
            Transform::from_translation(nest_pos.0.extend(2.0))
                .with_rotation(Quat::from_rotation_z(angle)),
        ));
    }

    colony.pending_respawn -= to_spawn;
    colony.active += to_spawn;
}
```

Register it in `build_and_run`:

```rust
.add_systems(Update, (update_score_ui, update_fps_ui, ant_respawn_system))
```

- [ ] **Step 3: Update UI to show population**

`Colony` is always available after Startup completes, so use `Res<Colony>` directly (not `Option`).

Replace the `update_score_ui` function in `src/main.rs` with:

```rust
fn update_score_ui(
    score: Res<FoodScore>,
    colony: Res<Colony>,
    mut query: Query<&mut Text, With<ScoreText>>,
) {
    if score.is_changed() || colony.is_changed() {
        for mut text in query.iter_mut() {
            *text = Text::new(format!(
                "Food: {}  Ants: {} / {}",
                score.collected, colony.active, ANT_COUNT
            ));
        }
    }
}
```

- [ ] **Step 4: Verify it compiles**

```bash
cargo build 2>&1 | head -40
```

Expected: success.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "main: add Colony resource, ant respawn system, and population UI"
```

---

## Task 11: Food cluster spawning

**Files:**
- Modify: `src/main.rs`

Replace the single-item-per-source food spawning with `FOOD_CLUSTER_SIZE` items scattered within `FOOD_CLUSTER_RADIUS` grid cells of a random cluster center.

- [ ] **Step 1: Rewrite the food spawning loop in spawn_nest_and_food**

In `src/main.rs`, the `source` variable is already built above the loop (filters open cells by nest distance). Replace only the `for _ in 0..FOOD_SOURCE_COUNT` block and everything below it in that function with:

```rust
use std::collections::HashSet;

let mut occupied: HashSet<usize> = HashSet::new();

for _ in 0..FOOD_SOURCE_COUNT {
    if source.is_empty() {
        break;
    }
    // Pick a cluster center from valid (far-from-nest) open cells
    let center_idx = source[rng.gen_range(0..source.len())];
    let center_gx = center_idx % GRID_W;
    let center_gy = center_idx / GRID_W;
    let center_world = terrain::grid_to_world(center_gx, center_gy);

    // Scatter FOOD_CLUSTER_SIZE items around the center
    for _ in 0..FOOD_CLUSTER_SIZE {
        let mut placed = false;
        for _ in 0..10 {
            // Random offset within FOOD_CLUSTER_RADIUS world units
            let angle = rng.gen::<f32>() * std::f32::consts::TAU;
            let radius = rng.gen::<f32>() * FOOD_CLUSTER_RADIUS;
            let offset_world = Vec2::new(angle.cos() * radius, angle.sin() * radius);
            let candidate = center_world + offset_world;

            if let Some((gx, gy)) = terrain::world_to_grid(candidate) {
                let cell_idx = terrain::idx(gx, gy);
                if !world_map.walls[cell_idx] && !occupied.contains(&cell_idx) {
                    occupied.insert(cell_idx);
                    let pos = terrain::grid_to_world(gx, gy);
                    spawn_food(&mut commands, &mut meshes, &mut materials, pos);
                    placed = true;
                    break;
                }
            }
        }
        // If no valid cell found after 10 retries, silently skip this item
        let _ = placed;
    }
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo build 2>&1 | head -20
```

Expected: success.

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "main: replace single food spawns with FOOD_CLUSTER_SIZE blob clusters"
```

---

## Task 12: Remove old cave constants from config.rs

**Files:**
- Modify: `src/config.rs`

All old `CAVE_BLOB_*`, `CAVE_SMOOTH_*`, `CAVE_BIRTH_LIMIT`, `CAVE_DEATH_LIMIT` constants are no longer referenced by any source file (terrain.rs uses only `CAVE_BORDER_THICKNESS` and `CAVE_CENTER_EXCLUSION`).

- [ ] **Step 1: Remove unused cave constants**

In `src/config.rs`, remove these lines entirely:

```rust
pub const CAVE_BLOB_COUNT_MIN: usize = 3;
pub const CAVE_BLOB_COUNT_MAX: usize = 5;
pub const CAVE_BLOB_RADIUS_MIN: f32 = 18.0;
pub const CAVE_BLOB_RADIUS_MAX: f32 = 35.0;
pub const CAVE_BLOB_NOISE: f32 = 7.0;
pub const CAVE_SMOOTH_ITERATIONS: usize = 3;
pub const CAVE_BIRTH_LIMIT: usize = 5;
pub const CAVE_DEATH_LIMIT: usize = 2;
pub const FOOD_MIN_NEST_DIST_CELLS: usize = 30;
```

Wait — `FOOD_MIN_NEST_DIST_CELLS` is still used in `main.rs` (`spawn_nest_and_food` filters food candidates by distance). Keep it.

Remove only:

```rust
pub const CAVE_BLOB_COUNT_MIN: usize = 3;
pub const CAVE_BLOB_COUNT_MAX: usize = 5;
pub const CAVE_BLOB_RADIUS_MIN: f32 = 18.0;
pub const CAVE_BLOB_RADIUS_MAX: f32 = 35.0;
pub const CAVE_BLOB_NOISE: f32 = 7.0;
pub const CAVE_SMOOTH_ITERATIONS: usize = 3;
pub const CAVE_BIRTH_LIMIT: usize = 5;
pub const CAVE_DEATH_LIMIT: usize = 2;
```

- [ ] **Step 2: Full build — verify zero errors**

```bash
cargo build 2>&1
```

Expected: success with zero errors. Possible warnings for unused variables — acceptable.

- [ ] **Step 3: Commit**

```bash
git add src/config.rs
git commit -m "config: remove unused blob/CA cave constants"
```

---

## Task 13: Release build — Linux and Windows

- [ ] **Step 1: Build Linux release binary**

```bash
cargo build --release 2>&1 | tail -5
```

Expected: `Finished release [optimized] target(s) in ...` with binary at `target/release/ant_simulation`.

- [ ] **Step 2: Build Windows release binary**

```bash
cargo build --release --target x86_64-pc-windows-gnu 2>&1 | tail -5
```

Expected: `Finished release [optimized] target(s) in ...` with binary at `target/x86_64-pc-windows-gnu/release/ant_simulation.exe`.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "release: verify Linux and Windows release builds pass after comprehensive upgrade"
```

---

## Self-Review Checklist

After all tasks are complete, verify spec coverage:

| Spec requirement | Covered by |
|---|---|
| FBM Perlin noise | Task 2 (noise.rs) |
| Marching squares terrain mesh | Task 3 (terrain.rs) |
| WorldMap.density field for future editing | Task 3 |
| Same WorldMap API (walls, open_cells, coord helpers) | Task 4 |
| Terrain mesh at z=0 | Task 5 |
| Pheromone overlay at z=1 (wall pixels removed) | Tasks 5, 6 |
| Ants at z=2, food/nest at z=3 | Tasks 5, 8 |
| Wander force (attenuated by signal) | Task 8 |
| Pheromone force (continuous, scales with signal) | Task 8 |
| Seek force (food for Searching, nest for Returning) | Tasks 7, 8 |
| score_sensor + directional alignment unchanged | Not modified |
| Ant age/lifetime fields | Task 9 |
| Colony resource | Task 9 |
| ant_age_system (despawn + colony tracking) | Task 9 |
| ant_respawn_system (batch respawn at nest) | Task 10 |
| Population UI | Task 10 |
| Food cluster spawning (FOOD_CLUSTER_SIZE items per source) | Task 11 |
| Old blob/CA constants removed | Task 12 |
| Both release targets build | Task 13 |
