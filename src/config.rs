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
pub const PHEROMONE_FOLLOW_WEIGHT: f32 = 8.0; // scales signal strength → follow probability (signal * weight clamped to 1.0)
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

// Cave / terrain shared
pub const CAVE_BORDER_THICKNESS: usize = 4; // grid cells thick on every edge (visible rock border)
pub const CAVE_CENTER_EXCLUSION: usize = 35; // grid-cell radius around center always kept open

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

// Minimum grid-cell distance between food spawns and the nest
pub const FOOD_MIN_NEST_DIST_CELLS: usize = 30;
