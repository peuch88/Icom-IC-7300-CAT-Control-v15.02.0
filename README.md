### Téléchargement Versions:

[setup_icom_controller_v15.02.0.exe](https://github.com/peuch88/Icom-IC-7300-CAT-Control-v15.02.0/blob/main/setup_icom_controller_v15.02.0.exe)

[setup_icom_controller_v15.03.0.exe](https://github.com/peuch88/Icom-IC-7300-CAT-Control-v15.02.0/blob/main/setup_icom_controller_v15.03.0.exe)

[setup_icom_controller_v15.03.12.exe](https://github.com/peuch88/Icom-IC-7300-CAT-Control-v15.02.0/blob/main/setup_icom_controller_v15.03.12.exe)

[setup_icom_controller_v15.04.0.exe](https://github.com/peuch88/Icom-IC-7300-CAT-Control-v15.02.0/blob/main/setup_icom_controller_v15.04.0.exe)


utilisation et configurations

[Manuel d'utilisation](https://github.com/peuch88/Icom-IC-7300-CAT-Control-v15.02.0/blob/main/Tutoriel-géneral.html)



        NOTICE D'UTILISATION - CONTRÔLEUR ICOM IC-7300 CAT PRO

Version du logiciel : 15.4.0
Éditeur : J-C Pouchain Production
Site Web : https://14frs1525.fr

Ce document contient l'intégralité des instructions pour installer le
logiciel, configurer physiquement votre émetteur-récepteur Icom IC-7300,
manipuler l'interface et gérer les bases de données intégrées (mémoires
locales et base mondiale de radiodiffusion EiBi Space).

------------------------------------------------------------------------
1. INSTALLATION DU LOGICIEL (SOFTWARE)
------------------------------------------------------------------------
telecharger la derniere version sur https://14frs1525.fr

------------------------------------------------------------------------
2. CONFIGURATION PHYSIQUE DU POSTE ICOM IC-7300
------------------------------------------------------------------------
Pour que l'ordinateur et le transceiver communiquent correctement (en
particulier pour recevoir le flux rapide de l'analyseur de spectre),
vous devez configurer le menu interne de votre IC-7300.

Connectez l'IC-7300 à votre PC avec un câble USB de qualité, allumez le
poste, puis modifiez les réglages suivants sur l'écran tactile du poste :

  1. Entrez dans le menu : MENU -> SET -> Connectors -> CI-V.
  2. CI-V Baud Rate : Réglez sur "115200" (obligatoire pour la vitesse du
     spectre).
  3. CI-V Address : Réglez sur "94h" (adresse d'origine d'usine de l'IC-7300).
  4. CI-V Transceive : Réglez sur "ON" (permet à la radio de notifier
     instantanément le PC lorsque vous tournez le bouton de VFO).
  5. CI-V USB Port : Réglez impérativement sur "Unlink from [REMOTE]".
     * Pourquoi ? Le mode délié (Unlink) permet au port USB de communiquer
       à 115200 bauds de manière indépendante, sans être limité par la
       vitesse lente de la prise jack REMOTE physique.
  6. CI-V USB Baud Rate : Réglez sur "115200".
  7. CI-V USB Echo Back : Réglez sur "ON".

------------------------------------------------------------------------
3. UTILISATION DE L'INTERFACE GRAPHIQUE (IHM)
------------------------------------------------------------------------
L'interface est skeuomorphique et imite l'ergonomie d'un véritable
appareil de radiocommunication.

Connexion initiale :
  Au démarrage, cliquez sur le bouton "Configuration COM" dans la barre
  supérieure. Sélectionnez le port COM correspondant à votre IC-7300
  (généralement détecté automatiquement) et vérifiez que la vitesse est
  réglée sur "115200". Cliquez ensuite sur "Connecter le Transceiver" en
  haut à droite. Le voyant LED passe du Rouge (OFFLINE) au Vert (ONLINE).

Contrôle de fréquence et double VFO :
  * Saisie directe : Cliquez sur les chiffres du cadran LCD central pour
    modifier le pas de réglage (Tuning Step).
  * Molette de la souris : Survolez le cadran LCD ou le bouton de VFO
    rotatif virtuel et utilisez la molette de votre souris pour faire
    défiler la fréquence.
  * Clavier physique : Lorsque votre souris survole la zone du VFO :
    - Fleche Gauche / Fleche Droite : Diminue ou augmente le pas de
      réglage (STEP) de 1 Hz à 1 MHz.
    - Fleche Haut / Fleche Bas : Modifie la fréquence active du pas
      sélectionné.
  * Double VFO & SPLIT :
    - Le cadre affiche l'état des deux VFO (VFO A et VFO B). Cliquez sur
      "Activer" à côté du VFO inactif pour basculer dessus.
    - "A / B" : Permet d'échanger l'état des deux VFO.
    - "A = B" : Recopie la fréquence, le mode et le filtre du VFO actif
      sur le VFO inactif.
    - "SPLIT" : Active le mode SPLIT (réception sur le VFO actif, émission
      sur le VFO inactif). Le bouton s'allune en rouge.

Commandes tactiles de réception (Filtres, NB, NR) :
  * Filtres (FIL1, FIL2, FIL3) : Commute la largeur de bande passante DSP
    interne de la radio.
  * P.AMP & AGC : Ajuste les préamplificateurs de réception (AMP1, AMP2
    ou OFF) et la constante de temps du contrôle automatique de gain
    (FAST, MID, SLOW).
  * ATT & TUNER : Active l'atténuateur RF de 20 dB et déclenche le
    coupleur d'antenne automatique interne de l'IC-7300.
  * NB & NR (Noise Blanker & Noise Reduction) : Active les filtres
    numériques d'atténuation des parasites impulsionnels (NB) ou du
    souffle résiduel de la bande (NR).

Contrôle de l'Émission (PTT & LOCK) :
  * Appuyez sur la Barre d'espace de votre clavier pour passer instantanément
    en émission (PTT actif) tant que le pointeur de votre souris est
    positionné au-dessus de l'application. Relâchez la touche pour repasser
    en réception.
  * Cliquer sur le bouton "LOCK" active le mode de verrouillage de sécurité
    de l'émission. Lorsqu'il est sur ON, la barre d'espace agit comme un
    interrupteur à bascule (Toggle) : une impulsion pour émettre, une
    impulsion pour recevoir.

Ajustement des Gains et Niveaux :
   * Cliquez sur le bouton "Réglages Gains" dans la barre supérieure pour
  ouvrir la console de mixage. Vous y trouverez les potentiomètres
  linéaires pour :
    - Le volume (AF Gain) et le gain de réception RF (Noise floor).
    - Le silencieux (Squelch) et la puissance d'émission RF (0 à 100 %).
    - Les niveaux d'intensité du Noise Blanker (NB) et de la Noise
      Reduction (NR).
    - Les gains micro, compresseur, retour moniteur et les volumes
      d'entrée/sortie de la carte son USB interne du poste.

Analyseur de Spectre & Waterfall :
 * Cliquez sur "Spectre / Waterfall" dans l'en-tête pour ouvrir la
  fenêtre de l'analyseur de spectre :
    - Cliquez sur "ACTIVER LE FLUX" pour démarrer l'affichage du spectre
       en temps réel.
    - Spectre FFT : Affiche la courbe d'intensité des signaux entourant
       votre fréquence. Un repère vertical jaune semi-transparent indique
       la fréquence centrale exacte de votre récepteur.
    - Cascade Waterfall : Affiche l'historique temporel des signaux
       défilant vers le bas.
    - Accord par clic (Tuning) : Cliquez directement sur un signal visuel
       de la cascade (Waterfall) pour accorder instantanément votre
       récepteur sur cette fréquence, arrondie automatiquement au kilohertz
       le plus proche.
    - Ajustement de la cascade : 
      * Ouvrez le menu déroulant "Reglages Cascade & Couleurs" pour :
         - Changer la palette de couleurs (Arc-en-ciel standard, Glace
           bleu/cyan, Magma feu, ou Niveaux de gris).
         - Ajuster le Seuil de bruit (Offset en dB) pour effacer le bruit
           de fond visuel.
         - Ajuster le Contraste (Gain x) pour faire briller les signaux
           très faibles.
         * Note importante : Veillez à sélectionner dans le menu déroulant
           "Span" la même valeur que celle affichée sur l'écran physique de
           votre Icom (par exemple ±25 kHz) pour garantir que l'accord par
           clic soit d'une précision géométrique absolue.

------------------------------------------------------------------------
4. GESTION DES BASES DE DONNÉES & BACKUPS
------------------------------------------------------------------------

Base de données des Mémoires Locales (SQLite) :
  * Cliquez sur le bouton "Gérer les Mémoires" dans la barre supérieure
    pour afficher le gestionnaire.
    - Ajout de mémoire : Saisissez la catégorie, le nom de la station et
      cliquez sur "Ajouter à la Base". Vous pouvez aussi cliquer sur
      "Capturer l'état actuel" pour copier instantanément la fréquence, le
      mode, le préampli et le filtre en cours d'utilisation sur la radio.
    - Rappel de mémoire : Cliquez simplement sur une ligne de la liste des
      mémoires pour appliquer instantanément l'état complet au transceiver.
    - Édition : Utilisez l'icône de crayon "✏" pour modifier une mémoire ou
      la corbeille "🗑" pour la supprimer définitivement.

Base de données mondiale EiBi Space :
  * L'application intègre le support de la base mondiale de radiodiffusion
    EiBi Space.
    - Dans l'onglet de droite "EIBI", cliquez sur "📥 Télécharger" pour
      récupérer automatiquement la dernière base à jour depuis les
      serveurs d'EiBi Space. L'importation s'effectue de manière
      sécurisée en arrière-plan.
    - Vous pouvez effectuer des recherches par nom de station ou par
      fréquence en kHz.
    - Stations Probables (UTC Watchdog) : En bas à droite de l'écran
      principal, l'application compare en permanence l'heure de votre
      système (convertie à l'heure universelle UTC) et votre fréquence
      d'écoute actuelle pour lister les stations qui diffusent
      actuellement sur cette fréquence. Les stations actives affichent un
      voyant vert d'activité. Cliquez sur l'icône "ℹ" pour lancer une
      recherche Google à propos de cette station.

Importation / Exportation CSV :
  Cliquez sur "Import/Export CSV" pour sauvegarder ou restaurer vos
  configurations sous forme de fichiers tableurs éditables (au format
  universel CSV séparé par des points-virgules) :
    - settings_backup.csv : Vos paramètres système (port COM, vitesse,
      dernier état du VFO).
    - memories_backup.csv : L'intégralité de vos mémoires locales SQL.
    - eibi_backup.csv : La base de données EiBi importée.

