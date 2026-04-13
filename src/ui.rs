use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use crate::sim_config::SimConfig;
use crate::AppState;
use crate::ant::Colony;
use crate::food::FoodScore;
use crate::SimStats;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin)
           .add_systems(Update, sidebar_system)
           .add_systems(Update, game_over_ui_system.run_if(in_state(AppState::GameOver)));
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

fn game_over_ui_system(
    mut contexts: EguiContexts,
    mut next_state: ResMut<NextState<AppState>>,
    colony: Res<Colony>,
    score: Res<FoodScore>,
    stats: Res<SimStats>,
) {
    let total_secs = stats.elapsed_secs as u32;
    let minutes = total_secs / 60;
    let seconds = total_secs % 60;
    let survival_time = format!("{:02}:{:02}", minutes, seconds);

    let ctx = contexts.ctx_mut();
    let screen = ctx.screen_rect();

    egui::Area::new(egui::Id::new("game_over_overlay"))
        .fixed_pos(egui::pos2(0.0, 0.0))
        .order(egui::Order::Background)
        .show(ctx, |ui| {
            ui.painter().rect_filled(
                egui::Rect::from_min_size(egui::pos2(0.0, 0.0), screen.size()),
                0.0,
                egui::Color32::from_black_alpha(180),
            );
        });

    egui::Area::new(egui::Id::new("game_over_panel"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(20, 10, 5))
                .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(180, 80, 20)))
                .inner_margin(egui::Margin::symmetric(40_i8, 30_i8))
                .corner_radius(8.0)
                .show(ui, |ui| {
                    ui.set_min_width(320.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("GAME OVER")
                                .size(52.0)
                                .color(egui::Color32::from_rgb(220, 60, 40))
                                .strong(),
                        );
                        ui.label(
                            egui::RichText::new("The colony has perished.")
                                .size(15.0)
                                .color(egui::Color32::from_rgb(160, 140, 120)),
                        );
                        ui.add_space(20.0);

                        // Stats grid
                        egui::Grid::new("stats_grid")
                            .num_columns(2)
                            .spacing([24.0, 8.0])
                            .show(ui, |ui| {
                                let label = |text: &str| {
                                    egui::RichText::new(text)
                                        .size(14.0)
                                        .color(egui::Color32::from_rgb(160, 140, 120))
                                };
                                let value = |text: String, color: egui::Color32| {
                                    egui::RichText::new(text).size(14.0).color(color).strong()
                                };

                                ui.label(label("Survival time"));
                                ui.label(value(survival_time, egui::Color32::from_rgb(220, 200, 100)));
                                ui.end_row();

                                ui.label(label("Peak population"));
                                ui.label(value(
                                    stats.peak_population.to_string(),
                                    egui::Color32::from_rgb(100, 180, 255),
                                ));
                                ui.end_row();

                                ui.label(label("Total ants born"));
                                ui.label(value(
                                    colony.total_died.to_string(),
                                    egui::Color32::from_rgb(160, 140, 120),
                                ));
                                ui.end_row();

                                ui.label(label("Food delivered"));
                                ui.label(value(
                                    score.collected.to_string(),
                                    egui::Color32::from_rgb(100, 220, 100),
                                ));
                                ui.end_row();

                                ui.label(label("Ants born per food"));
                                let ratio = if score.collected > 0 {
                                    format!("{:.1}", colony.total_died as f32 / score.collected as f32)
                                } else {
                                    "—".to_string()
                                };
                                ui.label(value(ratio, egui::Color32::from_rgb(200, 160, 100)));
                                ui.end_row();
                            });

                        ui.add_space(24.0);
                        let btn = egui::Button::new(
                            egui::RichText::new("  Restart  ").size(18.0).strong(),
                        )
                        .fill(egui::Color32::from_rgb(140, 50, 20))
                        .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgb(220, 100, 60)));
                        if ui.add(btn).clicked() {
                            next_state.set(AppState::Restarting);
                        }
                    });
                });
        });
}
