// Version : V15.03.37 - Retrait définitif et rigoureux de toutes les méthodes convert_samples résiduelles pour rodio 0.22
// Module de gestion de l'automate de manipulation de messages et balises vocales/données pour l'Icom IC-7300

use eframe::egui::{self, Color32, RichText};
use std::time::{Duration, Instant};
use rodio::Source; // Requis pour extraire la durée du décodeur
use rodio::cpal::traits::{HostTrait, DeviceTrait}; // Requis pour énumérer les cartes son physiques du système

use crate::gui::app::Ic7300App;
use crate::comm::Command;
use crate::gui::widgets::custom_3d_button_sized;

#[derive(Clone, Debug)]
pub struct KeyerMemory {
    pub label: String,
    pub duration_secs: f32, // Durée d'émission (TX) en secondes
    pub interval_secs: f32, // Intervalle de réception (RX) en secondes
    pub data_mode: bool,     // Si vrai, force l'activation du mode DATA (-D) lors de l'émission
    pub mp3_path: String,    // Chemin facultatif vers un fichier MP3 pour la lecture automatique
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum KeyerStage {
    Idle,
    Transmitting { start_time: Instant, duration: Duration },
    Listening { start_time: Instant, duration: Duration },
}

pub struct KeyerState {
    pub show_window: bool,
    pub memories: Vec<KeyerMemory>,
    pub selected_idx: usize,
    pub current_stage: KeyerStage,
    pub is_active: bool,
    pub audio_error: Option<String>, // Stocke les messages d'erreurs pour l'affichage IHM
    pub active_device_name: String,  // Nom de la carte son actuellement ciblée
    pub chosen_device_name: String,  // Option de sélection manuelle ou automatique
    
    // Garde en mémoire le périphérique de sortie et le Player actif pour l'interrompre proprement
    pub audio_playback: Option<(rodio::MixerDeviceSink, rodio::Player)>,
}

impl KeyerState {
    pub fn new() -> Self {
        let mut memories = Vec::with_capacity(20);
        for i in 1..=20 {
            memories.push(KeyerMemory {
                label: format!("Slot Mémoire #{}", i),
                duration_secs: 5.0,
                interval_secs: 10.0,
                data_mode: true,
                mp3_path: "".to_owned(),
            });
        }
        
        let mut state = Self {
            show_window: false,
            memories,
            selected_idx: 0,
            current_stage: KeyerStage::Idle,
            is_active: false,
            audio_error: None,
            active_device_name: "Recherche...".to_owned(),
            chosen_device_name: "Sélection automatique".to_owned(),
            audio_playback: None,
        };
        
        state.refresh_device_name();
        state
    }

    /// Analyse et actualise le nom de la carte son ciblée
    pub fn refresh_device_name(&mut self) {
        if self.chosen_device_name == "Sélection automatique" {
            if let Some(device) = find_icom_device() {
                if let Ok(desc) = device.description() {
                    self.active_device_name = format!("Auto (Icom) : {}", desc.name());
                    return;
                }
            }
        } else {
            if let Some(device) = find_device_by_name(&self.chosen_device_name) {
                if let Ok(desc) = device.description() {
                    self.active_device_name = format!("Manuel : {}", desc.name());
                    return;
                }
            }
        }
        
        let host = rodio::cpal::default_host();
        if let Some(device) = host.default_output_device() {
            if let Ok(desc) = device.description() {
                self.active_device_name = format!("Système (Défaut) : {}", desc.name());
                return;
            }
        }
        
        self.active_device_name = "Aucune carte son détectée !".to_owned();
    }
}

/// Recherche de la carte son physique USB Codec de l'Icom IC-7300 parmi les périphériques raccordés
fn find_icom_device() -> Option<rodio::Device> {
    let host = rodio::cpal::default_host();
    if let Ok(devices) = host.output_devices() {
        for device in devices {
            // rodio 0.22 / cpal 0.17+ : utilise description() pour obtenir les métadonnées de l'appareil
            if let Ok(desc) = device.description() {
                // Utilise la méthode publique desc.name() au lieu du champ privé desc.name
                let name_lower = desc.name().to_lowercase();
                // Recherche les mot-clés d'identification de l'interface Codec de l'Icom
                if name_lower.contains("codec") || name_lower.contains("icom") || name_lower.contains("7300") {
                    return Some(device.into());
                }
            }
        }
    }
    None
}

/// Recherche d'un périphérique audio physique à partir de son nom indexé unique
fn find_device_by_name(target_name: &str) -> Option<rodio::Device> {
    if target_name == "Sélection automatique" {
        return find_icom_device();
    }
    let host = rodio::cpal::default_host();
    if let Ok(devices) = host.output_devices() {
        for (i, device) in devices.enumerate() {
            if let Ok(desc) = device.description() {
                let unique_name = format!("{}. {}", i + 1, desc.name());
                if unique_name == target_name {
                    return Some(device.into());
                }
            }
        }
    }
    None
}

/// Énumère tous les périphériques de sortie physiques et leur attribue un index absolu unique
fn get_available_devices_unique_names() -> Vec<String> {
    let mut names = vec!["Sélection automatique".to_owned()];
    let host = rodio::cpal::default_host();
    if let Ok(devices) = host.output_devices() {
        for (i, device) in devices.enumerate() {
            if let Ok(desc) = device.description() {
                names.push(format!("{}. {}", i + 1, desc.name()));
            }
        }
    }
    names
}

impl Ic7300App {
    /// Exécute la logique temporelle de l'automate d'appel à chaque rafraîchissement d'IHM
    pub fn update_keyer_logic(&mut self, ctx: &egui::Context) {
        if !self.keyer_state.is_active {
            return;
        }

        // Force la boucle d'egui à repasser fréquemment pour cadencer précisément les chronomètres
        ctx.request_repaint_after(Duration::from_millis(50));

        let now = Instant::now();
        let sel_idx = self.keyer_state.selected_idx;
        let memory = &self.keyer_state.memories[sel_idx];

        match self.keyer_state.current_stage {
            KeyerStage::Idle => {
                // Entrée dans le premier cycle : Émission (TX)
                self.keyer_state.current_stage = KeyerStage::Transmitting {
                    start_time: now,
                    duration: Duration::from_secs_f32(memory.duration_secs),
                };

                // Commutation automatique du mode DATA s'il est requis
                if memory.data_mode {
                    self.is_data_mode = true;
                    self.last_user_write = now;
                    if self.is_connected {
                        self.send_cmd(Command::SetDataMode(true));
                    }
                }

                // Activation automatique du PTT
                self.is_tx = true;
                self.last_user_write = now;
                if self.is_connected {
                    self.send_cmd(Command::SetPTT(true));
                }

                // Démarrage de la lecture audio MP3 asynchrone si configuré
                if !memory.mp3_path.trim().is_empty() {
                    // Sélection intelligente du périphérique physique indexé
                    let builder_res = if let Some(device) = find_device_by_name(&self.keyer_state.chosen_device_name) {
                        rodio::DeviceSinkBuilder::from_device(device)
                            .and_then(|builder| builder.open_sink_or_fallback())
                    } else {
                        rodio::DeviceSinkBuilder::open_default_sink()
                    };

                    match builder_res {
                        Ok(mut handle) => {
                            handle.log_on_drop(false); // Désactive la notification de drop en console
                            self.keyer_state.audio_error = None;
                            let player = rodio::Player::connect_new(&handle.mixer());
                            player.set_volume(1.0); // Force le volume à 100%
                            
                            match std::fs::File::open(&memory.mp3_path) {
                                Ok(file) => {
                                    match rodio::Decoder::try_from(file) {
                                        Ok(source) => {
                                            player.append(source); // Plus besoin de convert_samples, f32 natif sous le capot
                                            // Conserve les instances actives en mémoire (la libération coupera instantanément le flux)
                                            self.keyer_state.audio_playback = Some((handle, player));
                                        }
                                        Err(e) => {
                                            self.keyer_state.audio_error = Some(format!("Erreur décodage MP3 : {}", e));
                                            self.keyer_state.audio_playback = None;
                                        }
                                    }
                                }
                                Err(e) => {
                                    self.keyer_state.audio_error = Some(format!("Fichier audio introuvable : {}", e));
                                    self.keyer_state.audio_playback = None;
                                }
                            }
                        }
                        Err(e) => {
                            self.keyer_state.audio_error = Some(format!("Initialisation de la carte son échouée : {}", e));
                            self.keyer_state.audio_playback = None;
                        }
                    }
                }
            }
            KeyerStage::Transmitting { start_time, duration } => {
                // --- SURVEILLANCE ARRET AUTOMATIQUE ---
                // Si l'état d'émission PTT a été manuellement désactivé par l'utilisateur
                if !self.is_tx {
                    self.abort_keyer_and_disable_data();
                    return;
                }

                // Fin de la durée d'émission planifiée
                if now.duration_since(start_time) >= duration {
                    // Arrêt et libération immédiate du flux audio MP3 par libération RAII
                    self.keyer_state.audio_playback = None;

                    // Passage en écoute (RX)
                    self.keyer_state.current_stage = KeyerStage::Listening {
                        start_time: now,
                        duration: Duration::from_secs_f32(memory.interval_secs),
                    };

                    // Relâchement automatique du PTT
                    self.is_tx = false;
                    self.last_user_write = now;
                    if self.is_connected {
                        self.send_cmd(Command::SetPTT(false));
                    }
                }
            }
            KeyerStage::Listening { start_time, duration } => {
                // --- SURVEILLANCE ARRET AUTOMATIQUE ---
                // Si le PTT est déclenché manuellement par l'utilisateur durant la phase d'écoute
                if self.is_tx {
                    self.abort_keyer_and_disable_data();
                    return;
                }

                // Fin de l'intervalle d'écoute planifié
                if now.duration_since(start_time) >= duration {
                    // Reprise du cycle d'émission (TX)
                    self.keyer_state.current_stage = KeyerStage::Transmitting {
                        start_time: now,
                        duration: Duration::from_secs_f32(memory.duration_secs),
                    };

                    // Commutation automatique du mode DATA s'il est requis
                    if memory.data_mode {
                        self.is_data_mode = true;
                        self.last_user_write = now;
                        if self.is_connected {
                            self.send_cmd(Command::SetDataMode(true));
                        }
                    }

                    // Réactivation du PTT
                    self.is_tx = true;
                    self.last_user_write = now;
                    if self.is_connected {
                        self.send_cmd(Command::SetPTT(true));
                    }

                    // Redémarrage de la lecture audio MP3
                    if !memory.mp3_path.trim().is_empty() {
                        let builder_res = if let Some(device) = find_device_by_name(&self.keyer_state.chosen_device_name) {
                            rodio::DeviceSinkBuilder::from_device(device)
                                .and_then(|builder| builder.open_sink_or_fallback())
                        } else {
                            rodio::DeviceSinkBuilder::open_default_sink()
                        };

                        if let Ok(mut handle) = builder_res {
                            handle.log_on_drop(false);
                            self.keyer_state.audio_error = None;
                            let player = rodio::Player::connect_new(&handle.mixer());
                            player.set_volume(1.0);
                            
                            if let Ok(file) = std::fs::File::open(&memory.mp3_path) {
                                if let Ok(source) = rodio::Decoder::try_from(file) {
                                    player.append(source);
                                    self.keyer_state.audio_playback = Some((handle, player));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Force l'arrêt immédiat de l'automate d'appel automatique et désactive le mode DATA
    fn abort_keyer_and_disable_data(&mut self) {
        self.keyer_state.is_active = false;
        self.keyer_state.current_stage = KeyerStage::Idle;
        self.keyer_state.audio_playback = None; // Coupe instantanément le son MP3 par libération RAII
        self.is_tx = false;
        self.is_data_mode = false;
        self.last_user_write = Instant::now();
        
        if self.is_connected {
            self.send_cmd(Command::SetPTT(false));
            self.send_cmd(Command::SetDataMode(false));
        }
    }

    /// Rendu graphique de la fenêtre modale du lanceur d'appel automatique
    pub fn show_keyer_window(&mut self, ctx: &egui::Context) {
        let mut show = self.keyer_state.show_window;
        if !show { return; }

        egui::Window::new("Lanceur d'Appel Automatique & Balise")
            .open(&mut show)
            .collapsible(false)
            .resizable(true) // <--- Permet l'ajustement dynamique de la taille au besoin
            .default_width(550.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Partie Gauche : Sélections des 20 mémoires
                    ui.vertical(|ui| {
                        ui.set_width(170.0);
                        ui.set_min_height(340.0); // Force la colonne gauche à s'étirer à 340px pour s'aligner sur la droite
                        
                        ui.label(RichText::new("Mémoires d'Appel").strong().color(Color32::LIGHT_GRAY));
                        ui.add_space(4.0);
                        
                        egui::ScrollArea::vertical()
                            .id_source("keyer_slots_scroll")
                            .max_height(310.0) // Augmenté à 310px pour correspondre à l'espace minimal forcé
                            .auto_shrink([false, false]) // Empêche egui de replier la zone sur un seul élément
                            .show(ui, |ui| {
                                for i in 0..20 {
                                    let label = &self.keyer_state.memories[i].label;
                                    let is_selected = self.keyer_state.selected_idx == i;
                                    let is_currently_playing = self.keyer_state.is_active && self.keyer_state.selected_idx == i;
                                    
                                    let text = format!("{:02}. {}", i + 1, label);
                                    let color = if is_currently_playing {
                                        Color32::from_rgb(0, 230, 118) // Vert si actif
                                    } else if is_selected {
                                        Color32::from_rgb(33, 150, 243) // Bleu si sélectionné
                                    } else {
                                        Color32::WHITE
                                    };
                                    
                                    if ui.selectable_label(is_selected, RichText::new(text).color(color)).clicked() {
                                        if !self.keyer_state.is_active {
                                            self.keyer_state.selected_idx = i;
                                            self.keyer_state.audio_error = None;
                                        }
                                    }
                                }
                            });
                    });

                    ui.separator();

                    // Partie Droite : Configuration de la mémoire active et commandes de l'automate
                    ui.vertical(|ui| {
                        ui.set_width(340.0);
                        let idx = self.keyer_state.selected_idx;
                        
                        // Variables d'édition locales
                        let mut label = self.keyer_state.memories[idx].label.clone();
                        let mut duration = self.keyer_state.memories[idx].duration_secs;
                        let mut interval = self.keyer_state.memories[idx].interval_secs;
                        let mut data_mode = self.keyer_state.memories[idx].data_mode;
                        let mut mp3_path = self.keyer_state.memories[idx].mp3_path.clone();
                        
                        ui.label(RichText::new(format!("Configuration de la Mémoire #{}", idx + 1)).strong().color(Color32::from_rgb(100, 200, 255)));
                        ui.add_space(6.0);

                        ui.add_enabled_ui(!self.keyer_state.is_active, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Nom :");
                                ui.text_edit_singleline(&mut label);
                            });
                            ui.add_space(6.0);

                            // Ajout du champ pour charger un fichier MP3 avec Uploader
                            ui.horizontal(|ui| {
                                ui.label("Fichier MP3 :");
                                ui.text_edit_singleline(&mut mp3_path).on_hover_text("Chemin système relatif du fichier MP3");
                                
                                // Bouton d'upload et de copie locale asynchrone vers /samples/
                                if ui.button("Uploader 📁").on_hover_text("Sélectionne un MP3 pour le copier dans le dossier local 'samples'").clicked() {
                                    if let Some(path) = rfd::FileDialog::new()
                                        .add_filter("Audio MP3", &["mp3"])
                                        .pick_file() 
                                    {
                                        // 1. Création asynchrone sécurisée du dossier samples si inexistant
                                        let _ = std::fs::create_dir_all("samples");
                                        
                                        // 2. Copie locale vers ./samples/
                                        if let Some(filename) = path.file_name() {
                                            let dest_path = std::path::Path::new("samples").join(filename);
                                            if std::fs::copy(&path, &dest_path).is_ok() {
                                                if let Some(dest_str) = dest_path.to_str() {
                                                    mp3_path = dest_str.to_owned();
                                                    self.keyer_state.audio_error = None;
                                                    
                                                    // 3. Calcul automatique de la durée de la copie locale (sans convert_samples)
                                                    if let Ok(file) = std::fs::File::open(&mp3_path) {
                                                        if let Ok(decoder) = rodio::Decoder::try_from(file) {
                                                            if let Some(dur) = decoder.total_duration() {
                                                                duration = dur.as_secs_f32();
                                                            }
                                                        }
                                                    }

                                                    // Sauvegarde immédiate du chemin et de la durée dans la base SQLite (Uploader)
                                                    self.keyer_state.memories[idx].mp3_path = mp3_path.clone();
                                                    self.keyer_state.memories[idx].duration_secs = duration;
                                                    self.save_settings();
                                                }
                                            }
                                        }
                                    }
                                }

                                // Bouton de calcul manuel (sans convert_samples)
                                if ui.button("Calculer ⏱").on_hover_text("Force le calcul de la durée sur le fichier actuellement ciblé").clicked() {
                                    self.keyer_state.audio_error = None;
                                    if let Ok(file) = std::fs::File::open(&mp3_path) {
                                        if let Ok(decoder) = rodio::Decoder::try_from(file) {
                                            if let Some(dur) = decoder.total_duration() {
                                                duration = dur.as_secs_f32();
                                                
                                                // Sauvegarde immédiate de la durée calculée
                                                self.keyer_state.memories[idx].duration_secs = duration;
                                                self.save_settings();
                                            }
                                        }
                                    } else {
                                        self.keyer_state.audio_error = Some("Fichier introuvable pour le calcul".to_owned());
                                    }
                                }
                            });
                            ui.add_space(6.0);

                            ui.label("Durée d'émission (TX) :");
                            ui.add(egui::Slider::new(&mut duration, 1.0..=60.0).text("secondes"));
                            ui.add_space(6.0);

                            ui.label("Intervalle d'écoute (RX) :");
                            ui.add(egui::Slider::new(&mut interval, 1.0..=120.0).text("secondes"));
                            ui.add_space(8.0);

                            ui.checkbox(&mut data_mode, "Mode DATA obligatoire (PTT + DATA automatique)");
                        });

                        // Écriture et sauvegarde réactive des variables modifiées à la volée dans la base SQLite
                        let changed = label != self.keyer_state.memories[idx].label
                            || duration != self.keyer_state.memories[idx].duration_secs
                            || interval != self.keyer_state.memories[idx].interval_secs
                            || data_mode != self.keyer_state.memories[idx].data_mode
                            || mp3_path != self.keyer_state.memories[idx].mp3_path;

                        if changed {
                            self.keyer_state.memories[idx].label = label;
                            self.keyer_state.memories[idx].duration_secs = duration;
                            self.keyer_state.memories[idx].interval_secs = interval;
                            self.keyer_state.memories[idx].data_mode = data_mode;
                            self.keyer_state.memories[idx].mp3_path = mp3_path;
                            self.save_settings(); // Sauvegarde en temps réel à chaque micro-modification du formulaire
                        }

                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(6.0);

                        // --- SÉLECTION MANUELLE DU PÉRIPHÉRIQUE AUDIO (ROUTAGE INDEXÉ) ---
                        ui.horizontal(|ui| {
                            ui.label("Carte son cible :");
                            
                            // Récupération asynchrone des cartes son physiques avec index unique pour le ComboBox
                            let available_devices = get_available_devices_unique_names();

                            let old_selection = self.keyer_state.chosen_device_name.clone();
                            egui::ComboBox::from_id_source("keyer_audio_device_cb")
                                .selected_text(&self.keyer_state.chosen_device_name)
                                .width(160.0)
                                .show_ui(ui, |ui| {
                                    for device_name in &available_devices {
                                        ui.selectable_value(&mut self.keyer_state.chosen_device_name, device_name.clone(), device_name);
                                    }
                                });

                            // Si l'utilisateur change de carte son, on recalcule le liseré d'affichage et on persiste
                            if self.keyer_state.chosen_device_name != old_selection {
                                self.keyer_state.refresh_device_name();
                                self.save_settings(); // Sauvegarde instantanée du choix de routage
                            }

                            // Bouton de ré-analyse rapide (Polling manuel)
                            if ui.button("🔄").on_hover_text("Re-scanner les cartes son du PC (ex: si câble USB branché tardivement)").clicked() {
                                self.keyer_state.refresh_device_name();
                            }
                        });
                        ui.add_space(4.0);

                        // Affichage textuel du périphérique réellement raccordé (Verrouillé ou Repli par défaut)
                        ui.horizontal(|ui| {
                            ui.label("Statut routage :");
                            let is_icom = self.keyer_state.active_device_name.contains("Icom") || self.keyer_state.active_device_name.contains("CODEC");
                            let color = if is_icom {
                                Color32::from_rgb(0, 230, 118) // Vert si Icom CODEC est raccordé ou verrouillé
                            } else {
                                Color32::from_rgb(255, 152, 0) // Orange s'il y a un repli par défaut (haut-parleur)
                            };
                            ui.label(RichText::new(&self.keyer_state.active_device_name).color(color).strong());
                        });

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(10.0);

                        // Actions de démarrage et d'arrêt de la boucle
                        ui.horizontal(|ui| {
                            if !self.keyer_state.is_active {
                                let start_color = Color32::from_rgb(13, 71, 161);
                                if custom_3d_button_sized(ui, "▶ DÉMARRER LA BOUCLE", false, start_color, egui::vec2(160.0, 32.0)) {
                                    self.keyer_state.is_active = true;
                                    self.keyer_state.current_stage = KeyerStage::Idle;
                                }
                            } else {
                                let stop_color = Color32::from_rgb(183, 28, 28);
                                if custom_3d_button_sized(ui, "■ ARRÊTER LA BOUCLE", true, stop_color, egui::vec2(160.0, 32.0)) {
                                    self.abort_keyer_and_disable_data();
                                }
                            }
                        });

                        ui.add_space(10.0);

                        // Indicateur de statut de l'automate
                        if self.keyer_state.is_active {
                            match self.keyer_state.current_stage {
                                KeyerStage::Idle => {
                                    ui.label("Démarrage de la balise...");
                                }
                                KeyerStage::Transmitting { start_time, duration } => {
                                    let elapsed = start_time.elapsed().as_secs_f32();
                                    let remaining = (duration.as_secs_f32() - elapsed).max(0.0);
                                    let pct = (elapsed / duration.as_secs_f32()).clamp(0.0, 1.0);
                                    
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("🔴 ÉMISSION EN COURS :").color(Color32::from_rgb(239, 83, 80)).strong());
                                        ui.label(format!("{:.1}s restantes", remaining));
                                    });
                                    ui.add(egui::ProgressBar::new(pct).show_percentage());
                                }
                                KeyerStage::Listening { start_time, duration } => {
                                    let elapsed = start_time.elapsed().as_secs_f32();
                                    let remaining = (duration.as_secs_f32() - elapsed).max(0.0);
                                    let pct = (elapsed / duration.as_secs_f32()).clamp(0.0, 1.0);
                                    
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("🔴 ÉCOUTE EN COURS :").color(Color32::from_rgb(102, 187, 106)).strong());
                                        ui.label(format!("{:.1}s restantes", remaining));
                                    });
                                    ui.add(egui::ProgressBar::new(pct).show_percentage());
                                }
                            }
                        } else {
                            ui.label(RichText::new("Boucle automatique en arrêt").color(Color32::GRAY).italics());
                        }

                        // Affichage dynamique des alertes ou erreurs de diagnostic sonore (Fichier ou Périphérique)
                        if let Some(err) = &self.keyer_state.audio_error {
                            ui.add_space(4.0);
                            ui.label(RichText::new(format!("⚠️ {}", err)).color(Color32::from_rgb(255, 152, 0)).strong());
                        }
                    });
                });
            });

        self.keyer_state.show_window = show;
    }
}