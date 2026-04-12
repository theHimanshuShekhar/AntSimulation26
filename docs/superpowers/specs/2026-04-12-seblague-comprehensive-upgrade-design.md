# Comprehensive Upgrade: SebLague Ant Simulation Port

**Date:** 2026-04-12
**Scope:** Big-bang rewrite of terrain, steering, population management, and food systems
**Reference:** https://github.com/SebLague/Ant-Simulation

---

## Overview

This spec covers porting four major systems from SebLague's Unity ant simulation into the existing Rust + Bevy project. The upgrade replaces the boolean-grid cave with FBM + marching squares terrain, improves ant steering with composable angle forces, adds ant lifetime and population management, and changes food spawning to clustered blobs.

The pheromone grid (`GRID_W × GRID_H` flat arrays) is **unchanged** — it remains the backbone for ant sensing and diffusion. Only terrain rendering and generation change.

---

## Architecture

```
src/
├── config.rs       — updated: new constants, old blob/CA constants removed
├── noise.rs        — NEW: FBM Perlin noise generator
├── terrain.rs      — NEW: replaces world.rs; FBM → logical wall grid + marching squares mesh
├── pheromone.rs    — unchanged
├── ant.rs          — steering rewrite + lifetime/age component
├── food.rs         — unchanged (spawn_food helper used by main.rs cluster loop)
├── render.rs       — terrain mesh rendering; pheromone overlay loses wall-pixel logic (walls now rendered by terrain mesh)
└── main.rs         — Colony resource, population UI, respawn system
```

**Key invariant:** `WorldMap` still exposes `walls: Vec<bool>` and `open_cells: Vec<usize>` with the same semantics. Code in `ant.rs` and `pheromone.rs` that calls `world_to_grid` / `is_wall` continues to work without modification. Only the generation path changes.

**Extensibility hook:** `WorldMap` stores `density: Vec<f32>` (the raw FBM field). Future terrain editing regenerates the mesh and logical grid from a modified density field without touching any other system.

---

## Section 1: Terrain System (FBM + Marching Squares)

### New files
- `src/noise.rs` — FBM generator
- `src/terrain.rs` — replaces `src/world.rs`

### `noise.rs`
Implements Fractal Brownian Motion as a sum of `FBM_LAYERS` Perlin octaves:

```
for each layer:
    value += amplitude * perlin(pos * frequency)
    frequency *= FBM_LACUNARITY
    amplitude *= FBM_PERSISTENCE
```

No external crate. Gradient noise implemented in ~60 lines using a permutation table and smoothstep interpolation. Returns a normalized `Vec<f32>` of length `GRID_W * GRID_H` with values in `[0.0, 1.0]`.

### `terrain.rs` — WorldMap

```rust
pub struct WorldMap {
    pub density: Vec<f32>,      // raw FBM field (kept for future editing)
    pub walls: Vec<bool>,       // density > TERRAIN_ISO_LEVEL → wall
    pub open_cells: Vec<usize>, // open cells with ≥2-cell clearance from walls
}
```

**Generation steps:**
1. Generate FBM density field at `GRID_W × GRID_H`
2. Zero density within `CAVE_CENTER_EXCLUSION` radius of center (guarantees nest area is always open)
3. Threshold at `TERRAIN_ISO_LEVEL` → `walls: Vec<bool>`
4. BFS flood-fill connectivity from center (same as today — unreachable open cells become walls)
5. Collect `open_cells` with ≥2-cell clearance (same logic as today)
6. Run marching squares on density field → `Mesh`

**Marching squares:**
- Each 2×2 block of cells produces a 4-bit configuration (16 cases)
- Vertices on wall/open edges are linearly interpolated to the iso-contour (smooth edges)
- Output: single `Mesh` for Bevy's `Mesh2d` spawned at z=0 with `TERRAIN_COLOR`
- Ants use the logical `walls` bool grid for collision — the mesh is rendering only

**Z-layer update:**
- z=0: terrain mesh (replaces old grey wall pixels in pheromone texture)
- z=1: pheromone overlay fullscreen sprite (wall-pixel branch in `pheromone_texture_update_system` removed — open cells only)
- z=2: ants
- z=3: food sources and nest

`pheromone_texture_update_system` loses the `if world_map.walls[grid_i]` branch — wall cells are now transparent (alpha=0) in the overlay, letting the terrain mesh show through beneath.

### Removed from `config.rs`
`CAVE_BLOB_COUNT_MIN/MAX`, `CAVE_BLOB_RADIUS_MIN/MAX`, `CAVE_BLOB_NOISE`, `CAVE_SMOOTH_ITERATIONS`, `CAVE_BIRTH_LIMIT`, `CAVE_DEATH_LIMIT`

### New config constants
```rust
pub const FBM_LAYERS: usize = 6;
pub const FBM_SCALE: f32 = 3.5;
pub const FBM_LACUNARITY: f32 = 2.0;
pub const FBM_PERSISTENCE: f32 = 0.5;
pub const TERRAIN_ISO_LEVEL: f32 = 0.52;
pub const TERRAIN_COLOR: Color = Color::srgb(0.25, 0.22, 0.18);
```

`CAVE_BORDER_THICKNESS` and `CAVE_CENTER_EXCLUSION` are kept (still used by terrain gen).

---

## Section 2: Hybrid Steering (Composable Angle Forces)

**File:** `src/ant.rs` — `ant_behavior_system` rewrite

### Problem with current steering
Binary switch: ant either follows strongest sensor (discrete `±SENSOR_ANGLE * 0.5` turn) or does full Gaussian random walk. No smooth scaling with signal strength — 99% signal behaves identically to 1%.

### New model: three additive angle forces

```
Δangle = w_wander   * wander_force
       + w_pheromone * pheromone_force
       + w_seek      * seek_force
```

| Force | Computation | Active when |
|---|---|---|
| **Wander** | Gaussian noise on angle | Always |
| **Pheromone** | Angle toward strongest sensor, scaled by `total_signal` | Pheromone signal > threshold |
| **Seek** | `angle_diff` toward food or nest | Within `SEEK_RADIUS`, or nest-bias fallback |

**Wander weight** attenuates as signal strengthens:
```
w_wander = 1.0 - (total_signal * PHEROMONE_FOLLOW_WEIGHT).min(0.85)
```
Keeps ≥15% wander even on strongest trail — prevents lock-in.

**Pheromone force** is continuous (not binary):
```
pheromone_force = angle toward strongest sensor
w_pheromone     = total_signal.min(1.0) * PHEROMONE_WEIGHT
```

**Seek force** replaces the `angle_diff * 0.05` nest-bias:
- `Searching`: activates within `SEEK_RADIUS` of any food source
- `Returning`: activates when no home pheromone signal; weight ramps with distance from nest

`score_sensor` and directional alignment scoring are **unchanged** — they feed directly into `pheromone_force`.

### New config constants
```rust
pub const WANDER_WEIGHT: f32 = 1.0;
pub const PHEROMONE_WEIGHT: f32 = 2.5;
pub const SEEK_WEIGHT: f32 = 1.2;
pub const SEEK_RADIUS: f32 = 60.0;
```

---

## Section 3: Ant Lifetime & Population Management

**Files:** `src/ant.rs` (component + age system), `src/main.rs` (Colony resource + respawn system + UI)

### `Ant` component changes
```rust
pub struct Ant {
    pub angle: f32,
    pub state: AntState,
    pub age: f32,       // seconds alive
    pub lifetime: f32,  // seconds until death (randomized at spawn)
}
```
`lifetime` randomized per ant at spawn in `[ANT_LIFETIME_MIN, ANT_LIFETIME_MAX]` — spreads deaths out, avoids mass die-offs.

### `Colony` resource
```rust
#[derive(Resource)]
pub struct Colony {
    pub active: usize,
    pub total_died: usize,
    pub pending_respawn: usize,
    pub respawn_timer: f32,
}
```

### Systems

**`ant_age_system`** (in `ant.rs`, runs each Update):
- Increment `ant.age += dt` for all ants
- When `age >= lifetime`: despawn entity, `colony.active -= 1`, `colony.pending_respawn += 1`

**`ant_respawn_system`** (in `main.rs`, runs each Update):
- Tick `colony.respawn_timer += dt`
- Every `ANT_RESPAWN_INTERVAL` seconds: spawn `min(pending_respawn, ANT_RESPAWN_BATCH)` new ants at nest with fresh `age = 0` and new random `lifetime`
- Total population capped at `ANT_COUNT`

### UI
```
Food collected: 23    Ants: 1847 / 2000    FPS: 60
```
`Colony` is a `Resource` — existing `update_score_ui` reads it directly alongside `FoodScore`.

### New config constants
```rust
pub const ANT_LIFETIME_MIN: f32 = 30.0;
pub const ANT_LIFETIME_MAX: f32 = 90.0;
pub const ANT_RESPAWN_INTERVAL: f32 = 1.0;
pub const ANT_RESPAWN_BATCH: usize = 20;
```

---

## Section 4: Food Clustering

**File:** `src/main.rs` — `spawn_nest_and_food` rewrite; `src/config.rs` — new constants

### Problem with current spawning
Single item per food source. Ants exhaust a single point quickly with no spatial spread.

### New model: blob clusters
Each `FOOD_SOURCE_COUNT` source becomes a cluster of `FOOD_CLUSTER_SIZE` items scattered within `FOOD_CLUSTER_RADIUS` grid cells of a randomly chosen center.

### Spawn algorithm
```
for each source (0..FOOD_SOURCE_COUNT):
    pick random open cell as cluster center (same nest-distance filter as today)
    for each item (0..FOOD_CLUSTER_SIZE):
        sample random (dx, dy) within FOOD_CLUSTER_RADIUS
        find nearest open cell to offset position
        skip if cell is wall or already occupied (retry up to 10 times)
        spawn food item
```

Individual food items are visually identical to today (small circle). Cluster density makes groupings visually obvious without a special marker.

### New config constants
```rust
pub const FOOD_CLUSTER_SIZE: usize = 8;
pub const FOOD_CLUSTER_RADIUS: f32 = 20.0;
```

`FOOD_SOURCE_COUNT` and `FOOD_PER_SOURCE` unchanged.

---

## What Is Not Changed

- `pheromone.rs` — grid structure, decay, diffusion, texture encoding all unchanged
- `score_sensor` / `score_sensors` — directional alignment logic unchanged
- `handle_collision` — wall-bounce logic unchanged (still uses `walls: Vec<bool>`)
- `world_to_grid` / `grid_to_world` / `idx` — coordinate helpers moved to `terrain.rs`, same signatures
- `GRID_W`, `GRID_H`, `WINDOW_W`, `WINDOW_H`, pheromone constants — unchanged
- `CAVE_BORDER_THICKNESS`, `CAVE_CENTER_EXCLUSION` — kept, used by terrain gen

---

## Out of Scope

- Real-time terrain editing (density field stored for future implementation)
- Full physics velocity/acceleration model (hybrid angle-force model chosen instead)
- Ant visual differentiation by state (color change when carrying food — possible future work)
