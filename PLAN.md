# AntSimulation Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix correctness bugs and eliminate performance inefficiencies across the ant simulation codebase without changing observable behavior.

**Architecture:** Each task is self-contained and targets one file/concern. Tasks are ordered by impact: asset leaks first (biggest runtime cost), then correctness, then minor cleanups. No new files are created — all changes are within existing modules.

**Tech Stack:** Rust, Bevy 0.15, ECS pattern. No test suite — verify by running `cargo run` and observing simulation runs correctly after each task.

---

## File Map

| File | Tasks |
|------|-------|
| `src/main.rs` | 1, 3, 9, 10 |
| `src/ant.rs` | 2, 4, 5, 7 |
| `src/pheromone.rs` | 6, 8 |
| `src/terrain.rs` | 3 (minor) |
| `src/config.rs` | 10 |
| `src/food.rs` | 1 |

---

### Task 1: Cache Mesh/Material Handles — Eliminate Per-Spawn Asset Leaks

**Problem:** `ant_respawn_system` and `spawn_food` call `meshes.add()`/`materials.add()` on every invocation, creating a new GPU asset per respawn batch. Over a long session this leaks unbounded memory.

**Fix:** Create two resources — `AntAssets` and `FoodAssets` — populated once at startup, then cloned wherever ants/food are spawned.

**Files:**
- Modify: `src/main.rs`
- Modify: `src/food.rs`

- [ ] **Step 1: Add `AntAssets` resource to `src/main.rs`**

Add this struct and `From` impl just above `spawn_ants`:

```rust
#[derive(Resource)]
pub struct AntAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<MeshMaterial2d<ColorMaterial>>,
}
```

- [ ] **Step 2: Populate `AntAssets` in `spawn_ants`, remove per-ant alloc**

Replace the body of `spawn_ants`:

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
            Ant { angle, state: AntState::Searching, age: 0.0, lifetime },
            Mesh2d(ant_mesh.clone()),
            MeshMaterial2d(searching_material.clone()),
            Transform::from_translation(nest_pos.0.extend(2.0))
                .with_rotation(Quat::from_rotation_z(angle)),
        ));
    }

    commands.insert_resource(AntAssets {
        mesh: ant_mesh,
        material: searching_material,
    });
    commands.insert_resource(Colony::new(ANT_COUNT));
}
```

- [ ] **Step 3: Update `ant_respawn_system` to use `AntAssets`**

```rust
fn ant_respawn_system(
    mut commands: Commands,
    mut colony: ResMut<Colony>,
    nest_pos: Res<NestPosition>,
    ant_assets: Res<AntAssets>,
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

    for _ in 0..to_spawn {
        let angle = rng.gen::<f32>() * std::f32::consts::TAU;
        let lifetime = rng.gen_range(ANT_LIFETIME_MIN..ANT_LIFETIME_MAX);
        commands.spawn((
            Ant { angle, state: AntState::Searching, age: 0.0, lifetime },
            Mesh2d(ant_assets.mesh.clone()),
            MeshMaterial2d(ant_assets.material.clone()),
            Transform::from_translation(nest_pos.0.extend(2.0))
                .with_rotation(Quat::from_rotation_z(angle)),
        ));
    }

    colony.pending_respawn -= to_spawn;
    colony.active += to_spawn;
}
```

- [ ] **Step 4: Add `FoodAssets` resource to `src/food.rs`**

Add just above `spawn_food`:

```rust
#[derive(Resource)]
pub struct FoodAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<MeshMaterial2d<ColorMaterial>>,
}
```

- [ ] **Step 5: Create a `setup_food_assets` startup system in `src/food.rs`**

```rust
pub fn setup_food_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.insert_resource(FoodAssets {
        mesh: meshes.add(Circle::new(8.0)),
        material: materials.add(ColorMaterial::from(Color::srgb(0.1, 0.9, 0.1))),
    });
}
```

- [ ] **Step 6: Update `spawn_food` signature to accept `&FoodAssets`**

```rust
pub fn spawn_food(
    commands: &mut Commands,
    assets: &FoodAssets,
    pos: Vec2,
) {
    commands.spawn((
        Food { units: FOOD_PER_SOURCE },
        Mesh2d(assets.mesh.clone()),
        MeshMaterial2d(assets.material.clone()),
        Transform::from_translation(pos.extend(3.0)),
    ));
}
```

- [ ] **Step 7: Update all callers of `spawn_food`**

In `src/food.rs` → `food_respawn_system`: replace `meshes`/`materials` params with `food_assets: Res<FoodAssets>` and call `spawn_food(&mut commands, &food_assets, pos)`.

In `src/main.rs` → `spawn_nest_and_food`: add `food_assets: Res<FoodAssets>` param, remove `meshes`/`materials`, call `spawn_food(&mut commands, &food_assets, pos)`.

- [ ] **Step 8: Register `setup_food_assets` in `FoodPlugin` before it's used**

In `FoodPlugin::build`:

```rust
app.insert_resource(FoodScore::default())
    .add_systems(Startup, setup_food_assets)
    .add_systems(Update, (food_interaction_system, nest_interaction_system, food_respawn_system));
```

Also add `.after(setup_food_assets)` ordering for `spawn_nest_and_food` in `main.rs` Startup chain.

- [ ] **Step 9: Build and verify**

```bash
cargo build 2>&1 | head -40
```

Expected: No errors. Run `cargo run` and confirm ants spawn, food appears, respawn works.

- [ ] **Step 10: Commit**

```bash
git add src/main.rs src/food.rs
git commit -m "perf: cache ant and food mesh/material handles to prevent asset leaks"
```

---

### Task 2: Fix RNG — Pass `SmallRng` into `handle_collision`

**Problem:** `handle_collision` calls `rand::random::<f32>()` (locks the thread-local RNG) instead of using the `SmallRng` already threaded through `ant_behavior_system`. This is inconsistent and incurs unnecessary locking on every wall hit.

**Files:**
- Modify: `src/ant.rs`

- [ ] **Step 1: Update `handle_collision` signature to accept `rng`**

```rust
fn handle_collision(
    old_pos: Vec2,
    new_pos: Vec2,
    angle: &mut f32,
    world_map: &WorldMap,
    rng: &mut impl Rng,
) -> Vec2 {
```

- [ ] **Step 2: Replace `rand::random::<f32>()` calls inside `handle_collision`**

Change both occurrences of `rand::random::<f32>()` to `rng.gen::<f32>()`:

```rust
            let try_angle = if attempt == 0 {
                *angle + PI + (rng.gen::<f32>() - 0.5) * 1.0
            } else {
                rng.gen::<f32>() * 2.0 * PI
            };
```

- [ ] **Step 3: Update the call site in `ant_behavior_system`**

```rust
        let new_pos = handle_collision(pos, new_pos, &mut ant.angle, &world_map, rng);
```

- [ ] **Step 4: Build and verify**

```bash
cargo build 2>&1 | head -20
```

- [ ] **Step 5: Commit**

```bash
git add src/ant.rs
git commit -m "fix: use SmallRng in handle_collision instead of thread_rng"
```

---

### Task 3: Squared-Distance Comparisons — Remove Unnecessary `sqrt` Calls

**Problem:** Three places compute `sqrt` just to compare against a threshold, which is wasteful. Compare squared distances instead.

**Files:**
- Modify: `src/ant.rs` (food seek filter)
- Modify: `src/main.rs` (food candidate filter)
- Modify: `src/terrain.rs` (center exclusion)

- [ ] **Step 1: Fix food seek in `ant_behavior_system` (`src/ant.rs`)**

Replace:
```rust
                let nearest = food_positions.0.iter()
                    .filter(|&&fp| fp.distance(pos) < SEEK_RADIUS)
                    .min_by(|&&a, &&b| {
                        a.distance(pos).partial_cmp(&b.distance(pos)).unwrap_or(std::cmp::Ordering::Equal)
                    });
```

With:
```rust
                let seek_radius_sq = SEEK_RADIUS * SEEK_RADIUS;
                let nearest = food_positions.0.iter()
                    .filter(|&&fp| fp.distance_squared(pos) < seek_radius_sq)
                    .min_by(|&&a, &&b| {
                        a.distance_squared(pos)
                            .partial_cmp(&b.distance_squared(pos))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
```

- [ ] **Step 2: Fix food candidate filter in `spawn_nest_and_food` (`src/main.rs`)**

Replace:
```rust
            let dist = ((dx * dx + dy * dy) as f32).sqrt() as usize;
            dist >= FOOD_MIN_NEST_DIST_CELLS
```

With:
```rust
            let dist_sq = (dx * dx + dy * dy) as usize;
            dist_sq >= FOOD_MIN_NEST_DIST_CELLS * FOOD_MIN_NEST_DIST_CELLS
```

- [ ] **Step 3: Fix center exclusion in `generate_terrain` (`src/terrain.rs`)**

Replace:
```rust
            if ((dx * dx + dy * dy) as f32).sqrt() < CAVE_CENTER_EXCLUSION as f32 {
```

With:
```rust
            let excl = CAVE_CENTER_EXCLUSION as i32;
            if dx * dx + dy * dy < excl * excl {
```

(Move `excl` definition outside the inner loop, before `for x in 0..GRID_W`.)

- [ ] **Step 4: Build and verify**

```bash
cargo build 2>&1 | head -20
```

- [ ] **Step 5: Commit**

```bash
git add src/ant.rs src/main.rs src/terrain.rs
git commit -m "perf: replace sqrt distance comparisons with squared distance"
```

---

### Task 4: Fix Pheromone Bounds Check — Guard Against Wrong Channel Length

**Problem:** `PheromoneGrid::deposit` checks `idx >= self.home.len()` even for `Food` deposits. A `Food` deposit against a truncated grid would silently skip the length check.

**Files:**
- Modify: `src/pheromone.rs`

- [ ] **Step 1: Replace single bound check with per-channel check**

In `PheromoneGrid::deposit`, replace:

```rust
        let n = self.home.len();
        if idx >= n {
            return;
        }
```

With:

```rust
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
```

And remove the duplicate `match kind` block that follows.

The full corrected `deposit` method:

```rust
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
        dx[idx] += dir.x * amount;
        dy[idx] += dir.y * amount;
        self.dirty = true;
    }
```

- [ ] **Step 2: Build and verify**

```bash
cargo build 2>&1 | head -20
```

- [ ] **Step 3: Commit**

```bash
git add src/pheromone.rs
git commit -m "fix: guard pheromone deposit bounds check against the correct channel"
```

---

### Task 5: Double-Buffer Pheromone Diffusion — Eliminate Per-Tick Vec Clone

**Problem:** `pheromone_decay_system` clones `home` and `food` Vecs on every decay tick (every 0.25 s), allocating ~300 KB per tick. Use pre-allocated scratch buffers instead.

**Files:**
- Modify: `src/pheromone.rs`

- [ ] **Step 1: Add scratch buffer fields to `PheromoneGrid`**

```rust
pub struct PheromoneGrid {
    pub home: Vec<f32>,
    pub home_dir_x: Vec<f32>,
    pub home_dir_y: Vec<f32>,
    pub food: Vec<f32>,
    pub food_dir_x: Vec<f32>,
    pub food_dir_y: Vec<f32>,
    pub dirty: bool,
    // Pre-allocated diffusion scratch buffers — avoids per-tick Vec allocation
    scratch_home: Vec<f32>,
    scratch_food: Vec<f32>,
}
```

- [ ] **Step 2: Initialize scratch buffers in `PheromoneGrid::new()`**

```rust
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
```

- [ ] **Step 3: Replace `.clone()` in `pheromone_decay_system` with scratch copy**

Replace:
```rust
        let home_copy = grid.home.clone();
        let food_copy = grid.food.clone();
        for y in 1..GRID_H - 1 {
            for x in 1..GRID_W - 1 {
                let i = y * GRID_W + x;
                if world_map.walls[i] { continue; }
                let neighbors = [ ... ];
                let sum_h: f32 = neighbors.iter().map(|&n| home_copy[n]).sum();
                let sum_f: f32 = neighbors.iter().map(|&n| food_copy[n]).sum();
                grid.home[i] = (grid.home[i] * 0.6 + (sum_h / 4.0) * 0.4).min(1.0);
                grid.food[i] = (grid.food[i] * 0.6 + (sum_f / 4.0) * 0.4).min(1.0);
            }
        }
```

With:
```rust
        // Copy current values into scratch buffers (no alloc)
        grid.scratch_home.copy_from_slice(&grid.home);
        grid.scratch_food.copy_from_slice(&grid.food);

        for y in 1..GRID_H - 1 {
            for x in 1..GRID_W - 1 {
                let i = y * GRID_W + x;
                if world_map.walls[i] { continue; }
                let neighbors = [
                    (y - 1) * GRID_W + x,
                    (y + 1) * GRID_W + x,
                    y * GRID_W + (x - 1),
                    y * GRID_W + (x + 1),
                ];
                let sum_h: f32 = neighbors.iter().map(|&n| grid.scratch_home[n]).sum();
                let sum_f: f32 = neighbors.iter().map(|&n| grid.scratch_food[n]).sum();
                grid.home[i] = (grid.home[i] * 0.6 + (sum_h / 4.0) * 0.4).min(1.0);
                grid.food[i] = (grid.food[i] * 0.6 + (sum_f / 4.0) * 0.4).min(1.0);
            }
        }
```

- [ ] **Step 4: Build and verify**

```bash
cargo build 2>&1 | head -20
```

- [ ] **Step 5: Commit**

```bash
git add src/pheromone.rs
git commit -m "perf: pre-allocate diffusion scratch buffers to avoid per-tick Vec clone"
```

---

### Task 6: Hoist Visibility Check Out of Texture Update Inner Loop

**Problem:** `pheromone_texture_update_system` checks `overlay.visible` per pixel (36k iterations). Extract two separate loops.

**Files:**
- Modify: `src/pheromone.rs`

- [ ] **Step 1: Restructure `pheromone_texture_update_system`**

Replace the inner `if overlay.visible && !world_map.walls[grid_i]` with two outer code paths:

```rust
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

    let Some(image) = images.get_mut(&overlay.texture) else { return };
    let pixels = &mut image.data;

    if overlay.visible {
        for gy in 0..GRID_H {
            for gx in 0..GRID_W {
                let grid_i = gy * GRID_W + gx;
                let tex_row = GRID_H - 1 - gy;
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
                    pixels[base + 1] = food_val;
                    pixels[base + 2] = home_val;
                    pixels[base + 3] = food_val.max(home_val);
                }
            }
        }
    } else {
        // Overlay hidden: zero out entire texture
        pixels.fill(0);
    }
}
```

- [ ] **Step 2: Build and verify**

```bash
cargo build 2>&1 | head -20
```

- [ ] **Step 3: Commit**

```bash
git add src/pheromone.rs
git commit -m "perf: hoist pheromone visibility check out of per-pixel inner loop"
```

---

### Task 7: Pheromone Decay — Extract Channel Helper, Skip Walls

**Problem:** Decay loop duplicates identical home/food blocks and iterates wall cells unnecessarily.

**Files:**
- Modify: `src/pheromone.rs`

- [ ] **Step 1: Extract inline decay helper in `pheromone_decay_system`**

Replace the hand-duplicated decay block with a local closure:

```rust
    let decay = |v: &mut f32, dx: &mut f32, dy: &mut f32| {
        *v *= DECAY_FACTOR;
        *dx *= DECAY_FACTOR;
        *dy *= DECAY_FACTOR;
        if *v < 0.001 {
            *v = 0.0;
            *dx = 0.0;
            *dy = 0.0;
        }
    };

    for i in 0..GRID_W * GRID_H {
        if world_map.walls[i] { continue; }
        decay(&mut grid.home[i], &mut grid.home_dir_x[i], &mut grid.home_dir_y[i]);
        decay(&mut grid.food[i], &mut grid.food_dir_x[i], &mut grid.food_dir_y[i]);
    }
```

Note: Rust borrow rules prevent a closure that mutates the grid fields directly through `&mut grid`. Write it as a free function called with individual mutable references as shown above, or use an inline macro-style. If the borrow checker objects, inline the body twice (the original pattern) — correctness over cleverness.

Alternative if closure has borrow issues — just deduplicate the clear condition:

```rust
    for i in 0..GRID_W * GRID_H {
        if world_map.walls[i] { continue; }

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
```

- [ ] **Step 2: Build and verify**

```bash
cargo build 2>&1 | head -20
```

- [ ] **Step 3: Commit**

```bash
git add src/pheromone.rs
git commit -m "perf: skip wall cells during pheromone decay"
```

---

### Task 8: Remove Dead Code and Replace HashSet with Flat Bool Array

**Problem:**
- `let _ = placed;` in `spawn_nest_and_food` is dead code.
- `HashSet<usize>` for occupied cells has hash overhead; a `Vec<bool>` of grid size is faster and simpler.

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Remove dead `let _ = placed;` and the `placed` variable**

Remove lines 131 (`let mut placed = false;`) and 148 (`let _ = placed;`) and the `placed = true;` assignment at line 143.

The inner loop becomes:

```rust
        for _ in 0..FOOD_CLUSTER_SIZE {
            for _ in 0..10 {
                let angle = rng.gen::<f32>() * std::f32::consts::TAU;
                let radius = rng.gen::<f32>() * FOOD_CLUSTER_RADIUS;
                let offset_world = Vec2::new(angle.cos() * radius, angle.sin() * radius);
                let candidate = center_world + offset_world;

                if let Some((gx, gy)) = terrain::world_to_grid(candidate) {
                    let cell_idx = terrain::idx(gx, gy);
                    if !world_map.walls[cell_idx] && !occupied[cell_idx] {
                        occupied[cell_idx] = true;
                        let pos = terrain::grid_to_world(gx, gy);
                        spawn_food(&mut commands, &food_assets, pos);
                        break;
                    }
                }
            }
        }
```

- [ ] **Step 2: Replace `HashSet<usize>` with `Vec<bool>`**

Replace:
```rust
    let mut occupied: HashSet<usize> = HashSet::new();
```
With:
```rust
    let mut occupied = vec![false; GRID_W * GRID_H];
```

Replace `!occupied.contains(&cell_idx)` with `!occupied[cell_idx]`.
Replace `occupied.insert(cell_idx)` with `occupied[cell_idx] = true`.

- [ ] **Step 3: Remove the `HashSet` import if now unused**

In `src/main.rs`, remove or verify `use std::collections::HashSet;` is gone.

- [ ] **Step 4: Build and verify**

```bash
cargo build 2>&1 | head -20
```

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "fix: remove dead placed variable; replace HashSet with flat bool array for occupied cells"
```

---

### Task 9: Fix System Ordering — Ant Age Before Behavior

**Problem:** `ant_age_system` and `ant_behavior_system` run in undefined order within the same `Update` set. An ant can be despawned mid-frame after its behavior already ran (harmless), or before (skipped behavior). More importantly, `ant_respawn_system` is in a separate `Update` registration in `main.rs`, disconnected from `AntPlugin`'s scheduling.

**Files:**
- Modify: `src/ant.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Explicitly order ant systems in `AntPlugin`**

```rust
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
```

- [ ] **Step 2: Move `ant_respawn_system` registration into `AntPlugin`**

Remove `ant_respawn_system` from the `App::add_systems(Update, ...)` call in `main.rs`.

In `AntPlugin::build`, add:

```rust
                    ant_respawn_system.after(ant_age_system),
```

Move the `ant_respawn_system` function from `src/main.rs` to `src/ant.rs`, making it `pub(crate)` or keeping it as `pub`. Update imports accordingly (`NestPosition`, `AntAssets` need to be imported or passed differently).

If moving is complex (due to `NestPosition`/`AntAssets` living in `main.rs`), keep `ant_respawn_system` in `main.rs` but add explicit ordering in the `Update` registration:

```rust
.add_systems(
    Update,
    (
        update_score_ui,
        update_fps_ui,
        ant_respawn_system.after(ant::ant_age_system),
    )
)
```

- [ ] **Step 3: Build and verify**

```bash
cargo build 2>&1 | head -20
```

- [ ] **Step 4: Commit**

```bash
git add src/ant.rs src/main.rs
git commit -m "fix: enforce ant system ordering — age before behavior before flush"
```

---

### Task 10: Promote Magic Numbers to Config Constants

**Problem:** Several tuning values are hardcoded in logic rather than in `config.rs`, making them invisible to anyone tuning the simulation.

**Files:**
- Modify: `src/config.rs`
- Modify: `src/ant.rs`
- Modify: `src/pheromone.rs`

- [ ] **Step 1: Add constants to `src/config.rs`**

```rust
// Ant collision probe
pub const ANT_COLLISION_RADIUS: f32 = 4.0;       // footprint probe radius (px)
pub const ANT_BOUNDARY_MARGIN: f32 = 5.0;         // world-edge buffer (px)

// Ant steering tuning
pub const PHEROMONE_FOLLOW_MAX: f32 = 0.85;       // max wander suppression fraction
pub const BASE_NOISE_FRACTION: f32 = 0.15;        // base noise scalar vs wander

// Sensor directional scoring
pub const SENSOR_MIN_ALIGNMENT_WEIGHT: f32 = 0.2; // floor alignment contribution

// Pheromone diffusion blend
pub const DIFFUSION_SELF_WEIGHT: f32 = 0.6;       // current cell weight in box blur
pub const DIFFUSION_NEIGHBOR_WEIGHT: f32 = 0.4;   // neighbor average weight

// Pheromone zero-floor
pub const PHEROMONE_ZERO_THRESHOLD: f32 = 0.001;  // below this → snap to 0
```

- [ ] **Step 2: Replace magic numbers in `src/ant.rs`**

| Old | New |
|-----|-----|
| `4.0_f32` (collision radius) | `ANT_COLLISION_RADIUS` |
| `5.0` (boundary margins) | `ANT_BOUNDARY_MARGIN` |
| `0.85` (wander clamp) | `PHEROMONE_FOLLOW_MAX` |
| `0.15` (base noise) | `BASE_NOISE_FRACTION` |
| `0.2 + 0.8 * ...` | `SENSOR_MIN_ALIGNMENT_WEIGHT + (1.0 - SENSOR_MIN_ALIGNMENT_WEIGHT) * ...` |

- [ ] **Step 3: Replace magic numbers in `src/pheromone.rs`**

| Old | New |
|-----|-----|
| `0.001` (zero floor) | `PHEROMONE_ZERO_THRESHOLD` |
| `0.6` / `0.4` (diffusion blend) | `DIFFUSION_SELF_WEIGHT` / `DIFFUSION_NEIGHBOR_WEIGHT` |

- [ ] **Step 4: Build and verify**

```bash
cargo build 2>&1 | head -20
```

- [ ] **Step 5: Commit**

```bash
git add src/config.rs src/ant.rs src/pheromone.rs
git commit -m "refactor: promote magic numbers to named config constants"
```

---

### Task 11: Final Build — Windows Cross-Compile

Per project convention, always produce a Windows binary after changes.

- [ ] **Step 1: Build Windows target**

```bash
cargo build --release --target x86_64-pc-windows-gnu 2>&1 | tail -5
```

Expected: `Finished release [optimized] target(s) in ...`

- [ ] **Step 2: Confirm Linux release also builds**

```bash
cargo build --release 2>&1 | tail -5
```

- [ ] **Step 3: Final commit if anything leftover**

```bash
git status
```

If clean, no commit needed.

---

## Summary of Changes

| # | File | Change | Impact |
|---|------|--------|--------|
| 1 | main.rs, food.rs | Cache mesh/material in AntAssets/FoodAssets | Memory leak fix |
| 2 | ant.rs | SmallRng → handle_collision | Consistency, avoids thread lock |
| 3 | ant.rs, main.rs, terrain.rs | Squared distance comparisons | Minor perf |
| 4 | pheromone.rs | Correct bounds check channel | Correctness |
| 5 | pheromone.rs | Pre-alloc diffusion scratch bufs | ~300 KB alloc per 0.25s removed |
| 6 | pheromone.rs | Hoist visibility check | Minor perf |
| 7 | pheromone.rs | Skip walls in decay, dedup blocks | Minor perf + readability |
| 8 | main.rs | Remove dead code, HashSet→Vec<bool> | Readability + minor perf |
| 9 | ant.rs, main.rs | Explicit system ordering | Correctness |
| 10 | config.rs, ant.rs, pheromone.rs | Named constants | Maintainability |
| 11 | — | Windows build | Deployment requirement |
