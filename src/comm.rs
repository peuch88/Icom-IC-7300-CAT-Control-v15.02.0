// Version : V15.04.01 - Implémentation du Proxy CAT intégré asynchrone (Bridge virtuel CI-V) pour les logiciels tiers comme MMSSTV
// Module de communication série asynchrone CI-V pour l'Icom IC-7300

use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use crossbeam_channel::{Sender, Receiver};

pub const IC7300_ADDR: u8 = 0x94;
pub const PC_ADDR: u8 = 0xE0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RadioMode {
    Lsb = 0x00, 
    Usb = 0x01, 
    Am = 0x02, 
    Cw = 0x03, 
    Fm = 0x05,
}

#[derive(Clone, Debug)]
pub enum RadioUpdate {
    Frequency(u64),
    ModeAndFilter(RadioMode, u8, bool),
    ActiveVfo(u8),    // 0 = VFO A, 1 = VFO B
    SplitState(bool), // true = Active, false = Désactive
    Meter(u8, u8),
    PTTState(bool),
    AfGain(u8),
    RfGain(u8),
    Squelch(u8),
    RfPower(u8),
    MicGain(u8),
    CompLevel(u8),
    MonitorLevel(u8),
    Preamp(u8),
    Attenuator(bool),
    Agc(u8),
    Tuner(bool),
    NoiseBlanker(bool),
    NoiseReduction(bool),
    NoiseBlankerLevel(u8),
    NoiseReductionLevel(u8),
    ScopeSweep(Vec<u8>),       // Ligne de spectre complète (475 points)
    ScopeSpan(u32),            // Signalisation dynamique du Span physique décodé (total span en Hz)
    UsbRxLevel(u8),
    UsbTxLevel(u8),
    Disconnected(String),
}

#[derive(Debug)]
pub enum Command {
    SetFrequency(u64),
    SetModeAndFilter(RadioMode, u8),
    SetDataMode(bool),
    SetPTT(bool),
    SetVfo(u8),        // 0 = VFO A, 1 = VFO B
    SwapVfo,           // Swap VFO A / B (0xB0)
    EqualizeVfo,       // VFO A = VFO B (0xA0)
    SetSplit(bool),    // Active/Désactive SPLIT (0x0F)
    SetAfGain(u8),
    SetRfGain(u8),
    SetSquelch(u8),
    SetMicGain(u8),
    SetRfPower(u8),
    SetCompLevel(u8),
    SetMonitorLevel(u8),
    SetPreamp(u8),
    SetAttenuator(bool),
    SetAgc(u8),
    SetTuner(bool),
    SetNoiseBlanker(bool),
    SetNoiseReduction(bool),
    SetNoiseBlankerLevel(u8),
    SetNoiseReductionLevel(u8),
    SetScopeOutput(bool),     // Commande d'activation/désactivation du Scope physique + flux (27 10 & 27 11)
    SetScopeSpan(u32),        // Force le Span de l'analyseur sur la radio (total span en Hz)
    SetPower(bool),           // Commande d'alimentation ON/OFF (18 00 / 18 01)
    SetUsbRxLevel(u8),
    SetUsbTxLevel(u8),
    Disconnect,
}

pub fn freq_to_bcd(mut freq: u64) -> Vec<u8> {
    let mut bcd = Vec::with_capacity(5);
    for _ in 0..5 {
        let lower = (freq % 10) as u8; freq /= 10;
        let upper = (freq % 10) as u8; freq /= 10;
        bcd.push((upper << 4) | lower);
    }
    bcd
}

pub fn level_to_bcd(level: u8) -> [u8; 2] {
    let hundred = level / 100;
    let rem = level % 100;
    let tens = rem / 10;
    let units = rem % 10;
    [hundred, (tens << 4) | units]
}

pub fn bcd_to_u8(bytes: &[u8]) -> u8 {
    if bytes.is_empty() { return 0; }
    if bytes.len() == 1 {
        let b = bytes[0]; 
        ((b & 0x0F) + ((b >> 4) & 0x0F) * 10).min(255) as u8
    } else {
        let mut val = 0u32; let mut multiplier = 1u32;
        for &b in bytes.iter().rev() {
            val += (b & 0x0F) as u32 * multiplier; multiplier *= 10;
            val += ((b >> 4) & 0x0F) as u32 * multiplier; multiplier *= 10;
        }
        val.min(255) as u8
    }
}

/// Convertit le Span d'IHM (Hz) vers les 6 octets BCD standard de la commande 27 15 (LSB First)
pub fn span_to_bcd_bytes(total_span: u32) -> [u8; 6] {
    match total_span {
        5_000 => [0x00, 0x00, 0x25, 0x00, 0x00, 0x00],     // ±2.5 kHz -> Écran: 2500 Hz -> Byte 3 = 0x25
        10_000 => [0x00, 0x00, 0x50, 0x00, 0x00, 0x00],    // ±5.0 kHz -> Écran: 5000 Hz -> Byte 3 = 0x50
        20_000 => [0x00, 0x00, 0x00, 0x01, 0x00, 0x00],    // ±10 kHz  -> Écran: 10000 Hz -> Byte 4 = 0x01 (car 10 kHz)
        50_000 => [0x00, 0x00, 0x50, 0x02, 0x00, 0x00],    // ±25 kHz  -> Écran: 25000 Hz -> Byte 3 = 0x50, Byte 4 = 0x02
        100_000 => [0x00, 0x00, 0x00, 0x05, 0x00, 0x00],   // ±50 kHz  -> Écran: 50000 Hz -> Byte 4 = 0x05 (car 50 kHz)
        200_000 => [0x00, 0x00, 0x00, 0x10, 0x00, 0x00],   // ±100 kHz -> Écran: 100000 Hz -> Byte 4 = 0x10 (car 100 kHz)
        500_000 => [0x00, 0x00, 0x00, 0x25, 0x00, 0x00],   // ±250 kHz -> Écran: 250000 Hz -> Byte 4 = 0x25 (car 250 kHz)
        1_000_000 => [0x00, 0x00, 0x00, 0x50, 0x00, 0x00], // ±500 kHz -> Écran: 500000 Hz -> Byte 4 = 0x50 (car 500 kHz)
        _ => [0x00, 0x00, 0x50, 0x00, 0x00, 0x00],         // Par défaut ±5.0 kHz (10 000 Hz)
    }
}

/// Décodeur hybride : Reçoit 5 octets (depuis le flux 27 00) ou 6 octets (depuis la trame 27 15) et extrait la valeur réelle
pub fn bcd_to_span_val(bcd: &[u8]) -> u32 {
    if bcd.len() < 5 { return 0; }
    
    // Normalisation des octets :
    // - Si longueur = 6 octets (polling) : Byte 3, 4 et 5 sont aux indices bcd[2], bcd[3], bcd[4]
    // - Si longueur = 5 octets (extrait du flux 27 00) : Byte 3, 4 et 5 sont décalés d'un cran à gauche, aux indices bcd[1], bcd[2], bcd[3]
    let (byte_3, byte_4, byte_5) = if bcd.len() == 6 {
        (bcd[2], bcd[3], bcd[4])
    } else {
        (bcd[1], bcd[2], bcd[3])
    };
    
    let d_1k = (byte_3 >> 4) & 0x0F;   // Chiffre des 1 kHz (poids fort de l'octet 3)
    let d_100h = byte_3 & 0x0F;        // Chiffre des 100 Hz (poids faible de l'octet 3)
    
    let d_100k = (byte_4 >> 4) & 0x0F; // Chiffre des 100 kHz (poids fort de l'octet 4)
    let d_10k = byte_4 & 0x0F;         // Chiffre des 10 kHz (poids faible de l'octet 4)
    
    let d_10m = (byte_5 >> 4) & 0x0F;  // Chiffre des 10 MHz (poids fort de l'octet 5)
    let d_1m = byte_5 & 0x0F;          // Chiffre des 1 MHz (poids faible de l'octet 5)
    
    let val_hz = d_10m as u32 * 10_000_000
               + d_1m as u32 * 1_000_000
               + d_100k as u32 * 100_000 
               + d_10k as u32 * 10_000 
               + d_1k as u32 * 1_000 
               + d_100h as u32 * 100;
               
    // Mappe l'écart d'affichage physique de la radio vers la largeur de spectre totale modélisée
    match val_hz {
        2_500 => 5_000,
        5_000 => 10_000,
        10_000 => 20_000,
        25_000 => 5_000,
        50_000 => 100_000,
        100_000 => 200_000,
        250_000 => 500_000,
        500_000 => 1_000_000,
        _ => 0,
    }
}

pub fn build_civ_frame(cmd: u8, subcmd: Option<u8>, data: &[u8]) -> Vec<u8> {
    let mut frame = vec![0xFE, 0xFE, IC7300_ADDR, PC_ADDR, cmd];
    if let Some(sc) = subcmd { frame.push(sc); }
    frame.extend_from_slice(data); frame.push(0xFD);
    frame
}

/// Démarre l'écoute asynchrone d'un port série virtuel (Proxy CAT / Bridge) pour répondre aux requêtes de MMSSTV (ou autre logiciel)
pub fn spawn_proxy_thread(
    proxy_port_name: String,
    baud_rate: u32,
    tx_to_main: Sender<Command>,        // Canal pour injecter des ordres de VFO/PTT vers l'app
    rx_from_main: Receiver<RadioUpdate>, // Canal pour synchroniser le cache à partir de l'état réel
    exit_flag: Arc<AtomicBool>,
) -> Result<(), String> {
    let port_builder = serialport::new(&proxy_port_name, baud_rate).timeout(Duration::from_millis(10));
    let mut port = match port_builder.open() {
        Ok(p) => p,
        Err(e) => return Err(format!("Erreur d'ouverture du port proxy virtuel: {}", e)),
    };

    thread::spawn(move || {
        let mut read_buf = vec![0; 1024];
        let mut serial_buffer = Vec::new();
        
        let mut current_frequency = 14_074_000u64;
        let mut current_mode = RadioMode::Usb;
        let mut current_filter = 1u8;
        let mut current_data_mode = false;

        while !exit_flag.load(Ordering::SeqCst) {
            // 1. Mise à jour en temps réel du cache local du proxy à partir de l'état réel de la radio
            while let Ok(update) = rx_from_main.try_recv() {
                match update {
                    RadioUpdate::Frequency(f) => current_frequency = f,
                    RadioUpdate::ModeAndFilter(m, filter, is_data) => {
                        current_mode = m;
                        current_filter = filter;
                        current_data_mode = is_data;
                    }
                    _ => {}
                }
            }

            // 2. Lecture des requêtes CAT provenant de MMSSTV (ou autre)
            match port.read(&mut read_buf) {
                Ok(bytes_read) => {
                    if bytes_read > 0 {
                        serial_buffer.extend_from_slice(&read_buf[..bytes_read]);
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(_) => {
                    break; // Déconnexion du port virtuel
                }
            }

            // 3. Décodage et réponse aux trames CI-V simulées
            while let Some(fe_idx) = serial_buffer.windows(2).position(|w| w == [0xFE, 0xFE]) {
                if let Some(fd_idx) = serial_buffer[fe_idx..].iter().position(|&b| b == 0xFD) {
                    let frame_end = fe_idx + fd_idx;
                    let frame = &serial_buffer[fe_idx..=frame_end];
                    
                    // Vérifie si la trame est destinée à la radio (94) en provenance du PC (E0)
                    if frame.len() >= 6 && frame[2] == IC7300_ADDR && frame[3] == PC_ADDR {
                        let cmd = frame[4];
                        match cmd {
                            0x03 => {
                                // MMSSTV demande la fréquence actuelle. Réponse immédiate avec le cache :
                                let bcd = freq_to_bcd(current_frequency);
                                let mut resp = vec![0xFE, 0xFE, PC_ADDR, IC7300_ADDR, 0x03];
                                resp.extend_from_slice(&bcd);
                                resp.push(0xFD);
                                let _ = port.write_all(&resp);
                            }
                            0x04 => {
                                // MMSSTV demande le mode de modulation actuel :
                                let mode_byte = current_mode as u8;
                                let mut resp = vec![0xFE, 0xFE, PC_ADDR, IC7300_ADDR, 0x04, mode_byte, current_filter];
                                if current_data_mode {
                                    resp.push(0x01); // Mode DATA
                                }
                                resp.push(0xFD);
                                let _ = port.write_all(&resp);
                            }
                            0x05 => {
                                // MMSSTV modifie la fréquence d'accord :
                                if frame.len() >= 11 {
                                    let mut f = 0u64;
                                    let mut multiplier = 1u64;
                                    for i in 0..5 {
                                        let b = frame[5 + i];
                                        let lower = b & 0x0F;
                                        let upper = (b >> 4) & 0x0F;
                                        f += (lower as u64) * multiplier; multiplier *= 10;
                                        f += (upper as u64) * multiplier; multiplier *= 10;
                                    }
                                    if f >= 30_000 && f <= 74_800_000 {
                                        // On injecte la commande vers la radio physique
                                        let _ = tx_to_main.send(Command::SetFrequency(f));
                                    }
                                }
                                // Réponse OK standard de la radio
                                let _ = port.write_all(&[0xFE, 0xFE, PC_ADDR, IC7300_ADDR, 0xFB, 0xFD]);
                            }
                            0x1C => {
                                // MMSSTV active ou désactive le PTT :
                                if frame.len() >= 8 && frame[5] == 0x00 {
                                    let ptt_state = frame[6] == 0x01;
                                    // Injecte la commande PTT vers l'automate de notre application
                                    let _ = tx_to_main.send(Command::SetPTT(ptt_state));
                                }
                                // Réponse OK standard
                                let _ = port.write_all(&[0xFE, 0xFE, PC_ADDR, IC7300_ADDR, 0xFB, 0xFD]);
                            }
                            _ => {
                                // Répond positivement (ACK OK) à toute autre commande pour satisfaire MMSSTV
                                let _ = port.write_all(&[0xFE, 0xFE, PC_ADDR, IC7300_ADDR, 0xFB, 0xFD]);
                            }
                        }
                    }
                    serial_buffer.drain(..=frame_end);
                } else {
                    if serial_buffer.len() > 1024 { serial_buffer.drain(..=fe_idx); }
                    break;
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
    });

    Ok(())
}

/// Démarre le thread de communication série avec un Watchdog de connexion actif
pub fn spawn_radio_thread(
    port_name: String,
    baud_rate: u32,
    proxy_port_name: Option<String>, // Nom optionnel du port série virtuel de proxy (ex: Some("COM15"))
    tx_cmd_clone: Sender<Command>,   // Clone du canal émetteur de commandes pour le proxy
    rx: Receiver<Command>,
    tx_radio: Sender<RadioUpdate>,
    exit_flag: Arc<AtomicBool>,
    ctx_clone: eframe::egui::Context,
) -> Result<(), String> {
    let port_builder = serialport::new(&port_name, baud_rate).timeout(Duration::from_millis(10));
    let mut port = match port_builder.open() {
        Ok(p) => p,
        Err(e) => return Err(format!("Erreur d'ouverture: {}", e)),
    };

    thread::spawn(move || {
        let mut read_buf = vec![0; 1024];
        let mut serial_buffer = Vec::new();
        let mut last_meter_poll = Instant::now();
        let mut last_ptt_poll = Instant::now();
        let mut last_status_poll = Instant::now();
        
        // Initialisation du Watchdog (Chien de garde) de connexion
        let mut last_successful_read = Instant::now();

        // Tampon d'accumulation pour la reconstruction du spectre de 475 points défragmenté en 11 paquets
        let mut scope_accumulation_buffer = Vec::with_capacity(512);

        let mut current_filter = 1;
        let mut current_data_mode = false;
        let mut is_tx = false;
        let mut rx_poll_index = 0;
        let mut tx_poll_index = 0;

        // Canal interne asynchrone pour synchroniser le cache du Proxy CAT virtuel
        let (tx_to_proxy, rx_for_proxy) = crossbeam_channel::unbounded::<RadioUpdate>();

        // Lancement asynchrone du thread de Proxy CAT s'il est configuré
        if let Some(proxy_port) = proxy_port_name {
            let _ = spawn_proxy_thread(
                proxy_port,
                baud_rate,
                tx_cmd_clone,
                rx_for_proxy,
                exit_flag.clone(),
            );
        }

        let send_update = |update: RadioUpdate| {
            // Met à jour la file d'attente de l'IHM principale
            if tx_radio.send(update.clone()).is_ok() { ctx_clone.request_repaint(); }
            // Met à jour en temps réel le cache local du Proxy CAT
            let _ = tx_to_proxy.send(update);
        };

        while !exit_flag.load(Ordering::SeqCst) {
            // Lecture des commandes de l'IHM ou du Proxy CAT
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    Command::SetFrequency(freq) => { let _ = port.write_all(&build_civ_frame(0x05, None, &freq_to_bcd(freq))); }
                    Command::SetModeAndFilter(mode, filter) => { 
                        current_filter = filter;
                        let _ = port.write_all(&build_civ_frame(0x06, None, &[mode as u8, filter])); 
                    }
                    Command::SetDataMode(val) => {
                        current_data_mode = val;
                        if val {
                            let _ = port.write_all(&build_civ_frame(0x1A, Some(0x06), &[0x01, current_filter]));
                        } else {
                            let _ = port.write_all(&build_civ_frame(0x1A, Some(0x06), &[0x00, 0x00]));
                        }
                    }
                    Command::SetPTT(val) => { 
                        is_tx = val;
                        let _ = port.write_all(&build_civ_frame(0x1C, Some(0x00), &[if val { 1 } else { 0 }])); 
                    }
                    Command::SetVfo(vfo) => {
                        let subcmd = if vfo == 0 { 0x00 } else { 0x01 };
                        let _ = port.write_all(&build_civ_frame(0x07, Some(subcmd), &[]));
                    }
                    Command::SwapVfo => { let _ = port.write_all(&build_civ_frame(0x07, Some(0xB0), &[])); }
                    Command::EqualizeVfo => { let _ = port.write_all(&build_civ_frame(0x07, Some(0xA0), &[])); }
                    Command::SetSplit(val) => {
                        let subcmd = if val { 0x01 } else { 0x00 };
                        let _ = port.write_all(&build_civ_frame(0x0F, Some(subcmd), &[]));
                    }
                    Command::SetAfGain(val) => { let _ = port.write_all(&build_civ_frame(0x14, Some(0x01), &level_to_bcd(val))); }
                    Command::SetRfGain(val) => { let _ = port.write_all(&build_civ_frame(0x14, Some(0x02), &level_to_bcd(val))); }
                    Command::SetSquelch(val) => { let _ = port.write_all(&build_civ_frame(0x14, Some(0x03), &level_to_bcd(val))); }
                    Command::SetRfPower(val) => { let _ = port.write_all(&build_civ_frame(0x14, Some(0x0A), &level_to_bcd(val))); }
                    Command::SetMicGain(val) => { let _ = port.write_all(&build_civ_frame(0x14, Some(0x0B), &level_to_bcd(val))); }
                    Command::SetCompLevel(val) => { let _ = port.write_all(&build_civ_frame(0x14, Some(0x0E), &level_to_bcd(val))); }
                    Command::SetMonitorLevel(val) => { let _ = port.write_all(&build_civ_frame(0x14, Some(0x15), &level_to_bcd(val))); }
                    Command::SetPreamp(val) => { let _ = port.write_all(&build_civ_frame(0x16, Some(0x02), &[val])); }
                    Command::SetAttenuator(val) => { let _ = port.write_all(&build_civ_frame(0x11, None, &[if val { 0x20 } else { 0x00 }])); }
                    Command::SetAgc(val) => { let _ = port.write_all(&build_civ_frame(0x16, Some(0x12), &[val])); }
                    Command::SetTuner(val) => { let _ = port.write_all(&build_civ_frame(0x1C, Some(0x01), &[if val { 1 } else { 0 }])); }
                    Command::SetNoiseBlanker(val) => { let _ = port.write_all(&build_civ_frame(0x16, Some(0x22), &[if val { 1 } else { 0 }])); }
                    Command::SetNoiseReduction(val) => { let _ = port.write_all(&build_civ_frame(0x16, Some(0x40), &[if val { 1 } else { 0 }])); }
                    Command::SetNoiseBlankerLevel(val) => { let _ = port.write_all(&build_civ_frame(0x14, Some(0x12), &level_to_bcd(val))); }
                    Command::SetNoiseReductionLevel(val) => { let _ = port.write_all(&build_civ_frame(0x14, Some(0x06), &level_to_bcd(val))); }
                    Command::SetScopeOutput(val) => {
                        let state_byte = if val { 1 } else { 0 };
                        let _ = port.write_all(&build_civ_frame(0x27, Some(0x10), &[state_byte]));
                        thread::sleep(Duration::from_millis(20)); 
                        let _ = port.write_all(&build_civ_frame(0x27, Some(0x11), &[state_byte]));
                    }
                    Command::SetScopeSpan(span) => {
                        // FORCE LE MODE CENTER (27 14 00 00) - 2 octets requis : MAIN scope (0x00) + Center mode (0x00)
                        let _ = port.write_all(&build_civ_frame(0x27, Some(0x14), &[0x00, 0x00]));
                        thread::sleep(Duration::from_millis(25));
                        
                        // Envoi de la commande de Span réalignée
                        let bcd = span_to_bcd_bytes(span);
                        let _ = port.write_all(&build_civ_frame(0x27, Some(0x15), &bcd));
                    }
                    Command::SetPower(val) => {
                        if val {
                            // WAKE UP PREAMBLE (150 octets 0xFE à 115200 bauds requis pour réveiller le CPU en veille)
                            let mut wake_frame = vec![0xFE; 150];
                            wake_frame.extend_from_slice(&build_civ_frame(0x18, Some(0x01), &[]));
                            let _ = port.write_all(&wake_frame);
                        } else {
                            // Power OFF (18 00)
                            let _ = port.write_all(&build_civ_frame(0x18, Some(0x00), &[]));
                        }
                    }
                    Command::SetUsbRxLevel(val) => {
                        let bcd = level_to_bcd(val);
                        let _ = port.write_all(&build_civ_frame(0x1A, Some(0x05), &[0x00, 0x60, bcd[0], bcd[1]]));
                    }
                    Command::SetUsbTxLevel(val) => {
                        let bcd = level_to_bcd(val);
                        let _ = port.write_all(&build_civ_frame(0x1A, Some(0x05), &[0x00, 0x65, bcd[0], bcd[1]]));
                    }
                    Command::Disconnect => break,
                }
                thread::sleep(Duration::from_millis(20));
            }

            let now = Instant::now();

            // Chien de garde (Watchdog) de connexion
            if now.duration_since(last_successful_read) >= Duration::from_millis(2500) {
                send_update(RadioUpdate::Disconnected(
                    "Liaison CAT perdue : l'émetteur ne répond plus (hors tension ou câble déconnecté)".to_owned()
                ));
                break;
            }

            if !is_tx {
                if now.duration_since(last_meter_poll) >= Duration::from_millis(40) {
                    last_meter_poll = now;
                    let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x15, 0x02, 0xFD]);
                }
                if now.duration_since(last_ptt_poll) >= Duration::from_millis(80) {
                    last_ptt_poll = now;
                    let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x1C, 0x00, 0xFD]);
                }
                if now.duration_since(last_status_poll) >= Duration::from_millis(150) {
                    last_status_poll = now;
                    match rx_poll_index {
                        0 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x03, 0xFD]); }
                        1 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x04, 0xFD]); }
                        2 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x07, 0xFD]); }
                        3 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x0F, 0xFD]); }
                        4 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x14, 0x01, 0xFD]); }
                        5 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x14, 0x02, 0xFD]); }
                        6 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x14, 0x03, 0xFD]); }
                        7 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x14, 0x0A, 0xFD]); }
                        8 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x14, 0x0B, 0xFD]); }
                        9 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x14, 0x0E, 0xFD]); }
                        10 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x14, 0x15, 0xFD]); }
                        11 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x16, 0x02, 0xFD]); }
                        12 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x11, 0xFD]); }
                        13 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x16, 0x12, 0xFD]); }
                        14 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x1C, 0x01, 0xFD]); }
                        15 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x1A, 0x05, 0x00, 0x60, 0xFD]); }
                        16 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x1A, 0x05, 0x00, 0x65, 0xFD]); }
                        17 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x16, 0x22, 0xFD]); }
                        18 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x16, 0x40, 0xFD]); }
                        19 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x14, 0x12, 0xFD]); }
                        20 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x14, 0x06, 0xFD]); }
                        21 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x27, 0x15, 0xFD]); } // Interroge l'état actuel du Span
                        _ => {}
                    }
                    rx_poll_index = (rx_poll_index + 1) % 22;
                }
            } else {
                if now.duration_since(last_meter_poll) >= Duration::from_millis(40) {
                    last_meter_poll = now;
                    match tx_poll_index {
                        0 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x15, 0x11, 0xFD]); }
                        1 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x15, 0x12, 0xFD]); }
                        2 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x15, 0x13, 0xFD]); }
                        3 => { let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x15, 0x14, 0xFD]); }
                        _ => {}
                    }
                    tx_poll_index = (tx_poll_index + 1) % 4;
                }
                if now.duration_since(last_ptt_poll) >= Duration::from_millis(80) {
                    last_ptt_poll = now;
                    let _ = port.write_all(&[0xFE, 0xFE, IC7300_ADDR, PC_ADDR, 0x1C, 0x00, 0xFD]);
                }
            }

            match port.read(&mut read_buf) {
                Ok(bytes_read) => {
                    if bytes_read > 0 { serial_buffer.extend_from_slice(&read_buf[..bytes_read]); }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) => {
                    send_update(RadioUpdate::Disconnected(format!("Liaison série perdue : {}", e)));
                    break;
                }
            }

            while let Some(fe_idx) = serial_buffer.windows(2).position(|w| w == [0xFE, 0xFE]) {
                if let Some(fd_idx) = serial_buffer[fe_idx..].iter().position(|&b| b == 0xFD) {
                    let frame_end = fe_idx + fd_idx;
                    let frame = &serial_buffer[fe_idx..=frame_end];
                    if frame.len() >= 6 && (frame[2] == PC_ADDR || frame[2] == 0x00) && frame[3] == IC7300_ADDR {
                        
                        // Réinitialisation du Watchdog
                        last_successful_read = Instant::now();

                        let cmd = frame[4];
                        
                        // Décodage des trames d'analyseur de spectre et de Span
                        if cmd == 0x27 && frame.len() >= 9 {
                            let sub_cmd = frame[5];
                            if sub_cmd == 0x00 {
                                let order_curr = frame[7];
                                if order_curr == 0x01 {
                                    scope_accumulation_buffer.clear();
                                    
                                    // Extraction passive du Span depuis le 1er paquet du flux de spectre (27 00).
                                    // Le 1er paquet fait exactement 22 octets de long en mode Center/SCROLL-C.
                                    if frame.len() >= 22 {
                                        let scope_mode = frame[9];
                                        if scope_mode == 0x00 || scope_mode == 0x02 { // 0x00 = Center, 0x02 = SCROLL-C
                                            let bcd_span = &frame[15..20]; // 5 octets
                                            let detected_span = bcd_to_span_val(bcd_span);
                                            if detected_span > 0 {
                                                send_update(RadioUpdate::ScopeSpan(detected_span));
                                            }
                                        }
                                    }
                                } else if order_curr >= 0x02 && order_curr <= 0x11 {
                                    if frame.len() >= 10 {
                                        let data_slice = &frame[9..frame.len() - 1];
                                        scope_accumulation_buffer.extend_from_slice(data_slice);
                                    }
                                    if order_curr == 0x11 {
                                        if !scope_accumulation_buffer.is_empty() {
                                            send_update(RadioUpdate::ScopeSweep(scope_accumulation_buffer.clone()));
                                        }
                                    }
                                }
                            } else if sub_cmd == 0x15 && frame.len() >= 13 {
                                // Réception et décodage de l'état du Span envoyé par l'Icom (ex: via polling de secours)
                                let bcd_data = &frame[6..12]; // 6 octets
                                let detected_span = bcd_to_span_val(bcd_data);
                                if detected_span > 0 {
                                    send_update(RadioUpdate::ScopeSpan(detected_span));
                                }
                            }
                        }
                        else if (cmd == 0x03 || cmd == 0x00) && frame.len() >= 11 {
                            let mut f = 0u64;
                            let mut multiplier = 1u64;
                            for i in 0..5 {
                                let b = frame[5 + i];
                                let lower = b & 0x0F;
                                let upper = (b >> 4) & 0x0F;
                                f += (lower as u64) * multiplier; multiplier *= 10;
                                f += (upper as u64) * multiplier; multiplier *= 10;
                            }
                            if f >= 30_000 && f <= 74_800_000 { send_update(RadioUpdate::Frequency(f)); }
                        }
                        else if (cmd == 0x04 || cmd == 0x01) && frame.len() >= 8 {
                            let m_byte = frame[5];
                            let (is_data, f_byte) = if frame.len() >= 9 { (frame[6] == 0x01, frame[7]) } else { (current_data_mode, frame[6]) };
                            let mode = match m_byte {
                                0x00 => Some(RadioMode::Lsb), 0x01 => Some(RadioMode::Usb),
                                0x02 => Some(RadioMode::Am), 0x03 => Some(RadioMode::Cw),
                                0x05 => Some(RadioMode::Fm), _ => None,
                            };
                            if let Some(m) = mode { send_update(RadioUpdate::ModeAndFilter(m, f_byte, is_data)); }
                        }
                        else if cmd == 0x07 && frame.len() >= 7 {
                            let sub_cmd = frame[5];
                            if sub_cmd == 0x00 {
                                send_update(RadioUpdate::ActiveVfo(0));
                            } else if sub_cmd == 0x01 {
                                send_update(RadioUpdate::ActiveVfo(1));
                            }
                        }
                        else if cmd == 0x0F && frame.len() >= 7 {
                            let sub_cmd = frame[5];
                            send_update(RadioUpdate::SplitState(sub_cmd == 0x01));
                        }
                        else if cmd == 0x15 && frame.len() >= 8 {
                            let sub_cmd = frame[5];
                            let val = bcd_to_u8(&frame[6..frame.len() - 1]);
                            send_update(RadioUpdate::Meter(sub_cmd, val));
                        }
                        else if cmd == 0x11 && frame.len() >= 7 {
                            let val = frame[5];
                            send_update(RadioUpdate::Attenuator(val == 0x20));
                        }
                        else if cmd == 0x14 && frame.len() >= 8 {
                            let sub_cmd = frame[5];
                            let val = bcd_to_u8(&frame[6..frame.len() - 1]);
                            match sub_cmd {
                                0x01 => send_update(RadioUpdate::AfGain(val)),
                                0x02 => send_update(RadioUpdate::RfGain(val)),
                                0x03 => send_update(RadioUpdate::Squelch(val)),
                                0x06 => send_update(RadioUpdate::NoiseReductionLevel(val)),
                                0x0A => send_update(RadioUpdate::RfPower(val)),
                                0x0B => send_update(RadioUpdate::MicGain(val)),
                                0x0E => send_update(RadioUpdate::CompLevel(val)),
                                0x12 => send_update(RadioUpdate::NoiseBlankerLevel(val)),
                                0x15 => send_update(RadioUpdate::MonitorLevel(val)),
                                _ => {}
                            }
                        }
                        else if cmd == 0x16 && frame.len() >= 7 {
                            let sub_cmd = frame[5];
                            let val = frame[6];
                            match sub_cmd {
                                0x02 => send_update(RadioUpdate::Preamp(val)),
                                0x12 => send_update(RadioUpdate::Agc(val)),
                                0x22 => send_update(RadioUpdate::NoiseBlanker(val == 0x01)),
                                0x40 => send_update(RadioUpdate::NoiseReduction(val == 0x01)),
                                _ => {}
                            }
                        }
                        else if cmd == 0x1A && frame.len() >= 11 {
                            let sub_cmd = frame[5];
                            if sub_cmd == 0x05 {
                                let idx_high = frame[6];
                                let idx_low = frame[7];
                                let val = bcd_to_u8(&frame[8..frame.len() - 1]);
                                if idx_high == 0x00 {
                                    if idx_low == 0x60 {
                                        send_update(RadioUpdate::UsbRxLevel(val));
                                    } else if idx_low == 0x65 {
                                        send_update(RadioUpdate::UsbTxLevel(val));
                                    }
                                }
                            }
                        }
                        else if cmd == 0x1C && frame.len() >= 7 {
                            let sub_cmd = frame[5];
                            let val = frame[6];
                            match sub_cmd {
                                0x00 => {
                                    let is_tx_state = val == 0x01;
                                    is_tx = is_tx_state;
                                    send_update(RadioUpdate::PTTState(is_tx_state));
                                }
                                0x01 => send_update(RadioUpdate::Tuner(val == 0x01)),
                                _ => {}
                            }
                        }
                    }
                    serial_buffer.drain(..=frame_end);
                } else {
                    if serial_buffer.len() > 1024 { serial_buffer.drain(..=fe_idx); }
                    break;
                }
            }
            thread::sleep(Duration::from_millis(5));
        }
    });

    Ok(())
}