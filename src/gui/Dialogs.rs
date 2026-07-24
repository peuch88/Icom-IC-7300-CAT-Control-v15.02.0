// src/gui/dialogs.rs

use eframe::egui::{self, Color32, RichText, ScrollArea, Stroke};
use std::time::Instant;
use crate::comm::{RadioMode, Command};
use crate::gui::app::{Ic7300App, VERSION};
use crate::gui::widgets::custom_gain_slider;
use crate::database::{
    db_delete_memory, db_update_memory, db_add_memory, init_and_load_db,
    export_settings_csv, import_settings_csv, export_memories_csv,
    import_memories_csv, export_eibi_csv, import_eibi_csv, db_load_settings,
    DbMemoryEntry,
};

impl Ic7300App {
    pub fn show_wiki_window(&mut self, ctx: &egui::Context) {
        let mut show_info = self.show_info_window;
        if show_info {
            egui::Window::new(format!("Mini-Wiki - Guide d'Utilisation v{}", VERSION)).open(&mut show_info).collapsible(true).resizable(true).default_size([540.0, 420.0]).show(ctx, |ui| {
                ScrollArea::vertical().id_source("info_scroll").max_height(350.0).show(ui, |ui| {
                    ui.label(RichText::new(&self.info_text).font(egui::FontId::monospace(13.0)));
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Recharger info.txt").clicked() { if let Ok(content) = std::fs::read_to_string("info.txt") { self.info_text = content; } }
                    ui.label(RichText::new("(Modifiez le fichier \"info.txt\" de l'application pour éditer ce texte)").italics().color(Color32::GRAY).size(11.0));
                });
            });
        }
        self.show_info_window = show_info;
    }

    pub fn show_config_window(&mut self, ctx: &egui::Context) {
        let mut show_config = self.show_config_window;
        if show_config {
            egui::Window::new("Paramètres de Communication").open(&mut show_config).collapsible(false).resizable(false).show(ctx, |ui| {
                ui.add_enabled_ui(!self.is_connected, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Port Série :");
                        egui::ComboBox::from_id_source("port_cb").selected_text(&self.port_name).width(150.0).show_ui(ui, |ui| {
                            if self.available_ports.is_empty() { ui.label("Aucun port détecté."); } else {
                                for p in &self.available_ports { 
                                    ui.selectable_value(&mut self.port_name, p.clone(), p.as_str()); 
                                }
                            }
                        });
                        if ui.button("Rafraîchir").clicked() { self.refresh_com_ports(); }
                    });
                    ui.add_space(5.0);
                    ui.horizontal(|ui| { ui.label("Port Manuel :"); ui.text_edit_singleline(&mut self.port_name); });
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.label("Baud Rate :");
                        egui::ComboBox::from_id_source("baud_cb").selected_text(self.baud_rate.to_string()).width(150.0).show_ui(ui, |ui| {
                            for &b in &[4800, 9600, 19200, 38400, 57600, 115200] { ui.selectable_value(&mut self.baud_rate, b, b.to_string()); }
                        });
                    });
                });
                if self.is_connected {
                    ui.add_space(10.0);
                    ui.label(RichText::new("Veuillez vous déconnecter pour modifier ces paramètres.").color(Color32::from_rgb(255, 150, 0)));
                }
            });
        }
        self.show_config_window = show_config;
    }

    pub fn show_memories_window(&mut self, ctx: &egui::Context) {
        let mut show_mem = self.show_mem_manager;
        if show_mem {
            egui::Window::new("Gestionnaire des Mémoires SQL").open(&mut show_mem).collapsible(false).resizable(true).default_size([650.0, 500.0]).show(ctx, |ui| {
                ui.group(|ui| {
                    let title = if self.mem_editing_id.is_some() { "Modifier la mémoire" } else { "➕ Ajouter une mémoire" };
                    ui.label(RichText::new(title).strong().color(Color32::from_rgb(100, 200, 255)));
                    ui.add_space(5.0);
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label("Catégorie / Bande :");
                            ui.text_edit_singleline(&mut self.mem_edit_category);
                            ui.horizontal(|ui| {
                                if ui.small_button("Urgence").clicked() { self.mem_edit_category = "🔴 URGENCE & SÉCURITÉ HF".to_owned(); }
                                if ui.small_button("SWL").clicked() { self.mem_edit_category = "📻 RADIODIFFUSION (SWL)".to_owned(); }
                                if ui.small_button("Aéro").clicked() { self.mem_edit_category = "✈️ AVIATION & VOLMET".to_owned(); }
                                if ui.small_button("Heure").clicked() { self.mem_edit_category = "⏰ SIGNAUX HORAIRES".to_owned(); }
                            });
                        });
                        ui.add_space(10.0);
                        ui.vertical(|ui| { ui.label("Description / Nom :"); ui.text_edit_singleline(&mut self.mem_edit_name); });
                    });
                    ui.add_space(5.0);
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| { ui.label("Fréquence (MHz) :"); ui.text_edit_singleline(&mut self.mem_edit_freq_mhz); });
                        ui.add_space(10.0);
                        ui.vertical(|ui| {
                            ui.label("Mode :");
                            egui::ComboBox::from_id_source("edit_mode_cb").selected_text(format!("{:?}", self.mem_edit_mode)).width(100.0).show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.mem_edit_mode, RadioMode::Lsb, "LSB");
                                ui.selectable_value(&mut self.mem_edit_mode, RadioMode::Usb, "USB");
                                ui.selectable_value(&mut self.mem_edit_mode, RadioMode::Am, "AM");
                                ui.selectable_value(&mut self.mem_edit_mode, RadioMode::Cw, "CW");
                                ui.selectable_value(&mut self.mem_edit_mode, RadioMode::Fm, "FM");
                            });
                        });
                    });
                    ui.add_space(5.0);
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.mem_edit_is_data, "Mode DATA (D)");
                        ui.add_space(10.0);
                        ui.label("Filtre :");
                        ui.radio_value(&mut self.mem_edit_filter, 1, "FIL1");
                        ui.radio_value(&mut self.mem_edit_filter, 2, "FIL2");
                        ui.radio_value(&mut self.mem_edit_filter, 3, "FIL3");
                        ui.add_space(10.0);
                        ui.label("P.AMP :");
                        ui.radio_value(&mut self.mem_edit_preamp, 0, "OFF");
                        ui.radio_value(&mut self.mem_edit_preamp, 1, "AMP1");
                        ui.radio_value(&mut self.mem_edit_preamp, 2, "AMP2");
                    });
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let parsed_freq = self.mem_edit_freq_mhz.replace(",", ".").parse::<f64>();
                        let is_valid = parsed_freq.is_ok() && !self.mem_edit_category.trim().is_empty() && !self.mem_edit_name.trim().is_empty();
                        ui.add_enabled_ui(is_valid, |ui| {
                            if let Some(id) = self.mem_editing_id {
                                if ui.button("Enregistrer les Modifications").clicked() {
                                    if let Ok(mhz) = parsed_freq {
                                        let hz = (mhz * 1_000_000.0) as u64;
                                        let _ = db_update_memory(id, &self.mem_edit_category, &self.mem_edit_name, hz, self.mem_edit_mode, self.mem_edit_is_data, self.mem_edit_filter, self.mem_edit_preamp);
                                        self.memories = init_and_load_db(); self.mem_editing_id = None; self.mem_edit_name.clear(); self.mem_edit_freq_mhz.clear();
                                    }
                                }
                            } else {
                                if ui.button("➕ Ajouter à la Base").clicked() {
                                    if let Ok(mhz) = parsed_freq {
                                        let hz = (mhz * 1_000_000.0) as u64;
                                        let _ = db_add_memory(&self.mem_edit_category, &self.mem_edit_name, hz, self.mem_edit_mode, self.mem_edit_is_data, self.mem_edit_filter, self.mem_edit_preamp);
                                        self.memories = init_and_load_db(); self.mem_edit_name.clear(); self.mem_edit_freq_mhz.clear();
                                    }
                                }
                            }
                        });
                        if ui.button("Capturer l'état actuel").on_hover_text("Remplit le formulaire avec l'état en direct.").clicked() {
                            self.mem_edit_freq_mhz = format!("{:.6}", self.frequency as f64 / 1_000_000.0);
                            self.mem_edit_mode = self.mode; self.mem_edit_is_data = self.is_data_mode; self.mem_edit_filter = self.filter; self.mem_edit_preamp = self.preamp;
                        }
                        if self.mem_editing_id.is_some() {
                            if ui.button("Annuler").clicked() { self.mem_editing_id = None; self.mem_edit_name.clear(); self.mem_edit_freq_mhz.clear(); }
                        }
                    });
                });
                ui.add_space(10.0); ui.separator();
                
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Liste des Mémoires Actuelles").strong().color(Color32::LIGHT_GRAY));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("➕ Tout Déplier").clicked() { self.categories_force_open = Some(true); }
                        if ui.small_button("➖ Tout Replier").clicked() { self.categories_force_open = Some(false); }
                    });
                });
                
                ScrollArea::vertical().id_source("manager_scroll").max_height(210.0).show(ui, |ui| {
                    let mut categories: Vec<String> = self.memories.iter().map(|m| m.category.clone()).collect();
                    categories.sort(); categories.dedup();
                    if categories.is_empty() { ui.label("Aucune mémoire enregistrée."); }
                    
                    let mut action_recall = None;
                    for cat in categories {
                        let id = ui.make_persistent_id(&cat);
                        let mut collapsing_state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);
                        if let Some(force) = self.categories_force_open { collapsing_state.set_open(force); }

                        collapsing_state.show_header(ui, |ui| { ui.strong(&cat); })
                        .body(|ui| {
                            let entries: Vec<DbMemoryEntry> = self.memories.iter().filter(|m| m.category == cat).cloned().collect();
                            for entry in entries {
                                let mut btn_clicked = false;
                                let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::click());
                                let is_active = (self.frequency as i64 - entry.frequency as i64).abs() <= 1000;
                                let painter = ui.painter();
                                if is_active {
                                    painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(46, 125, 50, 40));
                                    painter.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgba_unmultiplied(46, 125, 50, 120)));
                                } else if response.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    painter.rect_filled(rect, 4.0, Color32::from_white_alpha(12));
                                }
                                ui.allocate_ui_at_rect(rect, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(format!("{:07.3} MHz", entry.frequency as f64 / 1_000_000.0)).strong().color(Color32::WHITE));
                                        let mode_str = match entry.mode { RadioMode::Usb => "USB", RadioMode::Lsb => "LSB", RadioMode::Am => "AM", RadioMode::Fm => "FM", RadioMode::Cw => "CW" };
                                        let secondary_text = if entry.is_data { format!("[{}-D / FIL{} / AMP{}]", mode_str, entry.filter, entry.preamp) } else { format!("[{} / FIL{} / AMP{}]", mode_str, entry.filter, entry.preamp) };
                                        ui.label(RichText::new(secondary_text).color(Color32::DARK_GRAY).size(11.0));
                                        ui.label(&entry.name);
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.button("🗑").on_hover_text("Supprimer").clicked() { let _ = db_delete_memory(entry.id); self.memories = init_and_load_db(); btn_clicked = true; }
                                            if ui.button("✏").on_hover_text("Modifier").clicked() {
                                                self.mem_editing_id = Some(entry.id); self.mem_edit_category = entry.category.clone(); self.mem_edit_name = entry.name.clone();
                                                self.mem_edit_freq_mhz = format!("{:.6}", entry.frequency as f64 / 1_000_000.0); self.mem_edit_mode = entry.mode;
                                                self.mem_edit_is_data = entry.is_data; self.mem_edit_filter = entry.filter; self.mem_edit_preamp = entry.preamp; btn_clicked = true;
                                            }
                                        });
                                    });
                                });
                                if response.clicked() && !btn_clicked { action_recall = Some(entry); }
                                ui.add_space(4.0);
                            }
                        });
                    }
                    if let Some(entry) = action_recall { self.recall_memory(entry.frequency, entry.mode, entry.is_data, entry.filter, entry.preamp); }
                });
                self.categories_force_open = None;
            });
        }
        self.show_mem_manager = show_mem;
    }

    pub fn show_csv_window(&mut self, ctx: &egui::Context) {
        let mut show_csv = self.show_csv_manager;
        if show_csv {
            egui::Window::new("Importateur / Exportateur CSV").open(&mut show_csv).collapsible(false).resizable(false).default_size([420.0, 360.0]).show(ctx, |ui| {
                ui.label(RichText::new("Gestion des Backups CSV").strong().color(Color32::from_rgb(100, 200, 255)));
                ui.add_space(5.0);
                ui.group(|ui| {
                    ui.label(RichText::new("Réglages (settings)").strong());
                    ui.horizontal(|ui| { ui.label("Fichier :"); ui.text_edit_singleline(&mut self.csv_settings_path); });
                    ui.horizontal(|ui| {
                        if ui.button("Exporter").clicked() {
                            match export_settings_csv(&self.csv_settings_path) {
                                Ok(_) => self.csv_status = "Réglages exportés !".to_owned(),
                                Err(e) => self.csv_status = format!("Erreur : {}", e),
                            }
                        }
                        if ui.button("Importer").clicked() {
                            match import_settings_csv(&self.csv_settings_path) {
                                Ok(_) => {
                                    self.csv_status = "Réglages importés !".to_owned();
                                    let saved = db_load_settings();
                                    if let Some(val) = saved.get("port_name") { self.port_name = val.clone(); }
                                    if let Some(val) = saved.get("baud_rate") { if let Ok(parsed) = val.parse() { self.baud_rate = parsed; } }
                                    if let Some(val) = saved.get("frequency") { if let Ok(parsed) = val.parse() { self.set_frequency_from_i64(parsed); } }
                                    if let Some(val) = saved.get("tx_lock") { self.tx_lock = val == "1"; }
                                }
                                Err(e) => self.csv_status = format!("Erreur : {}", e),
                            }
                        }
                    });
                });
                ui.add_space(5.0);
                ui.group(|ui| {
                    ui.label(RichText::new("Mémoires (memories)").strong());
                    ui.horizontal(|ui| { ui.label("Fichier :"); ui.text_edit_singleline(&mut self.csv_memories_path); });
                    ui.horizontal(|ui| {
                        if ui.button("Exporter").clicked() {
                            match export_memories_csv(&self.csv_memories_path) {
                                Ok(_) => self.csv_status = "Mémoires exportées !".to_owned(), Err(e) => self.csv_status = format!("Erreur : {}", e),
                            }
                        }
                        if ui.button("Importer").clicked() {
                            match import_memories_csv(&self.csv_memories_path) {
                                Ok(_) => { self.csv_status = "Mémoires restaurées !".to_owned(); self.memories = init_and_load_db(); }
                                Err(e) => self.csv_status = format!("Erreur : {}", e),
                            }
                        }
                    });
                });
                ui.add_space(5.0);
                ui.group(|ui| {
                    ui.label(RichText::new("Base mondiale EiBi (eibi)").strong());
                    ui.horizontal(|ui| { ui.label("Fichier :"); ui.text_edit_singleline(&mut self.csv_eibi_path); });
                    ui.horizontal(|ui| {
                        if ui.button("Exporter").clicked() {
                            match export_eibi_csv(&self.csv_eibi_path) { Ok(_) => self.csv_status = "EiBi exportée !".to_owned(), Err(e) => self.csv_status = format!("Erreur : {}", e), }
                        }
                        if ui.button("Importer").clicked() {
                            match import_eibi_csv(&self.csv_eibi_path) { Ok(_) => { self.csv_status = "EiBi importée !".to_owned(); self.refresh_eibi_results(); } Err(e) => self.csv_status = format!("Erreur : {}", e), }
                        }
                    });
                });
                ui.add_space(10.0); ui.separator();
                ui.label(RichText::new(format!("Statut : {}", self.csv_status)).italics().color(Color32::YELLOW));
            });
        }
        self.show_csv_manager = show_csv;
    }

    pub fn show_gains_window(&mut self, ctx: &egui::Context) {
        let mut show_gains = self.show_gains_window;
        let mut close_clicked = false;
        if show_gains {
            egui::Window::new("Réglages Gains & Niveaux")
                .open(&mut show_gains) 
                .collapsible(false)
                .resizable(false)
                .default_width(320.0)
                .show(ctx, |ui| { 
                    ui.vertical(|ui| {
                        let dark_blue = Color32::from_rgb(13, 71, 161);
                        let dark_red = Color32::from_rgb(183, 28, 28);

                        // Réception
                        if custom_gain_slider(ui, &mut self.af_gain, "Volume / AF Gain", dark_blue) {
                            self.last_user_write = Instant::now(); if self.is_connected { self.send_cmd(Command::SetAfGain(self.af_gain)); }
                        }
                        ui.add_space(8.0);
                        if custom_gain_slider(ui, &mut self.rf_gain, "Gain Réception RF", dark_blue) {
                            self.last_user_write = Instant::now(); if self.is_connected { self.send_cmd(Command::SetRfGain(self.rf_gain)); }
                        }
                        ui.add_space(8.0);
                        if custom_gain_slider(ui, &mut self.squelch, "Squelch", dark_blue) {
                            self.last_user_write = Instant::now(); if self.is_connected { self.send_cmd(Command::SetSquelch(self.squelch)); }
                        }
                        
                        // Réduction de bruit & parasites (NB & NR)
                        ui.add_space(8.0);
                        if custom_gain_slider(ui, &mut self.noise_blanker_level, "Niveau Noise Blanker (NB)", dark_blue) {
                            self.last_user_write = Instant::now(); if self.is_connected { self.send_cmd(Command::SetNoiseBlankerLevel(self.noise_blanker_level)); }
                        }
                        ui.add_space(8.0);
                        if custom_gain_slider(ui, &mut self.noise_reduction_level, "Niveau Noise Reduction (NR)", dark_blue) {
                            self.last_user_write = Instant::now(); if self.is_connected { self.send_cmd(Command::SetNoiseReductionLevel(self.noise_reduction_level)); }
                        }

                        // Émission & Modulation
                        ui.add_space(8.0);
                        if custom_gain_slider(ui, &mut self.mic_gain, "Gain Microphone", dark_blue) {
                            self.last_user_write = Instant::now(); if self.is_connected { self.send_cmd(Command::SetMicGain(self.mic_gain)); }
                        }
                        ui.add_space(8.0);
                        if custom_gain_slider(ui, &mut self.comp_level, "Niveau Compresseur", dark_blue) {
                            self.last_user_write = Instant::now(); if self.is_connected { self.send_cmd(Command::SetCompLevel(self.comp_level)); }
                        }
                        ui.add_space(8.0);
                        if custom_gain_slider(ui, &mut self.monitor_level, "Niveau Moniteur", dark_blue) {
                            self.last_user_write = Instant::now(); if self.is_connected { self.send_cmd(Command::SetMonitorLevel(self.monitor_level)); }
                        }
                        ui.add_space(8.0);
                        if custom_gain_slider(ui, &mut self.rf_power, "Puissance Émission RF", dark_red) {
                            self.last_user_write = Instant::now(); if self.is_connected { self.send_cmd(Command::SetRfPower(self.rf_power)); }
                        }
                        
                        // Carte son USB (RX / TX)
                        ui.add_space(8.0);
                        if custom_gain_slider(ui, &mut self.usb_rx_level, "Volume Carte Son USB (RX)", dark_blue) {
                            self.last_user_write = Instant::now(); if self.is_connected { self.send_cmd(Command::SetUsbRxLevel(self.usb_rx_level)); }
                        }
                        ui.add_space(8.0);
                        if custom_gain_slider(ui, &mut self.usb_tx_level, "Modulation Carte Son USB (TX)", dark_red) {
                            self.last_user_write = Instant::now(); if self.is_connected { self.send_cmd(Command::SetUsbTxLevel(self.usb_tx_level)); }
                        }
                        ui.add_space(12.0);
                        ui.vertical_centered_justified(|ui| { if ui.button("Fermer").clicked() { close_clicked = true; } });
                    });
                });
        }
        if close_clicked { show_gains = false; }
        self.show_gains_window = show_gains;
    }
}