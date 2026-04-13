// ── Window ─────────────────────────────────────────────────────────────────
pub const WINDOW_W: f32 = 1280.0;
pub const WINDOW_H: f32 = 720.0;

// ── Grid ───────────────────────────────────────────────────────────────────
pub const GRID_W: usize = 512;
pub const GRID_H: usize = 288;

// ── Ants — population ──────────────────────────────────────────────────────
pub const ANT_COUNT: usize = 2000;

// ── Ants — lifetime ────────────────────────────────────────────────────────
pub const ANT_LIFETIME_MIN: f32 = 30.0;
pub const ANT_LIFETIME_MAX: f32 = 90.0;

// ── Ants — movement ────────────────────────────────────────────────────────
pub const ANT_SPEED: f32 = 80.0;          // pixels per second

// ── Ants — sensors ─────────────────────────────────────────────────────────
pub const SENSOR_ANGLE: f32 = 0.52;       // radians ~30° sensor spread
pub const SENSOR_DIST: f32 = 40.0;        // pixels ahead sensors are placed
pub const SENSOR_MIN_ALIGNMENT: f32 = 0.2;   // floor alignment contribution (prevents full suppression)

// ── Ants — steering forces ─────────────────────────────────────────────────
pub const WANDER_WEIGHT: f32 = 1.0;
pub const PHEROMONE_WEIGHT: f32 = 2.5;
pub const SEEK_WEIGHT: f32 = 1.2;
pub const SEEK_RADIUS: f32 = 60.0;
pub const PHEROMONE_FOLLOW_WEIGHT: f32 = 8.0; // scales signal strength → follow probability (signal * weight clamped to 1.0)
pub const PHEROMONE_FOLLOW_MAX: f32 = 0.85;  // max fraction of wander suppressed by signal
pub const BASE_NOISE_FRACTION: f32 = 0.15;   // base noise scalar relative to wander weight
pub const ANT_TURN_NOISE: f32 = 0.26;     // radians ~15° gaussian noise per frame

// ── Ants — collision / boundary ────────────────────────────────────────────
pub const ANT_COLLISION_RADIUS: f32 = 2.0;   // footprint probe radius in pixels
pub const ANT_BOUNDARY_MARGIN: f32 = 5.0;    // world-edge buffer in pixels
pub const ANT_PROBE_DIST_MULT: f32 = 3.0;    // probe_dist = collision_radius * this
pub const ANT_WALL_BOUNCE_NOISE: f32 = 1.0;  // half-range of angle noise on first bounce attempt
pub const ANT_STUCK_THRESHOLD: u8 = 10;      // frames stuck before teleporting to nest

// ── Pheromones ─────────────────────────────────────────────────────────────
pub const DEPOSIT_STRENGTH: f32 = 0.3;    // amount deposited per tick, clamped to 1.0
pub const DECAY_FACTOR: f32 = 0.97;       // multiplied each decay tick
pub const DECAY_INTERVAL: f32 = 0.25;     // seconds between decay ticks
pub const DIFFUSION_ENABLED: bool = true;  // whether to apply box-blur diffusion after decay
pub const DIFFUSION_SELF_WEIGHT: f32 = 0.6;      // current cell weight in box-blur
pub const DIFFUSION_NEIGHBOR_WEIGHT: f32 = 0.4;  // neighbor average weight in box-blur
pub const PHEROMONE_ZERO_THRESHOLD: f32 = 0.001; // values below this snap to 0.0
pub const DIRECTION_ZERO_THRESHOLD: f32 = 1e-6;       // min length_squared to treat direction as valid
pub const ANT_NEST_SEEK_SIGNAL_THRESHOLD: f32 = 0.01; // signal below this triggers nest-direction bias

// ── Food ───────────────────────────────────────────────────────────────────
pub const FOOD_PER_SOURCE: u32 = 50;
pub const FOOD_SOURCE_COUNT: usize = 4;
pub const FOOD_INTERACTION_RADIUS: f32 = 8.0;  // pixels
pub const NEST_INTERACTION_RADIUS: f32 = 20.0; // pixels
pub const FOOD_RESPAWN_DELAY: f32 = 5.0;       // seconds
pub const FOOD_CLUSTER_SIZE: usize = 8;
pub const FOOD_CLUSTER_RADIUS: f32 = 20.0;
pub const FOOD_MIN_NEST_DIST_CELLS: usize = 30;

// ── Terrain ────────────────────────────────────────────────────────────────
pub const CAVE_BORDER_THICKNESS: usize = 4; // grid cells thick on every edge (visible rock border)
pub const CAVE_CENTER_EXCLUSION: usize = 35; // grid-cell radius around center always kept open
pub const FBM_LAYERS: usize = 6;
pub const FBM_SCALE: f32 = 3.5;
pub const FBM_LACUNARITY: f32 = 2.0;
pub const FBM_PERSISTENCE: f32 = 0.5;
pub const TERRAIN_ISO_LEVEL: f32 = 0.52;
