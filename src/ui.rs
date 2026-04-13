use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use crate::sim_config::SimConfig;
use crate::AppState;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin)
           .add_systems(Update, sidebar_system);
    }
}

fn sidebar_system(
    mut contexts: EguiContexts,
    mut config: ResMut<SimConfig>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    egui::SidePanel::left("config_panel")
        .min_width(280.0)
        .show(contexts.ctx_mut(), |ui| {
            ui.heading("Ant Simulation");
            ui.separator();

            egui::CollapsingHeader::new("Colony")
                .default_open(true)
                .show(ui, |ui| {
                    ui.add(egui::Slider::new(&mut config.ant_count, 100_usize..=5000_usize).text("Ant Count"));
                    ui.add(egui::Slider::new(&mut config.ant_speed, 20.0_f32..=200.0_f32).text("Ant Speed"));
                    ui.add(egui::Slider::new(&mut config.ant_lifetime_min, 5.0_f32..=60.0_f32).text("Lifetime Min (s)"));
                    ui.add(egui::Slider::new(&mut config.ant_lifetime_max, 10.0_f32..=180.0_f32).text("Lifetime Max (s)"));
                    ui.add(egui::Slider::new(&mut config.ant_respawn_interval, 0.1_f32..=10.0_f32).text("Respawn Interval (s)"));
                    ui.add(egui::Slider::new(&mut config.ant_respawn_batch, 1_usize..=100_usize).text("Respawn Batch"));
                });

            ui.add_space(4.0);

            egui::CollapsingHeader::new("Food")
                .default_open(true)
                .show(ui, |ui| {
                    ui.add(egui::Slider::new(&mut config.food_source_count, 1_usize..=12_usize).text("Food Sources"));
                    ui.add(egui::Slider::new(&mut config.food_per_source, 10_u32..=200_u32).text("Food Per Source"));
                    ui.checkbox(&mut config.food_respawns, "Food Respawns");
                    ui.add_enabled(
                        config.food_respawns,
                        egui::Slider::new(&mut config.food_respawn_delay, 0.5_f32..=30.0_f32).text("Respawn Delay (s)"),
                    );
                    ui.add(egui::Slider::new(&mut config.food_cluster_size, 1_usize..=20_usize).text("Cluster Size"));
                    ui.add(egui::Slider::new(&mut config.food_cluster_radius, 5.0_f32..=60.0_f32).text("Cluster Radius (px)"));
                });

            ui.add_space(4.0);

            egui::CollapsingHeader::new("Pheromones")
                .default_open(true)
                .show(ui, |ui| {
                    ui.add(egui::Slider::new(&mut config.deposit_strength, 0.05_f32..=1.0_f32).text("Deposit Strength"));
                    ui.add(egui::Slider::new(&mut config.decay_factor, 0.90_f32..=0.999_f32).text("Decay Factor"));
                    ui.add(egui::Slider::new(&mut config.decay_interval, 0.05_f32..=2.0_f32).text("Decay Interval (s)"));
                    ui.checkbox(&mut config.diffusion_enabled, "Diffusion Enabled");
                });

            ui.add_space(4.0);

            egui::CollapsingHeader::new("Terrain")
                .default_open(false)
                .show(ui, |ui| {
                    ui.add(egui::Slider::new(&mut config.terrain_iso_level, 0.30_f32..=0.75_f32).text("Cave Density"));
                    ui.add(egui::Slider::new(&mut config.fbm_layers, 1_usize..=8_usize).text("FBM Layers"));
                    ui.add(egui::Slider::new(&mut config.fbm_scale, 1.0_f32..=8.0_f32).text("FBM Scale"));
                    ui.add(egui::Slider::new(&mut config.cave_center_exclusion, 10_usize..=60_usize).text("Center Exclusion"));
                });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            ui.vertical_centered(|ui| {
                if ui.button("  Restart Simulation  ").clicked() {
                    next_state.set(AppState::Restarting);
                }
            });

            ui.add_space(4.0);
            ui.label(egui::RichText::new("Changes apply on restart").weak().small());
        });
}
