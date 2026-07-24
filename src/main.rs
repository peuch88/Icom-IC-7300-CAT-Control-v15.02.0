// src/main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod comm;
mod database;
mod gui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1300.0, 900.0])
            .with_min_inner_size([950.0, 600.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "Icom IC-7300 Pro Control --JC Pouchain-- - V15.02.0", 
        options, 
        Box::new(|_cc| Box::new(gui::Ic7300App::default()))
    )
}