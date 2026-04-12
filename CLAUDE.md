# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Run in development mode (faster compile, optimized deps)
cargo run

# Build release binary for Linux
cargo build --release
# Output: target/release/ant_simulation

# Build release binary for Windows (requires x86_64-w64-mingw32-gcc)
cargo build --release --target x86_64-pc-windows-gnu
# Output: target/x86_64-pc-windows-gnu/release/ant_simulation.exe

# Build both targets
make all

# Clean build artifacts
make clean
```

There are no tests in this project.

## Architecture

This is a Bevy 0.15 simulation using the ECS (Entity-Component-System) pattern. All simulation parameters live in `src/config.rs` as compile-time constants — this is the first place to look when tuning behavior.

### Plugin Structure

Each module exposes a Bevy `Plugin` that is registered in `main.rs`:

| Plugin | Responsibility |
|--------|---------------|
| `WorldPlugin` | Cave generation on startup via cellular automata |
| `PheromonePlugin` | Pheromone grid decay, diffusion, and texture upload |
| `AntPlugin` | Per-frame ant steering, movement, and deposit buffering |
| `FoodPlugin` | Food/nest interaction and food respawn timers |
| `RenderPlugin` | Pheromone overlay texture creation and P-key toggle |

### Key Resources

- `WorldMap` — flat `Vec<bool>` wall grid + list of valid spawn cells (with ≥2-cell clearance from walls)
- `PheromoneGrid` — two `Vec<f32>` channels (`home`, `food`), 0.0–1.0, with a `dirty` flag gating texture uploads
- `PheromoneOverlay` — holds the `Handle<Image>` for the fullscreen pheromone sprite
- `AntDeposits` — staging buffer so ant behavior (parallel) writes deposits without directly mutating `PheromoneGrid`; flushed each frame by `ant_deposit_flush_system`
- `NestPosition` — resource storing nest `Vec2`; ants use `(0,0)` as a fallback home bias when pheromone signal is absent

### Coordinate System

- World space: origin at center, extends ±640 (x) and ±360 (y)
- Grid space: `GRID_W × GRID_H` (256×144), flat row-major index `y * GRID_W + x`
- Conversion helpers: `grid_to_world` / `world_to_grid` in `src/world.rs`

### Z-Layering

- z=0: pheromone overlay texture (fullscreen sprite)
- z=1: ants
- z=2: food sources and nest

### Ant Behavior Loop (per frame, `ant_behavior_system`)

1. Sample 3 pheromone sensors (left/ahead/right at `SENSOR_DIST` pixels, `SENSOR_ANGLE` radians spread)
2. Follow the pheromone type matching state: `Searching` follows Food trail, `Returning` follows Home trail
3. Add 10% sensor noise, steer toward strongest signal; fall back to gaussian random walk
4. Gentle nest-direction bias when `Returning` with no home signal
5. Move forward, bounce off walls and world boundaries
6. Stage a pheromone deposit (`Home` when searching, `Food` when returning)

### Pheromone Texture Encoding

The overlay texture is `RGBA8`: R=unused, G=food intensity, B=home intensity, A=max(food,home). Walls render as dark grey (60,60,60,255). Toggle visibility with **P**.
