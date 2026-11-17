//! `WindowSettings` : les cinq reglages de la colonie, chacun une bascule.
//!
//! Java reference : `core/client/gui/townhall/WindowSettings.java` (70 lignes),
//! `gui/townhall/layoutsettings.xml` (32), la classe qui dessine et bascule chaque ligne
//! (`core/colony/buildings/modules/settings/BoolSetting.java`, `ISetting.java:36-57`),
//! le module qui les porte (`TownHallSettingsModuleView`, `SettingsModuleView.java:117-125`)
//! et les deux endroits qui les declarent
//! (`BuildingTownHall.java:64-83`, `BuildingModules.java:526-532`).
//!
//! **Les cinq sont des booleens, et c'est la seule chose que la fenetre sait dessiner.**
//! `WindowSettings` ne choisit pas le widget : elle passe le panneau a
//! `setting.setupHandler(...)` (`:29-38`) et `BoolSetting` y cherche un `trigger`
//! (`BoolSetting.java:92`), ecrit `ON` ou `OFF` dessus (`:105`) et bascule la valeur
//! (`:110-113`). Un sixieme reglage d'un autre type aurait fallu un autre XML ; les cinq
//! cles de cet hotel de ville sont cinq `BoolSetting` et le resteront tant que
//! `BuildingModules.java:527-531` ne change pas.
//!
//! Cinq faits que le source impose :
//!
//! 1. **Les reglages sont ceux de la colonie, pas du batiment.**
//!    `buildingView.getColony().getSettings()` (`:27`) : deux hotels de ville d'une meme
//!    colonie lisent et ecrivent le meme module, et le module s'appelle
//!    `townhall_settings` (`BuildingModules.java:526`).
//! 2. **L'ordre du code n'est pas l'ordre du XML.** La classe lie `job`, `movein`,
//!    `housing`, `entermessages`, `tape` (`:29-38`, `:53-62`) ; le XML pose `movein` en
//!    haut (`layoutsettings.xml:3`) puis `job` (`:9`). La lecture suit l'ecran, la
//!    construction suit la classe : [`Row::ALL`] est l'ordre de la classe.
//! 3. **Chaque ligne a deux infobulles.** Celle de la `view` est dans le XML
//!    (`layoutsettings.xml:3`, `:9`, `:15`, `:21`, `:27`) ; celle du bouton vient de
//!    `ISetting.setHoverPane`, qui fabrique la cle `com.minecolonies.coremod.setting.
//!    tooltip.` suivie de l'id du reglage (`ISetting.java:117`), donc d'un id qui
//!    contient un **deux-points** : `...tooltip.minecolonies:autohiring`.
//! 4. **Le libelle initial du bouton n'est pas celui que la classe ecrit.** Le XML dit
//!    `gui.townhall.off` pour trois lignes (`:5`, `:23`, `:29`) et `gui.hiring.off` pour
//!    les deux autres (`:11`, `:17`) — et cette derniere vaut « Automatic », pas
//!    « Off ». La classe ecrase les cinq des le premier `onUpdate` avec
//!    `retrieveon`/`retrieveoff` (`WindowConstants.java:1144`, `:1149`).
//! 5. **Le clic bascule d'abord, envoie ensuite.** `SettingsModuleView.trigger` verifie
//!    que le reglage est actif, appelle `setting.trigger()` (`:120-122`) **puis**
//!    envoie `TriggerSettingMessage` (`:123`). La valeur affichee change donc avant la
//!    reponse du serveur, et le `onUpdate` suivant (`WindowSettings.java:51-63`) la
//!    reecrit telle que le serveur la connait.

use crate::ui::field::{Scope, Value};

use super::pane::{Pane, Source};
use super::{Button, Click};

/// La classe Java (`WindowSettings.java:11`).
pub const JAVA: &str = "WindowSettings";

/// Le XML (`:25`).
pub const RESOURCE: &str = "minecolonies:gui/townhall/layoutsettings.xml";

/// La cle de module du module de reglages de l'hotel de ville
/// (`BuildingModules.java:526`).
pub const MODULE_KEY: &str = "townhall_settings";

/// L'id du bouton de l'onglet, qui est aussi ce que `getWindowId` rend
/// (`WindowSettings.java:66-69`, `WindowConstants.java:42`).
pub const TAB_ID: &str = "settings";

/// Le libelle de cet onglet (`windowtownhall.xml:32`).
pub const TAB_KEY: &str = "com.minecolonies.coremod.gui.townhalltab.settings";

/// Le bouton que chaque ligne porte, dans la `view` de la ligne
/// (`layoutsettings.xml:5`, `BoolSetting.java:92`).
pub const TRIGGER: &str = "trigger";

/// Le mot que la classe ecrit quand le reglage est vrai
/// (`WindowConstants.java:1144` → `TranslationConstants.java:71`).
pub const ON_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.retrieveon";

/// Le mot qu'elle ecrit quand il est faux (`WindowConstants.java:1149`).
pub const OFF_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.retrieveoff";

/// Le prefixe de l'infobulle du reglage, suivi de son id (`ISetting.java:117`).
pub const TOOLTIP_PREFIX: &str = "com.minecolonies.coremod.setting.tooltip.";

/// Le message que le clic envoie (`SettingsModuleView.java:123`).
pub const MESSAGE: &str = "TriggerSettingMessage";

/// Un des cinq reglages, dans l'ordre ou la classe les lie
/// (`WindowSettings.java:29-38`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: cinq lignes dans le XML et cinq cles dans le module
pub enum Row {
    /// Embauche automatique (`BuildingTownHall.java:73`).
    AutoHiring,
    /// Arrivee de nouveaux citoyens (`:64`).
    MoveIn,
    /// Logement automatique (`:78`).
    AutoHousing,
    /// Messages d'entree et de sortie (`:68`).
    EnterLeave,
    /// Ruban de construction (`:83`).
    Tape,
}

impl Row {
    /// Les cinq, dans l'ordre de `WindowSettings.java:29-38`.
    pub const ALL: [Row; 5] = [
        Row::AutoHiring,
        Row::MoveIn,
        Row::AutoHousing,
        Row::EnterLeave,
        Row::Tape,
    ];

    /// L'id de la `view` de la ligne, celui que la classe passe a `findPaneByID`
    /// (`WindowSettings.java:30`, `:32`, `:34`, `:36`, `:38`).
    pub const fn view(self) -> &'static str {
        match self {
            Row::AutoHiring => "job",
            Row::MoveIn => "movein",
            Row::AutoHousing => "housing",
            Row::EnterLeave => "entermessages",
            Row::Tape => "tape",
        }
    }

    /// L'id du reglage dans le module — la `ResourceLocation` que `SettingKey` porte
    /// (`BuildingTownHall.java:64`, `:68`, `:73`, `:78`, `:83`).
    pub const fn unique_id(self) -> &'static str {
        match self {
            Row::AutoHiring => "minecolonies:autohiring",
            Row::MoveIn => "minecolonies:kidspawn",
            Row::AutoHousing => "minecolonies:autohousing",
            Row::EnterLeave => "minecolonies:enterleave",
            Row::Tape => "minecolonies:tape",
        }
    }

    /// Le libelle de la ligne, ecrit dans un `text` **fils de la view**
    /// (`layoutsettings.xml:4`, `:10`, `:16`, `:22`, `:28`).
    pub const fn label_key(self) -> &'static str {
        match self {
            Row::AutoHiring => "com.minecolonies.coremod.gui.townhall.jobhiring",
            Row::MoveIn => "com.minecolonies.coremod.gui.townhall.movein",
            Row::AutoHousing => "com.minecolonies.coremod.gui.townhall.housing",
            Row::EnterLeave => "com.minecolonies.coremod.gui.townhall.entermsgs",
            Row::Tape => "com.minecolonies.coremod.gui.townhall.tape",
        }
    }

    /// L'infobulle de la `view`, dans le XML (`:3`, `:9`, `:15`, `:21`, `:27`).
    pub const fn row_tooltip_key(self) -> &'static str {
        match self {
            Row::AutoHiring => "com.minecolonies.coremod.gui.townhall.jobhiring.tooltip",
            Row::MoveIn => "com.minecolonies.coremod.gui.townhall.movein.tooltip",
            Row::AutoHousing => "com.minecolonies.coremod.gui.townhall.housing.tooltip",
            Row::EnterLeave => "com.minecolonies.coremod.gui.townhall.entermsgs.tooltip",
            Row::Tape => "com.minecolonies.coremod.gui.townhall.tape.tooltip",
        }
    }

    /// Le libelle que le XML pose **avant** le premier `onUpdate` — deux cles pour cinq
    /// lignes, et l'une des deux vaut « Automatic » (`:11`, `:17`).
    pub const fn xml_label_key(self) -> &'static str {
        match self {
            Row::AutoHiring | Row::AutoHousing => "com.minecolonies.coremod.gui.hiring.off",
            Row::MoveIn | Row::EnterLeave | Row::Tape => {
                "com.minecolonies.coremod.gui.townhall.off"
            }
        }
    }

    /// Le bouton de la ligne et la ligne du XML qui le declare.
    pub const fn declared(self) -> (&'static str, u32) {
        match self {
            Row::AutoHiring => ("layoutsettings.xml", 11),
            Row::MoveIn => ("layoutsettings.xml", 5),
            Row::AutoHousing => ("layoutsettings.xml", 17),
            Row::EnterLeave => ("layoutsettings.xml", 23),
            Row::Tape => ("layoutsettings.xml", 29),
        }
    }

    /// Le panneau de la ligne : le texte du bouton, qui porte `ON` ou `OFF`
    /// (`BoolSetting.java:105`).
    pub const fn pane(self) -> Pane {
        Pane::inside(self.view(), TRIGGER, Value::Text, Source::Settings, self.declared())
    }

    /// L'infobulle du bouton, celle du reglage et pas celle de la ligne
    /// (`ISetting.java:117`).
    pub fn tooltip_key(self) -> String {
        format!("{TOOLTIP_PREFIX}{}", self.unique_id())
    }

    /// La valeur par defaut : vraie pour les cinq (`BuildingModules.java:527-531`).
    pub const fn default_on(self) -> bool {
        true
    }
}

/// Le texte du bouton pour une valeur (`BoolSetting.java:105`).
pub const fn text(on: bool) -> &'static str {
    if on {
        ON_KEY
    } else {
        OFF_KEY
    }
}

/// Ce qu'un clic sur une bascule rend : la nouvelle valeur et le message envoye.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trigger {
    /// La valeur **apres** la bascule.
    pub on: bool,
    /// Le message que le clic envoie (`SettingsModuleView.java:123`).
    pub message: &'static str,
}

/// Un clic sur une bascule, tel que `SettingsModuleView.trigger` le traite
/// (`:117-125`).
///
/// L'ordre est celui de Java et il compte : un reglage inactif **ne bascule pas** et
/// n'envoie rien. Pour un `BoolSetting`, `isActive` rend toujours vrai
/// (`ISetting.java:164-167`) — la garde est donc sans effet sur ces cinq lignes, et le
/// refus n'est atteignable qu'en passant `false`, ce que fait un reglage compose.
pub fn trigger(active: bool, on: bool) -> Result<Trigger, super::Refusal> {
    if !active {
        return Err(super::Refusal::SettingInactive);
    }
    Ok(Trigger { on: !on, message: MESSAGE })
}

/// L'etat des cinq bascules, tel que la fenetre l'affiche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Board {
    values: [bool; 5],
}

impl Default for Board {
    fn default() -> Board {
        Board { values: [true; 5] }
    }
}

impl Board {
    /// Les cinq valeurs par defaut : vraies (`BuildingModules.java:527-531`).
    pub fn new() -> Board {
        Board::default()
    }

    /// Le module **tel que la fenetre le trouve a l'ouverture**
    /// (`WindowSettings.java:29-38`).
    ///
    /// Java ne verifie pas le `getSetting` de `@Nullable` : une cle absente du module
    /// fait lever la construction de la fenetre. Le port nomme le refus.
    pub fn open(keys: &[&str]) -> Result<Board, super::Refusal> {
        for row in Row::ALL {
            if !keys.contains(&row.unique_id()) {
                return Err(super::Refusal::SettingsModuleMissing);
            }
        }
        Ok(Board::new())
    }

    /// La valeur d'une ligne.
    pub fn on(&self, row: Row) -> bool {
        self.values[row as usize]
    }

    /// Le texte du bouton de cette ligne, tel que `render` l'ecrit
    /// (`BoolSetting.java:103-105`).
    pub fn text(&self, row: Row) -> &'static str {
        text(self.on(row))
    }

    /// Vrai quand le bouton de cette ligne est actif — toujours, pour un booleen
    /// (`BoolSetting.java:104`).
    pub fn enabled(&self, _row: Row) -> bool {
        true
    }

    /// Un clic : bascule, puis rend la valeur et le message (`:110-113`, `:117-125`).
    pub fn click(&mut self, row: Row) -> Result<Trigger, super::Refusal> {
        let triggered = trigger(true, self.on(row))?;
        self.values[row as usize] = triggered.on;
        Ok(triggered)
    }

    /// Les cinq valeurs, dans l'ordre de [`Row::ALL`] — ce que `onUpdate` reecrit
    /// (`WindowSettings.java:51-63`).
    pub fn render(&self) -> [(&'static str, &'static str); 5] {
        let mut out = [("", ""); 5];
        for (index, row) in Row::ALL.iter().enumerate() {
            out[index] = (row.view(), text(self.values[index]));
        }
        out
    }
}

/// Le bouton d'une ligne : il vit dans la `view` de la ligne et envoie le message.
const fn button(row: Row) -> Button {
    Button {
        id: TRIGGER,
        label: "",
        click: Click::Sends { message: MESSAGE, closes: false },
        scope: Scope::Row(row.view()),
        refusals: &[],
        declared: row.declared(),
    }
}

/// Les cinq boutons, dans l'ordre de [`Row::ALL`].
pub static BUTTONS: [Button; 5] = [
    button(Row::AutoHiring),
    button(Row::MoveIn),
    button(Row::AutoHousing),
    button(Row::EnterLeave),
    button(Row::Tape),
];

/// Les cinq panneaux, dans l'ordre de [`Row::ALL`] : le texte de chaque bouton.
pub static PANES: [Pane; 5] = [
    Row::AutoHiring.pane(),
    Row::MoveIn.pane(),
    Row::AutoHousing.pane(),
    Row::EnterLeave.pane(),
    Row::Tape.pane(),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_rows_five_ids_and_the_class_order_is_not_the_xml_order() {
        assert_eq!(Row::ALL.len(), 5);
        let ids: Vec<&str> = Row::ALL.iter().map(|row| row.unique_id()).collect();
        assert_eq!(
            ids,
            [
                "minecolonies:autohiring",
                "minecolonies:kidspawn",
                "minecolonies:autohousing",
                "minecolonies:enterleave",
                "minecolonies:tape",
            ],
            "BuildingTownHall.java:64-83"
        );
        let views: Vec<&str> = Row::ALL.iter().map(|row| row.view()).collect();
        assert_eq!(views, ["job", "movein", "housing", "entermessages", "tape"]);
        // L'ordre du XML est movein, job, housing, entermessages, tape.
        let xml_top: u32 = Row::MoveIn.declared().1;
        assert!(xml_top < Row::AutoHiring.declared().1, "layoutsettings.xml:3 puis :9");
        assert!(Row::ALL[0] == Row::AutoHiring, "la classe lie job en premier");
    }

    #[test]
    fn the_five_label_keys_and_the_five_row_tooltips_are_ten_keys() {
        let labels: Vec<&str> = Row::ALL.iter().map(|row| row.label_key()).collect();
        assert_eq!(
            labels,
            [
                "com.minecolonies.coremod.gui.townhall.jobhiring",
                "com.minecolonies.coremod.gui.townhall.movein",
                "com.minecolonies.coremod.gui.townhall.housing",
                "com.minecolonies.coremod.gui.townhall.entermsgs",
                "com.minecolonies.coremod.gui.townhall.tape",
            ]
        );
        for (index, row) in Row::ALL.iter().enumerate() {
            assert!(row.row_tooltip_key().ends_with(".tooltip"), "{}", row.view());
            assert_ne!(row.row_tooltip_key(), row.label_key());
            for other in &Row::ALL[index + 1..] {
                assert_ne!(row.label_key(), other.label_key());
                assert_ne!(row.row_tooltip_key(), other.row_tooltip_key());
            }
        }
    }

    #[test]
    fn the_setting_tooltip_key_carries_a_colon_inside_a_lang_key() {
        // ISetting.java:117 concatene le prefixe et la ResourceLocation entiere, donc
        // `minecolonies:autohiring` avec son namespace.
        assert_eq!(
            Row::AutoHiring.tooltip_key(),
            "com.minecolonies.coremod.setting.tooltip.minecolonies:autohiring"
        );
        assert_eq!(
            Row::MoveIn.tooltip_key(),
            "com.minecolonies.coremod.setting.tooltip.minecolonies:kidspawn"
        );
        for row in Row::ALL {
            let key = row.tooltip_key();
            assert!(key.starts_with(TOOLTIP_PREFIX));
            assert!(key.contains(':'), "un id de reglage est un ResourceLocation");
            assert_ne!(key, row.row_tooltip_key(), "deux infobulles, deux cles");
        }
    }

    #[test]
    fn the_five_defaults_are_on_and_a_missing_key_stops_the_window() {
        let all: Vec<&str> = Row::ALL.iter().map(|row| row.unique_id()).collect();
        let board = Board::open(&all).expect("le module porte ses cinq cles");
        for row in Row::ALL {
            assert!(board.on(row), "BuildingModules.java:527-531");
            assert_eq!(board.text(row), ON_KEY);
            assert!(board.enabled(row), "BoolSetting.java:104 : toujours actif");
        }
        // Une cle absente fait lever Java a la construction (WindowSettings.java:29-38).
        let short = &all[..4];
        assert_eq!(Board::open(short), Err(super::super::Refusal::SettingsModuleMissing));
        assert_eq!(Board::open(&[]), Err(super::super::Refusal::SettingsModuleMissing));
        // Une cle en trop ne gene pas : le module en porte d'autres.
        let mut extra = all.clone();
        extra.push("minecolonies:tavernsettings");
        assert!(Board::open(&extra).is_ok());
    }

    #[test]
    fn a_click_flips_first_and_names_the_message_it_sends() {
        let mut board = Board::new();
        let clicked = board.click(Row::Tape).expect("un booleen est actif");
        assert!(!clicked.on);
        assert_eq!(clicked.message, "TriggerSettingMessage");
        assert_eq!(board.text(Row::Tape), OFF_KEY);
        assert_eq!(board.text(Row::MoveIn), ON_KEY, "une ligne ne touche pas les autres");
        // Deux clics ramenent la valeur de depart (BoolSetting.java:110-113).
        let again = board.click(Row::Tape).expect("toujours actif");
        assert!(again.on);
        assert_eq!(board.text(Row::Tape), ON_KEY);
        // Un reglage inactif ne bascule pas et n'envoie rien (SettingsModuleView.java:120).
        assert_eq!(trigger(false, true), Err(super::super::Refusal::SettingInactive));
        assert_eq!(super::super::Refusal::SettingInactive.key(), None, "Java se tait");
    }

    #[test]
    fn render_rewrites_the_five_views_in_the_class_order() {
        let mut board = Board::new();
        board.click(Row::AutoHousing).expect("actif");
        let rendered = board.render();
        assert_eq!(rendered.len(), 5);
        assert_eq!(rendered[0], ("job", ON_KEY));
        assert_eq!(rendered[2], ("housing", OFF_KEY));
        for (index, (view, label)) in rendered.iter().enumerate() {
            assert_eq!(*view, Row::ALL[index].view());
            assert!(label == &ON_KEY || label == &OFF_KEY);
        }
        // Le libelle du XML n'est pas celui de la classe : deux cles pour cinq lignes.
        let xml_keys: Vec<&str> = Row::ALL.iter().map(|row| row.xml_label_key()).collect();
        assert_eq!(xml_keys[0], "com.minecolonies.coremod.gui.hiring.off");
        assert_eq!(xml_keys[1], "com.minecolonies.coremod.gui.townhall.off");
        assert_ne!(xml_keys[0], xml_keys[1]);
        assert_ne!(xml_keys[0], ON_KEY);
        assert_ne!(xml_keys[1], OFF_KEY);
    }

    #[test]
    fn five_buttons_and_five_panes_in_the_row_scope() {
        assert_eq!(BUTTONS.len(), 5);
        assert_eq!(PANES.len(), 5);
        for (index, row) in Row::ALL.iter().enumerate() {
            assert_eq!(BUTTONS[index].id, TRIGGER);
            assert_eq!(BUTTONS[index].scope, Scope::Row(row.view()));
            assert_eq!(
                BUTTONS[index].click,
                Click::Sends { message: "TriggerSettingMessage", closes: false },
                "le clic qui bascule ne ferme pas la fenetre"
            );
            assert!(!BUTTONS[index].closes());
            assert!(BUTTONS[index].refusals.is_empty(), "BoolSetting.java:92 ne garde rien");
            assert_eq!(BUTTONS[index].declared, row.declared());
            assert_eq!(PANES[index].id, TRIGGER);
            assert_eq!(PANES[index].scope, Scope::Row(row.view()));
            assert_eq!(PANES[index].value, Value::Text);
            assert_eq!(PANES[index].source, Source::Settings);
            assert!(PANES[index].is_nested());
            assert!(BUTTONS[index].declared.0.ends_with("layoutsettings.xml"));
        }
        // Deux lignes nomment leur texte `desc`, aucun bouton ne s'appelle `desc`.
        assert!(!BUTTONS.iter().any(|button| button.id == "desc"));
    }
}
