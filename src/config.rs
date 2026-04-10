// Window
pub const WINDOW_W: f32 = 1280.0;
pub const WINDOW_H: f32 = 720.0;

// Pheromone grid (256x144 cells, ~5px per cell)
pub const GRID_W: usize = 256;
pub const GRID_H: usize = 144;

// Ants
pub const ANT_COUNT: usize = 2000;
pub const ANT_SPEED: f32 = 80.0;          // pixels per second
pub const ANT_TURN_NOISE: f32 = 0.26;     // radians ~15° gaussian noise per frame
pub const SENSOR_ANGLE: f32 = 0.52;       // radians ~30° sensor spread
pub const SENSOR_DIST: f32 = 40.0;        // pixels ahead sensors are placed

// Pheromones
pub const DEPOSIT_STRENGTH: f32 = 0.3;    // amount deposited per tick, clamped to 1.0
pub const DECAY_FACTOR: f32 = 0.97;       // multiplied each decay tick
pub const DECAY_INTERVAL: f32 = 0.25;     // seconds between decay ticks
pub const DIFFUSION_ENABLED: bool = true;  // whether to apply box-blur diffusion after decay

// Food
pub const FOOD_PER_SOURCE: u32 = 50;
pub const FOOD_SOURCE_COUNT: usize = 4;
pub const FOOD_INTERACTION_RADIUS: f32 = 8.0;  // pixels
pub const NEST_INTERACTION_RADIUS: f32 = 20.0; // pixels
pub const FOOD_RESPAWN_DELAY: f32 = 5.0;       // seconds

// Cave generation
pub const CAVE_INITIAL_WALL_CHANCE: f32 = 0.30; // probability cell starts as wall
pub const CAVE_SMOOTH_ITERATIONS: usize = 4;
pub const CAVE_BIRTH_LIMIT: usize = 5;    // >= N wall neighbors → become wall
pub const CAVE_DEATH_LIMIT: usize = 3;    // < N wall neighbors → become open
