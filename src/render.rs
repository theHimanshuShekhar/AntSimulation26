use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use crate::config::*;
use crate::pheromone::{PheromoneGrid, PheromoneOverlay};

/// Startup system: create the pheromone texture and spawn a full-screen sprite to display it
pub fn setup_pheromone_overlay(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
) {
    // Create a blank RGBA texture of GRID_W x GRID_H
    let size = Extent3d {
        width: GRID_W as u32,
        height: GRID_H as u32,
        depth_or_array_layers: 1,
    };
    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 0], // RGBA: transparent black
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    // Make texture nearest-neighbor (no blurring of pixel grid)
    image.sampler = ImageSampler::nearest();

    let texture_handle = images.add(image);

    // Spawn a full-screen sprite at z=0 (behind ants at z=1)
    commands.spawn((
        Sprite {
            image: texture_handle.clone(),
            custom_size: Some(Vec2::new(WINDOW_W, WINDOW_H)), // stretch to fill window
            ..default()
        },
        Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
    ));

    // Insert PheromoneOverlay resource so pheromone_texture_update_system can use it
    commands.insert_resource(PheromoneOverlay {
        texture: texture_handle,
        visible: true, // start with overlay visible
    });
}

/// Input system: toggle pheromone overlay on P key press
pub fn pheromone_overlay_toggle_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut overlay: ResMut<PheromoneOverlay>,
    mut grid: ResMut<PheromoneGrid>,
) {
    if keyboard.just_pressed(KeyCode::KeyP) {
        overlay.visible = !overlay.visible;
        grid.dirty = true; // force texture refresh
    }
}

/// Plugin that registers rendering systems
pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Startup, setup_pheromone_overlay)
            .add_systems(Update, pheromone_overlay_toggle_system);
    }
}
