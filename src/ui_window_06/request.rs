//! `WindowRequestDetail` : une requête, son demandeur, son résolveur, et trois
//! boutons dont **un ne fait rien**.
//!
//! 205 lignes de Java. Ce que la fenêtre **demande** : la requête (`IRequest`), la
//! colonie pour retrouver le résolveur (`:146`), et le module qui l'a ouverte — parce
//! que c'est **lui** qui décide si `fulfill` fait quelque chose ([`Fulfil`]). Ce
//! qu'elle **rend** : sept panneaux, un clic qui remplit, un clic qui annule, et un
//! clic qui n'a pas de gestionnaire.
//!
//! **`back` est dessiné et mort.** `windowrequestdetail.xml:19-20` lui donne un id et
//! un libellé ; `WindowRequestDetail.onButtonClicked` ne teste que `fulfill` et
//! `cancel` (`:192-203`) et n'appelle même pas `super.onButtonClicked`, qui est le
//! seul endroit où un `back` serait lié (`AbstractWindowSkeleton.java:130`). Le clic
//! est avalé.
//!
//! **Remplir dépend de qui a ouvert la fenêtre, pas de la requête.**
//! `RequestTreeWindowModule.isFulfillable` rend `false` (`:274-277`) ; le module du
//! citoyen le remplace (`RequestWindowCitizen.java:156-175`). La même fenêtre, la
//! même requête, deux réponses — d'où [`Fulfil`], qui porte la politique, et
//! `opener` de [`super::Window`], qui ne dit que le premier des deux.
//!
//! **Le résolveur manquant ne casse pas `cancel`.** Java avertit et sort d'`onOpened`
//! (`:146-158`) : les deux `setEnabled` ne sont jamais atteints, donc les deux
//! boutons restent à l'état de l'XML — **actifs**. `fulfill` ne fait alors rien, et
//! `cancel` annule quand même, puisque son gestionnaire ne teste aucune porte
//! (`:199-203`).

use crate::ui::action::Effect;
use crate::ui::field::{Field, Source, Value};

use super::refusal::Refusal;
use super::{Button, Header, Leaves, Opener, Window};

/// L'id de cette table.
pub const ID: &str = "request_detail";
/// La classe (`WindowRequestDetail.java:32`).
pub const JAVA: &str = "WindowRequestDetail";
/// Le XML (`:34`).
pub const RESOURCE: &str = "minecolonies:gui/windowrequestdetail.xml";
/// Le libellé de l'XML, sans id (`windowrequestdetail.xml:7-8`).
pub const HEADER_KEY: &str = "com.minecolonies.coremod.gui.citizen.requests";
/// Le demandeur (`:141` ; `WindowRequestDetail.java:39`).
pub const REQUESTER: &str = "requester";
/// L'icône qui tourne entre les piles de la requête (`:126-133`).
pub const STACK: &str = "requestStack";
/// La position du demandeur (`:142`).
pub const LOCATION: &str = "targetLocation";
/// Le résolveur (`:153`).
pub const RESOLVER: &str = "resolver";
/// Le détail, court ou long (`:115`, `:120`).
pub const SHORT_DETAIL: &str = "shortDetail";
/// L'icône du détail, pour une tâche (`:110-114`).
pub const DETAIL_ICON: &str = "detailIcon";
/// L'image de remplacement, quand il n'y a pas de pile (`:124-139`).
pub const DELIVERY_IMAGE: &str = "deliveryImage";
/// Le retour — **sans gestionnaire** (`windowrequestdetail.xml:19-20` ; `WindowConstants.java:882`).
pub const BACK: &str = "back";
/// Remplir la requête (`:160`, `:194-198` ; `WindowConstants.java:614`).
pub const FULFILL: &str = "fulfill";
/// Annuler la requête (`:161`, `:199-203` ; `WindowConstants.java:619`).
pub const CANCEL: &str = "cancel";

/// Le diviseur du compteur de vie, qui fait tourner l'icône (`:49`).
pub const LIFE_DIVIDER: u32 = 30;
/// Le message du clic « remplir », côté citoyen (`RequestWindowCitizen.java:237`).
pub const FULFILL_MESSAGE: &str = "TransferItemsToCitizenRequestMessage";
/// Le message du clic « annuler » (`RequestTreeWindowModule.java:332`).
pub const CANCEL_MESSAGE: &str = "UpdateRequestStateMessage";
/// L'état que le message d'annulation transporte (`:332`, `RequestState.CANCELLED`).
pub const CANCELLED_STATE: &str = "CANCELLED";
/// Le préfixe du panneau `resolver`, **en anglais dans le code** (`:153`).
pub const RESOLVER_PREFIX: &str = "Resolver: ";
/// La séquence de formatage que Java efface du détail long (`:120`).
pub const FORMAT_CHAR: &str = "§f";

/// Le compteur de vie, qui avance d'une image à l'autre sauf si Maj est tenue
/// (`:168-171`).
pub fn next_life(life: u32, shift_down: bool) -> u32 {
    if shift_down {
        life
    } else {
        life.saturating_add(1)
    }
}

/// L'index de la pile affichée : `(life / 30) % count` (`:132`, `:178`).
///
/// `None` quand la requête n'en montre aucune : Java le teste avant de diviser
/// (`:130`, `:176`).
pub fn stack_index(life: u32, stacks: usize) -> Option<usize> {
    if stacks == 0 {
        return None;
    }
    Some((life / LIFE_DIVIDER) as usize % stacks)
}

/// Le détail long, débarrassé de ses `§f` (`:120`).
pub fn strip_format(text: &str) -> String {
    text.replace(FORMAT_CHAR, "")
}

/// La branche de `onOpened` qui remplit le détail (`:108-122`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two branches of `instanceof IStackBasedTask`
pub enum Detail {
    /// Une tâche : l'icône porte la pile de la tâche et son compte, le texte son
    /// préfixe (`:110-115`).
    Task { count: i32 },
    /// Toute autre requête : l'icône est cachée et le texte est le détail long
    /// (`:117-121`).
    Long,
}

/// La branche du détail pour une requête.
pub fn detail(is_task: bool, count: i32) -> Detail {
    if is_task {
        Detail::Task { count }
    } else {
        Detail::Long
    }
}

impl Detail {
    /// Vrai quand le panneau `detailIcon` est montré (`:114`, `:119`).
    pub fn icon_visible(self) -> bool {
        matches!(self, Detail::Task { .. })
    }

    /// Vrai quand le texte est le préfixe de la tâche plutôt que le détail long.
    pub fn uses_prefix(self) -> bool {
        self.icon_visible()
    }
}

/// Qui a ouvert la fenêtre, donc quelle règle de remplissage s'applique.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two modules that open this window in Java
pub enum Fulfil {
    /// La clipboard : le module ne remplace pas `isFulfillable`, qui rend `false`
    /// (`RequestTreeWindowModule.java:274-277`). Le bouton ne s'allume jamais.
    Clipboard,
    /// La fenêtre des requêtes d'un citoyen (`RequestWindowCitizen.java:83`,
    /// `:156-175`).
    Citizen,
}

/// Les quatre faits dont la règle du citoyen a besoin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FulfilCase {
    /// `request.getRequest() instanceof IDeliverable` (`:158`).
    pub deliverable: bool,
    /// `request.hasParent()` et le demandeur est ailleurs (`:163-166`).
    pub parent_elsewhere: bool,
    /// La requête est dans les requêtes ouvertes du module (`:169-173`).
    pub known: bool,
    /// Créatif, ou l'objet est dans l'inventaire (`:175`).
    pub can_pay: bool,
}

/// Peut-on remplir cette requête ? La règle du module, dans son ordre.
pub fn may_fulfil(policy: Fulfil, case: FulfilCase) -> Result<(), Refusal> {
    if policy == Fulfil::Clipboard {
        return Err(Refusal::NotFulfillable);
    }
    if !case.deliverable {
        return Err(Refusal::NotFulfillable);
    }
    if case.parent_elsewhere {
        return Err(Refusal::NotFulfillable);
    }
    if !case.known {
        return Err(Refusal::NotFulfillable);
    }
    if !case.can_pay {
        return Err(Refusal::NotFulfillable);
    }
    Ok(())
}

/// Peut-on annuler cette requête ? Seulement une racine
/// (`RequestTreeWindowModule.java:258-269`).
pub fn may_cancel(depth: i32) -> Result<(), Refusal> {
    if depth <= 0 {
        Ok(())
    } else {
        Err(Refusal::NotCancellable)
    }
}

/// L'état des deux boutons à la fin d'`onOpened`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Buttons {
    pub fulfil: bool,
    pub cancel: bool,
}

/// Ce que `onOpened` laisse derrière lui, quand le résolveur manque et quand il est
/// là.
///
/// Sans résolveur, Java sort **avant** les deux `setEnabled` (`:151`) : les deux
/// boutons gardent l'état par défaut de BlockUI, qui est actif. C'est un
/// comportement, pas un oubli du port.
pub fn buttons(fulfil: bool, cancel: bool, resolver_present: bool) -> Buttons {
    if !resolver_present {
        return Buttons { fulfil: true, cancel: true };
    }
    Buttons { fulfil, cancel }
}

/// La ligne du panneau `resolver` : `"Resolver: <nom>"`, ou rien du tout (`:153`).
pub fn resolver_line(name: Option<&str>) -> Option<String> {
    name.map(|name| format!("{}{}", RESOLVER_PREFIX, name))
}

const F_STACK: Field = Field::new(STACK, Value::Icon, Source::Request);
const F_IMAGE: Field = Field::new(DELIVERY_IMAGE, Value::Icon, Source::Request);
const F_REQUESTER: Field = Field::new(REQUESTER, Value::Name, Source::Request);
const F_LOCATION: Field = Field::new(LOCATION, Value::Text, Source::Request);
const F_RESOLVER: Field = Field::new(RESOLVER, Value::Text, Source::Request);
const F_SHORT: Field = Field::new(SHORT_DETAIL, Value::Text, Source::Request);
const F_ICON: Field = Field::new(DETAIL_ICON, Value::Icon, Source::Request);

const PANES: &[Field] =
    &[F_STACK, F_IMAGE, F_REQUESTER, F_LOCATION, F_RESOLVER, F_SHORT, F_ICON];

const BUTTON_REFUSALS: &[Refusal] = &[Refusal::NotCancellable];
const FULFIL_REFUSALS: &[Refusal] = &[Refusal::NotFulfillable];

const BUTTONS: &[Button] = &[
    // Dessiné, jamais lié : `WindowRequestDetail.onButtonClicked` ne le teste pas.
    Button::dismiss(BACK, "com.minecolonies.coremod.gui.back", Leaves::Nothing),
    Button::guarded(
        FULFILL,
        "com.minecolonies.coremod.gui.citizen.requestsfulfill",
        Effect::Sends(FULFILL_MESSAGE),
        Leaves::Closes,
        FULFIL_REFUSALS,
    ),
    Button::guarded(
        CANCEL,
        "com.minecolonies.coremod.gui.citizen.requestscancel",
        Effect::Sends(CANCEL_MESSAGE),
        Leaves::Closes,
        BUTTON_REFUSALS,
    ),
];

/// La fenêtre.
pub static WINDOW: Window = Window {
    id: ID,
    java: JAVA,
    resource: RESOURCE,
    header: Header::Xml(HEADER_KEY),
    // La première des deux : la ligne `detail` de la clipboard
    // (`RequestTreeWindowModule.java:382-388`).
    opener: Opener::Window(crate::ui_window_02::clipboard::ID),
    panes: PANES,
    buttons: BUTTONS,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn citizen() -> FulfilCase {
        FulfilCase { deliverable: true, parent_elsewhere: false, known: true, can_pay: true }
    }

    #[test]
    fn the_back_button_is_drawn_and_has_no_handler() {
        let back = WINDOW.button(BACK).expect("back");
        assert!(back.is_dead());
        assert_eq!(back.leaves, Leaves::Nothing);
        assert_eq!(back.effect, None);
        assert_eq!(back.label, "com.minecolonies.coremod.gui.back");
        // Les deux autres, eux, font quelque chose.
        assert_eq!(WINDOW.dead_buttons(), [BACK]);
        assert_eq!(WINDOW.button(FULFILL).map(|b| b.leaves), Some(Leaves::Closes));
        assert_eq!(WINDOW.button(CANCEL).map(|b| b.leaves), Some(Leaves::Closes));
        assert_eq!(WINDOW.buttons.len(), 3);
    }

    #[test]
    fn filling_depends_on_who_opened_the_window() {
        // La clipboard ne remplit jamais, quelle que soit la requête.
        assert_eq!(may_fulfil(Fulfil::Clipboard, citizen()), Err(Refusal::NotFulfillable));
        // Le citoyen, dans son ordre : livrable, puis parent, puis connue, puis objet.
        assert_eq!(may_fulfil(Fulfil::Citizen, citizen()), Ok(()));
        let not_deliverable = FulfilCase { deliverable: false, ..citizen() };
        assert_eq!(may_fulfil(Fulfil::Citizen, not_deliverable), Err(Refusal::NotFulfillable));
        let elsewhere = FulfilCase { parent_elsewhere: true, ..citizen() };
        assert_eq!(may_fulfil(Fulfil::Citizen, elsewhere), Err(Refusal::NotFulfillable));
        let unknown = FulfilCase { known: false, ..citizen() };
        assert_eq!(may_fulfil(Fulfil::Citizen, unknown), Err(Refusal::NotFulfillable));
        let broke = FulfilCase { can_pay: false, ..citizen() };
        assert_eq!(may_fulfil(Fulfil::Citizen, broke), Err(Refusal::NotFulfillable));
        // Une seule des quatre portes suffit à refuser : quatre refus, un nom.
        assert_eq!(Refusal::NotFulfillable.key(), None);
    }

    #[test]
    fn only_a_root_request_is_cancellable() {
        assert_eq!(may_cancel(0), Ok(()));
        assert_eq!(may_cancel(1), Err(Refusal::NotCancellable));
        assert_eq!(may_cancel(3), Err(Refusal::NotCancellable));
        // Un nœud négatif n'existe pas côté serveur ; Java dit `<= 0`, le port aussi.
        assert_eq!(may_cancel(-1), Ok(()));
        assert!(WINDOW.button(CANCEL).is_some_and(|button| button.is_guarded()));
    }

    #[test]
    fn a_missing_resolver_leaves_both_buttons_switched_on() {
        // Sans résolveur, Java sort avant les deux `setEnabled` (`:151`).
        assert_eq!(buttons(false, false, false), Buttons { fulfil: true, cancel: true });
        // Avec, ce sont les portes qui décident.
        assert_eq!(buttons(true, false, true), Buttons { fulfil: true, cancel: false });
        assert_eq!(buttons(false, true, true), Buttons { fulfil: false, cancel: true });
        assert_eq!(resolver_line(Some("Builder")), Some("Resolver: Builder".to_owned()));
        assert_eq!(resolver_line(None), None);
        assert_eq!(RESOLVER_PREFIX, "Resolver: ", "écrit en dur dans le source :153");
    }

    #[test]
    fn the_icon_rotates_every_thirty_ticks_and_stops_when_shift_is_held() {
        assert_eq!(stack_index(0, 3), Some(0));
        assert_eq!(stack_index(29, 3), Some(0));
        assert_eq!(stack_index(30, 3), Some(1));
        assert_eq!(stack_index(60, 3), Some(2));
        assert_eq!(stack_index(90, 3), Some(0), "le modulo de Java :132");
        assert_eq!(stack_index(0, 0), None, "Java teste la liste vide avant de diviser");
        // Le compteur n'avance pas quand Maj est tenue (`:168-171`).
        assert_eq!(next_life(7, false), 8);
        assert_eq!(next_life(7, true), 7);
        // La pile et l'image de remplacement ne sont jamais montrées ensemble.
        assert_eq!(detail(true, 5), Detail::Task { count: 5 });
        assert!(detail(true, 5).icon_visible());
        assert!(!detail(false, 5).icon_visible());
        assert!(!Detail::Long.uses_prefix());
    }

    #[test]
    fn the_long_detail_has_its_format_codes_removed() {
        assert_eq!(strip_format("§fStone"), "Stone");
        assert_eq!(strip_format("§f§fDeux"), "Deux");
        assert_eq!(strip_format("Sans code"), "Sans code");
        // Seule cette séquence est effacée ; Java ne touche à rien d'autre (`:120`).
        assert_eq!(strip_format("§aVert"), "§aVert");
        assert_eq!(FORMAT_CHAR, "§f");
        // Les sept panneaux du XML, tous en lecture de requête.
        assert_eq!(WINDOW.panes.len(), 7);
        for pane in WINDOW.panes {
            assert_eq!(pane.source, Source::Request, "{}", pane.id);
        }
        assert_eq!(WINDOW.header, Header::Xml(HEADER_KEY));
        assert_eq!(WINDOW.pane(SHORT_DETAIL).map(|pane| pane.value), Some(Value::Text));
        assert_eq!(WINDOW.pane(REQUESTER).map(|pane| pane.value), Some(Value::Name));
    }
}
