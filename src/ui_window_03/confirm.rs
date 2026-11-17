//! `WindowConfirm` : deux textes, deux boutons, et **les deux ids à l'envers**.
//!
//! Java reference: `core/client/gui/WindowConfirm.java` (29 lignes),
//! `assets/minecolonies/gui/windowconfirm.xml:1-11`,
//! `AbstractWindowSkeleton.java:119-133` (l'enregistrement), `:254-261` (`close()`).
//!
//! **La classe ne dessine rien et ne lit rien.** Elle reçoit quatre choses de son
//! appelant — un parent, une `Runnable`, un titre, un avertissement — écrit les deux
//! textes, et lie les deux boutons (`:21-28`). Tout ce qu'un port peut en dire tient donc
//! dans [`State`] : deux clés de traduction et un booléen.
//!
//! # Le piège, et il est dans l'XML
//!
//! | id XML | libellé XML | handler Java | effet |
//! | --- | --- | --- | --- |
//! | `cancel` | `$(gui.yes)` | `action` (`:25`) | exécute l'action de l'appelant |
//! | `confirm` | `$(gui.cancel)` | `close` (`:24`) | ferme et rouvre le parent |
//!
//! Les deux ids disent l'inverse de leur libellé. Un port qui lierait par **id** ferait
//! de `confirm` le bouton « oui » et **inverserait la boîte** : c'est le libellé qui dit
//! ce que le bouton fait, et [`Button::label`] est l'autorité. (Le fait est déjà noté par
//! `src/ui_window_02/MANQUANT.md` §4 ; il est ici en code plutôt qu'en note.)
//!
//! # Ce que le bouton « oui » ne fait pas
//!
//! `close()` de la base rouvre le parent (`AbstractWindowSkeleton.java:254-261`), mais le
//! handler du « oui » **n'appelle pas `close()`** : il exécute la `Runnable` de l'appelant
//! (`WindowConfirm.java:25`) et rien d'autre. Sur les deux appelants de cet arbre, l'un
//! ferme lui-même (`ConnectionModuleWindow.java:113-121`, `close()` sur la fenêtre
//! englobante), l'autre **pas du tout** (`WindowBuildBuilding.java:216-229`,
//! `triggerConfirmAction` envoie deux messages et s'arrête) : la boîte reste donc ouverte
//! au-dessus de la fenêtre de construction. [`Turn::RunsAction`] dit exactement cela —
//! c'est l'action qui décide, pas la boîte.

use crate::ui::field::{Field, Source, Value};

use super::refusal::Refusal;

/// La classe Java qui construit la boîte.
pub const JAVA: &str = "WindowConfirm";
/// Le XML qu'elle charge (`WindowConfirm.java:23`).
pub const RESOURCE: &str = "minecolonies:gui/windowconfirm.xml";

/// Le panneau du titre (`WindowConstants.java:1169`, écrit à `WindowConfirm.java:26`).
pub const TITLE_LABEL: &str = "title";
/// Le panneau de l'avertissement (`WindowConstants.java:1174`, écrit à `:27`).
pub const WARNING_LABEL: &str = "warning";
/// L'id du bouton qui **exécute l'action** (`WindowConstants.java:477`, `:25`).
pub const YES_BUTTON: &str = "cancel";
/// L'id du bouton qui **ferme** (`WindowConstants.java:482`, `:24`).
pub const NO_BUTTON: &str = "confirm";
/// Le libellé du bouton « oui » (`windowconfirm.xml:7`).
pub const YES_LABEL: &str = "gui.yes";
/// Le libellé du bouton « non » (`windowconfirm.xml:10`).
pub const NO_LABEL: &str = "gui.cancel";

/// Les deux panneaux, tels que `src/ui` les décrirait : `Value::Text` et une clé.
///
/// `Source::Building` nomme le cas réel de `WindowBuildBuilding.java:209`
/// (`getHoverWarningForLevel()`, l'avertissement d'un niveau) ; l'autre appelant passe une
/// constante déjà traduite chez lui (`ConnectionModuleWindow.java:120`). La classe, elle,
/// ne reçoit que deux `String` : une origine « argument » manque à `src/ui::Source`, et
/// `MANQUANT.md` §3 la réclame au lieu de la fabriquer.
pub const FIELDS: &[Field] = &[
    Field::new(TITLE_LABEL, Value::Text, Source::Building),
    Field::new(WARNING_LABEL, Value::Text, Source::Building),
];

/// Les deux boutons, nommés par ce qu'ils **font** et non par leur id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    /// Le bouton au libellé `gui.yes`, d'id [`YES_BUTTON`].
    Yes,
    /// Le bouton au libellé `gui.cancel`, d'id [`NO_BUTTON`].
    No,
}

impl Button {
    /// Les deux boutons, dans l'ordre de l'XML.
    pub const ALL: [Button; 2] = [Button::Yes, Button::No];

    /// L'id de l'XML — celui que `registerButton` prend (`:24-25`).
    pub const fn id(self) -> &'static str {
        match self {
            Button::Yes => YES_BUTTON,
            Button::No => NO_BUTTON,
        }
    }

    /// Le libellé, et **c'est lui qui fait foi** : l'id de `Yes` est `cancel`.
    pub const fn label(self) -> &'static str {
        match self {
            Button::Yes => YES_LABEL,
            Button::No => NO_LABEL,
        }
    }

    /// Vrai pour le seul bouton qui exécute l'action de l'appelant (`:25`).
    pub const fn runs_action(self) -> bool {
        matches!(self, Button::Yes)
    }
}

/// Ce qu'un clic laisse derrière lui.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    /// Le clic a exécuté l'action de l'appelant : **la boîte ne se referme pas d'elle-même**
    /// (`:25`, et `WindowBuildBuilding.java:216-229` ne ferme rien).
    RunsAction,
    /// Le clic a fermé la boîte et rouvert le parent (`:24` → `AbstractWindowSkeleton.java:254-261`).
    Closes,
}

/// Ce que la boîte porte à un instant donné.
///
/// Les deux textes sont des `String`, pas des `&'static str` : le constructeur Java prend
/// deux `String` (`:21`), et les deux appelants n'y mettent pas la même chose — une **clé**
/// (`WindowBuildBuilding.java:209`) ou une phrase **déjà traduite**
/// (`ConnectionModuleWindow.java:120`, `.getString()`). Un type qui n'accepterait que des
/// clés refuserait le second appelant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    /// Ce que `Component.translatable(title)` traduit (`:26`).
    pub title: String,
    /// L'avertissement (`:27`). `""` est le cas de `ConnectionModuleWindow.java:120`, qui
    /// ne veut pas d'avertissement : `windowconfirm.xml:3` reste alors vide.
    pub warning: String,
    /// Vrai tant que la boîte est à l'écran. Elle naît ouverte (`open()` chez l'appelant).
    pub open: bool,
    /// Vrai quand le bouton « oui » a été cliqué une fois. Java n'a pas ce champ :
    /// il n'en a pas besoin, mais un appelant qui veut éviter deux envois le demande.
    pub answered: bool,
}

impl State {
    /// La boîte telle que l'appelant la construit (`:21`).
    pub fn new(title: impl Into<String>, warning: impl Into<String>) -> State {
        State { title: title.into(), warning: warning.into(), open: true, answered: false }
    }

    /// Le bouton de cet id, ou [`Refusal::UnknownButton`].
    pub fn button(&self, id: &str) -> Result<Button, Refusal> {
        if id == YES_BUTTON {
            Ok(Button::Yes)
        } else if id == NO_BUTTON {
            Ok(Button::No)
        } else {
            Err(Refusal::UnknownButton)
        }
    }

    /// Un clic, tel que `registerButton` le reçoit (`:24-25`).
    ///
    /// Le seul refus possible est un id que l'XML ne porte pas : la boîte ne teste aucune
    /// condition avant d'agir.
    pub fn press(&mut self, id: &str) -> Result<Turn, Refusal> {
        match self.button(id)? {
            Button::Yes => {
                self.answered = true;
                Ok(Turn::RunsAction)
            }
            Button::No => {
                self.open = false;
                Ok(Turn::Closes)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> State {
        State::new("com.minecolonies.core.gui.build.confirm.title", "warning")
    }

    #[test]
    fn the_yes_button_has_the_cancel_id_and_the_label_says_which_is_which() {
        // windowconfirm.xml:5-10 + WindowConfirm.java:24-25 : les deux ids sont croisés.
        assert_eq!(Button::Yes.id(), "cancel");
        assert_eq!(Button::Yes.label(), "gui.yes");
        assert_eq!(Button::No.id(), "confirm");
        assert_eq!(Button::No.label(), "gui.cancel");
        // Donc l'id seul ne dit pas le sens : c'est `label()` qui le dit.
        assert!(Button::Yes.runs_action());
        assert!(!Button::No.runs_action());
        assert_eq!(state().button("cancel").map(Button::runs_action), Ok(true));
        assert_eq!(state().button("confirm").map(Button::runs_action), Ok(false));
    }

    #[test]
    fn yes_runs_the_action_and_leaves_the_box_open() {
        let mut box_state = state();
        assert_eq!(box_state.press(YES_BUTTON), Ok(Turn::RunsAction));
        assert!(box_state.answered);
        // :25 n'appelle pas close() : la boîte est toujours là.
        assert!(box_state.open);
        // Et un second « oui » exécuterait l'action une seconde fois, comme Java.
        assert_eq!(box_state.press(YES_BUTTON), Ok(Turn::RunsAction));
    }

    #[test]
    fn no_closes_without_running_anything() {
        let mut box_state = state();
        assert_eq!(box_state.press(NO_BUTTON), Ok(Turn::Closes));
        assert!(!box_state.open);
        assert!(!box_state.answered, ":24 ne lie que close()");
    }

    #[test]
    fn an_unknown_id_is_refused_and_changes_nothing() {
        let mut box_state = state();
        assert_eq!(box_state.press("ok"), Err(Refusal::UnknownButton));
        assert!(box_state.open);
        assert!(!box_state.answered);
        // Les panneaux sont les deux clés, dans l'ordre de l'XML.
        assert_eq!(FIELDS.len(), 2);
        assert_eq!(FIELDS[0].id, TITLE_LABEL);
        assert_eq!(FIELDS[0].value, Value::Text);
        assert_eq!(FIELDS[1].id, WARNING_LABEL);
        assert_eq!(FIELDS[1].value, Value::Text);
    }

    #[test]
    fn the_title_is_a_key_and_not_a_sentence() {
        // :26-27 passent les deux chaînes par `Component.translatable` : ce sont des
        // clés, donc le port les garde telles quelles.
        let box_state = state();
        assert_eq!(box_state.title, "com.minecolonies.core.gui.build.confirm.title");
        assert!(box_state.title.starts_with("com.minecolonies."));
        // L'autre appelant passe une phrase déjà traduite et un avertissement vide
        // (ConnectionModuleWindow.java:120) : la boîte ne s'en aperçoit pas.
        let teleport = State::new("com.minecolonies.coremod.gui.townhall.tp", "");
        assert!(teleport.warning.is_empty());
    }
}
