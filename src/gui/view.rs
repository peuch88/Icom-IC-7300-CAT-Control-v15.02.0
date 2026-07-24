// src/gui/view.rs
// Version : 15.02.0 - Intégration graphique des boutons d'allumage ("⏻ Allumer") et d'extinction ("⏻ Éteindre") dans l'en-tête [1.1]

use eframe::egui::{self, Color32, RichText, ScrollArea, Frame, Margin, Rounding, Stroke};
use std::time::{Duration, Instant};
use crossbeam_channel::unbounded;

use crate::comm::{RadioMode, RadioUpdate, Command};
use crate::gui::app::{Ic7300App, VERSION, RightTab, AMATEUR_BANDS, CB_BANDS, PIRATE_BANDS};
use crate::gui::widgets::{
    rx_signal_color, custom_3d_button, custom_3d_button_sized,
    render_flexible_segmented_meter, format_vfo_freq, draw_led,
    draw_connection_led, fprint_err, url_encode
};
use crate::database::{
    DbMemoryEntry, download_and_import_eibi, is_time_in_range
};

const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

impl eframe::App for Ic7300App {
    /// Callback d'arrêt propre géré par eframe
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.disconnect();
        self.save_settings();
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let duration = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        let secs = duration.as_secs();
        let utc_hour = ((secs / 3600) % 24) as u32;
        let utc_min = ((secs / 60) % 60) as u32;

        if let Some(ref rx) = self.eibi_rx_status {
            match rx.try_recv() {
                Ok(status) => self.eibi_status = status,
                _ => {}
            }
        }

        if let Some(ref rx) = self.rx_radio {
            let mut freq_changed = false;
            let mut repaint_needed = false;
            let mut disconnect_msg = None;
            while let Ok(update) = rx.try_recv() {
                repaint_needed = true;
                match update {
                    RadioUpdate::Frequency(freq) => {
                        if self.last_user_write.elapsed() >= Duration::from_millis(500) {
                            if self.active_vfo == 0 { self.frequency_a = freq; } else { self.frequency_b = freq; }
                            self.frequency = freq; self.freq_input = format!("{:.6}", freq as f64 / 1_000_000.0);
                            freq_changed = true;
                        }
                    }
                    RadioUpdate::ModeAndFilter(mode, filter, is_data) => {
                        if self.last_user_write.elapsed() >= Duration::from_millis(500) {
                            if self.active_vfo == 0 { self.mode_a = mode; self.filter_a = filter; self.is_data_mode_a = is_data; }
                            else { self.mode_b = mode; self.filter_b = filter; self.is_data_mode_b = is_data; }
                            self.mode = mode; self.filter = filter; self.is_data_mode = is_data;
                        }
                    }
                    RadioUpdate::ActiveVfo(vfo) => {
                        if self.last_user_write.elapsed() >= Duration::from_millis(500) {
                            self.active_vfo = vfo;
                            if vfo == 0 {
                                self.frequency = self.frequency_a; self.mode = self.mode_a;
                                self.filter = self.filter_a; self.is_data_mode = self.is_data_mode_a;
                            } else {
                                self.frequency = self.frequency_b; self.mode = self.mode_b;
                                self.filter = self.filter_b; self.is_data_mode = self.is_data_mode_b;
                            }
                            self.freq_input = format!("{:.6}", self.frequency as f64 / 1_000_000.0);
                        }
                    }
                    RadioUpdate::SplitState(split) => { if self.last_user_write.elapsed() >= Duration::from_millis(500) { self.split_active = split; } }
                    RadioUpdate::Meter(sub_cmd, val) => {
                        match sub_cmd {
                            0x02 => self.s_meter = val, 0x11 => self.po_meter = val,
                            0x12 => self.swr_meter = val, 0x13 => self.alc_meter = val,
                            0x14 => self.comp_meter = val, _ => {}
                        }
                    }
                    RadioUpdate::PTTState(is_tx_state) => { self.is_tx = is_tx_state; }
                    RadioUpdate::AfGain(val) => { if self.last_user_write.elapsed() >= Duration::from_millis(500) { self.af_gain = val; } }
                    RadioUpdate::RfGain(val) => { if self.last_user_write.elapsed() >= Duration::from_millis(500) { self.rf_gain = val; } }
                    RadioUpdate::Squelch(val) => { if self.last_user_write.elapsed() >= Duration::from_millis(500) { self.squelch = val; } }
                    RadioUpdate::RfPower(val) => { if self.last_user_write.elapsed() >= Duration::from_millis(500) { self.rf_power = val; } }
                    RadioUpdate::MicGain(val) => { if self.last_user_write.elapsed() >= Duration::from_millis(500) { self.mic_gain = val; } }
                    RadioUpdate::CompLevel(val) => { if self.last_user_write.elapsed() >= Duration::from_millis(500) { self.comp_level = val; } }
                    RadioUpdate::MonitorLevel(val) => { if self.last_user_write.elapsed() >= Duration::from_millis(500) { self.monitor_level = val; } }
                    RadioUpdate::Preamp(val) => { if self.last_user_write.elapsed() >= Duration::from_millis(500) { self.preamp = val; } }
                    RadioUpdate::Attenuator(val) => { if self.last_user_write.elapsed() >= Duration::from_millis(500) { self.attenuator = val; } }
                    RadioUpdate::Agc(val) => { if self.last_user_write.elapsed() >= Duration::from_millis(500) { self.agc = val; } }
                    RadioUpdate::Tuner(val) => { if self.last_user_write.elapsed() >= Duration::from_millis(500) { self.tuner = val; } }
                    RadioUpdate::NoiseBlanker(val) => { if self.last_user_write.elapsed() >= Duration::from_millis(500) { self.noise_blanker = val; } }
                    RadioUpdate::NoiseReduction(val) => { if self.last_user_write.elapsed() >= Duration::from_millis(500) { self.noise_reduction = val; } }
                    RadioUpdate::NoiseBlankerLevel(val) => { if self.last_user_write.elapsed() >= Duration::from_millis(500) { self.noise_blanker_level = val; } }
                    RadioUpdate::NoiseReductionLevel(val) => { if self.last_user_write.elapsed() >= Duration::from_millis(500) { self.noise_reduction_level = val; } }
                    RadioUpdate::ScopeSweep(sweep) => {
                        // Réception et traitement du balayage de spectre défragmenté
                        self.scope_state.push_sweep(&sweep);
                    }
                    RadioUpdate::UsbRxLevel(val) => { if self.last_user_write.elapsed() >= Duration::from_millis(500) { self.usb_rx_level = val; } }
                    RadioUpdate::UsbTxLevel(val) => { if self.last_user_write.elapsed() >= Duration::from_millis(500) { self.usb_tx_level = val; } }
                    RadioUpdate::Disconnected(msg) => { disconnect_msg = Some(msg); }
                }
            }
            if let Some(msg) = disconnect_msg { self.disconnect(); self.eibi_status = msg; }
            if freq_changed { self.refresh_eibi_results(); }
            if repaint_needed { ctx.request_repaint(); }
        }

        let mut visuals = egui::Visuals::dark();
        visuals.window_rounding = Rounding::same(8.0);
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(25, 25, 30);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(45, 45, 50);
        ctx.set_visuals(visuals);

        let panel_frame = Frame::none().fill(Color32::from_rgb(28, 28, 33)).rounding(Rounding::same(8.0)).stroke(Stroke::new(1.5, Color32::from_rgb(50, 50, 55))).inner_margin(Margin::same(12.0));

        // --- EN-TÊTE ---
        egui::TopBottomPanel::top("top_panel").frame(Frame::default().fill(Color32::from_rgb(20, 20, 25)).inner_margin(8.0)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Configuration COM").clicked() { self.show_config_window = !self.show_config_window; }
                ui.separator();
                if ui.button("Gérer les Mémoires").clicked() { self.show_mem_manager = !self.show_mem_manager; }
                ui.separator();
                if ui.button("Réglages Gains").clicked() { self.show_gains_window = !self.show_gains_window; }
                ui.separator();
                if ui.button("Import/Export CSV").clicked() { self.show_csv_manager = !self.show_csv_manager; }
                ui.separator();
                if ui.button("Mini-Wiki / Info").clicked() { self.show_info_window = !self.show_info_window; }
                ui.separator();
                
                // Bouton d'accès au Spectre / Waterfall
                if ui.button("Spectre / Waterfall").clicked() { self.scope_state.show_window = !self.scope_state.show_window; }
                ui.separator();
                
                if !self.is_connected {
                    ui.label(RichText::new(format!("v{}", VERSION)).color(Color32::DARK_GRAY));
                    ui.separator();

                    // Dessin du voyant de connexion LED
                    draw_connection_led(ui, self.is_connected);
                    ui.add_space(2.0);

                    ui.label(RichText::new(format!("OFFLINE (Prêt sur {})", self.port_name)).color(Color32::LIGHT_GRAY).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(" Connecter le Transceiver ").clicked() { if let Err(e) = self.connect(ctx.clone()) { fprint_err(&e); } }
                        ui.separator();
                        
                        // Bouton d'allumage physique direct (Allumer et se connecter de force)
                        let power_color = Color32::from_rgb(183, 28, 28); // Rouge foncé discret
                        if custom_3d_button_sized(ui, "⏻ Allumer", false, power_color, egui::vec2(85.0, 22.0)) {
                            if let Err(e) = self.connect_and_power_on(ctx.clone()) { fprint_err(&e); }
                        }
                    });
                } else {
                    ui.label(RichText::new(format!("v{}", VERSION)).color(Color32::DARK_GRAY));
                    ui.separator();

                    // Dessin du voyant de connexion LED
                    draw_connection_led(ui, self.is_connected);
                    ui.add_space(2.0);

                    ui.label(RichText::new(format!("ONLINE ({} @ {} bauds)", self.port_name, self.baud_rate)).color(Color32::GREEN).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(" Déconnecter ").clicked() { self.disconnect(); }
                        ui.separator();
                        
                        // Bouton d'extinction propre (Éteindre et Déconnecter proprement)
                        let power_color = Color32::from_rgb(0, 230, 118); // Vert vif actif
                        if custom_3d_button_sized(ui, "⏻ Éteindre", true, power_color, egui::vec2(85.0, 22.0)) {
                            self.power_off_and_disconnect();
                        }
                    });
                }
            });
        });

        // Appels déportés vers src/gui/dialogs.rs et src/gui/scope.rs
        self.show_wiki_window(ctx);
        self.show_config_window(ctx);
        self.show_memories_window(ctx);
        self.show_csv_window(ctx);
        self.show_gains_window(ctx);
        self.show_scope_window(ctx); // Rendu de la fenêtre modale du Spectre

        // --- Layout principal à trois colonnes ---
        egui::CentralPanel::default().frame(Frame::default().fill(Color32::from_rgb(15, 15, 20)).inner_margin(10.0)).show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal(|ui| {
                    let spacing = 12.0;
                    let col_width = (ui.available_width() - spacing * 4.0 - 4.0) / 3.0;

                    // =========================================================
                    // COLONNE GAUCHE (VFO, Cadre, PTT, Modes & Alignements mathématiques)
                    // =========================================================
                    ui.vertical(|ui| {
                        ui.set_max_width(col_width);
                        let mut change_to_a = false;
                        let mut change_to_b = false;
                        let vfo_res = panel_frame.show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new("ETAT DOUBLE VFO").strong().color(Color32::LIGHT_GRAY));
                                ui.add_space(5.0);
                                
                                // VFO A Row
                                let is_vfo_a_active = self.active_vfo == 0;
                                let bg_a = if is_vfo_a_active { Color32::from_rgb(33, 33, 38) } else { Color32::from_rgb(18, 18, 22) };
                                let border_color_a = if is_vfo_a_active { Color32::from_rgb(0, 230, 118) } else { Color32::from_rgb(45, 45, 50) };
                                Frame::none().fill(bg_a).rounding(Rounding::same(4.0)).stroke(Stroke::new(1.0, border_color_a)).inner_margin(Margin::symmetric(8.0, 6.0)).show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        draw_led(ui, is_vfo_a_active);
                                        ui.add_space(2.0);
                                        ui.label(RichText::new("VFO A").strong().color(if is_vfo_a_active { Color32::WHITE } else { Color32::GRAY }).size(12.0));
                                        ui.add_space(6.0);
                                        ui.label(RichText::new(format!("{} MHz", format_vfo_freq(self.frequency_a))).strong().monospace().color(Color32::WHITE).size(13.0));
                                        let mode_str = match self.mode_a { RadioMode::Usb => "USB", RadioMode::Lsb => "LSB", RadioMode::Am => "AM", RadioMode::Fm => "FM", RadioMode::Cw => "CW" };
                                        ui.label(RichText::new(format!("[{} / FIL{}]", mode_str, self.filter_a)).size(10.0).color(Color32::GRAY));
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if !is_vfo_a_active {
                                                if ui.small_button("Activer").clicked() { change_to_a = true; }
                                            } else { ui.label(RichText::new("actif").italics().color(Color32::GREEN).size(10.0)); }
                                        });
                                    });
                                });

                                ui.add_space(4.0);

                                // VFO B Row
                                let is_vfo_b_active = self.active_vfo == 1;
                                let bg_b = if is_vfo_b_active { Color32::from_rgb(33, 33, 38) } else { Color32::from_rgb(18, 18, 22) };
                                let border_color_b = if is_vfo_b_active { Color32::from_rgb(0, 230, 118) } else { Color32::from_rgb(45, 45, 50) };
                                Frame::none().fill(bg_b).rounding(Rounding::same(4.0)).stroke(Stroke::new(1.0, border_color_b)).inner_margin(Margin::symmetric(8.0, 6.0)).show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        draw_led(ui, is_vfo_b_active);
                                        ui.add_space(2.0);
                                        ui.label(RichText::new("VFO B").strong().color(if is_vfo_b_active { Color32::WHITE } else { Color32::GRAY }).size(12.0));
                                        ui.add_space(6.0);
                                        ui.label(RichText::new(format!("{} MHz", format_vfo_freq(self.frequency_b))).strong().monospace().color(Color32::WHITE).size(13.0));
                                        let mode_str = match self.mode_b { RadioMode::Usb => "USB", RadioMode::Lsb => "LSB", RadioMode::Am => "AM", RadioMode::Fm => "FM", RadioMode::Cw => "CW" };
                                        ui.label(RichText::new(format!("[{} / FIL{}]", mode_str, self.filter_b)).size(10.0).color(Color32::GRAY));
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if !is_vfo_b_active {
                                                if ui.small_button("Activer").clicked() { change_to_b = true; }
                                            } else { ui.label(RichText::new("actif").italics().color(Color32::GREEN).size(10.0)); }
                                        });
                                    });
                                });

                                ui.add_space(8.0);

                                // Cadran LCD Principal
                                Frame::none().fill(Color32::from_rgb(5, 12, 8)).rounding(Rounding::same(6.0)).stroke(Stroke::new(3.0, Color32::from_rgb(40, 40, 45))).inner_margin(Margin::symmetric(14.0, 18.0)).show(ui, |ui| {
                                    let freq_str = format!("{:08}", self.frequency);
                                    let step_log = (self.vfo_step as f64).log10().round() as usize;
                                    let active_idx = 8 - step_log - 1;
                                    let time = ctx.input(|i| i.time);
                                    let blink_off = self.vfo_hovered && ((time * 4.0) as u64 % 2 == 0);
                                    
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 1.0;
                                        for (idx, ch) in freq_str.chars().enumerate() {
                                            if idx == 2 || idx == 5 { ui.label(RichText::new(".").size(36.0).color(Color32::from_rgb(120, 255, 180))); }
                                            let is_active_digit = idx == active_idx;
                                            let color = if is_active_digit {
                                                if blink_off { Color32::from_rgb(30, 45, 35) } else { Color32::from_rgb(255, 235, 59) }
                                            } else { Color32::from_rgb(120, 255, 180) };
                                            let digit_str = if let Some(d) = ch.to_digit(10) { DIGITS[d as usize] } else { "" };
                                            let label = egui::Label::new(RichText::new(digit_str).size(36.0).color(color).strong()).sense(egui::Sense::click());
                                            let response = ui.add(label);
                                            if response.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                                            if response.clicked() {
                                                let power = 8 - idx - 1; self.vfo_step = 10u64.pow(power as u32);
                                            }
                                        }
                                        ui.label(RichText::new(" MHz").size(22.0).color(Color32::from_rgb(80, 180, 120)));
                                    });
                                    ui.add_space(4.0);
                                    let step_str = match self.vfo_step {
                                        1 => "1 Hz", 10 => "10 Hz", 100 => "100 Hz", 1_000 => "1 kHz",
                                        10_000 => "10 kHz", 100_000 => "100 kHz", 1_000_000 => "1 MHz", _ => "Custom",
                                    };
                                    ui.label(RichText::new(format!("TUNING STEP: {}", step_str)).size(11.0).color(Color32::from_rgb(80, 180, 120)).italics());
                                });

                                ui.add_space(12.0);

                                // Bouton VFO rotatif tactile
                                ui.vertical_centered(|ui| {
                                    let (rect, response) = ui.allocate_exact_size(egui::vec2(120.0, 100.0), egui::Sense::drag());
                                    let center = rect.center(); let radius = 48.0;
                                    if response.dragged() {
                                        let delta = response.drag_delta(); let drag_val = delta.x - delta.y;
                                        if drag_val.abs() > 0.0 {
                                            self.vfo_angle += drag_val * 0.05; self.vfo_accumulator += drag_val;
                                            let ticks = (self.vfo_accumulator / 5.0).trunc() as i64;
                                            if ticks != 0 {
                                                self.vfo_accumulator -= (ticks * 5) as f32;
                                                let new_freq = self.frequency as i64 + (ticks * self.vfo_step as i64);
                                                self.set_frequency_from_i64(new_freq);
                                            }
                                        }
                                    }
                                    if response.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::Grab); }
                                    let painter = ui.painter();
                                    painter.circle_filled(center, radius + 2.0, Color32::from_rgb(15, 15, 18));
                                    painter.circle_filled(center, radius, Color32::from_rgb(45, 45, 48));
                                    painter.circle_stroke(center, radius - 4.0, Stroke::new(1.5, Color32::from_rgb(30, 30, 35)));
                                    painter.circle_stroke(center, radius - 8.0, Stroke::new(1.0, Color32::from_rgb(55, 55, 58)));
                                    painter.circle_filled(center, radius - 10.0, Color32::from_rgb(25, 25, 28));
                                    let dimple_pos = center + egui::vec2(self.vfo_angle.cos() * (radius - 15.0), self.vfo_angle.sin() * (radius - 15.0));
                                    painter.circle_filled(dimple_pos, 7.0, Color32::from_rgb(10, 10, 12));
                                    painter.circle_stroke(dimple_pos, 7.0, Stroke::new(1.5, Color32::from_rgb(85, 85, 90)));
                                });

                                ui.add_space(10.0); ui.separator();

                                // Commandes VFO & SPLIT compactées (hauteur de 22px et alignement parfait)
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    let btn_width = (ui.available_width() - 12.0) / 3.0;
                                    
                                    if custom_3d_button_sized(ui, "A / B", false, Color32::from_rgb(13, 71, 161), egui::vec2(btn_width, 22.0)) {
                                        self.last_user_write = Instant::now();
                                        if self.active_vfo == 0 {
                                            self.active_vfo = 1; self.frequency = self.frequency_b; self.mode = self.mode_b; self.filter = self.filter_b; self.is_data_mode = self.is_data_mode_b;
                                        } else {
                                            self.active_vfo = 0; self.frequency = self.frequency_a; self.mode = self.mode_a; self.filter = self.filter_a; self.is_data_mode = self.is_data_mode_a;
                                        }
                                        self.freq_input = format!("{:.6}", self.frequency as f64 / 1_000_000.0);
                                        if self.is_connected { self.send_cmd(Command::SwapVfo); }
                                    }
                                    
                                    if custom_3d_button_sized(ui, "A = B", false, Color32::from_rgb(13, 71, 161), egui::vec2(btn_width, 22.0)) {
                                        self.last_user_write = Instant::now();
                                        if self.active_vfo == 0 { self.frequency_b = self.frequency_a; self.mode_b = self.mode_a; self.filter_b = self.filter_a; self.is_data_mode_b = self.is_data_mode_a; }
                                        else { self.frequency_a = self.frequency_b; self.mode_a = self.mode_b; self.filter_a = self.filter_b; self.is_data_mode_a = self.is_data_mode_b; }
                                        if self.is_connected { self.send_cmd(Command::EqualizeVfo); }
                                    }
                                    
                                    let split_bg = if self.split_active { Color32::from_rgb(229, 57, 53) } else { Color32::from_rgb(50, 50, 55) };
                                    if custom_3d_button_sized(ui, "SPLIT", self.split_active, split_bg, egui::vec2(btn_width, 22.0)) {
                                        self.last_user_write = Instant::now(); self.split_active = !self.split_active;
                                        if self.is_connected { self.send_cmd(Command::SetSplit(self.split_active)); }
                                    }
                                });

                                ui.add_space(8.0); ui.separator();
                                ui.label(RichText::new("MODE & DATA").strong().color(Color32::from_rgb(100, 200, 255)));
                                ui.add_space(6.0);
                                
                                // Grille des modes
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    let btn_width = (ui.available_width() - 12.0) / 3.0;
                                    let is_active = self.mode == RadioMode::Lsb;
                                    if custom_3d_button_sized(ui, "LSB", is_active, Color32::from_rgb(33, 150, 243), egui::vec2(btn_width, 20.0)) {
                                        self.last_user_write = Instant::now(); self.mode = RadioMode::Lsb;
                                        if self.active_vfo == 0 { self.mode_a = RadioMode::Lsb; } else { self.mode_b = RadioMode::Lsb; }
                                        if self.is_connected { self.send_cmd(Command::SetModeAndFilter(self.mode, self.filter)); self.send_cmd(Command::SetDataMode(self.is_data_mode)); }
                                    }
                                    ui.add_space(4.0);
                                    let is_active = self.mode == RadioMode::Usb;
                                    if custom_3d_button_sized(ui, "USB", is_active, Color32::from_rgb(33, 150, 243), egui::vec2(btn_width, 20.0)) {
                                        self.last_user_write = Instant::now(); self.mode = RadioMode::Usb;
                                        if self.active_vfo == 0 { self.mode_a = RadioMode::Usb; } else { self.mode_b = RadioMode::Usb; }
                                        if self.is_connected { self.send_cmd(Command::SetModeAndFilter(self.mode, self.filter)); self.send_cmd(Command::SetDataMode(self.is_data_mode)); }
                                    }
                                    ui.add_space(4.0);
                                    let is_active = self.mode == RadioMode::Cw;
                                    if custom_3d_button_sized(ui, "CW", is_active, Color32::from_rgb(33, 150, 243), egui::vec2(btn_width, 20.0)) {
                                        self.last_user_write = Instant::now(); self.mode = RadioMode::Cw;
                                        if self.active_vfo == 0 { self.mode_a = RadioMode::Cw; } else { self.mode_b = RadioMode::Cw; }
                                        if self.is_connected { self.send_cmd(Command::SetModeAndFilter(self.mode, self.filter)); self.send_cmd(Command::SetDataMode(self.is_data_mode)); }
                                    }
                                });
                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    let btn_width = (ui.available_width() - 12.0) / 3.0;
                                    let is_active = self.mode == RadioMode::Am;
                                    if custom_3d_button_sized(ui, "AM", is_active, Color32::from_rgb(33, 150, 243), egui::vec2(btn_width, 20.0)) {
                                        self.last_user_write = Instant::now(); self.mode = RadioMode::Am;
                                        if self.active_vfo == 0 { self.mode_a = RadioMode::Am; } else { self.mode_b = RadioMode::Am; }
                                        if self.is_connected { self.send_cmd(Command::SetModeAndFilter(self.mode, self.filter)); self.send_cmd(Command::SetDataMode(self.is_data_mode)); }
                                    }
                                    ui.add_space(4.0);
                                    let is_active = self.mode == RadioMode::Fm;
                                    if custom_3d_button_sized(ui, "FM", is_active, Color32::from_rgb(33, 150, 243), egui::vec2(btn_width, 20.0)) {
                                        self.last_user_write = Instant::now(); self.mode = RadioMode::Fm;
                                        if self.active_vfo == 0 { self.mode_a = RadioMode::Fm; } else { self.mode_b = RadioMode::Fm; }
                                        if self.is_connected { self.send_cmd(Command::SetModeAndFilter(self.mode, self.filter)); self.send_cmd(Command::SetDataMode(self.is_data_mode)); }
                                    }
                                    ui.add_space(4.0);
                                    let is_active = self.is_data_mode;
                                    if custom_3d_button_sized(ui, "DATA (D)", is_active, Color32::from_rgb(0, 150, 136), egui::vec2(btn_width, 20.0)) {
                                        self.last_user_write = Instant::now(); self.is_data_mode = !self.is_data_mode;
                                        if self.active_vfo == 0 { self.is_data_mode_a = self.is_data_mode; } else { self.is_data_mode_b = self.is_data_mode; }
                                        if self.is_connected { self.send_cmd(Command::SetDataMode(self.is_data_mode)); }
                                    }
                                });
                            });
                        });
                        self.vfo_hovered = vfo_res.response.hovered();

                        // RESTAURATION DES COMPORTEMENTS ET ACTIONS DES BOUTONS DE COMMUTATION DE VFO
                        if change_to_a {
                            self.last_user_write = Instant::now();
                            self.active_vfo = 0;
                            self.frequency = self.frequency_a;
                            self.mode = self.mode_a;
                            self.filter = self.filter_a;
                            self.is_data_mode = self.is_data_mode_a;
                            self.freq_input = format!("{:.6}", self.frequency as f64 / 1_000_000.0);
                            if self.is_connected { self.send_cmd(Command::SetVfo(0)); }
                        }
                        if change_to_b {
                            self.last_user_write = Instant::now();
                            self.active_vfo = 1;
                            self.frequency = self.frequency_b;
                            self.mode = self.mode_b;
                            self.filter = self.filter_b;
                            self.is_data_mode = self.is_data_mode_b;
                            self.freq_input = format!("{:.6}", self.frequency as f64 / 1_000_000.0);
                            if self.is_connected { self.send_cmd(Command::SetVfo(1)); }
                        }

                        ui.add_space(10.0);
                        
                        // Bandeau de commande inférieur PTT / LOCK réajusté (hauteur 32px, lock 70px)
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            let total_width = ui.available_width();
                            let lock_width = 70.0;
                            let ptt_width = total_width - lock_width - 6.0;
                            
                            let (ptt_text, ptt_color) = if self.is_tx { ("TX ACTIF", Color32::from_rgb(200, 40, 40)) } else { ("RX ACTIF", Color32::from_rgb(60, 60, 70)) };
                            if custom_3d_button_sized(ui, ptt_text, self.is_tx, ptt_color, egui::vec2(ptt_width, 32.0)) {
                                self.last_user_write = Instant::now(); self.is_tx = !self.is_tx;
                                if self.is_connected { self.send_cmd(Command::SetPTT(self.is_tx)); }
                            }
                            let lock_color = if self.tx_lock { Color32::from_rgb(255, 145, 0) } else { Color32::from_rgb(55, 55, 60) };
                            let lock_text = if self.tx_lock { "LOCK\nON" } else { "LOCK\nOFF" };
                            if custom_3d_button_sized(ui, lock_text, self.tx_lock, lock_color, egui::vec2(lock_width, 32.0)) {
                                self.last_user_write = Instant::now(); self.tx_lock = !self.tx_lock;
                            }
                        });
                    });

                    ui.separator();

                    // ==========================================
                    // COLONNE MILIEU (Filtres, Réception, Métrologie)
                    // ==========================================
                    ui.vertical(|ui| {
                        ui.set_max_width(col_width);
                        panel_frame.show(ui, |ui| {
                            ui.label(RichText::new("FILTRES & RÉCEPTION").strong().color(Color32::from_rgb(100, 200, 255)));
                            ui.add_space(8.0);

                            let total_w = ui.available_width();
                            let label_w = 54.0; // Force l'alignement parfait des liserés de départ
                            let spacing_x = 6.0;
                            ui.spacing_mut().item_spacing.x = spacing_x;
                            
                            // 3 boutons par ligne (hauteur harmonisée à 22px)
                            let btn_width_3 = ((total_w - label_w - (2.0 * spacing_x)) / 3.0 - 2.0).max(10.0);

                            // 1. Filtres
                            ui.horizontal(|ui| {
                                ui.add_sized([label_w, 22.0], egui::Label::new("Filtre:"));
                                for f in 1..=3 {
                                    let is_active = self.filter == f;
                                    let bg = Color32::from_rgb(124, 77, 255);
                                    if custom_3d_button_sized(ui, &format!("FIL{}", f), is_active, bg, egui::vec2(btn_width_3, 22.0)) {
                                        self.last_user_write = Instant::now(); self.filter = f;
                                        if self.active_vfo == 0 { self.filter_a = f; } else { self.filter_b = f; }
                                        if self.is_connected { 
                                            self.send_cmd(Command::SetModeAndFilter(self.mode, self.filter)); 
                                            self.send_cmd(Command::SetDataMode(self.is_data_mode)); 
                                        }
                                    }
                                }
                            });
                            
                            ui.add_space(6.0);

                            // 2. Préamplificateur
                            ui.horizontal(|ui| {
                                ui.add_sized([label_w, 22.0], egui::Label::new("P.AMP:"));
                                for (l, v) in [("OFF", 0), ("AMP1", 1), ("AMP2", 2)] {
                                    let is_active = self.preamp == v;
                                    let bg = Color32::from_rgb(255, 87, 34);
                                    if custom_3d_button_sized(ui, l, is_active, bg, egui::vec2(btn_width_3, 22.0)) {
                                        self.last_user_write = Instant::now(); self.preamp = v;
                                        if self.is_connected { self.send_cmd(Command::SetPreamp(self.preamp)); }
                                    }
                                }
                            });

                            ui.add_space(6.0);

                            // 3. AGC
                            ui.horizontal(|ui| {
                                ui.add_sized([label_w, 22.0], egui::Label::new("AGC:"));
                                for (l, v) in [("FAST", 1), ("MID", 2), ("SLOW", 3)] {
                                    let is_active = self.agc == v;
                                    let bg = Color32::from_rgb(76, 175, 80);
                                    if custom_3d_button_sized(ui, l, is_active, bg, egui::vec2(btn_width_3, 22.0)) {
                                        self.last_user_write = Instant::now(); self.agc = v;
                                        if self.is_connected { self.send_cmd(Command::SetAgc(self.agc)); }
                                    }
                                }
                            });

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);

                            // 4. NB & NR (Toggles 3D Sarcelle)
                            let btn_width_2 = ((total_w - spacing_x) / 2.0 - 2.0).max(10.0);
                            ui.horizontal(|ui| {
                                let nb_active = self.noise_blanker;
                                let nb_bg = Color32::from_rgb(0, 150, 136); // Couleur NB
                                if custom_3d_button_sized(ui, "NOISE BLANKER", nb_active, nb_bg, egui::vec2(btn_width_2, 22.0)) {
                                    self.last_user_write = Instant::now(); self.noise_blanker = !self.noise_blanker;
                                    if self.is_connected { self.send_cmd(Command::SetNoiseBlanker(self.noise_blanker)); }
                                }
                                
                                let nr_active = self.noise_reduction;
                                let nr_bg = Color32::from_rgb(0, 150, 136); // Couleur NR
                                if custom_3d_button_sized(ui, "NOISE REDUCTION", nr_active, nr_bg, egui::vec2(btn_width_2, 22.0)) {
                                    self.last_user_write = Instant::now(); self.noise_reduction = !self.noise_reduction;
                                    if self.is_connected { self.send_cmd(Command::SetNoiseReduction(self.noise_reduction)); }
                                }
                            });

                            ui.add_space(6.0);

                            // 5. ATT & TUNER
                            ui.horizontal(|ui| {
                                let att_active = self.attenuator;
                                let att_bg = Color32::from_rgb(233, 30, 99);
                                if custom_3d_button_sized(ui, "ATT 20dB", att_active, att_bg, egui::vec2(btn_width_2, 22.0)) {
                                    self.last_user_write = Instant::now(); self.attenuator = !self.attenuator;
                                    if self.is_connected { self.send_cmd(Command::SetAttenuator(self.attenuator)); }
                                }
                                
                                let tuner_active = self.tuner;
                                let tuner_bg = Color32::from_rgb(255, 193, 7);
                                if custom_3d_button_sized(ui, "TUNER INT.", tuner_active, tuner_bg, egui::vec2(btn_width_2, 22.0)) {
                                    self.last_user_write = Instant::now(); self.tuner = !self.tuner;
                                    if self.is_connected { self.send_cmd(Command::SetTuner(self.tuner)); }
                                }
                            });
                        });

                        ui.add_space(10.0);

                        // Métrologie
                        panel_frame.show(ui, |ui| {
                            ui.label(RichText::new("MÉTROLOGIE").strong().color(Color32::from_rgb(100, 200, 255)));
                            ui.add_space(6.0);
                            if !self.is_tx {
                                let rx_color = rx_signal_color(self.s_meter);
                                render_flexible_segmented_meter(ui, "Indicateur Signal (S-Meter / RX)", self.s_meter, 241.0, rx_color, |val| {
                                    if val < 120 {
                                        let s = val / 13; if s == 0 { "S0".to_owned() } else { format!("S{}", s) }
                                    } else { format!("S9 +{}dB", (val - 120) / 2) }
                                });
                            } else {
                                render_flexible_segmented_meter(ui, "Puissance Émission (PO)", self.po_meter, 213.0, Color32::from_rgb(255, 110, 64), |val| {
                                    let watts = (val as f32 * (100.0 / 213.0)).round().min(100.0); format!("{:.0}W", watts)
                                });
                                ui.add_space(6.0);
                                render_flexible_segmented_meter(ui, "Rapport d'Ondes Stationnaires (SWR)", self.swr_meter, 120.0, Color32::from_rgb(245, 0, 87), |val| {
                                    if val == 0 { "1.0".to_owned() } else if val <= 120 { format!("{:.1}", 1.0 + (val as f32 / 120.0) * 2.0) } else { ">3.0".to_owned() }
                                });
                                ui.add_space(6.0);
                                render_flexible_segmented_meter(ui, "Contrôle ALC", self.alc_meter, 120.0, Color32::from_rgb(41, 182, 246), |val| {
                                    let pct = (val as f32 / 120.0 * 100.0).round().min(100.0); format!("{:.0}%", pct)
                                });
                                ui.add_space(6.0);
                                render_flexible_segmented_meter(ui, "Taux de Compression (COMP)", self.comp_meter, 210.0, Color32::from_rgb(255, 235, 59), |val| {
                                    let db = (val as f32 / 210.0 * 25.5).round().min(25.0); format!("{:.0}dB", db)
                                });
                            }
                        });

                        ui.add_space(10.0);

                        // Accès direct aux réglages de gains
                        panel_frame.show(ui, |ui| {
                            ui.vertical_centered_justified(|ui| {
                                if custom_3d_button(ui, "REGLAGES GAINS & PUISSANCE", self.show_gains_window, Color32::from_rgb(13, 71, 161)) {
                                    self.show_gains_window = !self.show_gains_window;
                                }
                            });
                        });
                    });

                    ui.separator();

                    // ==========================================
                    // COLONNE DROITE (Bandes sous forme de clavier matriciel tactile, Base EiBi et Stations Probables)
                    // ==========================================
                    ui.vertical(|ui| {
                        ui.set_max_width(col_width);
                        panel_frame.show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                let has_band_active = self.has_any_active_band();
                                ui.selectable_value(&mut self.right_tab, RightTab::Bandes, format!("BANDES {}", if has_band_active { "★" } else { "" }));
                                ui.selectable_value(&mut self.right_tab, RightTab::Memoires, "MEM.");
                                ui.selectable_value(&mut self.right_tab, RightTab::Eibi, "EIBI");
                            });
                            ui.separator();
                            ScrollArea::vertical().id_source("right_scroll").max_height(240.0).show(ui, |ui| {
                                match self.right_tab {
                                    RightTab::Bandes => {
                                        let total_w = ui.available_width();
                                        let spacing_x = 6.0;
                                        ui.spacing_mut().item_spacing.x = spacing_x;
                                        
                                        ui.label(RichText::new("Radioamateurs").strong().color(Color32::from_rgb(100, 200, 255)));
                                        ui.add_space(4.0);

                                        // Clavier matriciel tactile 5x2 (Largeur calculée rigoureusement)
                                        let btn_width_5 = ((total_w - (4.0 * spacing_x)) / 5.0 - 1.0).max(10.0);
                                        
                                        // Rangée 1 : 160m, 80m, 40m, 30m, 20m
                                        ui.horizontal(|ui| {
                                            for band in &AMATEUR_BANDS[0..5] {
                                                let is_active = self.is_freq_in_band(band);
                                                let bg_color = if is_active { Color32::from_rgb(13, 71, 161) } else { Color32::from_rgb(50, 50, 55) };
                                                if custom_3d_button_sized(ui, band.name, is_active, bg_color, egui::vec2(btn_width_5, 22.0)) {
                                                    self.change_band_and_mode(band.default_freq, band.default_mode);
                                                }
                                            }
                                        });
                                        ui.add_space(6.0);
                                        // Rangée 2 : 17m, 15m, 12m, 10m, 6m
                                        ui.horizontal(|ui| {
                                            for band in &AMATEUR_BANDS[5..10] {
                                                let is_active = self.is_freq_in_band(band);
                                                let bg_color = if is_active { Color32::from_rgb(13, 71, 161) } else { Color32::from_rgb(50, 50, 55) };
                                                if custom_3d_button_sized(ui, band.name, is_active, bg_color, egui::vec2(btn_width_5, 22.0)) {
                                                    self.change_band_and_mode(band.default_freq, band.default_mode);
                                                }
                                            }
                                        });

                                        ui.add_space(10.0);
                                        ui.separator();
                                        ui.add_space(6.0);

                                        ui.label(RichText::new("Cibi (11m) & Pirate").strong().color(Color32::from_rgb(255, 200, 100)));
                                        ui.add_space(4.0);

                                        // Clavier Cibi & Pirates optimisé
                                        // Rangée 1 : CB C19, SSTV, 11m DX (3 boutons plus larges pour les libellés longs)
                                        let btn_width_3 = ((total_w - (2.0 * spacing_x)) / 3.0 - 1.5).max(10.0);
                                        ui.horizontal(|ui| {
                                            // CB C19
                                            let band_cb = &CB_BANDS[0];
                                            let is_active = self.is_freq_in_band(band_cb);
                                            let bg_color = if is_active { Color32::from_rgb(230, 81, 0) } else { Color32::from_rgb(50, 50, 55) };
                                            if custom_3d_button_sized(ui, band_cb.name, is_active, bg_color, egui::vec2(btn_width_3, 22.0)) {
                                                self.change_band_and_mode(band_cb.default_freq, band_cb.default_mode);
                                            }
                                            
                                            // SSTV
                                            let band_sstv = &CB_BANDS[1];
                                            let is_active = self.is_freq_in_band(band_sstv);
                                            let bg_color = if is_active { Color32::from_rgb(230, 81, 0) } else { Color32::from_rgb(50, 50, 55) };
                                            if custom_3d_button_sized(ui, band_sstv.name, is_active, bg_color, egui::vec2(btn_width_3, 22.0)) {
                                                self.change_band_and_mode(band_sstv.default_freq, band_sstv.default_mode);
                                            }

                                            // 11m DX
                                            let band_dx = &PIRATE_BANDS[0];
                                            let is_active = self.is_freq_in_band(band_dx);
                                            let bg_color = if is_active { Color32::from_rgb(211, 47, 47) } else { Color32::from_rgb(50, 50, 55) };
                                            if custom_3d_button_sized(ui, band_dx.name, is_active, bg_color, egui::vec2(btn_width_3, 22.0)) {
                                                self.change_band_and_mode(band_dx.default_freq, band_dx.default_mode);
                                            }
                                        });
                                        ui.add_space(6.0);
                                        // Rangée 2 : 45m, 88m (2 boutons larges)
                                        let btn_width_2 = ((total_w - spacing_x) / 2.0 - 1.5).max(10.0);
                                        ui.horizontal(|ui| {
                                            // 45m
                                            let band_45 = &PIRATE_BANDS[1];
                                            let is_active = self.is_freq_in_band(band_45);
                                            let bg_color = if is_active { Color32::from_rgb(211, 47, 47) } else { Color32::from_rgb(50, 50, 55) };
                                            if custom_3d_button_sized(ui, band_45.name, is_active, bg_color, egui::vec2(btn_width_2, 22.0)) {
                                                self.change_band_and_mode(band_45.default_freq, band_45.default_mode);
                                            }

                                            // 88m
                                            let band_88 = &PIRATE_BANDS[2];
                                            let is_active = self.is_freq_in_band(band_88);
                                            let bg_color = if is_active { Color32::from_rgb(211, 47, 47) } else { Color32::from_rgb(50, 50, 55) };
                                            if custom_3d_button_sized(ui, band_88.name, is_active, bg_color, egui::vec2(btn_width_2, 22.0)) {
                                                self.change_band_and_mode(band_88.default_freq, band_88.default_mode);
                                            }
                                        });
                                    },
                                    RightTab::Memoires => {
                                        if self.memories.is_empty() { ui.label("Aucune mémoire."); } else {
                                            let mut categories: Vec<String> = self.memories.iter().map(|m| m.category.clone()).collect();
                                            categories.sort(); categories.dedup();
                                            let mut action_recall = None;
                                            for cat in categories {
                                                ui.label(RichText::new(&cat).color(Color32::from_rgb(255, 200, 100)).strong());
                                                let entries: Vec<DbMemoryEntry> = self.memories.iter().filter(|m| m.category == cat).cloned().collect();
                                                for entry in entries {
                                                    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 24.0), egui::Sense::click());
                                                    let is_active = (self.frequency as i64 - entry.frequency as i64).abs() <= 1000;
                                                    let painter = ui.painter();
                                                    if is_active { painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(46, 125, 50, 40)); }
                                                    else if response.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); painter.rect_filled(rect, 4.0, Color32::from_white_alpha(12)); }
                                                    ui.allocate_ui_at_rect(rect, |ui| {
                                                        ui.horizontal(|ui| {
                                                            let mode_str = match entry.mode { RadioMode::Usb => "USB", RadioMode::Lsb => "LSB", RadioMode::Am => "AM", RadioMode::Fm => "FM", RadioMode::Cw => "CW" };
                                                            ui.label(RichText::new(format!("[{}]", mode_str)).color(Color32::DARK_GRAY).size(10.0));
                                                            ui.label(RichText::new(format!("{:06.3}", entry.frequency as f64 / 1_000_000.0)).strong().color(Color32::WHITE));
                                                            ui.label(RichText::new(&entry.name).size(11.0));
                                                        });
                                                    });
                                                    if response.clicked() { action_recall = Some(entry); }
                                                }
                                            }
                                            if let Some(entry) = action_recall { self.recall_memory(entry.frequency, entry.mode, entry.is_data, entry.filter, entry.preamp); }
                                        }
                                    },
                                    RightTab::Eibi => {
                                        ui.horizontal(|ui| {
                                            if self.eibi_rx_status.is_none() {
                                                if ui.small_button("📥 Télécharger").clicked() {
                                                    let (tx, rx) = unbounded::<String>(); self.eibi_rx_status = Some(rx); download_and_import_eibi(tx, ctx.clone());
                                                }
                                            } else { ui.spinner(); }
                                        });
                                        ui.horizontal(|ui| {
                                            let response = ui.add(egui::TextEdit::singleline(&mut self.eibi_search_query).desired_width(120.0).hint_text("Fréq / Station"));
                                            if ui.button("🔍").clicked() || (response.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter))) { self.refresh_eibi_results(); }
                                        });
                                        let mut clicked_eibi_freq = None;
                                        for entry in &self.eibi_search_results {
                                            let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 24.0), egui::Sense::click());
                                            let painter = ui.painter();
                                            if response.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); painter.rect_filled(rect, 4.0, Color32::from_white_alpha(12)); }
                                            ui.allocate_ui_at_rect(rect, |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label(RichText::new(format!("{:06.3}", entry.frequency as f64 / 1_000_000.0)).strong());
                                                    ui.label(RichText::new(&entry.station).size(11.0));
                                                });
                                            });
                                            if response.clicked() { clicked_eibi_freq = Some(entry.frequency); }
                                        }
                                        if let Some(freq) = clicked_eibi_freq { self.change_band_and_mode(freq, RadioMode::Am); }
                                    }
                                }
                            });
                        });

                        ui.add_space(10.0);

                        // Stations Probables
                        panel_frame.show(ui, |ui| {
                            ui.label(RichText::new(format!("STATIONS PROBABLES (UTC {:02}:{:02})", utc_hour, utc_min)).strong().color(Color32::from_rgb(100, 200, 255)));
                            ui.add_space(6.0);
                            let matching_memories: Vec<DbMemoryEntry> = self.memories.iter().filter(|m| (m.frequency as i64 - self.frequency as i64).abs() <= 2000).cloned().collect();
                            if self.probable_stations.is_empty() && matching_memories.is_empty() {
                                ui.label(RichText::new("Aucune station à ±2 kHz.").italics().color(Color32::GRAY));
                            } else {
                                ScrollArea::vertical().id_source("probable_scroll").max_height(280.0).show(ui, |ui| {
                                    let mut recall_mem = None;
                                    for mem in &matching_memories {
                                        ui.horizontal(|ui| {
                                            let text_width = ui.available_width() - 32.0;
                                            let (rect, response) = ui.allocate_exact_size(egui::vec2(text_width, 24.0), egui::Sense::click());
                                            let painter = ui.painter();
                                            if response.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); painter.rect_filled(rect, 4.0, Color32::from_white_alpha(10)); }
                                            painter.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgba_unmultiplied(100, 200, 255, 120)));
                                            painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(100, 200, 255, 20));
                                            ui.allocate_ui_at_rect(rect, |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label(RichText::new("★ MEM").color(Color32::from_rgb(100, 200, 255)).size(10.0).strong());
                                                    ui.label(RichText::new(format!("{:06.3}", mem.frequency as f64 / 1_000_000.0)).strong().color(Color32::WHITE));
                                                    ui.label(RichText::new(&mem.name).strong().size(11.0));
                                                });
                                            });
                                            if response.clicked() { recall_mem = Some(mem.clone()); }
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if ui.small_button("ℹ").on_hover_text("Rechercher la station sur Google").clicked() {
                                                    ctx.open_url(egui::OpenUrl::new_tab(format!("https://www.google.com/search?q={}", url_encode(&format!("{} {} kHz", mem.name, mem.frequency / 1000)))));
                                                }
                                            });
                                        });
                                        ui.add_space(4.0);
                                    }
                                    if let Some(m) = recall_mem { self.recall_memory(m.frequency, m.mode, m.is_data, m.filter, m.preamp); }

                                    let mut tune_to_eibi_freq = None;
                                    for entry in &self.probable_stations {
                                        let is_on_air = is_time_in_range(&entry.time, utc_hour, utc_min);
                                        ui.horizontal(|ui| {
                                            let text_width = ui.available_width() - 32.0;
                                            let (rect, response) = ui.allocate_exact_size(egui::vec2(text_width, 24.0), egui::Sense::click());
                                            let painter = ui.painter();
                                            if is_on_air {
                                                painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(46, 125, 50, 45));
                                                painter.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgba_unmultiplied(46, 125, 50, 150)));
                                            } else if response.hovered() {
                                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); painter.rect_filled(rect, 4.0, Color32::from_white_alpha(10));
                                            }
                                            ui.allocate_ui_at_rect(rect, |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label(RichText::new(if is_on_air { "●" } else { "○" }).color(if is_on_air { Color32::GREEN } else { Color32::GRAY }).size(10.0));
                                                    ui.label(RichText::new(format!("[{}]", entry.time)).color(Color32::YELLOW).size(10.0));
                                                    ui.label(RichText::new(&entry.station).strong().color(if is_on_air { Color32::WHITE } else { Color32::GRAY }).size(11.0));
                                                });
                                            });
                                            if response.clicked() { tune_to_eibi_freq = Some(entry.frequency); }
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if ui.small_button("ℹ").on_hover_text("Rechercher la station sur Google").clicked() {
                                                    ctx.open_url(egui::OpenUrl::new_tab(format!("https://www.google.com/search?q={}", url_encode(&format!("{} {} kHz", entry.station, entry.frequency / 1000)))));
                                                }
                                            });
                                        });
                                        ui.add_space(4.0);
                                    }
                                    if let Some(freq) = tune_to_eibi_freq { self.change_band_and_mode(freq, RadioMode::Am); }
                                });
                            }
                        });
                    });
                });
            });
        });

        // --- Raccourcis claviers ---
        let is_typing = ctx.memory(|mem| mem.focused().is_some());
        let mouse_over_app = ctx.input(|i| i.pointer.hover_pos().is_some());

        if mouse_over_app && !is_typing {
            if self.tx_lock {
                if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                    self.last_user_write = Instant::now(); self.is_tx = !self.is_tx;
                    if self.is_connected { self.send_cmd(Command::SetPTT(self.is_tx)); }
                    ctx.request_repaint();
                }
            } else {
                if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                    if !self.is_tx {
                        self.last_user_write = Instant::now(); self.is_tx = true;
                        if self.is_connected { self.send_cmd(Command::SetPTT(self.is_tx)); }
                        ctx.request_repaint();
                    }
                }
                if ctx.input(|i| i.key_released(egui::Key::Space)) {
                    if self.is_tx {
                        self.last_user_write = Instant::now(); self.is_tx = false;
                        if self.is_connected { self.send_cmd(Command::SetPTT(self.is_tx)); }
                        ctx.request_repaint();
                    }
                }
            }
        }

        if self.vfo_hovered {
            ctx.request_repaint_after(Duration::from_millis(150));
            let scroll_delta = ctx.input(|i| i.raw_scroll_delta.y);
            if scroll_delta != 0.0 {
                let ticks = if scroll_delta > 0.0 { 1 } else { -1 };
                let new_freq = self.frequency as i64 + (ticks * self.vfo_step as i64);
                self.set_frequency_from_i64(new_freq);
            }

            if !is_typing {
                let steps = [1, 10, 100, 1_000, 10_000, 100_000, 1_000_000];
                if let Some(current_idx) = steps.iter().position(|&s| s == self.vfo_step) {
                    if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) { if current_idx < steps.len() - 1 { self.vfo_step = steps[current_idx + 1]; } }
                    if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) { if current_idx > 0 { self.vfo_step = steps[current_idx - 1]; } }
                }
                if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) { let new_freq = self.frequency as i64 + self.vfo_step as i64; self.set_frequency_from_i64(new_freq); }
                if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) { let new_freq = self.frequency as i64 - self.vfo_step as i64; self.set_frequency_from_i64(new_freq); }
            }
        }
    }
}