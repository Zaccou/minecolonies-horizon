//! `WindowTownHallDeleteAbandonColony` : l'avertissement, puis une boîte de confirmation
//! qui n'en montre qu'une moitié à la fois.
//!
//! Java référence : `core/client/gui/townhall/WindowTownHallDeleteAbandonColony.java` (121
//! lignes) et `gui/townhall/windowdeleteabandoncolony.xml` (40), plus les deux messages
//! serveur (`ColonyDeleteOwnMessage.java`, `ColonyAbandonOwnMessage.java`) et le message
//! qui ouvre l'écran (`OpenDeleteAbandonColonyMessage.java:42-55`).
//!
//! **L'avertissement dépend de la configuration, pas du joueur.** Si
//! `allowInfiniteColonies` est vrai, la classe **montre** le bouton `abandon` et écrit
//! `ABANDON_WARNING` (`:56-60`) ; sinon elle le **cache** et écrit `DELETE_WARNING`
//! (`:61-65`). Le message d'avertissement et la présence du bouton sont donc la même
//! décision, [`Warning`] — et les deux textes reçoivent la position de l'ancienne colonie et
//! son nom en `DARK_RED` (`:59`, `:64`).
//!
//! **La boîte de confirmation est le seul état de ces quatre écrans.** L'XML naît avec
//! `visible="false"` sur `dialog`, `confirmtext`, `confirmdelete`, `confirmabandon` et
//! `cancelaction` (`:25-39`) ; `delete` et `abandon` montrent la boîte et **choisissent leur
//! moitié** (`:82-102`), `cancelaction` la cache (`:104-111`). Aucun des deux ne teste la
//! configuration : un `abandon` cliqué alors que le bouton est caché fonctionnerait quand
//! même, et c'est un fait du source, pas une décision du port.
//!
//! **Les deux confirmations envoient deux messages, dans cet ordre** : la destruction
//! (`ColonyDeleteOwn` / `ColonyAbandonOwn`) puis une relecture de l'hôtel de ville
//! (`GetColonyInfo`) — c'est ce qui rafraîchit l'écran qui a ouvert celui-ci
//! (`:70-71`, `:77-78`). Puis la fenêtre se ferme.
//!
//! **Un seul refus possible, et il vient du serveur** : le joueur ne possède aucune colonie,
//! et le serveur répond `MESSAGE_INFO_COLONY_NOT_FOUND` (`ColonyDeleteOwnMessage.java:57`).
//! Côté fenêtre, rien ne refuse jamais : les trois boutons de la rangée sont dessinés avec
//! une source `disabled` (`windowdeleteabandoncolony.xml:11`, `:16`, `:21`) et **aucune
//! ligne ne les désactive**.

use crate::network::founding::{
    ColonyAbandonOwn, ColonyDeleteOwn, GetColonyInfo, PickUpTownhall,
};
use crate::ui::field::Value;

use super::{Button, Header, Leaves, Opener, Pane, Refusal, Window, DELETE_ABANDON_ID};

/// La classe (`WindowTownHallDeleteAbandonColony.java:26`).
pub const JAVA: &str = "WindowTownHallDeleteAbandonColony";
/// Le XML (`:43`).
pub const RESOURCE: &str = "minecolonies:gui/townhall/windowdeleteabandoncolony.xml";
/// L'avertissement (`windowdeleteabandoncolony.xml:7`, écrit `:59` ou `:64`).
pub const WARNING: &str = "warningtext";
/// Le texte de la boîte (`:27`, écrit `:87` ou `:98`).
pub const CONFIRM: &str = "confirmtext";
/// L'image de la boîte (`:25`) : elle ne porte qu'une visibilité, donc aucun [`Value`].
pub const DIALOG: &str = "dialog";
/// La croix (`:5`, liée `:47`).
pub const CANCEL: &str = "cancel";
/// Prendre l'hôtel de ville en main (`:9-12`, lié `:48`).
pub const PICKUP: &str = "pickup";
/// Détruire (`:14-17`, lié `:49`).
pub const DELETE: &str = "delete";
/// Abandonner (`:19-22`, lié `:50`).
pub const ABANDON: &str = "abandon";
/// La moitié « détruire » de la boîte (`:29-31`, liée `:53`).
pub const CONFIRM_DELETE: &str = "confirmdelete";
/// La moitié « abandonner » (`:33-35`, liée `:54`).
pub const CONFIRM_ABANDON: &str = "confirmabandon";
/// Fermer la boîte sans rien faire (`:37-39`, liée `:51`) — un littéral, pas une constante
/// de `WindowConstants`.
pub const CANCEL_ACTION: &str = "cancelaction";

/// Le texte de confirmation de la destruction (`:31`).
pub const DELETE_PROCEED_KEY: &str = "com.minecolonies.core.gui.colony.delete.proceed";
/// Le texte de confirmation de l'abandon (`:32`).
pub const ABANDON_PROCEED_KEY: &str = "com.minecolonies.core.gui.colony.abandon.proceed";
/// L'avertissement quand il n'y a pas de colonie infinie (`:33`).
pub const DELETE_WARNING_KEY: &str = "com.minecolonies.core.gui.colony.delete.warning";
/// L'avertissement quand il y en a (`:34`).
pub const ABANDON_WARNING_KEY: &str = "com.minecolonies.core.gui.colony.abandon.warning";
/// Le libellé de `pickup` (`windowdeleteabandoncolony.xml:12`).
pub const PICKUP_KEY: &str = "com.minecolonies.core.founding.pickup";
/// Le libellé de `delete` (`:17`).
pub const DELETE_KEY: &str = "com.minecolonies.core.founding.delete";
/// Le libellé de `abandon` (`:22`).
pub const ABANDON_KEY: &str = "com.minecolonies.core.founding.abandon";
/// Le libellé de `confirmdelete` (`:31`).
pub const CONFIRM_DELETE_KEY: &str = "com.minecolonies.core.gui.colony.delete.confirm";
/// Le libellé de `confirmabandon` (`:35`).
pub const CONFIRM_ABANDON_KEY: &str = "com.minecolonies.core.gui.colony.abandon.confirm";
/// Le libellé de `cancelaction` (`:39`).
pub const CANCEL_ACTION_KEY: &str = "com.minecolonies.core.gui.colony.manage.cancel";
/// Le message qui ouvre cet écran (`OpenDeleteAbandonColonyMessage.java:54`).
pub const OPENER_CLASS: &str = "OpenDeleteAbandonColonyMessage";

const XML: &str = "minecolonies/gui/townhall/windowdeleteabandoncolony.xml";
const JAVA_FILE: &str = "WindowTownHallDeleteAbandonColony.java";

/// L'avertissement affiché, et ce qu'il décide.
///
/// Les deux branches de `:56-65` : la clé écrite dans `warningtext`, et le sort du bouton
/// `abandon`. Les deux vont ensemble — c'est pourquoi c'est **une** structure et non deux
/// valeurs à garder d'accord.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Warning {
    /// La clé que la classe écrit dans [`WARNING`].
    pub key: &'static str,
    /// Vrai quand le bouton `abandon` est montré (`:58`) ; faux quand il est caché (`:63`).
    pub abandon_shown: bool,
}

/// L'avertissement pour cette configuration (`WindowTownHallDeleteAbandonColony.java:56-65`).
///
/// `allow_infinite_colonies` est `MineColonies.getConfig().getServer().allowInfiniteColonies`
/// lu au client : le port n'a pas de lecteur de configuration, donc l'appelant le fournit.
pub fn warning(allow_infinite_colonies: bool) -> Warning {
    if allow_infinite_colonies {
        Warning { key: ABANDON_WARNING_KEY, abandon_shown: true }
    } else {
        Warning { key: DELETE_WARNING_KEY, abandon_shown: false }
    }
}

/// L'état de la boîte de confirmation — le seul état de ces quatre écrans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive : les trois états que les cinq panneaux de la boîte savent montrer
pub enum Dialog {
    /// Rien n'est montré : l'état d'ouverture, l'XML a `visible="false"` partout.
    Hidden,
    /// La moitié « détruire » est montrée (`:93-102`).
    Delete,
    /// La moitié « abandonner » (`:82-91`).
    Abandon,
}

impl Dialog {
    /// Les trois états, dans l'ordre où le source les produit.
    pub const ALL: [Dialog; 3] = [Dialog::Hidden, Dialog::Delete, Dialog::Abandon];

    /// La ligne de confirmation écrite, quand il y en a une.
    pub fn confirm_key(self) -> Option<&'static str> {
        match self {
            Dialog::Hidden => None,
            Dialog::Delete => Some(DELETE_PROCEED_KEY),
            Dialog::Abandon => Some(ABANDON_PROCEED_KEY),
        }
    }

    /// Les ids que cet état **rend visibles**, boîte comprise (`:84-90`, `:95-101`,
    /// `:106-110`).
    ///
    /// C'est la seule façon de décrire `dialog` : une image qui ne porte aucune valeur n'est
    /// pas un [`Pane`], et son id n'a donc nulle part ailleurs où vivre.
    pub fn shown(self) -> &'static [&'static str] {
        match self {
            Dialog::Hidden => &[],
            Dialog::Delete => &[DIALOG, CONFIRM, CONFIRM_DELETE, CANCEL_ACTION],
            Dialog::Abandon => &[DIALOG, CONFIRM, CONFIRM_ABANDON, CANCEL_ACTION],
        }
    }

    /// Vrai quand la boîte est ouverte.
    pub fn is_open(self) -> bool {
        self != Dialog::Hidden
    }
}

/// Ce qu'un clic laisse à l'appelant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Out {
    /// Un clic qui ne fait que bouger la boîte (`delete`, `abandon`, `cancelaction`).
    Stay(Dialog),
    /// Un clic qui ferme la fenêtre sans envoyer (`cancel`, `:47`).
    Close,
    /// `pickup` : reprendre l'hôtel de ville (`:48`, `:118`) et fermer.
    PickUp(PickUpTownhall),
    /// `confirmdelete` : détruire, puis relire l'hôtel de ville (`:75-80`).
    Delete {
        /// `ColonyDeleteOwnMessage.java:52-53`.
        colony: ColonyDeleteOwn,
        /// `GetColonyInfoMessage`, envoyé juste après (`WindowTownHallDeleteAbandonColony.java:78`).
        refresh: GetColonyInfo,
    },
    /// `confirmabandon` : abandonner, puis relire (`:68-73`).
    Abandon {
        /// `ColonyAbandonOwnMessage.java:51-53`.
        colony: ColonyAbandonOwn,
        /// Le même rafraîchissement (`:71`).
        refresh: GetColonyInfo,
    },
}

impl Out {
    /// Les types de message construits, **dans l'ordre d'envoi** ; vide pour un clic local.
    pub fn sends(&self) -> &'static [&'static str] {
        match self {
            Out::Stay(_) | Out::Close => &[],
            Out::PickUp(_) => &["PickUpTownhall"],
            Out::Delete { .. } => &["ColonyDeleteOwn", "GetColonyInfo"],
            Out::Abandon { .. } => &["ColonyAbandonOwn", "GetColonyInfo"],
        }
    }
}

/// Les transitions de la boîte, `(bouton, avant, après)` — la table que [`press`] applique.
///
/// Java ne teste **pas** l'état d'avant : `delete` puis `abandon` montre la moitié
/// « abandonner » (`:93`, `:82`), et `cancelaction` cache depuis n'importe où (`:104`).
pub const TRANSITIONS: &[(&str, Dialog, Dialog)] = &[
    (DELETE, Dialog::Hidden, Dialog::Delete),
    (DELETE, Dialog::Abandon, Dialog::Delete),
    (ABANDON, Dialog::Hidden, Dialog::Abandon),
    (ABANDON, Dialog::Delete, Dialog::Abandon),
    (CANCEL_ACTION, Dialog::Delete, Dialog::Hidden),
    (CANCEL_ACTION, Dialog::Abandon, Dialog::Hidden),
];

/// Le clic d'un bouton, dans un état de la boîte, pour un hôtel de ville en `pos`.
///
/// **L'état ne décide de rien ici.** Aucun des sept gestionnaires ne lit la boîte avant
/// d'agir (`:47-54`) : `delete` et `abandon` la montrent depuis n'importe quel état, et
/// `confirmdelete` répond même si la boîte est fermée. Le paramètre reste parce qu'un
/// appelant, lui, doit savoir où il en est — c'est la seule chose que le source ne dit pas.
pub fn press(state: Dialog, id: &str, pos: [i32; 3]) -> Result<Out, Refusal> {
    let _ = state;
    match id {
        DELETE => Ok(Out::Stay(Dialog::Delete)),
        ABANDON => Ok(Out::Stay(Dialog::Abandon)),
        CANCEL_ACTION => Ok(Out::Stay(Dialog::Hidden)),
        CANCEL => Ok(Out::Close),
        PICKUP => Ok(Out::PickUp(PickUpTownhall { pos })),
        CONFIRM_DELETE => Ok(Out::Delete { colony: ColonyDeleteOwn, refresh: GetColonyInfo { pos } }),
        CONFIRM_ABANDON => Ok(Out::Abandon { colony: ColonyAbandonOwn, refresh: GetColonyInfo { pos } }),
        _ => Err(Refusal::UnknownButton),
    }
}

/// Les deux panneaux écrits, dans l'ordre où le constructeur les écrit.
const PANES: &[Pane] = &[
    // `:59` (configuration à colonie infinie) ou `:64` (sinon).
    Pane::written(WARNING, Value::Text, super::Source::Config, (XML, 7), (JAVA_FILE, 59)),
    // `:87` (abandon) ou `:98` (destruction).
    Pane::written(CONFIRM, Value::Text, super::Source::Key, (XML, 27), (JAVA_FILE, 87)),
];

/// Les sept boutons, dans l'ordre des `registerButton` (`:47-54`).
const BUTTONS: &[Button] = &[
    Button::local(CANCEL, "", Leaves::Closes),
    Button::sending(PICKUP, PICKUP_KEY, &["PickUpTownhall"], Leaves::Closes),
    Button::local(DELETE, DELETE_KEY, Leaves::Stays),
    Button::local(ABANDON, ABANDON_KEY, Leaves::Stays),
    Button::local(CANCEL_ACTION, CANCEL_ACTION_KEY, Leaves::Stays),
    Button::guarded(
        CONFIRM_DELETE,
        CONFIRM_DELETE_KEY,
        &["ColonyDeleteOwn", "GetColonyInfo"],
        Leaves::Closes,
        &[Refusal::NotOwner],
    ),
    Button::guarded(
        CONFIRM_ABANDON,
        CONFIRM_ABANDON_KEY,
        &["ColonyAbandonOwn", "GetColonyInfo"],
        Leaves::Closes,
        &[Refusal::NotOwner],
    ),
];

/// L'écran.
pub static WINDOW: Window = Window {
    id: DELETE_ABANDON_ID,
    java: JAVA,
    resource: RESOURCE,
    header: Header::None,
    opener: Opener::Java(OPENER_CLASS),
    panes: PANES,
    buttons: BUTTONS,
    skeleton: true,
};

#[cfg(test)]
mod tests {
    use super::*;

    const POS: [i32; 3] = [12, 70, -40];

    #[test]
    fn the_warning_and_the_abandon_button_are_one_decision() {
        let infinite = warning(true);
        assert_eq!(infinite.key, ABANDON_WARNING_KEY);
        assert!(infinite.abandon_shown, ":58 montre le bouton");
        let finite = warning(false);
        assert_eq!(finite.key, DELETE_WARNING_KEY);
        assert!(!finite.abandon_shown, ":63 le cache");
        // Deux textes, deux clés : le message change avec la configuration.
        assert_ne!(infinite.key, finite.key);
        assert_ne!(DELETE_WARNING_KEY, ABANDON_WARNING_KEY);
        // Et le panneau dit d'où vient la branche.
        let warning_pane = match WINDOW.pane(WARNING) {
            Some(pane) => pane,
            None => panic!("l'avertissement a disparu"),
        };
        assert_eq!(warning_pane.source, super::super::Source::Config);
        assert!(warning_pane.written.is_some(), "la classe l'écrit :59 ou :64");
    }

    #[test]
    fn the_dialog_starts_hidden_and_cancel_action_comes_back_to_it() {
        // L'état d'ouverture : rien de montré, aucune confirmation écrite.
        assert_eq!(Dialog::ALL[0], Dialog::Hidden);
        assert!(!Dialog::Hidden.is_open());
        assert_eq!(Dialog::Hidden.shown(), &[] as &[&str]);
        assert_eq!(Dialog::Hidden.confirm_key(), None);
        // `delete` montre la boîte, `cancelaction` la cache.
        assert_eq!(press(Dialog::Hidden, DELETE, POS), Ok(Out::Stay(Dialog::Delete)));
        assert_eq!(press(Dialog::Delete, CANCEL_ACTION, POS), Ok(Out::Stay(Dialog::Hidden)));
        // `delete` puis `abandon` : Java ne teste pas l'état d'avant.
        assert_eq!(press(Dialog::Delete, ABANDON, POS), Ok(Out::Stay(Dialog::Abandon)));
        assert_eq!(press(Dialog::Abandon, DELETE, POS), Ok(Out::Stay(Dialog::Delete)));
        // Et la table de transitions est exactement ce que la fonction fait.
        for &(button, before, after) in TRANSITIONS {
            assert_eq!(
                press(before, button, POS),
                Ok(Out::Stay(after)),
                "{button} depuis {before:?}"
            );
        }
    }

    #[test]
    fn each_half_of_the_dialog_shows_its_own_text_and_its_own_confirm_button() {
        let delete = Dialog::Delete.shown();
        assert!(delete.contains(&DIALOG) && delete.contains(&CONFIRM));
        assert!(delete.contains(&CONFIRM_DELETE));
        assert!(!delete.contains(&CONFIRM_ABANDON), "une seule moitié à la fois");
        let abandon = Dialog::Abandon.shown();
        assert!(abandon.contains(&CONFIRM_ABANDON));
        assert!(!abandon.contains(&CONFIRM_DELETE));
        // Les deux moitiés sont au même endroit de l'XML (110, 160) : c'est bien une
        // bascule, pas deux boutons côte à côte.
        assert_eq!(Dialog::Delete.confirm_key(), Some(DELETE_PROCEED_KEY));
        assert_eq!(Dialog::Abandon.confirm_key(), Some(ABANDON_PROCEED_KEY));
        // Le texte écrit suit la moitié choisie.
        assert_eq!(WINDOW.pane(CONFIRM).map(|pane| pane.value), Some(Value::Text));
    }

    #[test]
    fn confirming_sends_two_messages_colony_first_then_the_refresh() {
        match press(Dialog::Delete, CONFIRM_DELETE, POS) {
            Ok(Out::Delete { colony, refresh }) => {
                assert_eq!(colony, ColonyDeleteOwn, "aucun champ : le propriétaire est le clic");
                assert_eq!(refresh.pos, POS, ":78 relit l'hôtel de ville d'où l'on vient");
            }
            other => panic!("{other:?}"),
        }
        match press(Dialog::Abandon, CONFIRM_ABANDON, POS) {
            Ok(Out::Abandon { colony, refresh }) => {
                assert_eq!(colony, ColonyAbandonOwn);
                assert_eq!(refresh.pos, POS);
            }
            other => panic!("{other:?}"),
        }
        // L'ordre d'envoi est celui de Java : la colonie, puis la relecture.
        let sent = press(Dialog::Delete, CONFIRM_DELETE, POS).map(|out| out.sends());
        assert_eq!(sent, Ok(&["ColonyDeleteOwn", "GetColonyInfo"][..]));
        let sent = press(Dialog::Abandon, CONFIRM_ABANDON, POS).map(|out| out.sends());
        assert_eq!(sent, Ok(&["ColonyAbandonOwn", "GetColonyInfo"][..]));
        // `pickup` n'envoie qu'un message, et il porte la position de l'hôtel de ville.
        match press(Dialog::Hidden, PICKUP, POS) {
            Ok(Out::PickUp(message)) => assert_eq!(message.pos, POS),
            other => panic!("{other:?}"),
        }
        assert_eq!(press(Dialog::Hidden, PICKUP, POS).map(|out| out.sends()), Ok(&["PickUpTownhall"][..]));
    }

    #[test]
    fn the_table_and_the_behaviour_name_the_same_messages() {
        assert_eq!(super::super::window_by_java(JAVA).map(|window| window.id), Ok(DELETE_ABANDON_ID));
        // Les quatre boutons qui n'envoient rien…
        let local: Vec<&str> = BUTTONS.iter().filter(|button| button.is_local()).map(|b| b.id).collect();
        assert_eq!(local, [CANCEL, DELETE, ABANDON, CANCEL_ACTION]);
        // …et les trois qui envoient, avec le même ordre que `Out::sends`.
        for id in [PICKUP, CONFIRM_DELETE, CONFIRM_ABANDON] {
            let button = match WINDOW.button(id) {
                Some(button) => button,
                None => panic!("{id} a disparu"),
            };
            assert!(!button.is_local());
            for class in button.sends {
                assert!(super::super::message_is_known(*class), "{class} n'est pas un type du port");
            }
        }
        // Une confirmation peut être refusée par le serveur, un clic local jamais.
        assert_eq!(WINDOW.button(CONFIRM_DELETE).map(|b| b.is_guarded()), Some(true));
        assert_eq!(WINDOW.button(DELETE).map(|b| b.is_guarded()), Some(false));
        assert_eq!(press(Dialog::Hidden, "confirm", POS), Err(Refusal::UnknownButton));
    }
}
