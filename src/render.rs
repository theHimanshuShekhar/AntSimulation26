use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use crate::config::*;
use crate::pheromone::{PheromoneGrid, PheromoneOverlay};
use crate::terrain::{WorldMap, marching_squares_mesh};

/// Startup system: build the marching squares terrain mesh and spawn it at z=0.
/// Must run after terrain_startup_system so WorldMap is available.
pub fn setup_terrain_mesh(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    world_map: Res<WorldMap>,
) {
    let mesh = marching_squares_mesh(&world_map.density);
    commands.spawn((
        Mesh2d(meshes.add(mesh)),
        MeshMaterial2d(materials.add(ColorMaterial::from(Color::srgb(0.25, 0.22, 0.18)))),
        Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)), // z=0: terrain is bottommost
    ));
}

/// Startup system: create the pheromone overlay texture and spawn it at z=1 (above terrain).
/// Must run after terrain_startup_system so WorldMap is available for wall pre-fill.
pub fn setup_pheromone_overlay(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    world_map: Res<WorldMap>,
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

    // Wall pixels never change — pre-fill them once here so the per-frame texture
    // update loop can skip wall cells entirely. Walls are transparent (0,0,0,0)
    // which is already the fill value, so this is a no-op for the data but
    // documents intent and allows future wall color changes in one place.
    let pixels = &mut image.data;
    for gy in 0..GRID_H {
        for gx in 0..GRID_W {
            let grid_i = gy * GRID_W + gx;
            if world_map.walls[grid_i] {
                let tex_row = GRID_H - 1 - gy; // Y-flip (matches texture update convention)
                let base = (tex_row * GRID_W + gx) * 4;
                pixels[base]     = 0;
                pixels[base + 1] = 0;
                pixels[base + 2] = 0;
                pixels[base + 3] = 0;
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
        Transform::from_translation(Vec3::new(0.0, 0.0, 1.0)), // z=1: above terrain mesh
    ));

    commands.insert_resource(PheromoneOverlay {
        texture: texture_handle,
        visible: true,
        walls_drawn: true,
    });
}

/// Input system: toggle pheromone overlay on P key press.
pub fn pheromone_overlay_toggle_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut overlay: ResMut<PheromoneOverlay>,
    mut grid: ResMut<PheromoneGrid>,
) {
    if keyboard.just_pressed(KeyCode::KeyP) {
        overlay.visible = !overlay.visible;
        grid.dirty = true;
    }
}

/// Plugin that registers rendering systems.
pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(
                Startup,
                (
                    setup_terrain_mesh.after(crate::terrain::terrain_startup_system),
                    setup_pheromone_overlay.after(crate::terrain::terrain_startup_system),
                ),
            )
            .add_systems(Update, pheromone_overlay_toggle_system);
    }
}
