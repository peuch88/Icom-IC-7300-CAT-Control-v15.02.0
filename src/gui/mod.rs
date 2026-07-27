// Version: V15.4.0 - Ajout de la déclaration du sous-module keyer pour l'automate de balise d'appels
// Déclaration des sous-modules de l'interface graphique du contrôleur CAT Icom IC-7300

pub mod app;
pub mod view;
pub mod widgets;
pub mod dialogs;
pub mod scope;
pub mod keyer; // Sous-module du lanceur d'appels automatique et de la balise
pub use app::Ic7300App;