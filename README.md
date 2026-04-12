# Ant Simulation

A real-time ant colony simulation built with [Bevy 0.15](https://bevyengine.org/). Ants navigate a procedurally generated cave environment, forage for food, and communicate through two-channel pheromone trails — all rendered at 60+ FPS with 2000 simultaneous agents.

## Features

- **Procedural cave terrain** — FBM noise + marching squares mesh generation with a guaranteed open area at the center for the nest
- **Two-channel pheromone system** — separate Home and Food pheromone grids with decay, diffusion, and directional trail encoding
- **Composable ant steering** — weighted combination of pheromone following, random wander, and food seek forces
- **Ant lifecycle** — each ant has a randomized lifetime; dead ants are replaced by the colony respawn system
- **Food clustering** — food sources spawn in spatial clusters at a minimum distance from the nest and respawn after collection
- **Live HUD** — food collected counter, active/total ant count, and FPS display

## Demo

> _Run the simulation and press **P** to toggle the pheromone overlay._

## Getting Started

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (stable)
- For Windows cross-compilation: `x86_64-w64-mingw32-gcc` and the `x86_64-pc-windows-gnu` Rust target

```bash
rustup target add x86_64-pc-windows-gnu
```

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

# Windows
cargo build --release --target x86_64-pc-windows-gnu
# Output: target/x86_64-pc-windows-gnu/release/ant_simulation.exe

# Both at once
make all
```

## Controls

| Key | Action |
|-----|--------|
| **P** | Toggle pheromone overlay |

## Architecture

Built on Bevy's ECS pattern. All simulation parameters are compile-time constants in `src/config.rs`.

| Module | Responsibility |
|--------|----------------|
| `terrain.rs` | FBM + marching squares cave generation; `WorldMap` resource |
| `pheromone.rs` | Two-channel grid (Home/Food), decay, diffusion, texture upload |
| `ant.rs` | Per-frame steering, sensor sampling, deposit buffering, lifetime |
| `food.rs` | Food/nest interaction, score tracking, respawn timers |
| `render.rs` | Pheromone overlay texture; P-key toggle |
| `noise.rs` | FBM noise primitives |
| `config.rs` | All tunable constants |

### Pheromone Overlay Encoding

The overlay texture is `RGBA8`:

| Channel | Meaning |
|---------|---------|
| R | Unused |
| G | Food pheromone intensity |
| B | Home pheromone intensity |
| A | `max(food, home)` — controls visibility |

Walls render as dark grey `(60, 60, 60, 255)`.

### Ant Behavior Loop (per frame)

1. Sample three pheromone sensors: left / ahead / right at `SENSOR_DIST` pixels, `SENSOR_ANGLE` radians spread
2. Follow the channel matching current state (`Searching` → Food trail, `Returning` → Home trail)
3. Score each sensor by intensity × directional alignment — prevents 180° reversals
4. Add gaussian wander noise; fall back to random walk when signal is absent
5. Apply gentle nest-direction bias when `Returning` with no Home signal
6. Move forward; bounce off walls and world boundaries
7. Stage a pheromone deposit (flushed each frame by `ant_deposit_flush_system`)

### Key Tuning Constants (`src/config.rs`)

| Constant | Default | Description |
|----------|---------|-------------|
| `ANT_COUNT` | 2000 | Total colony size |
| `ANT_SPEED` | 80 px/s | Movement speed |
| `SENSOR_DIST` | 40 px | How far ahead sensors are placed |
| `SENSOR_ANGLE` | 0.52 rad | Spread between left/right sensors |
| `PHEROMONE_FOLLOW_WEIGHT` | 8.0 | Signal → follow probability scale |
| `DEPOSIT_STRENGTH` | 0.3 | Amount deposited per tick |
| `DECAY_FACTOR` | 0.97 | Per-tick pheromone decay multiplier |
| `DIFFUSION_ENABLED` | true | Box-blur diffusion after decay |
| `FOOD_SOURCE_COUNT` | 4 | Number of food cluster centers |
| `FOOD_CLUSTER_SIZE` | 8 | Food items per cluster |
| `ANT_LIFETIME_MIN/MAX` | 30–90 s | Ant lifespan range |

## License

MIT
