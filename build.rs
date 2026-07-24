// Version 1.0.0
// Script de compilation pour l'intégration de l'icône d'application Windows (.exe)

fn main() {
    // Vérifie si le système d'exploitation cible est Windows
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("icon.ico"); // Chemin vers votre fichier d'icône à la racine
        res.compile().unwrap();
    }
}