// src/comm.rs
// Version : 15.02.0 - Ajout de la gestion de mise sous/hors tension (Power ON/OFF) par trame de réveil série longue

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
    SetPower(bool),           // Ajouté : Commande d'alimentation ON/OFF (18 00 / 18 01)
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

pub fn build_civ_frame(cmd: u8, subcmd: Option<u8>, data: &[u8]) -> Vec<u8> {
    let mut frame = vec![0xFE, 0xFE, IC7300_ADDR, PC_ADDR, cmd];
    if let Some(sc) = subcmd { frame.push(sc); }
    frame.extend_from_slice(data); frame.push(0xFD);
    frame
}

/// Démarre le thread de communication série avec un Watchdog de connexion actif
pub fn spawn_radio_thread(
    port_name: String,
    baud_rate: u32,
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

        let send_update = |update: RadioUpdate| {
            if tx_radio.send(update).is_ok() { ctx_clone.request_repaint(); }
        };

        while !exit_flag.load(Ordering::SeqCst) {
            // Lecture des commandes de l'UI
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
                        _ => {}
                    }
                    rx_poll_index = (rx_poll_index + 1) % 21;
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
                        
                        // Décodage des trames du spectre
                        if cmd == 0x27 && frame.len() >= 9 {
                            let sub_cmd = frame[5];
                            if sub_cmd == 0x00 {
                                let order_curr = frame[7];
                                if order_curr == 0x01 {
                                    scope_accumulation_buffer.clear();
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