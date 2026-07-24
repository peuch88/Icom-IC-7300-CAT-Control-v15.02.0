// src/gui/widgets.rs
// V14.51.0
use eframe::egui::{self, Color32, Stroke, RichText};

/// Calcule la couleur progressive du signal RX (Vert -> Jaune -> Rouge)
pub fn rx_signal_color(val: u8) -> Color32 {
    let t = (val as f32 / 241.0).clamp(0.0, 1.0);
    if t < 0.5 {
        let factor = t * 2.0;
        let r = (factor * 255.0) as u8;
        let g = (230.0 + factor * (215.0 - 230.0)) as u8;
        let b = (118.0 * (1.0 - factor)) as u8;
        Color32::from_rgb(r, g, b)
    } else {
        let factor = (t - 0.5) * 2.0;
        let r = 255;
        let g = (215.0 * (1.0 - factor)) as u8;
        let b = (40.0 * factor) as u8;
        Color32::from_rgb(r, g, b)
    }
}

/// Dessine un bouton radio 3D skeuomorphique compact (ombre 1.5px, enfoncement 1.0px)
pub fn custom_3d_button_sized(ui: &mut egui::Ui, text: &str, active: bool, color: Color32, size: egui::Vec2) -> bool {
    let text_color = Color32::WHITE;
    let bg_color = if active { color } else { Color32::from_rgb(50, 50, 55) };
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    
    if response.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    let painter = ui.painter();
    
    // Ombre de projection réduite de 2.0 à 1.5 pour les petits boutons
    let mut shadow_rect = rect;
    shadow_rect.min.y += 1.5; shadow_rect.max.y += 1.5;
    painter.rect_filled(shadow_rect, 4.0, Color32::from_rgb(15, 15, 18));
    
    // Enfoncement réduit à 1.0 pixel
    let body_rect = if response.is_pointer_button_down_on() || active {
        egui::Rect::from_min_max(
            egui::pos2(rect.min.x, rect.min.y + 1.0),
            egui::pos2(rect.max.x, rect.max.y + 1.0),
        )
    } else {
        rect
    };
    
    painter.rect_filled(body_rect, 4.0, bg_color);
    
    if !active && !response.is_pointer_button_down_on() {
        painter.line_segment(
            [egui::pos2(body_rect.min.x + 1.0, body_rect.min.y + 1.0), egui::pos2(body_rect.max.x - 1.0, body_rect.min.y + 1.0)],
            Stroke::new(1.0, Color32::from_rgb(90, 90, 95))
        );
        painter.line_segment(
            [egui::pos2(body_rect.min.x + 1.0, body_rect.min.y + 1.0), egui::pos2(body_rect.min.x + 1.0, body_rect.max.y - 1.0)],
            Stroke::new(1.0, Color32::from_rgb(90, 90, 95))
        );
    } else {
        painter.line_segment(
            [egui::pos2(body_rect.min.x + 1.0, body_rect.min.y + 1.0), egui::pos2(body_rect.max.x - 1.0, body_rect.min.y + 1.0)],
            Stroke::new(1.2, Color32::from_rgb(10, 10, 12))
        );
        painter.line_segment(
            [egui::pos2(body_rect.min.x + 1.0, body_rect.min.y + 1.0), egui::pos2(body_rect.min.x + 1.0, body_rect.max.y - 1.0)],
            Stroke::new(1.2, Color32::from_rgb(10, 10, 12))
        );
    }
    
    painter.rect_stroke(body_rect, 4.0, Stroke::new(1.0, Color32::from_rgb(30, 30, 35)));
    if response.hovered() && !active { painter.rect_filled(body_rect, 4.0, Color32::from_white_alpha(15)); }
    
    let text_pos = body_rect.center();
    painter.text(text_pos, egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(11.0), text_color);
    
    response.clicked()
}

/// Helper de bouton 3D recouvrant la largeur disponible par défaut
pub fn custom_3d_button(ui: &mut egui::Ui, text: &str, active: bool, color: Color32) -> bool {
    custom_3d_button_sized(ui, text, active, color, egui::vec2(ui.available_width(), 26.0))
}

/// Rendu de mesure flexible à segments LED (Vert, Jaune, Rouge)
pub fn render_flexible_segmented_meter(ui: &mut egui::Ui, label: &str, value: u8, max_val: f32, default_color: Color32, formatter: impl Fn(u8) -> String) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).strong().color(Color32::LIGHT_GRAY).size(11.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(formatter(value)).strong().color(default_color).size(11.0));
            });
        });
        ui.add_space(2.0);
        
        let progress = (value as f32 / max_val).clamp(0.0, 1.0);
        let desired_size = egui::vec2(ui.available_width(), 14.0);
        let (rect, _response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
        
        let painter = ui.painter();
        painter.rect_filled(rect, 4.0, Color32::from_rgb(18, 18, 22));
        painter.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgb(45, 45, 50)));
        
        let total_segments = 24; 
        let gap = 1.5;
        let total_gaps_width = (total_segments - 1) as f32 * gap;
        let segment_width = (rect.width() - 2.0 * gap - total_gaps_width) / total_segments as f32;
        let active_segments = (progress * total_segments as f32).round() as usize;
        
        for i in 0..total_segments {
            let seg_x = rect.min.x + gap + i as f32 * (segment_width + gap);
            let seg_rect = egui::Rect::from_min_max(
                egui::pos2(seg_x, rect.min.y + 2.0),
                egui::pos2(seg_x + segment_width, rect.max.y - 2.0)
            );
            
            let is_active = i < active_segments;
            let seg_color = if is_active {
                if i < 14 { Color32::from_rgb(0, 230, 118) }
                else if i < 20 { Color32::from_rgb(255, 215, 0) }
                else { Color32::from_rgb(255, 40, 40) }
            } else {
                if i < 14 { Color32::from_rgb(10, 45, 25) }
                else if i < 20 { Color32::from_rgb(55, 45, 10) }
                else { Color32::from_rgb(55, 15, 15) }
            };
            painter.rect_filled(seg_rect, 1.0, seg_color);
        }
    });
}

/// Potentiomètre horizontal imitant un fader de console avec tête métallique argentée
pub fn custom_gain_slider(ui: &mut egui::Ui, value: &mut u8, label: &str, color: Color32) -> bool {
    let mut changed = false;
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).strong().color(Color32::LIGHT_GRAY).size(11.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let pct = ((*value as f32 / 255.0) * 100.0).round() as u32;
                ui.label(RichText::new(format!("{} %", pct)).strong().color(color).size(11.0));
            });
        });
        ui.add_space(2.0);
        
        let desired_size = egui::vec2(ui.available_width(), 14.0);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());
        
        if response.clicked() || response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let relative_x = (pos.x - rect.left()) / rect.width();
                let clamped_x = relative_x.clamp(0.0, 1.0);
                let new_val = (clamped_x * 255.0).round() as u8;
                if *value != new_val { *value = new_val; changed = true; }
            }
        }
        
        let painter = ui.painter();
        painter.rect_filled(rect, 3.0, Color32::from_rgb(15, 15, 18));
        painter.rect_stroke(rect, 3.0, Stroke::new(1.0, Color32::from_rgb(45, 45, 50)));
        
        let fill_width = rect.width() * (*value as f32 / 255.0);
        if fill_width > 0.0 {
            let fill_rect = egui::Rect::from_min_max(rect.min, egui::pos2(rect.min.x + fill_width, rect.max.y));
            painter.rect_filled(fill_rect, 3.0, color);
            let gloss_rect = egui::Rect::from_min_max(fill_rect.min, egui::pos2(fill_rect.max.x, fill_rect.min.y + 4.0));
            painter.rect_filled(gloss_rect, 1.0, Color32::from_white_alpha(30));
        }
        
        if fill_width > 0.0 || *value == 0 {
            let handle_x = rect.min.x + fill_width;
            let handle_width = 8.0;
            let handle_rect = egui::Rect::from_min_max(
                egui::pos2(handle_x - handle_width / 2.0, rect.min.y - 2.0),
                egui::pos2(handle_x + handle_width / 2.0, rect.max.y + 2.0)
            ).intersect(rect.expand(2.0));
            
            painter.rect_filled(handle_rect, 2.0, Color32::from_rgb(180, 180, 185));
            painter.rect_stroke(handle_rect, 2.0, Stroke::new(1.0, Color32::from_rgb(50, 50, 55)));
            painter.line_segment(
                [egui::pos2(handle_x, handle_rect.min.y + 1.0), egui::pos2(handle_x, handle_rect.max.y - 1.0)],
                Stroke::new(1.5, Color32::from_rgb(30, 30, 35))
            );
        }
        if response.hovered() || response.dragged() { painter.rect_stroke(rect, 3.0, Stroke::new(1.0, Color32::WHITE)); }
    });
    changed
}

/// Formate la fréquence avec des points séparateurs
pub fn format_vfo_freq(freq: u64) -> String {
    let freq_str = format!("{:08}", freq);
    let mut res = String::new();
    for (idx, ch) in freq_str.chars().enumerate() {
        if idx == 2 || idx == 5 { res.push('.'); }
        res.push(ch);
    }
    res
}

/// Dessine de manière vectorielle une LED d'indicateur d'état standard (Vert/Gris)
pub fn draw_led(ui: &mut egui::Ui, active: bool) {
    let (dot_rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
    let dot_color = if active { Color32::from_rgb(0, 230, 118) } else { Color32::from_rgb(80, 80, 85) };
    if active {
        ui.painter().circle_filled(dot_rect.center(), 5.5, Color32::from_rgba_unmultiplied(0, 230, 118, 40)); 
    }
    ui.painter().circle_filled(dot_rect.center(), 3.5, dot_color);
    ui.painter().circle_stroke(dot_rect.center(), 3.5, Stroke::new(1.0, Color32::from_rgb(30, 30, 35)));
}

/// Dessine une LED de connexion (Vert si en ligne, Rouge si hors ligne, avec halo)
pub fn draw_connection_led(ui: &mut egui::Ui, connected: bool) {
    let (dot_rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
    let dot_color = if connected { Color32::from_rgb(0, 230, 118) } else { Color32::from_rgb(229, 57, 53) };
    let halo_color = if connected {
        Color32::from_rgba_unmultiplied(0, 230, 118, 40)
    } else {
        Color32::from_rgba_unmultiplied(229, 57, 53, 40)
    };
    ui.painter().circle_filled(dot_rect.center(), 6.5, halo_color); // Halo lumineux
    ui.painter().circle_filled(dot_rect.center(), 4.0, dot_color);  // Noyau LED
    ui.painter().circle_stroke(dot_rect.center(), 4.0, Stroke::new(1.0, Color32::from_rgb(30, 30, 35)));
}

/// Encodage URL minimal
pub fn url_encode(input: &str) -> String {
    let mut encoded = String::new();
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => { encoded.push(b as char); }
            b' ' => { encoded.push('+'); }
            _ => { encoded.push_str(&format!("%{:02X}", b)); }
        }
    }
    encoded
}

#[inline(never)]
pub fn fprint_err(e: &str) {
    eprintln!("{}", e);
}