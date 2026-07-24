// src/gui/scope.rs

use eframe::egui::{self, Color32, RichText, Stroke};
use std::time::{Duration, Instant};

use crate::gui::app::Ic7300App;
use crate::comm::Command;
use crate::gui::widgets::custom_3d_button_sized;

pub struct ScopeState {
    pub show_window: bool,
    pub enabled: bool,
    pub sweep_buffer: Vec<u8>,
    pub current_sweep: Vec<u8>,
    pub waterfall_image: egui::ColorImage,
    pub waterfall_texture: Option<egui::TextureHandle>,
    pub dirty: bool,
    pub center_frequency: u64,
    pub span: u32,
    pub last_texture_update: Instant,
    
    // Réglages de contraste et couleur
    pub waterfall_offset: f32,       // Seuil de coupure du bruit de fond (0.0 à 80.0)
    pub waterfall_gain: f32,         // Gain de contraste (0.5 à 3.0)
    pub waterfall_palette: u8,       // Index de la palette (0=SDR, 1=Ice, 2=Magma, 3=Grayscale)
    pub waterfall_history: Vec<Vec<u8>>, // Tampon d'historique pour recalculer la cascade en direct
}

impl ScopeState {
    pub fn new() -> Self {
        let width = 475; // Résolution par défaut (s'adaptera automatiquement)
        let height = 100; // Historique vertical (100 lignes)
        let pixels = vec![Color32::BLACK; width * height];
        
        Self {
            show_window: false,
            enabled: false,
            sweep_buffer: Vec::new(),
            current_sweep: vec![0; width],
            waterfall_image: egui::ColorImage {
                size: [width, height],
                pixels,
            },
            waterfall_texture: None,
            dirty: true,
            center_frequency: 14_074_000,
            span: 50_000, // ±25 kHz par défaut
            last_texture_update: Instant::now(),
            waterfall_offset: 15.0,
            waterfall_gain: 2.5,
            waterfall_palette: 0,
            waterfall_history: Vec::new(),
        }
    }

    /// Détecte dynamiquement la taille du balayage, adapte la texture et insère la nouvelle ligne
    pub fn push_sweep(&mut self, sweep: &[u8]) {
        if sweep.is_empty() { return; }
        
        let width = sweep.len(); // Détection dynamique de la largeur du spectre émis par la radio (ex: 375 ou 475)
        let height = self.waterfall_image.size[1];

        // Sécurité résiliente : Si la largeur émise par le poste diffère de celle en mémoire,
        // nous réallouons dynamiquement l'image et l'historique pour correspondre exactement à l'émetteur.
        if self.waterfall_image.size[0] != width {
            self.waterfall_image = egui::ColorImage {
                size: [width, height],
                pixels: vec![Color32::BLACK; width * height],
            };
            self.waterfall_history.clear();
        }

        self.current_sweep = sweep.to_vec();

        // Insérer la nouvelle ligne d'amplitude en tête de l'historique circulaire
        self.waterfall_history.insert(0, sweep.to_vec());
        if self.waterfall_history.len() > height {
            self.waterfall_history.pop();
        }

        // Forcer le rendu complet de l'historique
        self.redraw_waterfall();
    }

    /// Recalcule l'intégralité des pixels de l'image de la cascade à partir de l'historique
    pub fn redraw_waterfall(&mut self) {
        let width = self.waterfall_image.size[0];
        let height = self.waterfall_image.size[1];

        for y in 0..height {
            let row_idx = y * width;
            if y < self.waterfall_history.len() {
                let sweep = &self.waterfall_history[y];
                // Sécurité supplémentaire : s'assurer que la ligne d'historique correspond à la largeur de l'image
                if sweep.len() == width {
                    for x in 0..width {
                        let amp = sweep[x];
                        self.waterfall_image.pixels[row_idx + x] = self.amplitude_to_color(amp);
                    }
                } else {
                    for x in 0..width {
                        self.waterfall_image.pixels[row_idx + x] = Color32::BLACK;
                    }
                }
            } else {
                for x in 0..width {
                    self.waterfall_image.pixels[row_idx + x] = Color32::BLACK;
                }
            }
        }
        self.dirty = true;
    }

    /// Convertit une valeur d'amplitude brute en couleur selon les réglages utilisateur actifs (Gain, Offset, Palette)
    fn amplitude_to_color(&self, val: u8) -> Color32 {
        // Appliquer le seuil de bruit (offset) et le gain de contraste
        let val_adjusted = (val as f32 - self.waterfall_offset) * self.waterfall_gain;
        let t = (val_adjusted / 160.0).clamp(0.0, 1.0);

        match self.waterfall_palette {
            0 => {
                // 1. Arc-en-ciel (Standard SDR)
                if t < 0.2 {
                    let f = t * 5.0;
                    Color32::from_rgb(0, 0, (f * 64.0) as u8) // Noir -> Bleu nuit
                } else if t < 0.5 {
                    let f = (t - 0.2) * 3.33;
                    Color32::from_rgb(0, (f * 200.0) as u8, 64 + (f * 100.0) as u8) // Bleu -> Cyan
                } else if t < 0.8 {
                    let f = (t - 0.5) * 3.33;
                    Color32::from_rgb((f * 255.0) as u8, 200 + (f * 55.0) as u8, (164.0 - f * 164.0) as u8) // Cyan -> Jaune ambré
                } else {
                    let f = (t - 0.8) * 5.0;
                    Color32::from_rgb(255, (255.0 - f * 255.0) as u8, 0) // Jaune -> Rouge néon
                }
            }
            1 => {
                // 2. Bleu & Sarcelle (Ice/Cyan) - Très reposant de nuit
                let r = (t * t * 180.0) as u8;
                let g = (t * 240.0) as u8;
                let b = (50.0 + t * 205.0) as u8;
                Color32::from_rgb(r, g, b)
            }
            2 => {
                // 3. Magma / Feu
                if t < 0.4 {
                    let f = t * 2.5;
                    Color32::from_rgb((f * 80.0) as u8, 0, (f * 140.0) as u8) // Noir -> Violet/Indigo
                } else if t < 0.8 {
                    let f = (t - 0.4) * 2.5;
                    Color32::from_rgb(80 + (f * 175.0) as u8, (f * 120.0) as u8, 140 - (f * 100.0) as u8) // Violet -> Orange
                } else {
                    let f = (t - 0.8) * 5.0;
                    Color32::from_rgb(255, 120 + (f * 135.0) as u8, (f * 180.0) as u8) // Orange -> Jaune/Blanc
                }
            }
            _ => {
                // 4. Monochrome (Niveaux de gris classiques)
                let val_gray = (t * 255.0) as u8;
                Color32::from_rgb(val_gray, val_gray, val_gray)
            }
        }
    }
}

/// Dessine le spectre vectoriel temps réel (grille, repère de fréquence centrale et courbe d'intensité)
fn paint_fft(ui: &mut egui::Ui, sweep: &[u8]) {
    let desired_size = egui::vec2(ui.available_width(), 80.0);
    let (rect, _response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    // Dessin du fond d'écran sombre (Oscilloscope noir)
    painter.rect_filled(rect, 4.0, Color32::from_rgb(10, 12, 10));

    // Optimisation de la grille : une liste de segments dessinés d'un seul coup
    let grid_stroke = Stroke::new(1.0, Color32::from_rgb(25, 30, 25));
    let mut grid_shapes = Vec::with_capacity(14);
    
    let num_vertical_lines = 10;
    for i in 1..num_vertical_lines {
        let x = rect.min.x + (rect.width() / num_vertical_lines as f32) * i as f32;
        grid_shapes.push(egui::Shape::line_segment([egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)], grid_stroke));
    }
    let num_horizontal_lines = 4;
    for i in 1..num_horizontal_lines {
        let y = rect.min.y + (rect.height() / num_horizontal_lines as f32) * i as f32;
        grid_shapes.push(egui::Shape::line_segment([egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)], grid_stroke));
    }
    painter.extend(grid_shapes);

    // Dessin du repère de fréquence centrale (fine ligne jaune verticale semi-transparente)
    let center_x = rect.min.x + rect.width() / 2.0;
    let center_stroke = Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 235, 59, 120)); // Jaune ambré à 47% de transparence
    painter.line_segment(
        [egui::pos2(center_x, rect.min.y), egui::pos2(center_x, rect.max.y)],
        center_stroke
    );

    // Optimisation de la courbe FFT : une seule Polyline continue au lieu de segments individuels
    if !sweep.is_empty() {
        let mut points = Vec::with_capacity(sweep.len());
        let width_points = sweep.len();
        for (x_idx, &amp) in sweep.iter().enumerate() {
            let x = rect.min.x + (rect.width() / width_points as f32) * x_idx as f32;
            let norm_amp = (amp as f32 / 160.0).clamp(0.0, 1.0);
            let y = rect.max.y - (rect.height() * norm_amp);
            points.push(egui::pos2(x, y));
        }
        
        let curve_stroke = Stroke::new(1.5, Color32::from_rgb(0, 230, 118));
        painter.add(egui::Shape::line(points, curve_stroke)); // Polyline d'egui (rendu fluide et sans coût CPU)
    }
}

impl Ic7300App {
    /// Affiche la fenêtre modale de l'analyseur de spectre et de la cascade
    pub fn show_scope_window(&mut self, ctx: &egui::Context) {
        let mut show = self.scope_state.show_window;
        if !show { return; }

        // Reconstruction de la texture egui avec bridage temporel (max 30 FPS pour préserver la fluidité)
        if self.scope_state.dirty && self.scope_state.last_texture_update.elapsed() >= Duration::from_millis(33) {
            self.scope_state.waterfall_texture = Some(ctx.load_texture(
                "waterfall_texture",
                self.scope_state.waterfall_image.clone(),
                egui::TextureOptions::LINEAR,
            ));
            self.scope_state.dirty = false;
            self.scope_state.last_texture_update = Instant::now();
        }

        egui::Window::new("Analyseur de Spectre & Waterfall")
            .open(&mut show)
            .collapsible(false)
            .resizable(false)
            .default_width(495.0)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    // Ligne de contrôle (Activer/Désactiver le flux)
                    ui.horizontal(|ui| {
                        let (btn_text, btn_color) = if self.scope_state.enabled {
                            ("DÉSACTIVER LE FLUX", Color32::from_rgb(211, 47, 47))
                        } else {
                            ("ACTIVER LE FLUX", Color32::from_rgb(13, 71, 161))
                        };
                        
                        if custom_3d_button_sized(ui, btn_text, self.scope_state.enabled, btn_color, egui::vec2(150.0, 22.0)) {
                            self.scope_state.enabled = !self.scope_state.enabled;
                            self.last_user_write = Instant::now();
                            if self.is_connected {
                                self.send_cmd(Command::SetScopeOutput(self.scope_state.enabled));
                            }
                        }
                        
                        ui.separator();
                        
                        // Sélecteur de Span
                        ui.label(RichText::new("Span :").strong());
                        egui::ComboBox::from_id_source("scope_span_cb")
                            .selected_text(format!("±{} kHz", self.scope_state.span / 2000))
                            .width(110.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.scope_state.span, 5_000, "±2.5 kHz");
                                ui.selectable_value(&mut self.scope_state.span, 10_000, "±5.0 kHz");
                                ui.selectable_value(&mut self.scope_state.span, 20_000, "±10 kHz");
                                ui.selectable_value(&mut self.scope_state.span, 50_000, "±25 kHz");
                                ui.selectable_value(&mut self.scope_state.span, 100_000, "±50 kHz");
                                ui.selectable_value(&mut self.scope_state.span, 200_000, "±100 kHz");
                                ui.selectable_value(&mut self.scope_state.span, 500_000, "±250 kHz");
                                ui.selectable_value(&mut self.scope_state.span, 1_000_000, "±500 kHz");
                            });
                    });
                    
                    // ==========================================
                    // RÉGLAGES DE LA CASCADE ET DES COULEURS
                    // ==========================================
                    ui.add_space(4.0);
                    ui.collapsing("⚙ Réglages Cascade & Couleurs", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Palette :");
                            let p_selected = self.scope_state.waterfall_palette;
                            let mut palette_changed = false;
                            egui::ComboBox::from_id_source("palette_cb")
                                .selected_text(match p_selected {
                                    0 => "Arc-en-ciel (SDR)",
                                    1 => "Bleu-Cyan (Glace)",
                                    2 => "Magma (Feu)",
                                    _ => "Niveaux de Gris",
                                })
                                .width(150.0)
                                .show_ui(ui, |ui| {
                                    if ui.selectable_value(&mut self.scope_state.waterfall_palette, 0, "Arc-en-ciel (SDR)").clicked() { palette_changed = true; }
                                    if ui.selectable_value(&mut self.scope_state.waterfall_palette, 1, "Bleu-Cyan (Glace)").clicked() { palette_changed = true; }
                                    if ui.selectable_value(&mut self.scope_state.waterfall_palette, 2, "Magma (Feu)").clicked() { palette_changed = true; }
                                    if ui.selectable_value(&mut self.scope_state.waterfall_palette, 3, "Niveaux de Gris").clicked() { palette_changed = true; }
                                });
                            if palette_changed {
                                self.scope_state.redraw_waterfall();
                            }
                        });
                        ui.add_space(4.0);
                        
                        ui.horizontal(|ui| {
                            ui.label("Seuil de Bruit :");
                            let r_offset = ui.add(egui::Slider::new(&mut self.scope_state.waterfall_offset, 0.0..=150.0).text("dB"));
                            if r_offset.changed() {
                                self.scope_state.redraw_waterfall();
                            }
                            ui.separator();
                            ui.label("Contraste :");
                            let r_gain = ui.add(egui::Slider::new(&mut self.scope_state.waterfall_gain, 0.5..=5.0).text("x"));
                            if r_gain.changed() {
                                self.scope_state.redraw_waterfall();
                            }
                        });
                    });
                    ui.add_space(6.0);

                    // 1. Graphe FFT (Courbe ultra-fluide avec son repère central jaune)
                    ui.label(RichText::new("Spectre FFT en Temps Réel").size(11.0).color(Color32::LIGHT_GRAY));
                    paint_fft(ui, &self.scope_state.current_sweep);
                    ui.add_space(6.0);

                    // 2. Cascade (Waterfall avec tracé dynamique du repère central jaune et accord par clic)
                    ui.label(RichText::new("Historique Cascade (Waterfall) - Cliquer pour accorder").size(11.0).color(Color32::LIGHT_GRAY));
                    if let Some(texture) = &self.scope_state.waterfall_texture {
                        let size = egui::vec2(ui.available_width(), 120.0);
                        
                        // Rendu de l'image de la cascade réceptive aux clics de souris
                        let response = ui.add(egui::Image::new(texture)
                            .fit_to_exact_size(size)
                            .sense(egui::Sense::click()) // Active la sensibilité du clic sur le waterfall
                        );
                        let rect = response.rect;
                        
                        // Change le pointeur de la souris en petite main lors du survol
                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        
                        // Traitement du clic de souris pour l'accord tactile
                        if response.clicked() {
                            if let Some(click_pos) = response.interact_pointer_pos() {
                                let relative_x = (click_pos.x - rect.min.x) / rect.width();
                                let offset_ratio = relative_x - 0.5; // Écart par rapport au centre (-0.5 à +0.5)
                                
                                // Interpolation avec le Span de l'émetteur
                                let freq_offset = offset_ratio * (self.scope_state.span as f32);
                                
                                // Utilisation de la fréquence active comme centre absolu pour éviter tout décalage
                                let target_freq = self.frequency as f64 + freq_offset as f64;
                                
                                // Arrondir la fréquence au kHz le plus proche
                                let rounded_freq_khz = (target_freq / 1000.0).round();
                                let final_freq = (rounded_freq_khz * 1000.0) as u64;
                                
                                // Envoi de la nouvelle fréquence d'accord au poste
                                self.set_frequency_from_i64(final_freq as i64);
                            }
                        }
                        
                        // Tracé d'un repère jaune semi-transparent exactement au milieu de la cascade
                        let center_x = rect.min.x + rect.width() / 2.0;
                        let center_stroke = Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 235, 59, 100)); // Jaune à 39% d'opacité
                        ui.painter().line_segment(
                            [egui::pos2(center_x, rect.min.y), egui::pos2(center_x, rect.max.y)],
                            center_stroke
                        );
                    }
                });
            });

        self.scope_state.show_window = show;
    }
}