// src/gui/app.rs
// Version : 15.02.0 - Ajout des méthodes d'allumage automatisé (connect_and_power_on) et d'extinction propre (power_off_and_disconnect)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant}; // Import de Duration réintégré
use crossbeam_channel::{unbounded, Sender, Receiver};
use eframe::egui::{self, Color32};

use crate::comm::{RadioMode, RadioUpdate, Command, spawn_radio_thread};
use crate::database::{
    init_and_load_db, db_load_settings, db_save_settings_batch, search_eibi, get_probable_stations,
    DbMemoryEntry, EibiEntry
};
use crate::gui::scope::ScopeState;

pub const VERSION: &str = "15.02.0"; // Version de l'application
const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

#[derive(Clone, Copy, PartialEq)]
pub enum RightTab {
    Bandes,
    Memoires,
    Eibi,
}

pub struct BandInfo {
    pub name: &'static str,
    pub min: u64,
    pub max: u64,
    pub default_freq: u64,
    pub default_mode: RadioMode,
}

pub const AMATEUR_BANDS: &[BandInfo] = &[
    BandInfo { name: "160m", min: 1_800_000, max: 2_000_000, default_freq: 1_840_000, default_mode: RadioMode::Lsb },
    BandInfo { name: "80m", min: 3_500_000, max: 3_800_000, default_freq: 3_650_000, default_mode: RadioMode::Lsb },
    BandInfo { name: "40m", min: 7_000_000, max: 7_200_000, default_freq: 7_100_000, default_mode: RadioMode::Lsb },
    BandInfo { name: "30m", min: 10_100_000, max: 10_150_000, default_freq: 10_100_000, default_mode: RadioMode::Cw },
    BandInfo { name: "20m", min: 14_000_000, max: 14_350_000, default_freq: 14_200_000, default_mode: RadioMode::Usb },
    BandInfo { name: "17m", min: 18_068_000, max: 18_168_000, default_freq: 18_100_000, default_mode: RadioMode::Usb },
    BandInfo { name: "15m", min: 21_000_000, max: 21_450_000, default_freq: 21_200_000, default_mode: RadioMode::Usb },
    BandInfo { name: "12m", min: 24_890_000, max: 24_990_000, default_freq: 24_900_000, default_mode: RadioMode::Usb },
    BandInfo { name: "10m", min: 28_000_000, max: 29_700_000, default_freq: 28_500_000, default_mode: RadioMode::Usb },
    BandInfo { name: "6m", min: 50_000_000, max: 52_000_000, default_freq: 50_100_000, default_mode: RadioMode::Usb },
];

pub const CB_BANDS: &[BandInfo] = &[
    BandInfo { name: "CB C19", min: 26_960_000, max: 27_410_000, default_freq: 27_185_000, default_mode: RadioMode::Am },
    BandInfo { name: "SSTV", min: 27_650_000, max: 27_750_000, default_freq: 27_700_000, default_mode: RadioMode::Usb },
];

pub const PIRATE_BANDS: &[BandInfo] = &[
    BandInfo { name: "11m DX", min: 27_553_000, max: 27_557_000, default_freq: 27_555_000, default_mode: RadioMode::Usb },
    BandInfo { name: "45m", min: 6_500_000, max: 6_700_000, default_freq: 6_660_000, default_mode: RadioMode::Lsb },
    BandInfo { name: "88m", min: 3_200_000, max: 3_500_000, default_freq: 3_420_000, default_mode: RadioMode::Lsb },
];

pub struct Ic7300App {
    pub(crate) port_name: String,
    pub(crate) baud_rate: u32,
    pub(crate) available_ports: Vec<String>,
    pub(crate) show_config_window: bool,
    pub(crate) show_gains_window: bool,
    pub(crate) is_connected: bool,
    pub(crate) frequency: u64,
    pub(crate) freq_input: String,
    pub(crate) mode: RadioMode,
    pub(crate) filter: u8,
    pub(crate) is_data_mode: bool,
    pub(crate) is_tx: bool,
    pub(crate) tx_lock: bool,
    pub(crate) af_gain: u8,
    pub(crate) rf_gain: u8,
    pub(crate) squelch: u8,
    pub(crate) mic_gain: u8,
    pub(crate) rf_power: u8,
    pub(crate) comp_level: u8,
    pub(crate) monitor_level: u8,
    pub(crate) preamp: u8,
    pub(crate) attenuator: bool,
    pub(crate) agc: u8,
    pub(crate) tuner: bool,
    pub(crate) noise_blanker: bool,
    pub(crate) noise_reduction: bool,
    pub(crate) noise_blanker_level: u8,
    pub(crate) noise_reduction_level: u8,
    pub(crate) scope_state: ScopeState,
    pub(crate) usb_rx_level: u8,
    pub(crate) usb_tx_level: u8,
    pub(crate) frequency_a: u64,
    pub(crate) frequency_b: u64,
    pub(crate) mode_a: RadioMode,
    pub(crate) mode_b: RadioMode,
    pub(crate) filter_a: u8,
    pub(crate) filter_b: u8,
    pub(crate) is_data_mode_a: bool,
    pub(crate) is_data_mode_b: bool,
    pub(crate) active_vfo: u8,
    pub(crate) split_active: bool,
    pub(crate) s_meter: u8,
    pub(crate) po_meter: u8,
    pub(crate) swr_meter: u8,
    pub(crate) alc_meter: u8,
    pub(crate) comp_meter: u8,
    pub(crate) memories: Vec<DbMemoryEntry>,
    pub(crate) eibi_status: String,
    pub(crate) eibi_rx_status: Option<Receiver<String>>,
    pub(crate) eibi_search_query: String,
    pub(crate) eibi_search_results: Vec<EibiEntry>,
    pub(crate) probable_stations: Vec<EibiEntry>,
    pub(crate) show_mem_manager: bool,
    pub(crate) mem_edit_category: String,
    pub(crate) mem_edit_name: String,
    pub(crate) mem_edit_freq_mhz: String,
    pub(crate) mem_edit_mode: RadioMode,
    pub(crate) mem_edit_is_data: bool,
    pub(crate) mem_edit_filter: u8,
    pub(crate) mem_edit_preamp: u8,
    pub(crate) mem_editing_id: Option<i32>,
    pub(crate) categories_force_open: Option<bool>,
    pub(crate) show_csv_manager: bool,
    pub(crate) csv_settings_path: String,
    pub(crate) csv_memories_path: String,
    pub(crate) csv_eibi_path: String,
    pub(crate) csv_status: String,
    pub(crate) show_info_window: bool,
    pub(crate) info_text: String,
    pub(crate) vfo_angle: f32,
    pub(crate) vfo_accumulator: f32,
    pub(crate) vfo_step: u64,
    pub(crate) vfo_hovered: bool,
    pub(crate) right_tab: RightTab,
    pub(crate) tx_cmd: Option<Sender<Command>>,
    pub(crate) rx_radio: Option<Receiver<RadioUpdate>>,
    pub(crate) thread_exit_flag: Arc<AtomicBool>,
    pub(crate) last_user_write: Instant,
}

impl Default for Ic7300App {
    fn default() -> Self {
        let mut ports = Vec::new();
        if let Ok(available) = serialport::available_ports() {
            for p in available { ports.push(p.port_name); }
        }
        let default_port = if ports.is_empty() {
            if cfg!(target_os = "windows") { "COM3".to_owned() } else { "/dev/ttyUSB0".to_owned() }
        } else { ports[0].clone() };

        let loaded_memories = init_and_load_db();

        let mut port_name = default_port;
        let mut baud_rate = 115200;
        let mut frequency = 14_074_000;
        let mut mode = RadioMode::Usb;
        let mut filter = 1;
        let mut is_data_mode = false;
        let mut af_gain = 60;
        let mut rf_gain = 255;
        let mut squelch = 0;
        let mut mic_gain = 128;
        let mut rf_power = 255;
        let mut comp_level = 128;
        let mut monitor_level = 128;
        let mut preamp = 0;
        let mut attenuator = false;
        let mut agc = 2;
        let mut tuner = false;
        let mut noise_blanker = false;
        let mut noise_reduction = false;
        let mut noise_blanker_level = 128;
        let mut noise_reduction_level = 128;
        let mut tx_lock = false;
        let mut vfo_step = 10;
        let mut usb_rx_level = 128;
        let mut usb_tx_level = 128;
        let mut active_vfo = 0;
        let mut split_active = false;

        // Éléments de l'analyseur de spectre et du Waterfall à recharger
        let mut scope_show_window = false;
        let mut scope_enabled = false;
        let mut scope_span = 50_000;
        let mut scope_waterfall_offset = 25.0;
        let mut scope_waterfall_gain = 1.0;
        let mut scope_waterfall_palette = 0;
        let mut scope_waterfall_width = 475;
        let mut scope_waterfall_height = 100;

        let saved_settings = db_load_settings();
        if let Some(val) = saved_settings.get("port_name") { port_name = val.clone(); }
        if let Some(val) = saved_settings.get("baud_rate") { if let Ok(parsed) = val.parse() { baud_rate = parsed; } }
        if let Some(val) = saved_settings.get("frequency") { if let Ok(parsed) = val.parse() { frequency = parsed; } }
        if let Some(val) = saved_settings.get("mode") {
            mode = match val.as_str() {
                "LSB" => RadioMode::Lsb, "USB" => RadioMode::Usb,
                "AM" => RadioMode::Am, "CW" => RadioMode::Cw,
                "FM" => RadioMode::Fm, _ => RadioMode::Usb,
            };
        }
        if let Some(val) = saved_settings.get("filter") { if let Ok(parsed) = val.parse() { filter = parsed; } }
        if let Some(val) = saved_settings.get("is_data_mode") { is_data_mode = val == "1"; }
        if let Some(val) = saved_settings.get("vfo_step") { if let Ok(parsed) = val.parse() { vfo_step = parsed; } }
        if let Some(val) = saved_settings.get("af_gain") { if let Ok(parsed) = val.parse() { af_gain = parsed; } }
        if let Some(val) = saved_settings.get("rf_gain") { if let Ok(parsed) = val.parse() { rf_gain = parsed; } }
        if let Some(val) = saved_settings.get("squelch") { if let Ok(parsed) = val.parse() { squelch = parsed; } }
        if let Some(val) = saved_settings.get("mic_gain") { if let Ok(parsed) = val.parse() { mic_gain = parsed; } }
        if let Some(val) = saved_settings.get("rf_power") { if let Ok(parsed) = val.parse() { rf_power = parsed; } }
        if let Some(val) = saved_settings.get("comp_level") { if let Ok(parsed) = val.parse() { comp_level = parsed; } }
        if let Some(val) = saved_settings.get("monitor_level") { if let Ok(parsed) = val.parse() { monitor_level = parsed; } }
        if let Some(val) = saved_settings.get("preamp") { if let Ok(parsed) = val.parse() { preamp = parsed; } }
        if let Some(val) = saved_settings.get("attenuator") { attenuator = val == "1"; }
        if let Some(val) = saved_settings.get("agc") { if let Ok(parsed) = val.parse() { agc = parsed; } }
        if let Some(val) = saved_settings.get("tuner") { tuner = val == "1"; }
        if let Some(val) = saved_settings.get("noise_blanker") { noise_blanker = val == "1"; }
        if let Some(val) = saved_settings.get("noise_reduction") { noise_reduction = val == "1"; }
        if let Some(val) = saved_settings.get("noise_blanker_level") { if let Ok(parsed) = val.parse() { noise_blanker_level = parsed; } }
        if let Some(val) = saved_settings.get("noise_reduction_level") { if let Ok(parsed) = val.parse() { noise_reduction_level = parsed; } }
        
        // Lecture des configurations de l'analyseur de spectre mémorisées
        if let Some(val) = saved_settings.get("scope_show_window") { scope_show_window = val == "1"; }
        if let Some(val) = saved_settings.get("scope_enabled") { scope_enabled = val == "1"; }
        if let Some(val) = saved_settings.get("scope_span") { if let Ok(parsed) = val.parse() { scope_span = parsed; } }
        if let Some(val) = saved_settings.get("scope_waterfall_offset") { if let Ok(parsed) = val.parse() { scope_waterfall_offset = parsed; } }
        if let Some(val) = saved_settings.get("scope_waterfall_gain") { if let Ok(parsed) = val.parse() { scope_waterfall_gain = parsed; } }
        if let Some(val) = saved_settings.get("scope_waterfall_palette") { if let Ok(parsed) = val.parse() { scope_waterfall_palette = parsed; } }
        if let Some(val) = saved_settings.get("scope_waterfall_width") { if let Ok(parsed) = val.parse() { scope_waterfall_width = parsed; } }
        if let Some(val) = saved_settings.get("scope_waterfall_height") { if let Ok(parsed) = val.parse() { scope_waterfall_height = parsed; } }

        if let Some(val) = saved_settings.get("tx_lock") { tx_lock = val == "1"; }
        if let Some(val) = saved_settings.get("usb_rx_level") { if let Ok(parsed) = val.parse() { usb_rx_level = parsed; } }
        if let Some(val) = saved_settings.get("usb_tx_level") { if let Ok(parsed) = val.parse() { usb_tx_level = parsed; } }
        if let Some(val) = saved_settings.get("active_vfo") { if let Ok(parsed) = val.parse() { active_vfo = parsed; } }
        if let Some(val) = saved_settings.get("split_active") { split_active = val == "1"; }

        let mut frequency_a = frequency;
        let mut frequency_b = frequency;
        let mut mode_a = mode;
        let mut mode_b = mode;
        let mut filter_a = filter;
        let mut filter_b = filter;
        let mut is_data_mode_a = is_data_mode;
        let mut is_data_mode_b = is_data_mode;

        if let Some(val) = saved_settings.get("frequency_a") { if let Ok(parsed) = val.parse() { frequency_a = parsed; } }
        if let Some(val) = saved_settings.get("frequency_b") { if let Ok(parsed) = val.parse() { frequency_b = parsed; } }
        if let Some(val) = saved_settings.get("mode_a") {
            mode_a = match val.as_str() {
                "LSB" => RadioMode::Lsb, "USB" => RadioMode::Usb,
                "AM" => RadioMode::Am, "CW" => RadioMode::Cw, "FM" => RadioMode::Fm, _ => RadioMode::Usb,
            };
        }
        if let Some(val) = saved_settings.get("mode_b") {
            mode_b = match val.as_str() {
                "LSB" => RadioMode::Lsb, "USB" => RadioMode::Usb,
                "AM" => RadioMode::Am, "CW" => RadioMode::Cw, "FM" => RadioMode::Fm, _ => RadioMode::Usb,
            };
        }
        if let Some(val) = saved_settings.get("filter_a") { if let Ok(parsed) = val.parse() { filter_a = parsed; } }
        if let Some(val) = saved_settings.get("filter_b") { if let Ok(parsed) = val.parse() { filter_b = parsed; } }
        if let Some(val) = saved_settings.get("is_data_mode_a") { is_data_mode_a = val == "1"; }
        if let Some(val) = saved_settings.get("is_data_mode_b") { is_data_mode_b = val == "1"; }

        // Alignement et synchronisation absolue de l'état d'initialisation sur le VFO actif restauré
        if active_vfo == 0 {
            frequency = frequency_a;
            mode = mode_a;
            filter = filter_a;
            is_data_mode = is_data_mode_a;
        } else {
            frequency = frequency_b;
            mode = mode_b;
            filter = filter_b;
            is_data_mode = is_data_mode_b;
        }

        let freq_input = format!("{:.6}", frequency as f64 / 1_000_000.0);
        let initial_eibi = search_eibi("", frequency);
        let initial_probable = get_probable_stations(frequency);

        // Initialisation de la structure ScopeState
        let mut scope_state = ScopeState::new();
        scope_state.show_window = scope_show_window;
        scope_state.enabled = scope_enabled;
        scope_state.span = scope_span;
        scope_state.waterfall_offset = scope_waterfall_offset;
        scope_state.waterfall_gain = scope_waterfall_gain;
        scope_state.waterfall_palette = scope_waterfall_palette;
        scope_state.center_frequency = frequency;

        // Allocation de la grille Waterfall de départ
        scope_state.waterfall_image = egui::ColorImage {
            size: [scope_waterfall_width, scope_waterfall_height],
            pixels: vec![Color32::BLACK; scope_waterfall_width * scope_waterfall_height],
        };
        scope_state.current_sweep = vec![0; scope_waterfall_width];

        // Restauration de l'historique binaire du Waterfall
        if let Ok(mut f) = std::fs::File::open("waterfall_history.bin") {
            use std::io::Read;
            let mut buf = Vec::new();
            if f.read_to_end(&mut buf).is_ok() {
                let chunks = buf.chunks_exact(scope_waterfall_width);
                for chunk in chunks {
                    scope_state.waterfall_history.push(chunk.to_vec());
                }
                scope_state.redraw_waterfall();
            }
        }

        let info_text = std::fs::read_to_string("info.txt").unwrap_or_else(|_| {
            let default_wiki = "\
============================================================
              CONTRÔLEUR ICOM IC-7300 - MINI WIKI
============================================================
Fichier info.txt éditable.

1. CONFIGURATION INITIALE : Ouvrez \"⚙ Paramètres de Communication\", choisissez le port COM de l'IC-7300 (115200 bauds) et connectez.
2. CONTRÔLE VFO : Molette ou Saisie en MHz, flèches clavier pour fréquence/pas (STEP).
3. PTT : Barre d'espace active l'émission si la souris est sur l'app. TX LOCK active le Toggle PTT.
4. BASES DE DONNÉES : SQL local + EiBi Space, synchro stations probables automatique à l'heure UTC.
5. BACKUPS : Sauvegardes de vos configurations et de vos mémoires complètes au format CSV.
".to_owned();
            let _ = std::fs::write("info.txt", &default_wiki);
            default_wiki
        });

        Self {
            port_name, baud_rate, available_ports: ports, 
            show_config_window: false, show_gains_window: false, is_connected: false,
            frequency, freq_input, mode, filter, is_data_mode, is_tx: false, tx_lock,
            af_gain, rf_gain, squelch, mic_gain, rf_power, comp_level, monitor_level, preamp, attenuator, agc, tuner,
            noise_blanker, noise_reduction,
            noise_blanker_level, noise_reduction_level,
            scope_state,
            usb_rx_level, usb_tx_level,
            frequency_a, frequency_b,
            mode_a, mode_b,
            filter_a, filter_b,
            is_data_mode_a, is_data_mode_b,
            active_vfo, split_active,
            s_meter: 0, po_meter: 0, swr_meter: 0, alc_meter: 0, comp_meter: 0,
            memories: loaded_memories, eibi_status: "Prêt (memories.db actif)".to_owned(), eibi_rx_status: None,
            eibi_search_query: "".to_owned(), eibi_search_results: initial_eibi, probable_stations: initial_probable,
            show_mem_manager: false, mem_edit_category: "📻 RADIODIFFUSION (SWL)".to_owned(), mem_edit_name: "".to_owned(),
            mem_edit_freq_mhz: "".to_owned(), mem_edit_mode: RadioMode::Am, mem_edit_is_data: false, mem_edit_filter: 1, mem_edit_preamp: 0,
            mem_editing_id: None, categories_force_open: None,
            show_csv_manager: false, csv_settings_path: "settings_backup.csv".to_owned(),
            csv_memories_path: "memories_backup.csv".to_owned(), csv_eibi_path: "eibi_backup.csv".to_owned(), csv_status: "En attente...".to_owned(),
            show_info_window: false, info_text, vfo_angle: 0.0, vfo_accumulator: 0.0, vfo_step, vfo_hovered: false,
            right_tab: RightTab::Bandes, tx_cmd: None, rx_radio: None, thread_exit_flag: Arc::new(AtomicBool::new(false)),
            last_user_write: Instant::now(),
        }
    }
}

impl Ic7300App {
    pub(crate) fn is_freq_in_band(&self, band: &BandInfo) -> bool {
        self.frequency >= band.min && self.frequency <= band.max
    }

    pub(crate) fn has_any_active_band(&self) -> bool {
        AMATEUR_BANDS.iter().any(|b| self.is_freq_in_band(b))
            || CB_BANDS.iter().any(|b| self.is_freq_in_band(b))
            || PIRATE_BANDS.iter().any(|b| self.is_freq_in_band(b))
    }

    pub(crate) fn connect(&mut self, ctx: egui::Context) -> Result<(), String> {
        let (tx, rx) = unbounded::<Command>();
        let (tx_radio, rx_radio) = unbounded::<RadioUpdate>();
        self.tx_cmd = Some(tx);
        self.rx_radio = Some(rx_radio);
        self.thread_exit_flag.store(false, Ordering::SeqCst);
        let exit_flag = self.thread_exit_flag.clone();
        let ctx_clone = ctx.clone();

        spawn_radio_thread(self.port_name.clone(), self.baud_rate, rx, tx_radio, exit_flag, ctx_clone)?;

        self.is_connected = true;
        self.show_config_window = false;
        self.sync_all_to_radio();
        Ok(())
    }

    pub(crate) fn connect_and_power_on(&mut self, ctx: egui::Context) -> Result<(), String> {
        self.connect(ctx)?;
        // Envoi de la commande de réveil série à l'allumage
        self.send_cmd(Command::SetPower(true));
        Ok(())
    }

    pub(crate) fn power_off_and_disconnect(&mut self) {
        if self.is_connected {
            self.send_cmd(Command::SetPower(false));
            // Attente de sécurité pour que la radio écrive la trame d'arrêt avant coupure de liaison
            std::thread::sleep(Duration::from_millis(100));
            self.disconnect();
        }
    }

    pub(crate) fn recall_memory(&mut self, frequency: u64, mode: RadioMode, is_data: bool, filter: u8, preamp: u8) {
        self.frequency = frequency;
        if self.active_vfo == 0 {
            self.frequency_a = frequency; self.mode_a = mode; self.filter_a = filter; self.is_data_mode_a = is_data;
        } else {
            self.frequency_b = frequency; self.mode_b = mode; self.filter_b = filter; self.is_data_mode_b = is_data;
        }
        self.freq_input = format!("{:.6}", frequency as f64 / 1_000_000.0);
        self.mode = mode; self.is_data_mode = is_data; self.filter = filter; self.preamp = preamp;
        self.last_user_write = Instant::now();

        if self.is_connected {
            self.send_cmd(Command::SetFrequency(self.frequency));
            self.send_cmd(Command::SetModeAndFilter(self.mode, self.filter));
            self.send_cmd(Command::SetDataMode(self.is_data_mode));
            self.send_cmd(Command::SetPreamp(self.preamp));
        }
        self.refresh_eibi_results();
    }

    pub(crate) fn sync_all_to_radio(&self) {
        // 1. Forcer le mode VFO, puis configurer et synchroniser le VFO A
        self.send_cmd(Command::SetVfo(0));
        self.send_cmd(Command::SetFrequency(self.frequency_a));
        self.send_cmd(Command::SetModeAndFilter(self.mode_a, self.filter_a));
        self.send_cmd(Command::SetDataMode(self.is_data_mode_a));

        // 2. Configurer et synchroniser le VFO B
        self.send_cmd(Command::SetVfo(1));
        self.send_cmd(Command::SetFrequency(self.frequency_b));
        self.send_cmd(Command::SetModeAndFilter(self.mode_b, self.filter_b));
        self.send_cmd(Command::SetDataMode(self.is_data_mode_b));

        // 3. Restaurer le VFO actif (A ou B)
        self.send_cmd(Command::SetVfo(self.active_vfo));

        // 4. Synchroniser tout le reste de l'état du poste
        self.send_cmd(Command::SetSplit(self.split_active));
        self.send_cmd(Command::SetAfGain(self.af_gain));
        self.send_cmd(Command::SetRfGain(self.rf_gain));
        self.send_cmd(Command::SetSquelch(self.squelch));
        self.send_cmd(Command::SetMicGain(self.mic_gain));
        self.send_cmd(Command::SetRfPower(self.rf_power));
        self.send_cmd(Command::SetCompLevel(self.comp_level));
        self.send_cmd(Command::SetMonitorLevel(self.monitor_level));
        self.send_cmd(Command::SetPreamp(self.preamp));
        self.send_cmd(Command::SetAttenuator(self.attenuator));
        self.send_cmd(Command::SetAgc(self.agc));
        self.send_cmd(Command::SetTuner(self.tuner));
        self.send_cmd(Command::SetNoiseBlanker(self.noise_blanker));
        self.send_cmd(Command::SetNoiseReduction(self.noise_reduction));
        self.send_cmd(Command::SetNoiseBlankerLevel(self.noise_blanker_level));
        self.send_cmd(Command::SetNoiseReductionLevel(self.noise_reduction_level));
        self.send_cmd(Command::SetScopeOutput(self.scope_state.enabled));
        self.send_cmd(Command::SetUsbRxLevel(self.usb_rx_level));
        self.send_cmd(Command::SetUsbTxLevel(self.usb_tx_level));
        self.send_cmd(Command::SetPTT(self.is_tx));
    }

    pub(crate) fn disconnect(&mut self) {
        if self.is_tx { self.send_cmd(Command::SetPTT(false)); }
        self.send_cmd(Command::Disconnect);
        self.thread_exit_flag.store(true, Ordering::SeqCst);
        self.is_connected = false; self.tx_cmd = None; self.rx_radio = None;
    }

    pub(crate) fn send_cmd(&self, cmd: Command) {
        if let Some(tx) = &self.tx_cmd { let _ = tx.send(cmd); }
    }

    pub(crate) fn set_frequency_from_i64(&mut self, freq: i64) {
        let clamped = freq.clamp(30_000, 74_800_000) as u64; 
        if clamped != self.frequency {
            self.frequency = clamped;
            if self.active_vfo == 0 { self.frequency_a = clamped; } else { self.frequency_b = clamped; }
            self.freq_input = format!("{:.6}", self.frequency as f64 / 1_000_000.0); 
            self.last_user_write = Instant::now(); 
            if self.is_connected { self.send_cmd(Command::SetFrequency(self.frequency)); }
            
            // Synchronisation de la fréquence centrale de l'analyseur de spectre
            self.scope_state.center_frequency = clamped;
            
            self.refresh_eibi_results();
        }
    }

    pub(crate) fn change_band_and_mode(&mut self, freq: u64, mode: RadioMode) {
        self.set_frequency_from_i64(freq as i64);
        self.mode = mode;
        if self.active_vfo == 0 { self.mode_a = mode; } else { self.mode_b = mode; }
        self.last_user_write = Instant::now(); 
        if self.is_connected {
            self.send_cmd(Command::SetModeAndFilter(self.mode, self.filter));
            self.send_cmd(Command::SetDataMode(self.is_data_mode));
        }
        self.refresh_eibi_results();
    }

    pub(crate) fn refresh_com_ports(&mut self) {
        if let Ok(available) = serialport::available_ports() {
            self.available_ports.clear();
            for p in available { self.available_ports.push(p.port_name); }
        }
    }

    pub(crate) fn refresh_eibi_results(&mut self) {
        self.eibi_search_results = search_eibi(&self.eibi_search_query, self.frequency);
        self.probable_stations = get_probable_stations(self.frequency); 
    }

    pub(crate) fn save_settings(&self) {
        let mode_str = match self.mode {
            RadioMode::Lsb => "LSB", RadioMode::Usb => "USB",
            RadioMode::Am => "AM", RadioMode::Cw => "CW", RadioMode::Fm => "FM",
        };
        let settings = vec![
            ("port_name", self.port_name.clone()),
            ("baud_rate", self.baud_rate.to_string()),
            ("frequency", self.frequency.to_string()),
            ("mode", mode_str.to_owned()),
            ("filter", self.filter.to_string()),
            ("is_data_mode", (if self.is_data_mode { "1" } else { "0" }).to_owned()),
            ("vfo_step", self.vfo_step.to_string()),
            ("af_gain", self.af_gain.to_string()),
            ("rf_gain", self.rf_gain.to_string()),
            ("squelch", self.squelch.to_string()),
            ("mic_gain", self.mic_gain.to_string()),
            ("rf_power", self.rf_power.to_string()),
            ("comp_level", self.comp_level.to_string()),
            ("monitor_level", self.monitor_level.to_string()),
            ("preamp", self.preamp.to_string()),
            ("attenuator", (if self.attenuator { "1" } else { "0" }).to_owned()),
            ("agc", self.agc.to_string()),
            ("tuner", (if self.tuner { "1" } else { "0" }).to_owned()),
            ("noise_blanker", (if self.noise_blanker { "1" } else { "0" }).to_owned()),
            ("noise_reduction", (if self.noise_reduction { "1" } else { "0" }).to_owned()),
            ("noise_blanker_level", self.noise_blanker_level.to_string()),
            ("noise_reduction_level", self.noise_reduction_level.to_string()),
            
            // Sauvegarde de l'état d'affichage et de configuration du spectre (Waterfall)
            ("scope_show_window", (if self.scope_state.show_window { "1" } else { "0" }).to_owned()),
            ("scope_enabled", (if self.scope_state.enabled { "1" } else { "0" }).to_owned()),
            ("scope_span", self.scope_state.span.to_string()),
            ("scope_waterfall_offset", self.scope_state.waterfall_offset.to_string()),
            ("scope_waterfall_gain", self.scope_state.waterfall_gain.to_string()),
            ("scope_waterfall_palette", self.scope_state.waterfall_palette.to_string()),
            ("scope_waterfall_width", self.scope_state.waterfall_image.size[0].to_string()),
            ("scope_waterfall_height", self.scope_state.waterfall_image.size[1].to_string()),
            
            ("tx_lock", (if self.tx_lock { "1" } else { "0" }).to_owned()),
            ("usb_rx_level", self.usb_rx_level.to_string()),
            ("usb_tx_level", self.usb_tx_level.to_string()),
            ("active_vfo", self.active_vfo.to_string()),
            ("split_active", (if self.split_active { "1" } else { "0" }).to_owned()),
            ("frequency_a", self.frequency_a.to_string()),
            ("frequency_b", self.frequency_b.to_string()),
            ("mode_a", format!("{:?}", self.mode_a).to_uppercase()),
            ("mode_b", format!("{:?}", self.mode_b).to_uppercase()),
            ("filter_a", self.filter_a.to_string()),
            ("filter_b", self.filter_b.to_string()),
            ("is_data_mode_a", (if self.is_data_mode_a { "1" } else { "0" }).to_owned()),
            ("is_data_mode_b", (if self.is_data_mode_b { "1" } else { "0" }).to_owned()),
        ];
        let _ = db_save_settings_batch(&settings);

        // Sauvegarde binaire de l'historique de défilement (pixels) du Waterfall
        if let Ok(mut f) = std::fs::File::create("waterfall_history.bin") {
            use std::io::Write;
            for row in &self.scope_state.waterfall_history {
                let _ = f.write_all(row);
            }
        }
    }
}