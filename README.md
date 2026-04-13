# Ant Simulation

A real-time ant colony simulation built with [Bevy 0.15](https://bevyengine.org/). Ants navigate a procedurally generated cave environment, forage for food, and communicate through two-channel pheromone trails — all rendered at 60+ FPS with 2000 simultaneous agents.

## Features

- **Procedural cave terrain** — FBM noise + marching squares mesh generation with a guaranteed open area at the center for the nest
- **Two-channel pheromone system** — separate Home and Food pheromone grids with intensity + direction encoding, decay, and diffusion
- **Composable ant steering** — weighted combination of pheromone following, random wander, and active food/nest seek forces
- **Ant lifecycle** — each ant has a randomized lifetime; dead ants are replaced in batches by a colony respawn system
- **Food clustering** — food sources spawn in spatial clusters at a minimum distance from the nest and respawn after collection
- **Live HUD** — food collected counter, active/total ant count, and FPS display
- **Runtime configuration sidebar** — egui panel for live parameter tuning across Colony, Food, Pheromones, and Terrain; restart button applies terrain/colony changes cleanly

## Download

Pre-built binaries for Linux and Windows are published automatically on every release — grab the latest from the [Releases](../../releases/latest) page.

## Getting Started

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (stable)

### Run (development)

```bash
cargo run
```

The `dev` feature enables dynamic linking and hot-reload for faster iteration:

```bash
cargo run --features dev
```

### Build (release)

```bash
# Linux
cargo build --release
# Output: target/release/ant_simulation

# Windows (requires gcc-mingw-w64-x86-64 and the windows-gnu target)
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
# Output: target/x86_64-pc-windows-gnu/release/ant_simulation.exe

# Both at once
make all
```

CI builds and publishes both binaries automatically on every push to `main`. Releases follow [Conventional Commits](https://www.conventionalcommits.org/): `fix:` bumps patch, `feat:` bumps minor, `BREAKING CHANGE:` bumps major.

## Controls

| Key | Action |
|-----|--------|
| **P** | Toggle pheromone overlay |

The configuration sidebar is always visible on the right side of the window.

## Architecture

Built on Bevy's ECS pattern. All compile-time simulation parameters are in `src/config.rs`; runtime-tunable parameters live in the `SimConfig` resource (`src/sim_config.rs`).

| Module | Responsibility |
|--------|----------------|
| `terrain.rs` | FBM + marching squares cave generation; `WorldMap` resource |
| `pheromone.rs` | Two-channel grid (Home/Food) with intensity + direction, decay, diffusion, texture upload |
| `ant.rs` | Per-frame steering, sensor sampling, seek forces, stuck detection, deposit buffering, lifetime |
| `food.rs` | Food/nest interaction, score tracking, respawn timers |
| `render.rs` | Pheromone overlay texture; P-key toggle |
| `sim_config.rs` | `SimConfig` resource; runtime parameters surfaced in the sidebar |
| `ui.rs` | egui sidebar; Colony, Food, Pheromones, Terrain sections; restart trigger |
| `noise.rs` | FBM noise primitives |
| `config.rs` | All compile-time constants |

### Pheromone Overlay Encoding

The overlay texture is `RGBA8`:

| Channel | Meaning |
|---------|---------|
| R | Unused |
| G | Food pheromone intensity |
| B | Home pheromone intensity |
| A | `max(food, home)` — controls visibility |

Walls render as dark grey `(60, 60, 60, 255)`. Each channel also stores an accumulated direction vector (`*_dir_x`, `*_dir_y`) used by ant sensors to score trail alignment and prevent 180° reversals.

### Ant Behavior Loop (per frame)

1. Sample three pheromone sensors: left / ahead / right at `SENSOR_DIST` pixels, `SENSOR_ANGLE` radians spread
2. Score each sensor by intensity × directional alignment — prevents 180° reversals
3. Follow the channel matching current state (`Searching` → Food trail, `Returning` → Home trail)
4. Add gaussian wander noise; fall back to random walk when signal is absent
5. Apply active seek force toward nearest food (within `SEEK_RADIUS`) when `Searching`, or toward nest when `Returning` with weak Home signal
6. Move forward; bounce off walls and world boundaries
7. Stage a pheromone deposit (flushed each frame by `ant_deposit_flush_system`); teleport to nest if stuck for `ANT_STUCK_THRESHOLD` frames

### Key Tuning Constants (`src/config.rs`)

| Constant | Default | Description |
|----------|---------|-------------|
| `ANT_COUNT` | 2000 | Total colony size |
| `ANT_SPEED` | 80 px/s | Movement speed |
| `ANT_LIFETIME_MIN/MAX` | 30–90 s | Ant lifespan range |
| `ANT_RESPAWN_BATCH` | 20 | Ants respawned per tick |
| `ANT_STUCK_THRESHOLD` | 10 frames | Frames before stuck ant teleports to nest |
| `SENSOR_DIST` | 40 px | How far ahead sensors are placed |
| `SENSOR_ANGLE` | 0.52 rad | Spread between left/right sensors |
| `SEEK_RADIUS` | 60 px | Radius for active food/nest seek force |
| `PHEROMONE_FOLLOW_WEIGHT` | 8.0 | Signal → follow probability scale |
| `DEPOSIT_STRENGTH` | 0.3 | Amount deposited per tick |
| `DECAY_FACTOR` | 0.97 | Per-tick pheromone decay multiplier |
| `DIFFUSION_ENABLED` | true | Box-blur diffusion after decay |
| `FOOD_SOURCE_COUNT` | 4 | Number of food cluster centers |
| `FOOD_CLUSTER_SIZE` | 8 | Food items per cluster |
| `FOOD_RESPAWN_DELAY` | 5.0 s | Delay before a depleted source respawns |

## License

MIT
