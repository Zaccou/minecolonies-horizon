//! `WindowReactivateBuilding` : l'écran qui remet un bâtiment en service.
//!
//! 73 lignes de Java, deux boutons, un seul panneau. Ce que la fenêtre **demande** :
//! le type du bloc cliqué (`TileEntityColonyBuilding.registryName`,
//! `WindowReactivateBuilding.java:44`), pour choisir entre **deux** messages. Ce
//! qu'elle **rend** : un clic qui envoie `ReactivateBuildingMessage(pos)` puis ferme
//! (`:59-63`), et un clic qui ferme sans rien envoyer (`:69-72`).
//!
//! **Le message dépend du bâtiment, pas du joueur** : une maison et la taverne
//! parlent aux *habitants* (`:45-48`), tout le reste parle aux *travailleurs*
//! (`:49-52`). Et si le bloc n'est pas un bâtiment de colonie du tout, la classe
//! n'écrit rien : le panneau garde le texte de l'XML
//! (`windowreactivatebuilding.xml:9`), qui est un troisième message. [`Message`] a
//! ces trois branches, [`Message::Default`] comprise — c'est l'état initial.
//!
//! **Aucun refus, et aucun titre.** Le clic envoie toujours ; c'est le serveur qui
//! peut refuser, et `ReactivateBuildingMessage` n'a pas d'autre retour que la remise
//! en service. Le XML n'a pas de panneau de titre : `Title` de `src/ui` n'a pas de
//! variante pour ça, d'où `Header::None`.

use crate::building::Kind;
use crate::ui::action::Effect;
use crate::ui::field::{Field, Source, Value};

use super::{Button, Header, Leaves, Opener, Window};

/// L'id de cette table.
pub const ID: &str = "reactivate";
/// La classe (`WindowReactivateBuilding.java:22`).
pub const JAVA: &str = "WindowReactivateBuilding";
/// Le XML (`:36`).
pub const RESOURCE: &str = "minecolonies:gui/windowreactivatebuilding.xml";
/// Le panneau du message (`windowreactivatebuilding.xml:9`).
pub const TEXT: &str = "text";
/// Le bouton qui remet en service (`:12-14` ; `WindowConstants.java:1039`).
pub const REACTIVATE: &str = "reactivate";
/// Le bouton qui ferme (`:6-7` ; `WindowConstants.java:477`).
pub const CANCEL: &str = "cancel";

/// Le texte que l'XML écrit avant que la classe ne le remplace
/// (`windowreactivatebuilding.xml:9`).
pub const DEFAULT_KEY: &str = "com.minecolonies.coremod.gui.reactivate.message";
/// Le message d'une maison ou de la taverne (`:47`).
pub const LIVING_KEY: &str = "com.minecolonies.core.gui.reactivate.message.living";
/// Le message de tout autre bâtiment (`:51`).
pub const WORKING_KEY: &str = "com.minecolonies.core.gui.reactivate.message.working";
/// Le libellé du bouton (`windowreactivatebuilding.xml:13`).
pub const REACTIVATE_KEY: &str = "com.minecolonies.coremod.gui.reactivate.button";
/// Le message envoyé au clic (`:61`).
pub const MESSAGE: &str = "ReactivateBuildingMessage";

/// Le message du panneau `text`, dans l'ordre où Java le choisit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three branches of the constructor, `Default` included
pub enum Message {
    /// Rien n'a été écrit : le panneau garde le texte de l'XML. C'est l'état
    /// d'ouverture, et l'état final quand le bloc n'est pas un bâtiment de colonie
    /// (`:42-53`).
    Default,
    /// Une maison ou la taverne (`:45-48`) : le message parle aux habitants et
    /// prend la clé du bâtiment en argument.
    Living(Kind),
    /// Tout autre bâtiment (`:49-52`).
    Working(Kind),
}

/// L'état d'ouverture : la classe n'a pas encore écrit le panneau.
pub const fn initial() -> Message {
    Message::Default
}

/// Le message que le constructeur écrit pour ce type de bloc, ou [`Message::Default`]
/// quand il n'y a pas de bâtiment — c'est la transition d'ouverture.
///
/// Java teste l'identité de l'entrée de registre contre `ModBuildings.home` et
/// `ModBuildings.tavern` (`:45`) ; le port teste [`Kind`], qui est la même chose par
/// son chemin de registre (`building::Kind::name`) — `ModBuildings.HOME_ID` vaut
/// `"residence"` (`api/colony/buildings/ModBuildings.java:25`).
pub fn written(kind: Option<Kind>) -> Message {
    match kind {
        None => Message::Default,
        Some(kind @ (Kind::Residence | Kind::Tavern)) => Message::Living(kind),
        Some(other) => Message::Working(other),
    }
}

impl Message {
    /// La clé affichée.
    pub fn key(self) -> &'static str {
        match self {
            Message::Default => DEFAULT_KEY,
            Message::Living(_) => LIVING_KEY,
            Message::Working(_) => WORKING_KEY,
        }
    }

    /// Le bâtiment dont la clé est l'argument du message, quand il y en a un.
    pub fn building(self) -> Option<Kind> {
        match self {
            Message::Default => None,
            Message::Living(kind) | Message::Working(kind) => Some(kind),
        }
    }

    /// Vrai quand le message parle aux habitants plutôt qu'aux travailleurs.
    pub fn is_living(self) -> bool {
        matches!(self, Message::Living(_))
    }
}

/// Le panneau, libellé par le texte que l'XML y met.
const F_TEXT: Field = Field::labelled(TEXT, DEFAULT_KEY, Value::Text, Source::Building);
const PANES: &[Field] = &[F_TEXT];

/// Les deux boutons (`:38-39`).
const BUTTONS: &[Button] = &[
    Button::plain(REACTIVATE, REACTIVATE_KEY, Effect::Sends(MESSAGE), Leaves::Closes),
    // Une croix : pas de libellé dans l'XML, et `cancelClicked` ne fait que `close()`.
    Button::dismiss(CANCEL, "", Leaves::Closes),
];

/// La fenêtre.
pub static WINDOW: Window = Window {
    id: ID,
    java: JAVA,
    resource: RESOURCE,
    header: Header::None,
    // Le clic droit sur un bloc de colonie désactivé (`AbstractColonyBlock.java:205`).
    opener: Opener::Java("AbstractColonyBlock"),
    panes: PANES,
    buttons: BUTTONS,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_home_and_the_tavern_talk_to_their_residents_and_the_rest_to_a_worker() {
        assert_eq!(written(Some(Kind::Residence)), Message::Living(Kind::Residence));
        assert_eq!(written(Some(Kind::Tavern)), Message::Living(Kind::Tavern));
        assert!(written(Some(Kind::Residence)).is_living());
        assert!(written(Some(Kind::Tavern)).is_living());
        // Tout autre bâtiment : le message des travailleurs, même un entrepôt vide.
        for kind in [Kind::Builder, Kind::Farmer, Kind::Warehouse, Kind::TownHall] {
            assert_eq!(written(Some(kind)), Message::Working(kind));
            assert!(!written(Some(kind)).is_living());
        }
        assert_eq!(written(None), Message::Default);
        assert_eq!(initial(), Message::Default);
        // Deux bâtiments de la même branche ne donnent pas le même message : la clé
        // du bâtiment est l'argument, donc le type compte.
        assert_ne!(written(Some(Kind::Residence)), written(Some(Kind::Farmer)));
        // Les deux seuls bâtiments de la branche « habitants ».
        let living: Vec<&str> = Kind::ALL
            .iter()
            .filter(|kind| written(Some(**kind)).is_living())
            .map(|kind| kind.name())
            .collect();
        assert_eq!(living, ["residence", "tavern"], "WindowReactivateBuilding.java:45");
    }

    #[test]
    fn the_three_messages_are_three_keys_and_two_of_them_name_a_building() {
        assert_eq!(Message::Default.key(), DEFAULT_KEY);
        assert_eq!(Message::Living(Kind::Tavern).key(), LIVING_KEY);
        assert_eq!(Message::Working(Kind::Farmer).key(), WORKING_KEY);
        assert_ne!(LIVING_KEY, WORKING_KEY);
        assert_ne!(DEFAULT_KEY, LIVING_KEY);
        assert_eq!(Message::Default.building(), None);
        assert_eq!(Message::Living(Kind::Tavern).building(), Some(Kind::Tavern));
        assert_eq!(Message::Working(Kind::Farmer).building(), Some(Kind::Farmer));
        for key in [DEFAULT_KEY, LIVING_KEY, WORKING_KEY, REACTIVATE_KEY] {
            assert!(key.starts_with("com.minecolonies."), "{key}");
        }
    }

    #[test]
    fn the_pane_keeps_the_xml_text_until_the_constructor_writes_it() {
        let text = match WINDOW.pane(TEXT) {
            Some(pane) => pane,
            None => panic!("pas de panneau text"),
        };
        // Le libellé du panneau est le texte par défaut de l'XML : c'est ce qui
        // s'affiche quand la classe n'écrit rien.
        assert_eq!(text.label, Some(DEFAULT_KEY));
        assert_eq!(text.value, Value::Text);
        // Le message nomme le **bâtiment**, pas la colonie : la classe lit l'entrée
        // de registre du bloc (`WindowReactivateBuilding.java:44-51`,
        // `buildingEntry.getTranslationKey()`), et `Source::Building` est cette
        // lecture. Ce n'est donc **pas** une donnée de colonie au sens de
        // `ui::field::Source::is_colony_data`, qui ne compte que `Colony`, `Citizen`
        // et `Request` (`field.rs:140-142`) — l'assertion inverse était fausse.
        assert_eq!(text.source, Source::Building);
        assert!(!text.source.is_colony_data());
        assert_eq!(WINDOW.panes.len(), 1);
        // `written(None)` laisse l'état d'ouverture : la transition ne se fait pas.
        assert_eq!(written(None), initial());
        // Et `written(Some(..))` la fait, dans les deux branches.
        assert_ne!(written(Some(Kind::Residence)), initial());
        assert_ne!(written(Some(Kind::Builder)), initial());
    }

    #[test]
    fn both_buttons_close_the_window_and_only_one_sends_anything() {
        let reactivate = WINDOW.button(REACTIVATE).expect("reactivate");
        assert_eq!(reactivate.sends(), Some(MESSAGE));
        assert_eq!(reactivate.leaves, Leaves::Closes);
        assert_eq!(reactivate.label, REACTIVATE_KEY);
        assert!(!reactivate.is_guarded(), "le clic n'est jamais refusé par la fenêtre");
        let cancel = WINDOW.button(CANCEL).expect("cancel");
        assert_eq!(cancel.sends(), None, "cancelClicked ne fait que close()");
        assert_eq!(cancel.effect, None, "il ne demande rien du tout");
        assert_eq!(cancel.label, "", "windowreactivatebuilding.xml:6-7 : une image");
        assert_eq!(cancel.leaves, Leaves::Closes);
        assert_ne!(reactivate.effect, cancel.effect);
    }
}
