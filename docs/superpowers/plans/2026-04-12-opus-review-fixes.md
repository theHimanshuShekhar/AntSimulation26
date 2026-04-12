# Opus Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement all outstanding fixes from OPUS_REVIEW.md, skipping items already resolved in prior commits.

**Architecture:** Each task is a focused edit to one or two files. No new files needed. Build verification after each task (no tests per CLAUDE.md). After all tasks, build both targets per project convention.

**Tech Stack:** Rust, Bevy 0.15 ECS, `rand` crate.

---

## Pre-flight: Already-fixed items (mark complete, no action needed)

The following review items are already resolved in the codebase — confirmed by reading source:

- **#2** `distance_squared` in ant seek — DONE (`food_positions.0.iter()` uses `distance_squared`)
- **#3** Dead `placed` variable — DONE (removed from `main.rs`)
- **#4** sqrt → squared distance for food placement — DONE (`dist_sq >= FOOD_MIN_NEST_DIST_CELLS * FOOD_MIN_NEST_DIST_CELLS`)
- **#6** SmallRng passed to `handle_collision` — DONE (signature `rng: &mut impl Rng`)
- **#7** AntAssets resource cached — DONE (`AntAssets { mesh, material }` resource exists)
- **#8** FoodAssets resource cached — DONE (`FoodAssets { mesh, material }` resource exists)
- **#9 (partial)** Double-buffer scratch alloc — DONE (`scratch_home`/`scratch_food` pre-allocated in `PheromoneGrid::new`)
- **#10** Wall short-circuit in decay — DONE (`if world_map.walls[i] { continue; }`)
- **#17** `overlay.visible` hoisted out of pixel loop — DONE (entire loop is inside `if overlay.visible { ... } else { pixels.fill(0); }`)
- **#20** SmallRng used in 8-attempt loop — DONE (passes `rng` from outer scope)
- **#22** Squared distance in center exclusion — DONE (`dx * dx + dy * dy < excl_sq`)
- **#23** HashSet → bool vec for occupied cells — DONE (`vec![false; GRID_W * GRID_H]`)
- **#24** System ordering — DONE (`ant_age_system → ant_behavior_system → ant_deposit_flush_system`)
- **#25** Mesh/material pervasive caching — DONE (AntAssets + FoodAssets)
- **#26** SeedableRng import — DONE (used by `SmallRng::from_entropy()`)
- **#27** HashSet import removal — DONE (import not present in main.rs)

---

## Task 1: Diffuse direction channels alongside intensity (#9 direction drift)

**Files:**
- Modify: `src/pheromone.rs` — diffusion block (~lines 145–170)

**Problem:** Diffusion only blurs `home[i]` and `food[i]` intensities. The direction components (`home_dir_x`, `home_dir_y`, `food_dir_x`, `food_dir_y`) are never diffused, so after a few ticks intensity and direction diverge.

- [ ] **Step 1: Add scratch buffers for direction components in `PheromoneGrid`**

In `src/pheromone.rs`, add four new scratch fields to `PheromoneGrid`:

```rust
// In PheromoneGrid struct, add after scratch_food:
scratch_home_dx: Vec<f32>,
scratch_home_dy: Vec<f32>,
scratch_food_dx: Vec<f32>,
scratch_food_dy: Vec<f32>,
```

In `PheromoneGrid::new()`, initialize them:

```rust
scratch_home_dx: vec![0.0; n],
scratch_home_dy: vec![0.0; n],
scratch_food_dx: vec![0.0; n],
scratch_food_dy: vec![0.0; n],
```

- [ ] **Step 2: Copy direction channels into scratch before diffusion**

In `pheromone_decay_system`, inside `if DIFFUSION_ENABLED {`, after the existing scratch copies add:

```rust
for i in 0..GRID_W * GRID_H {
    grid.scratch_home[i] = grid.home[i];
    grid.scratch_food[i] = grid.food[i];
    grid.scratch_home_dx[i] = grid.home_dir_x[i];
    grid.scratch_home_dy[i] = grid.home_dir_y[i];
    grid.scratch_food_dx[i] = grid.food_dir_x[i];
    grid.scratch_food_dy[i] = grid.food_dir_y[i];
}
```

(Replace the existing 2-field copy loop entirely with this 6-field version.)

- [ ] **Step 3: Diffuse direction channels in the inner loop**

In the `for y in 1..GRID_H - 1` loop, after updating `grid.home[i]` and `grid.food[i]`, add:

```rust
let sum_hdx: f32 = neighbors.iter().map(|&n| grid.scratch_home_dx[n]).sum();
let sum_hdy: f32 = neighbors.iter().map(|&n| grid.scratch_home_dy[n]).sum();
let sum_fdx: f32 = neighbors.iter().map(|&n| grid.scratch_food_dx[n]).sum();
let sum_fdy: f32 = neighbors.iter().map(|&n| grid.scratch_food_dy[n]).sum();
grid.home_dir_x[i] = grid.home_dir_x[i] * DIFFUSION_SELF_WEIGHT + (sum_hdx / 4.0) * DIFFUSION_NEIGHBOR_WEIGHT;
grid.home_dir_y[i] = grid.home_dir_y[i] * DIFFUSION_SELF_WEIGHT + (sum_hdy / 4.0) * DIFFUSION_NEIGHBOR_WEIGHT;
grid.food_dir_x[i] = grid.food_dir_x[i] * DIFFUSION_SELF_WEIGHT + (sum_fdx / 4.0) * DIFFUSION_NEIGHBOR_WEIGHT;
grid.food_dir_y[i] = grid.food_dir_y[i] * DIFFUSION_SELF_WEIGHT + (sum_fdy / 4.0) * DIFFUSION_NEIGHBOR_WEIGHT;
```

- [ ] **Step 4: Build and verify**

```bash
cargo build 2>&1 | tail -5
```

Expected: no errors or warnings related to pheromone.rs.

- [ ] **Step 5: Commit**

```bash
git add src/pheromone.rs
git commit -m "fix: diffuse direction channels alongside intensity to prevent drift (#9)"
```

---

## Task 2: angle_diff — replace while loops with rem_euclid (#21)

**Files:**
- Modify: `src/ant.rs` — `angle_diff` function (~lines 166–176)

**Problem:** The `while diff > PI` / `while diff < -PI` loops are correct but `rem_euclid` is a one-liner.

- [ ] **Step 1: Replace angle_diff body**

Replace:
```rust
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
```

With:
```rust
fn angle_diff(target: f32, current: f32) -> f32 {
    use std::f32::consts::PI;
    let diff = (target - current).rem_euclid(2.0 * PI);
    if diff > PI { diff - 2.0 * PI } else { diff }
}
```

- [ ] **Step 2: Build**

```bash
cargo build 2>&1 | tail -5
```

Expected: clean build.

- [ ] **Step 3: Commit**

```bash
git add src/ant.rs
git commit -m "refactor: simplify angle_diff using rem_euclid (#21)"
```

---

## Task 3: Mutate score/fps Text in-place instead of re-allocating (#19)

**Files:**
- Modify: `src/main.rs` — `update_score_ui` and `update_fps_ui` (~lines 273–285, 309–319)

**Problem:** Both systems call `*text = Text::new(...)` which allocates a new `Text` struct every change tick. Mutating the string in-place avoids that.

- [ ] **Step 1: Patch update_score_ui**

Replace:
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

With:
```rust
fn update_score_ui(
    score: Res<FoodScore>,
    colony: Res<Colony>,
    mut query: Query<&mut Text, With<ScoreText>>,
) {
    if score.is_changed() || colony.is_changed() {
        for mut text in query.iter_mut() {
            text.0 = format!(
                "Food: {}  Ants: {} / {}",
                score.collected, colony.active, ANT_COUNT
            );
        }
    }
}
```

- [ ] **Step 2: Patch update_fps_ui**

Replace:
```rust
fn update_fps_ui(
    diagnostics: Res<DiagnosticsStore>,
    mut query: Query<&mut Text, With<FpsText>>,
) {
    if let Some(fps) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
    {
        for mut text in query.iter_mut() {
            *text = Text::new(format!("FPS: {:.0}", fps));
        }
    }
}
```

With:
```rust
fn update_fps_ui(
    diagnostics: Res<DiagnosticsStore>,
    mut query: Query<&mut Text, With<FpsText>>,
) {
    if let Some(fps) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
    {
        for mut text in query.iter_mut() {
            text.0 = format!("FPS: {:.0}", fps);
        }
    }
}
```

- [ ] **Step 3: Build**

```bash
cargo build 2>&1 | tail -5
```

Expected: clean build. (If `Text.0` doesn't compile in this Bevy version, try `text.sections[0].value = format!(...)` instead.)

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "perf: mutate Text content in-place to avoid per-frame allocation (#19)"
```

---

## Task 4: Extract decay_channel helper in pheromone.rs (#28)

**Files:**
- Modify: `src/pheromone.rs` — `pheromone_decay_system`, decay block (~lines 120–142)

**Problem:** The decay block for home and food channels is identical code repeated twice.

- [ ] **Step 1: Extract inline helper and apply**

Replace the repeated decay block inside `pheromone_decay_system` (the section inside `for i in 0..GRID_W * GRID_H`):

```rust
// Before (two near-identical blocks):
grid.home[i] *= DECAY_FACTOR;
grid.home_dir_x[i] *= DECAY_FACTOR;
grid.home_dir_y[i] *= DECAY_FACTOR;
if grid.home[i] < PHEROMONE_ZERO_THRESHOLD {
    grid.home[i] = 0.0;
    grid.home_dir_x[i] = 0.0;
    grid.home_dir_y[i] = 0.0;
}

grid.food[i] *= DECAY_FACTOR;
grid.food_dir_x[i] *= DECAY_FACTOR;
grid.food_dir_y[i] *= DECAY_FACTOR;
if grid.food[i] < PHEROMONE_ZERO_THRESHOLD {
    grid.food[i] = 0.0;
    grid.food_dir_x[i] = 0.0;
    grid.food_dir_y[i] = 0.0;
}
```

Add a module-level (not `pub`) helper function just above `pheromone_decay_system`:

```rust
/// Decay one pheromone channel at index `i` in-place.
#[inline]
fn decay_channel(intensity: &mut f32, dx: &mut f32, dy: &mut f32) {
    *intensity *= DECAY_FACTOR;
    *dx *= DECAY_FACTOR;
    *dy *= DECAY_FACTOR;
    if *intensity < PHEROMONE_ZERO_THRESHOLD {
        *intensity = 0.0;
        *dx = 0.0;
        *dy = 0.0;
    }
}
```

Then replace the repeated blocks with:

```rust
decay_channel(&mut grid.home[i], &mut grid.home_dir_x[i], &mut grid.home_dir_y[i]);
decay_channel(&mut grid.food[i], &mut grid.food_dir_x[i], &mut grid.food_dir_y[i]);
```

- [ ] **Step 2: Build**

```bash
cargo build 2>&1 | tail -5
```

Expected: clean build.

- [ ] **Step 3: Commit**

```bash
git add src/pheromone.rs
git commit -m "refactor: extract decay_channel helper to deduplicate decay loop (#28)"
```

---

## Task 5: Pre-fill wall pixels in texture at setup; skip walls in per-frame update (#11, #18)

**Files:**
- Modify: `src/render.rs` — `setup_pheromone_overlay` (adds wall pre-fill pass)
- Modify: `src/pheromone.rs` — `pheromone_texture_update_system` (skip wall cells)

**Problem:** Every frame, the texture update loop checks `world_map.walls[grid_i]` for each of 36,864 cells and writes transparent pixels for walls. Since walls never change, we can write them once at startup and skip them every frame.

- [ ] **Step 1: Accept WorldMap in setup_pheromone_overlay and pre-fill wall pixels**

The function signature needs `WorldMap`. Update `setup_pheromone_overlay` in `src/render.rs`:

```rust
pub fn setup_pheromone_overlay(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    world_map: Option<Res<WorldMap>>,
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

    // Pre-fill wall pixels once — walls never change after generation
    if let Some(world_map) = world_map {
        let pixels = &mut image.data;
        for gy in 0..GRID_H {
            for gx in 0..GRID_W {
                let grid_i = gy * GRID_W + gx;
                let tex_row = GRID_H - 1 - gy; // Y-flip
                let base = (tex_row * GRID_W + gx) * 4;
                if world_map.walls[grid_i] {
                    pixels[base]     = 0;
                    pixels[base + 1] = 0;
                    pixels[base + 2] = 0;
                    pixels[base + 3] = 0;
                }
            }
        }
    }

    let texture_handle = images.add(image);

    commands.spawn((
        Sprite {
            image: texture_handle.clone(),
            custom_size: Some(Vec2::new(WINDOW_W, WINDOW_H)),
            ..default()
        },
        Transform::from_translation(Vec3::new(0.0, 0.0, 1.0)),
    ));

    commands.insert_resource(PheromoneOverlay {
        texture: texture_handle,
        visible: true,
    });
}
```

Also update the ordering in `RenderPlugin::build` to ensure `setup_pheromone_overlay` runs after `terrain_startup_system`:

```rust
app.add_systems(
    Startup,
    (
        setup_terrain_mesh.after(crate::terrain::terrain_startup_system),
        setup_pheromone_overlay.after(crate::terrain::terrain_startup_system),
    ),
)
```

- [ ] **Step 2: Skip wall cells in pheromone_texture_update_system**

In `src/pheromone.rs`, in `pheromone_texture_update_system`, change the inner `if overlay.visible` loop:

```rust
if overlay.visible {
    for gy in 0..GRID_H {
        for gx in 0..GRID_W {
            let grid_i = gy * GRID_W + gx;
            // Wall pixels are pre-filled at startup — skip them
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
}
```

Note: When overlay is toggled back on after being hidden, we need to re-initialize wall pixels since `fill(0)` clears them. Add a `walls_dirty` flag to `PheromoneOverlay`, or simply always write walls when toggling visible. The simplest fix: when overlay becomes visible again (via P key toggle), mark `grid.dirty = true` (already done in `pheromone_overlay_toggle_system`) AND also re-write wall pixels in the texture update. Track this with a flag on the overlay:

In `PheromoneOverlay` struct (in `src/pheromone.rs`):
```rust
pub struct PheromoneOverlay {
    pub texture: Handle<Image>,
    pub visible: bool,
    pub walls_drawn: bool,  // true once wall pixels have been written to texture
}
```

Initialize `walls_drawn: false` in `setup_pheromone_overlay`.

In `pheromone_texture_update_system`, after the visible branch, write walls whenever `!overlay.walls_drawn` (which happens after a hide→show toggle):

```rust
if overlay.visible {
    if !overlay.walls_drawn {
        // Re-draw wall pixels after a hide→show toggle (fill(0) erased them)
        for gy in 0..GRID_H {
            for gx in 0..GRID_W {
                let grid_i = gy * GRID_W + gx;
                if world_map.walls[grid_i] {
                    let tex_row = GRID_H - 1 - gy;
                    let base = (tex_row * GRID_W + gx) * 4;
                    pixels[base] = 0;
                    pixels[base + 1] = 0;
                    pixels[base + 2] = 0;
                    pixels[base + 3] = 0;
                }
            }
        }
        overlay.walls_drawn = true;  // requires overlay to be ResMut
    }
    // ... pheromone cell loop (skip walls) ...
} else {
    pixels.fill(0);
    overlay.walls_drawn = false;  // requires overlay to be ResMut
}
```

Update the system signature to use `ResMut<PheromoneOverlay>`.

- [ ] **Step 3: Build**

```bash
cargo build 2>&1 | tail -5
```

Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add src/render.rs src/pheromone.rs
git commit -m "perf: pre-fill wall pixels at startup, skip wall cells in texture update (#11, #18)"
```

---

## Task 6: Add sensor tie-break comment (#5)

**Files:**
- Modify: `src/ant.rs` — pheromone force block (~lines 212–219)

**Problem:** When `left == right` (both equal, both nonzero, both less than `ahead`), the ant always turns right. This is non-obvious and could cause subtle asymmetry.

- [ ] **Step 1: Add comment**

In `ant_behavior_system`, find the pheromone delta block:

```rust
let phero_delta: f32 = if ahead >= left && ahead >= right {
    0.0
} else if left > right {
    -SENSOR_ANGLE
} else {
    SENSOR_ANGLE
};
```

Replace with:

```rust
// Tie-break: when left == right (and both < ahead), always pick right (+SENSOR_ANGLE).
// This is an intentional asymmetry — ants will consistently resolve equal-signal
// situations by turning right. Acceptable for simulation purposes.
let phero_delta: f32 = if ahead >= left && ahead >= right {
    0.0
} else if left > right {
    -SENSOR_ANGLE
} else {
    SENSOR_ANGLE
};
```

- [ ] **Step 2: Build**

```bash
cargo build 2>&1 | tail -5
```

- [ ] **Step 3: Commit**

```bash
git add src/ant.rs
git commit -m "docs: note sensor tie-break behavior in ant steering (#5)"
```

---

## Task 7: Promote remaining magic numbers to config constants (#30)

**Files:**
- Modify: `src/config.rs` — add constants
- Modify: `src/ant.rs` — replace magic numbers

**Remaining magic numbers to extract:**
- `r * 3.0` probe distance multiplier in `handle_collision` (`ant.rs:127`) → `ANT_PROBE_DIST_MULT: f32 = 3.0`
- `0.01` low-signal threshold in `ant_behavior_system` (`ant.rs:240`) for returning-ant nest bias → `ANT_NEST_SEEK_SIGNAL_THRESHOLD: f32 = 0.01`
- `1e-6` direction length-squared check in `score_sensor` (`ant.rs:68`) → `DIRECTION_ZERO_THRESHOLD: f32 = 1e-6`
- `1.0` noise half-range in the `attempt == 0` bounce case (`ant.rs:134`) → `ANT_WALL_BOUNCE_NOISE: f32 = 1.0`

- [ ] **Step 1: Add constants to config.rs**

Append to `src/config.rs`:

```rust
// Ant wall collision probe
pub const ANT_PROBE_DIST_MULT: f32 = 3.0;   // probe_dist = collision_radius * this
pub const ANT_WALL_BOUNCE_NOISE: f32 = 1.0;  // half-range of angle noise on first bounce attempt

// Pheromone thresholds
pub const DIRECTION_ZERO_THRESHOLD: f32 = 1e-6;     // min length_squared to treat direction as valid
pub const ANT_NEST_SEEK_SIGNAL_THRESHOLD: f32 = 0.01; // signal below this triggers nest-direction bias
```

- [ ] **Step 2: Replace magic numbers in ant.rs**

In `handle_collision` (~line 127):
```rust
let probe_dist = r * ANT_PROBE_DIST_MULT;
```

In `handle_collision` (~line 134), replace `* 1.0`:
```rust
*angle + PI + (rng.gen::<f32>() - 0.5) * ANT_WALL_BOUNCE_NOISE
```

In `score_sensor` (~line 68), replace `1e-6`:
```rust
if trail_dir.length_squared() < DIRECTION_ZERO_THRESHOLD {
```

In `ant_behavior_system` (~line 240), replace `0.01`:
```rust
if total_signal < ANT_NEST_SEEK_SIGNAL_THRESHOLD {
```

- [ ] **Step 3: Build**

```bash
cargo build 2>&1 | tail -5
```

- [ ] **Step 4: Commit**

```bash
git add src/config.rs src/ant.rs
git commit -m "refactor: promote remaining magic numbers to named config constants (#30)"
```

---

## Task 8: Fix BFS boundary — use explicit bounds check (#29)

**Files:**
- Modify: `src/terrain.rs` — BFS flood-fill loop (~lines 218–232)

**Problem:** The BFS uses `saturating_sub` and `.min(GRID_W - 1)`, which means the boundary cell itself can be re-pushed as its own neighbor (e.g., `ix = 0`, `saturating_sub(1) = 0`). The wall check prevents actual incorrectness but wastes queue operations.

- [ ] **Step 1: Replace BFS neighbor enumeration**

Find the BFS inner loop in `generate_terrain`:

```rust
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
```

Replace with explicit bounds checks that skip the current cell if at a boundary:

```rust
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
```

- [ ] **Step 2: Build**

```bash
cargo build 2>&1 | tail -5
```

- [ ] **Step 3: Commit**

```bash
git add src/terrain.rs
git commit -m "fix: use explicit bounds in BFS to avoid re-pushing boundary cells (#29)"
```

---

## Task 9: Add bounds-uniformity comment to deposit (#1)

**Files:**
- Modify: `src/pheromone.rs` — `deposit` function (~lines 48–69)

**Problem:** The bounds check `if idx >= intensity.len()` only checks the first Vec. All 6 Vecs are always the same length (initialized in `new()`) so this is safe, but it's non-obvious.

- [ ] **Step 1: Add explanatory comment**

In `PheromoneGrid::deposit`, above the bounds check:

```rust
// All six Vec fields are initialized to the same length in `new()`.
// Checking intensity.len() is sufficient to guard all of them.
if idx >= intensity.len() {
    return;
}
```

- [ ] **Step 2: Build and commit**

```bash
cargo build 2>&1 | tail -5
git add src/pheromone.rs
git commit -m "docs: clarify deposit bounds check covers all channel vecs equally (#1)"
```

---

## Final: Build both targets

- [ ] **Build Linux release**

```bash
cargo build --release 2>&1 | tail -10
```

- [ ] **Build Windows release**

```bash
cargo build --release --target x86_64-pc-windows-gnu 2>&1 | tail -10
```

Expected: both succeed with no errors.

---

## Review Coverage Summary

| # | Description | Status |
|---|-------------|--------|
| 1 | Deposit bounds fragility | Task 9 |
| 2 | Repeated distance sqrt in seek | Already fixed |
| 3 | Dead `placed` variable | Already fixed |
| 4 | sqrt in food placement dist check | Already fixed |
| 5 | Sensor tie-break comment | Task 6 |
| 6 | SmallRng in handle_collision | Already fixed |
| 7 | Ant asset leak | Already fixed |
| 8 | Food asset leak | Already fixed |
| 9 | Pheromone diffusion Vec clone + direction drift | Task 1 |
| 10 | Decay skips walls | Already fixed |
| 11 | Texture skips walls | Task 5 |
| 12 | Wall collision ordering | Acceptable (already correct per review) |
| 13 | 1-cell clearance fallback | Already correct (guarded by is_empty check) |
| 14 | O(N×M) seek scan | Acceptable at current scale |
| 15 | FoodPositions rebuild | Minor; acceptable |
| 16 | from_entropy per instance | Already fixed |
| 17 | visible check out of pixel loop | Already fixed |
| 18 | Wall check per pixel | Task 5 |
| 19 | Text reallocation in score UI | Task 3 |
| 20 | SmallRng in 8-attempt loop | Already fixed |
| 21 | angle_diff while loops | Task 2 |
| 22 | sqrt in center exclusion | Already fixed |
| 23 | HashSet → bool vec | Already fixed |
| 24 | System ordering | Already fixed |
| 25 | Mesh/material pervasive caching | Already fixed |
| 26 | Unused SeedableRng import | Used (from_entropy) |
| 27 | HashSet import | Already removed |
| 28 | decay_channel helper | Task 4 |
| 29 | BFS boundary revisit | Task 8 |
| 30 | Magic numbers → config | Task 7 |
| 31 | No tests | Skipped (per CLAUDE.md) |
