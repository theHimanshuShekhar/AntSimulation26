use bevy::prelude::*;
use crate::config::*;

/// Runtime-tunable simulation parameters, initialized from compile-time constants.
/// All systems read from this resource instead of bare constants so that the egui
/// sidebar can stage changes and apply them on restart.
#[derive(Resource)]
pub struct SimConfig {
    // Colony
    pub ant_count: usize,
    pub ant_speed: f32,
    pub ant_lifetime_min: f32,
    pub ant_lifetime_max: f32,
    pub ant_respawn_interval: f32,
    pub ant_respawn_batch: usize,

    // Food
    pub food_source_count: usize,
    pub food_per_source: u32,
    pub food_respawns: bool,
    pub food_respawn_delay: f32,
    pub food_cluster_size: usize,
    pub food_cluster_radius: f32,

    // Pheromones
    pub deposit_strength: f32,
    pub decay_factor: f32,
    pub decay_interval: f32,
    pub diffusion_enabled: bool,

    // Terrain
    pub terrain_iso_level: f32,
    pub fbm_layers: usize,
    pub fbm_scale: f32,
    pub cave_center_exclusion: usize,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            ant_count: ANT_COUNT,
            ant_speed: ANT_SPEED,
            ant_lifetime_min: ANT_LIFETIME_MIN,
            ant_lifetime_max: ANT_LIFETIME_MAX,
            ant_respawn_interval: ANT_RESPAWN_INTERVAL,
            ant_respawn_batch: ANT_RESPAWN_BATCH,
            food_source_count: FOOD_SOURCE_COUNT,
            food_per_source: FOOD_PER_SOURCE,
            food_respawns: true,
            food_respawn_delay: FOOD_RESPAWN_DELAY,
            food_cluster_size: FOOD_CLUSTER_SIZE,
            food_cluster_radius: FOOD_CLUSTER_RADIUS,
            deposit_strength: DEPOSIT_STRENGTH,
            decay_factor: DECAY_FACTOR,
            decay_interval: DECAY_INTERVAL,
            diffusion_enabled: DIFFUSION_ENABLED,
            terrain_iso_level: TERRAIN_ISO_LEVEL,
            fbm_layers: FBM_LAYERS,
            fbm_scale: FBM_SCALE,
            cave_center_exclusion: CAVE_CENTER_EXCLUSION,
        }
    }
}
