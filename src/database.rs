// src/database.rs
// V14.51.0
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::time::Duration;
use std::thread;
use crossbeam_channel::Sender;
use eframe::egui;
use crate::comm::RadioMode;

#[derive(Clone, Debug)]
pub struct DbMemoryEntry {
    pub id: i32,
    pub category: String,
    pub name: String,
    pub frequency: u64,
    pub mode: RadioMode,
    pub is_data: bool,
    pub filter: u8,
    pub preamp: u8,
}

#[derive(Clone, Debug)]
pub struct EibiEntry {
    pub frequency: u64,
    pub station: String,
    pub time: String,
    pub language: String,
    pub target: String,
}

pub fn open_db() -> Result<rusqlite::Connection, rusqlite::Error> {
    let conn = rusqlite::Connection::open("memories.db")?;
    let _ = conn.busy_timeout(Duration::from_secs(5));
    let _ = conn.execute("PRAGMA journal_mode = WAL;", []);
    Ok(conn)
}

pub fn db_reload_memories() -> Vec<DbMemoryEntry> {
    let conn = match open_db() {
        Ok(c) => c, Err(_) => return Vec::new(),
    };
    let mut stmt = match conn.prepare("SELECT id, category, name, frequency, mode, is_data, filter, preamp FROM memories ORDER BY category ASC, frequency ASC") {
        Ok(s) => s, Err(_) => return Vec::new(),
    };
    let entries_iter = stmt.query_map([], |row| {
        let mode_str: String = row.get(4)?;
        let mode = match mode_str.as_str() {
            "LSB" => RadioMode::Lsb, "USB" => RadioMode::Usb,
            "AM" => RadioMode::Am, "CW" => RadioMode::Cw, "FM" => RadioMode::Fm, _ => RadioMode::Am,
        };
        let is_data_val: i32 = row.get(5).unwrap_or(0);
        let filter_val: u8 = row.get(6).unwrap_or(1);
        let preamp_val: u8 = row.get(7).unwrap_or(0);
        Ok(DbMemoryEntry {
            id: row.get(0)?, category: row.get(1)?, name: row.get(2)?, frequency: row.get(3)?,
            mode, is_data: is_data_val == 1, filter: filter_val, preamp: preamp_val,
        })
    });
    match entries_iter {
        Ok(iter) => iter.filter_map(|e| e.ok()).collect(),
        Err(_) => Vec::new(),
    }
}

pub fn db_add_memory(category: &str, name: &str, freq: u64, mode: RadioMode, is_data: bool, filter: u8, preamp: u8) -> Result<(), rusqlite::Error> {
    let conn = open_db()?;
    let mode_str = match mode {
        RadioMode::Lsb => "LSB", RadioMode::Usb => "USB",
        RadioMode::Am => "AM", RadioMode::Cw => "CW", RadioMode::Fm => "FM",
    };
    conn.execute(
        "INSERT INTO memories (category, name, frequency, mode, is_data, filter, preamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![category, name, freq, mode_str, if is_data { 1 } else { 0 }, filter, preamp],
    )?;
    Ok(())
}

pub fn db_update_memory(id: i32, category: &str, name: &str, freq: u64, mode: RadioMode, is_data: bool, filter: u8, preamp: u8) -> Result<(), rusqlite::Error> {
    let conn = open_db()?;
    let mode_str = match mode {
        RadioMode::Lsb => "LSB", RadioMode::Usb => "USB",
        RadioMode::Am => "AM", RadioMode::Cw => "CW", RadioMode::Fm => "FM",
    };
    conn.execute(
        "UPDATE memories SET category = ?1, name = ?2, frequency = ?3, mode = ?4, is_data = ?5, filter = ?6, preamp = ?7 WHERE id = ?8",
        rusqlite::params![category, name, freq, mode_str, if is_data { 1 } else { 0 }, filter, preamp, id],
    )?;
    Ok(())
}

pub fn db_delete_memory(id: i32) -> Result<(), rusqlite::Error> {
    let conn = open_db()?;
    conn.execute("DELETE FROM memories WHERE id = ?1", rusqlite::params![id])?;
    Ok(())
}

pub fn db_save_settings_batch(settings: &[(&str, String)]) -> Result<(), rusqlite::Error> {
    let mut conn = open_db()?;
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare("INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value")?;
        for &(key, ref value) in settings { stmt.execute(rusqlite::params![key, value])?; }
    }
    tx.commit()?;
    Ok(())
}

pub fn db_save_setting(key: &str, value: &str) -> Result<(), rusqlite::Error> {
    let conn = open_db()?;
    conn.execute("INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value", rusqlite::params![key, value])?;
    Ok(())
}

pub fn db_load_settings() -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(conn) = open_db() {
        if let Ok(mut stmt) = conn.prepare("SELECT key, value FROM settings") {
            if let Ok(mut rows) = stmt.query([]) {
                while let Ok(Some(row)) = rows.next() {
                    if let (Ok(k), Ok(v)) = (row.get::<_, String>(0), row.get::<_, String>(1)) { map.insert(k, v); }
                }
            }
        }
    }
    map
}

pub fn init_and_load_db() -> Vec<DbMemoryEntry> {
    let conn = match open_db() {
        Ok(c) => c, Err(_) => return Vec::new(),
    };
    
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS memories (id INTEGER PRIMARY KEY AUTOINCREMENT, category TEXT NOT NULL, name TEXT NOT NULL, frequency INTEGER NOT NULL, mode TEXT NOT NULL);", []);
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS eibi (id INTEGER PRIMARY KEY AUTOINCREMENT, frequency INTEGER NOT NULL, station TEXT NOT NULL, time TEXT NOT NULL, language TEXT NOT NULL, target TEXT NOT NULL);", []);
    let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_eibi_freq ON eibi(frequency);", []);
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);", []);

    let has_column = |col_name: &str| -> bool {
        if let Ok(mut stmt) = conn.prepare("PRAGMA table_info(memories)") {
            if let Ok(mut rows) = stmt.query([]) {
                while let Ok(Some(row)) = rows.next() {
                    if let Ok(name) = row.get::<_, String>(1) {
                        if name == col_name { return true; }
                    }
                }
            }
        }
        false
    };

    if !has_column("is_data") { let _ = conn.execute("ALTER TABLE memories ADD COLUMN is_data INTEGER DEFAULT 0;", []); }
    if !has_column("filter") { let _ = conn.execute("ALTER TABLE memories ADD COLUMN filter INTEGER DEFAULT 1;", []); }
    if !has_column("preamp") { let _ = conn.execute("ALTER TABLE memories ADD COLUMN preamp INTEGER DEFAULT 0;", []); }

    let mut stmt = match conn.prepare("SELECT COUNT(*) FROM memories") {
        Ok(s) => s, Err(_) => return Vec::new(),
    };
    let count: i64 = stmt.query_row([], |row| row.get(0)).unwrap_or(0);

    if count == 0 {
        let swl_data = vec![
            ("🔴 URGENCE & SÉCURITÉ HF", "Détresse Marine (Fréq d'appel mondiale)", 2_182_000, "USB", 0, 1, 1),
            ("🔴 URGENCE & SÉCURITÉ HF", "ASN / GMDSS d'urgence mondiale", 8_414_500, "USB", 0, 1, 1),
            ("🔴 URGENCE & SÉCURITÉ HF", "Secours Inter-Marine National", 4_125_000, "USB", 0, 1, 1),
            ("⏰ SIGNAUX HORAIRES", "WWV Colorado USA (Standard)", 10_000_000, "AM", 0, 1, 0),
            ("⏰ SIGNAUX HORAIRES", "WWV Colorado USA (Alternatif)", 5_000_000, "AM", 0, 1, 0),
            ("⏰ SIGNAUX HORAIRES", "WWV Colorado USA (DX)", 15_000_000, "AM", 0, 1, 0),
            ("⏰ SIGNAUX HORAIRES", "CHU Canada (Heure Française HF)", 7_850_000, "AM", 0, 1, 0),
            ("⏰ SIGNAUX HORAIRES", "CHU Canada (Standard)", 3_330_000, "AM", 0, 1, 0),
            ("⏰ SIGNAUX HORAIRES", "RWM Moscou Russie", 4_996_000, "AM", 0, 1, 0),
            ("⏰ SIGNAUX HORAIRES", "RWM Moscou Russie (DX)", 14_996_000, "AM", 0, 1, 0),
            ("✈️ AVIATION & VOLMET", "Shannon Volmet Irlande (Météo Atlantique)", 5_505_000, "USB", 0, 1, 0),
            ("✈️ AVIATION & VOLMET", "Shannon Volmet (DX)", 8_957_000, "USB", 0, 1, 0),
            ("✈️ AVIATION & VOLMET", "RAF Militaire Volmet Royaume-Uni", 5_450_000, "USB", 0, 1, 0),
            ("✈️ AVIATION & VOLMET", "Gander Volmet Canada", 6_604_000, "USB", 0, 1, 0),
            ("✈️ AVIATION & VOLMET", "Aviation Civile Vol transatlantique", 3_476_000, "USB", 0, 1, 0),
            ("📻 RADIODIFFUSION (SWL)", "Channel 292 Allemagne (Musique/DX)", 6_070_000, "AM", 0, 1, 0),
            ("📻 RADIODIFFUSION (SWL)", "Radio Caroline UK (Bateau Pirate)", 3_985_000, "AM", 0, 1, 0),
            ("📻 RADIODIFFUSION (SWL)", "Radio Taiwan International (FR)", 11_995_000, "AM", 0, 1, 0),
            ("📻 RADIODIFFUSION (SWL)", "Radio Chine Internationale (Europe)", 13_645_000, "AM", 0, 1, 0),
            ("📻 RADIODIFFUSION (SWL)", "RFI Afrique (Ondes courtes)", 15_300_000, "AM", 0, 1, 0),
            ("📻 RADIODIFFUSION (SWL)", "Radio Slovaquie Internationale", 6_005_000, "AM", 0, 1, 0),
            ("📻 RADIODIFFUSION (SWL)", "The Mighty KBC (Musique rétro/Ondes courtes)", 5_960_000, "AM", 0, 1, 0),
            ("📻 RADIODIFFUSION (SWL)", "Voice of America (Grandes ondes/SW)", 9_900_000, "AM", 0, 1, 0),
        ];
        for (category, name, frequency, mode, is_data, filter, preamp) in swl_data {
            let _ = conn.execute("INSERT INTO memories (category, name, frequency, mode, is_data, filter, preamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)", rusqlite::params![category, name, frequency, mode, is_data, filter, preamp]);
        }
    }
    db_reload_memories()
}

pub fn download_and_import_eibi(tx_status: Sender<String>, ctx: egui::Context) {
    thread::spawn(move || {
        let send_status = |status: String| {
            if tx_status.send(status).is_ok() {
                ctx.request_repaint();
            }
        };

        send_status("Téléchargement de la base EiBi (A26)...".to_owned());
        let url = "http://www.eibispace.de/dx/sked-a26.csv";
        let response = match ureq::get(url).call() {
            Ok(res) => res,
            Err(e) => {
                send_status(format!("Erreur réseau : {}", e));
                return;
            }
        };

        let mut body_bytes = Vec::new();
        if response.into_reader().read_to_end(&mut body_bytes).is_err() {
            send_status("Erreur lors de la lecture des données réseau.".to_owned());
            return;
        }

        let body = String::from_utf8_lossy(&body_bytes).into_owned();
        send_status("Importation dans SQLite (Transaction)...".to_owned());

        let mut conn = match open_db() {
            Ok(c) => c,
            Err(_) => {
                send_status("Erreur : memories.db verrouillé.".to_owned());
                return;
            }
        };

        let tx = match conn.transaction() {
            Ok(t) => t,
            Err(_) => {
                send_status("Erreur de transaction SQLite.".to_owned());
                return;
            }
        };

        let _ = tx.execute("DELETE FROM eibi", []);
        let mut count = 0;
        for line in body.lines() {
            if line.starts_with("kHz") || line.trim().is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split(';').collect();
            if parts.len() < 7 {
                continue;
            }

            let khz_str = parts[0].trim();
            let time_str = parts[1].trim();
            let station_str = parts[4].trim();
            let lang_str = parts[5].trim();
            let target_str = parts[6].trim();

            if let Ok(khz) = khz_str.parse::<f64>() {
                let hz = (khz * 1000.0) as u64;
                let _ = tx.execute(
                    "INSERT INTO eibi (frequency, station, time, language, target) VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![hz, station_str, time_str, lang_str, target_str],
                );
                count += 1;
            }
        }

        if tx.commit().is_ok() {
            send_status(format!("Succès ! {} stations enregistrées.", count));
        } else {
            send_status("Erreur lors du commit final.".to_owned());
        }
    });
}

pub fn search_eibi(query: &str, current_freq: u64) -> Vec<EibiEntry> {
    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let map_fn = |row: &rusqlite::Row| -> rusqlite::Result<EibiEntry> {
        Ok(EibiEntry {
            frequency: row.get(0)?, station: row.get(1)?,
            time: row.get(2)?, language: row.get(3)?,
            target: row.get(4)?,
        })
    };

    if query.trim().is_empty() {
        let min_hz = current_freq.saturating_sub(100_000);
        let max_hz = current_freq.saturating_add(100_000);
        let query_sql = "SELECT frequency, station, time, language, target FROM eibi WHERE frequency >= ?1 AND frequency <= ?2 ORDER BY frequency ASC LIMIT 50";
        if let Ok(mut stmt) = conn.prepare(query_sql) {
            if let Ok(iter) = stmt.query_map(rusqlite::params![min_hz, max_hz], map_fn) {
                return iter.filter_map(|e| e.ok()).collect();
            }
        }
        return Vec::new();
    }

    if let Ok(khz) = query.parse::<f64>() {
        let hz = (khz * 1000.0) as u64;
        let min_hz = hz.saturating_sub(10_000);
        let max_hz = hz.saturating_add(10_000);
        let query_sql = "SELECT frequency, station, time, language, target FROM eibi WHERE frequency >= ?1 AND frequency <= ?2 ORDER BY frequency ASC LIMIT 50";
        if let Ok(mut stmt) = conn.prepare(query_sql) {
            if let Ok(iter) = stmt.query_map(rusqlite::params![min_hz, max_hz], map_fn) {
                return iter.filter_map(|e| e.ok()).collect();
            }
        }
    } else {
        let wildcard_query = format!("%{}%", query);
        let query_sql = "SELECT frequency, station, time, language, target FROM eibi WHERE station LIKE ?1 ORDER BY frequency ASC LIMIT 50";
        if let Ok(mut stmt) = conn.prepare(query_sql) {
            if let Ok(iter) = stmt.query_map(rusqlite::params![wildcard_query], map_fn) {
                return iter.filter_map(|e| e.ok()).collect();
            }
        }
    }
    Vec::new()
}

pub fn get_probable_stations(freq_hz: u64) -> Vec<EibiEntry> {
    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let min_hz = freq_hz.saturating_sub(2000);
    let max_hz = freq_hz.saturating_add(2000);
    let query_sql = "SELECT frequency, station, time, language, target FROM eibi WHERE frequency >= ?1 AND frequency <= ?2 ORDER BY frequency ASC";
    
    let map_fn = |row: &rusqlite::Row| -> rusqlite::Result<EibiEntry> {
        Ok(EibiEntry {
            frequency: row.get(0)?, station: row.get(1)?,
            time: row.get(2)?, language: row.get(3)?,
            target: row.get(4)?,
        })
    };

    if let Ok(mut stmt) = conn.prepare(query_sql) {
        if let Ok(iter) = stmt.query_map(rusqlite::params![min_hz, max_hz], map_fn) {
            return iter.filter_map(|e| e.ok()).collect();
        }
    }
    Vec::new()
}

pub fn is_time_in_range(time_range: &str, utc_hour: u32, utc_minute: u32) -> bool {
    let parts: Vec<&str> = time_range.split('-').collect();
    if parts.len() != 2 {
        return false;
    }
    let start_val = parts[0].parse::<u32>().unwrap_or(0);
    let end_val = parts[1].parse::<u32>().unwrap_or(0);
    let current_val = utc_hour * 100 + utc_minute;
    
    if start_val <= end_val {
        current_val >= start_val && current_val <= end_val
    } else {
        current_val >= start_val || current_val <= end_val
    }
}

pub fn escape_csv_field(field: &str) -> String {
    if field.contains(';') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        let escaped = field.replace("\"", "\"\"");
        format!("\"{}\"", escaped)
    } else {
        field.to_owned()
    }
}

pub fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                if in_quotes && chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next(); 
                } else {
                    in_quotes = !in_quotes;
                }
            }
            ';' if !in_quotes => {
                fields.push(current.clone());
                current.clear();
            }
            _ => {
                current.push(c);
            }
        }
    }
    fields.push(current);
    fields
}

pub fn export_settings_csv(path: &str) -> Result<(), String> {
    let saved = db_load_settings();
    let mut file = File::create(path).map_err(|e| e.to_string())?;
    writeln!(file, "key;value").map_err(|e| e.to_string())?;
    for (k, v) in saved {
        writeln!(file, "{};{}", escape_csv_field(&k), escape_csv_field(&v)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn import_settings_csv(path: &str) -> Result<(), String> {
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut lines = content.lines();
    let _header = lines.next(); 
    
    for line in lines {
        let fields = parse_csv_line(line);
        if fields.len() >= 2 {
            let _ = db_save_setting(&fields[0], &fields[1]);
        }
    }
    Ok(())
}

pub fn export_memories_csv(path: &str) -> Result<(), String> {
    let memories = db_reload_memories();
    let mut file = File::create(path).map_err(|e| e.to_string())?;
    writeln!(file, "category;name;frequency;mode;is_data;filter;preamp").map_err(|e| e.to_string())?;
    for m in memories {
        let mode_str = match m.mode {
            RadioMode::Lsb => "LSB", RadioMode::Usb => "USB",
            RadioMode::Am => "AM", RadioMode::Cw => "CW", RadioMode::Fm => "FM",
        };
        writeln!(
            file, 
            "{};{};{};{};{};{};{}", 
            escape_csv_field(&m.category), 
            escape_csv_field(&m.name), 
            m.frequency, 
            mode_str,
            if m.is_data { 1 } else { 0 },
            m.filter,
            m.preamp
        ).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn import_memories_csv(path: &str) -> Result<(), String> {
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut lines = content.lines();
    let _header = lines.next(); 

    let mut tx_conn = open_db().map_err(|e| e.to_string())?;
    let tx = tx_conn.transaction().map_err(|e| e.to_string())?;
    let _ = tx.execute("DELETE FROM memories", []);

    for line in lines {
        let fields = parse_csv_line(line);
        if fields.len() >= 4 {
            let category = &fields[0];
            let name = &fields[1];
            let freq: u64 = fields[2].parse().unwrap_or(0);
            let mode_str = &fields[3];
            
            let is_data_val: i32 = if fields.len() >= 5 { fields[4].parse().unwrap_or(0) } else { 0 };
            let filter_val: u8 = if fields.len() >= 6 { fields[5].parse().unwrap_or(1) } else { 1 };
            let preamp_val: u8 = if fields.len() >= 7 { fields[6].parse().unwrap_or(0) } else { 0 };

            let _ = tx.execute(
                "INSERT INTO memories (category, name, frequency, mode, is_data, filter, preamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![category, name, freq, mode_str, is_data_val, filter_val, preamp_val],
            );
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn export_eibi_csv(path: &str) -> Result<(), String> {
    let conn = open_db().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT frequency, station, time, language, target FROM eibi").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
        Ok(EibiEntry {
            frequency: row.get(0)?, station: row.get(1)?,
            time: row.get(2)?, language: row.get(3)?,
            target: row.get(4)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut file = File::create(path).map_err(|e| e.to_string())?;
    writeln!(file, "frequency;station;time;language;target").map_err(|e| e.to_string())?;
    for r in rows {
        if let Ok(entry) = r {
            writeln!(
                file, 
                "{};{};{};{};{}", 
                entry.frequency, 
                escape_csv_field(&entry.station), 
                escape_csv_field(&entry.time), 
                escape_csv_field(&entry.language), 
                escape_csv_field(&entry.target)
            ).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn import_eibi_csv(path: &str) -> Result<(), String> {
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut lines = content.lines();
    let _header = lines.next(); 

    let mut tx_conn = open_db().map_err(|e| e.to_string())?;
    let tx = tx_conn.transaction().map_err(|e| e.to_string())?;
    let _ = tx.execute("DELETE FROM eibi", []);

    for line in lines {
        let fields = parse_csv_line(line);
        if fields.len() >= 5 {
            let frequency: u64 = fields[0].parse().unwrap_or(0);
            let station = &fields[1];
            let time = &fields[2];
            let language = &fields[3];
            let target = &fields[4];
            let _ = tx.execute(
                "INSERT INTO eibi (frequency, station, time, language, target) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![frequency, station, time, language, target],
            );
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}