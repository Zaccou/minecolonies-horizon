//! Les quatre fenêtres de l'hôtel de ville : réactiver une colonie abandonnée, la
//! détruire ou l'abandonner, acheter une vague de mercenaires, renommer la colonie.
//!
//! Java référence : `core/client/gui/townhall/` — `WindowTownHallColonyReactivate.java`
//! (61 lignes), `WindowTownHallDeleteAbandonColony.java` (121),
//! `WindowTownHallMercenary.java` (73), `WindowTownHallNameEntry.java` (63) — et leurs
//! quatre XML sous `assets/minecolonies/gui/townhall/`.
//!
//! **Même forme que `src/ui`, pas une deuxième.** Un panneau est un [`Pane`] qui se rend
//! en [`crate::ui::Field`] ([`Pane::to_ui`]), une valeur est un [`crate::ui::field::Value`],
//! une portée un [`crate::ui::field::Scope`], une constante un
//! [`crate::ui_window_01::constants::Constant`], et « où le clic laisse la fenêtre » un
//! [`crate::ui_window_06::Leaves`]. Ce qui est propre à ce module est ce qu'aucune des
//! trois tables n'a :
//!
//! - [`Source`], les sept lecteurs de ces écrans — dont quatre que `src/ui::field::Source`
//!   n'a pas (le message qui a ouvert la fenêtre, le pack de récits, la configuration
//!   serveur, le texte tapé) ;
//! - [`Pane::written`], la ligne Java qui **écrit** le panneau, quand une classe l'écrit —
//!   c'est ce qui distingue le texte figé dans l'XML du texte composé à l'ouverture ;
//! - [`Button::sends`], la liste ordonnée des messages d'un clic : **trois boutons en
//!   envoient deux** (`confirmdelete`, `confirmabandon`) et `ui::action::Effect` n'en sait
//!   nommer qu'un par bouton ;
//! - l'état et ses transitions, qui vivent dans les quatre modules de fenêtre.
//!
//! | Fichier | Question |
//! | --- | --- |
//! | [`constants`] | les huit `WindowConstants` lus, avec leur ligne |
//! | [`refusal`] | neuf refus, dont **un seul** est prononcé |
//! | [`reactivate`] | le récit tiré au sort, le pré-nom, et le passage à l'écran suivant |
//! | [`delete_abandon`] | deux avertissements, une boîte de confirmation, deux messages |
//! | [`mercenary`] | la vague : `citoyens / 10 + 3`, neuf portraits, quarante minutes |
//! | [`name_entry`] | le pré-remplissage en minuscules, la frappe à 25, la coupe du message |
//!
//! # Les comptes, mesurés
//!
//! | Quoi | Combien | Où |
//! | --- | --- | --- |
//! | fenêtres | 4 | [`WINDOWS`] |
//! | classes Java / XML | 4 / 4 | [`WINDOWS`] |
//! | panneaux avec un `id` | **7** (5 textes, 1 saisie, 1 texte figé) | [`WINDOWS`] |
//! | panneaux sans `id` donc pas des panneaux | 3 visibilités + 9 portraits | `delete_abandon`, `mercenary` |
//! | boutons | **13** (2 + 7 + 2 + 2) | [`WINDOWS`] |
//! | boutons qui envoient | 5, pour **7 envois** | [`WINDOWS`] |
//! | types de message nommés | **6** | [`message_is_known`] |
//! | fenêtres `AbstractWindowSkeleton` | 2 | [`Window::skeleton`] |
//! | valeurs de `src/ui` employées | **2** (`Text`, `Input`) | [`WINDOWS`] |
//!
//! # Quatre prémisses du brief que le source contredit
//!
//! 1. **« les champs affichés (nom, niveau, progression, stock) »** : aucun de ces quatre
//!    écrans n'affiche de **niveau**, de **progression**, ni de **stock**. Sur les sept
//!    panneaux, six sont [`Value::Text`] — dont un figé dans l'XML, que personne n'écrit — et
//!    un est [`Value::Input`] ; le seul « nom » est le **contenu** du champ de saisie et un
//!    pré-nom tiré au sort interpolé dans un titre. Le test
//!    `the_seven_panes_hold_two_kinds_and_never_a_level` tient le compte.
//! 2. **« les actions (embaucher, renvoyer, prioriser) »** : aucune de ces fenêtres ne
//!    renvoie un citoyen ni ne priorise quoi que ce soit. Le seul « embaucher » est la
//!    vague de mercenaires, qui n'embauche **personne** : `spawnMercenariesInColony` fait
//!    apparaître des entités temporaires (`EntityMercenary.java:485-520`), et les boutons
//!    d'embauche, de renvoi et de priorité d'une hutte sont ceux de `src/ui` (déjà portés).
//! 3. **« réactiver » est déjà pris.** `WindowReactivateBuilding` — décrit par
//!    `crate::ui_window_06::reactivate` — n'est pas cette fenêtre : autre classe, autre
//!    XML, aucun panneau ni bouton commun, et un autre message (`ReactivateBuildingMessage`
//!    contre `CreateColonyMessage`). Deux fenêtres, un mot.
//! 4. **`create` ne crée rien.** Sur l'écran de réactivation, `create` ouvre
//!    `WindowTownHallColonyManage` avec `reactivate=true` (`:60`) ; le vrai envoi part du
//!    bouton `create` **de cet écran-là**. Et le booléen ne dit pas « réactiver » : il
//!    atterrit dans le champ `claim` de `CreateColonyMessage` (`:80`), où le serveur lit
//!    « réutilise le pack de structure de la hutte » (`:129-131`) — la réactivation, elle,
//!    est le tag `DEACTIVATED` de la hutte (`:134-142`). Voir [`reactivate::Handoff`].
//!
//! # Ce qui n'est pas ici
//!
//! Le **dessin** : ni `pos`, ni `size`, ni sprite, ni couleur — sauf quand une position
//! décide d'une donnée, ce qui n'arrive qu'aux neuf portraits de
//! [`mercenary`]. L'**envoi** : six classes de message sont nommées et construites,
//! aucune n'est envoyée — il n'y a ni client ni `Network` dans ce port. Le **son** : les
//! deux fenêtres qui jouent `BOOK_PAGE_TURN` à la construction et le
//! `LIGHTNING_BOLT_THUNDER` du clic mercenaire ne sont que des noms ici. Et la
//! **configuration serveur** `allowInfiniteColonies`, que l'appelant fournit
//! ([`delete_abandon::Warning`]) : le port n'a pas de lecteur de configuration.
//! `MANQUANT.md` nomme chaque trou et ce qu'il coûte.

pub mod constants;
pub mod delete_abandon;
pub mod mercenary;
pub mod name_entry;
pub mod reactivate;
pub mod refusal;

pub use crate::ui_window_06::{Header, Leaves, Opener};
pub use constants::{Constant, WINDOW_CONSTANTS};
pub use refusal::Refusal;

use crate::ui::field::{Field, Scope, Source as UiSource, Value};

/// L'id de l'écran de réactivation dans cette table.
pub const COLONY_REACTIVATE_ID: &str = "townhall.reactivate_colony";
/// L'id de l'écran qui détruit ou abandonne.
pub const DELETE_ABANDON_ID: &str = "townhall.delete_abandon";
/// L'id de l'écran des mercenaires.
pub const MERCENARY_ID: &str = "townhall.mercenary";
/// L'id du renommage de la colonie.
pub const COLONY_NAME_ID: &str = "townhall.colony_name";

/// Les deux messages que **toute** fenêtre `AbstractWindowSkeleton` envoie, quel que soit
/// son contenu : le premier à la construction (`AbstractWindowSkeleton.java:96`), le
/// second à chaque clic lié (`:196`). Deux des quatre fenêtres ici en héritent
/// ([`Window::skeleton`]) ; les deux autres étendent `BOWindow` et n'en envoient aucun.
pub static GENERIC_TRIGGERS: [&str; 2] =
    ["OpenGuiWindowTriggerMessage", "ClickGuiButtonTriggerMessage"];

/// La recharge d'une vague de mercenaires, en ticks : `TICKS_SECOND * SECONDS_A_MINUTE * 40`
/// (`Constants.java:34`) = `20 * 60 * 40` = **48 000**.
///
/// Le nom de la constante Java est `TICKS_FOURTY_MIN` — une faute d'orthographe dans la
/// source, recopiée ici en [`TICKS_FORTY_MIN`] pour que la valeur reste trouvable par
/// celui qui la cherche sous sa vraie graphie.
pub const TICKS_FORTY_MIN: i64 = 48_000;

/// D'où la valeur d'un panneau vient.
///
/// `crate::ui::field::Source` nomme les lecteurs d'une hutte ; quatre de ceux-ci n'y sont
/// pas et n'y ont pas leur place, parce qu'ils ne lisent **ni la colonie, ni la hutte** :
/// le message réseau qui a ouvert la fenêtre, le pack de récits du client, la
/// configuration serveur, et ce que le joueur a tapé.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive : les sept lecteurs de ces quatre écrans, et aucun autre
pub enum Source {
    /// Un champ du message client qui a ouvert la fenêtre et que la classe passe à son
    /// constructeur (`OpenReactivateColonyMessage.java:29-35`,
    /// `OpenDeleteAbandonColonyMessage.java:42-49`).
    Opened,
    /// Le pack de récits rechargé par `ColonyStoryListener`
    /// (`ColonyStoryListener.java:61-62`, `:88-97`) : des clés de traduction filtrées par
    /// biome, lues au client.
    Story,
    /// `IColonyView` — `getName()` (`WindowTownHallNameEntry.java:39`),
    /// `getCitizenCount()` (`WindowTownHallMercenary.java:41`),
    /// `getMercenaryUseTime()` (`ColonyView.java:1308-1312`). La **seule** source de ce
    /// module que `src/ui` sait nommer.
    ColonyView,
    /// La configuration serveur : `allowInfiniteColonies`
    /// (`WindowTownHallDeleteAbandonColony.java:56-65`).
    Config,
    /// Ce que le joueur a tapé dans le champ (`WindowTownHallNameEntry.java:47`).
    Input,
    /// Une clé de traduction que l'**XML** pose et qu'aucune classe ne réécrit
    /// (`windowtownhallmercenary.xml:15`).
    Key,
}

impl Source {
    /// Un nom stable, pour un journal ou un test.
    pub fn name(self) -> &'static str {
        match self {
            Source::Opened => "opened",
            Source::Story => "story",
            Source::ColonyView => "colony_view",
            Source::Config => "config",
            Source::Input => "input",
            Source::Key => "key",
        }
    }

    /// La source de `src/ui`, quand elle existe là-bas.
    ///
    /// Une seule correspondance, et c'est voulu : ranger le pack de récits ou la
    /// configuration sous `Source::Colony` serait faux — ni l'un ni l'autre n'est la
    /// colonie, et une valeur rangée au mauvais endroit est pire qu'une valeur non rangée.
    pub fn to_ui(self) -> Option<UiSource> {
        match self {
            Source::ColonyView => Some(UiSource::Colony),
            Source::Opened | Source::Story | Source::Config | Source::Input | Source::Key => None,
        }
    }

    /// Vrai quand la valeur vit dans la colonie. Seul [`Source::ColonyView`], pour la
    /// même raison.
    pub fn is_colony_data(self) -> bool {
        self.to_ui().is_some()
    }
}

/// Un panneau d'une de ces quatre fenêtres.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pane {
    /// L'`id` de l'XML, seule prise que la classe a sur le panneau
    /// (`findPaneOfTypeByID`, BlockUI ; appelé `WindowTownHallColonyReactivate.java:50`).
    pub id: &'static str,
    /// La clé du libellé posée à côté, quand il y en a une dans le même panneau :
    /// `None` partout ici — les quatre XML posent leurs légendes dans des `text` sans id,
    /// que [`Window::header`] porte.
    pub label: Option<&'static str>,
    /// Ce que le panneau tient.
    pub value: Value,
    /// D'où la valeur vient.
    pub source: Source,
    /// La fenêtre, ou une ligne de liste (aucune ici : ces quatre écrans n'ont pas de liste).
    pub scope: Scope,
    /// Le fichier XML et la ligne qui déclarent l'`id`, 1-based.
    pub declared: (&'static str, u32),
    /// La classe et la ligne qui **écrivent** le panneau ; `None` quand personne ne
    /// l'écrit et que le texte de l'XML reste tel quel.
    pub written: Option<(&'static str, u32)>,
}

impl Pane {
    /// Un panneau que la classe écrit.
    pub const fn written(
        id: &'static str,
        value: Value,
        source: Source,
        declared: (&'static str, u32),
        written: (&'static str, u32),
    ) -> Pane {
        Pane { id, label: None, value, source, scope: Scope::Window, declared, written: Some(written) }
    }

    /// Un panneau que personne n'écrit : le texte de l'XML est la valeur.
    pub const fn fixed(
        id: &'static str,
        value: Value,
        declared: (&'static str, u32),
    ) -> Pane {
        Pane {
            id,
            label: None,
            value,
            source: Source::Key,
            scope: Scope::Window,
            declared,
            written: None,
        }
    }

    /// Le panneau sous la forme de `src/ui`, quand la source y a un équivalent.
    pub fn to_ui(&self) -> Option<Field> {
        Some(Field {
            id: self.id,
            label: self.label,
            value: self.value,
            source: self.source.to_ui()?,
            scope: self.scope,
        })
    }
}

/// Un bouton d'une de ces quatre fenêtres.
///
/// Quatre champs, dont deux que `src/ui::action::Action` ne peut pas porter : `sends` est
/// une **liste** (`confirmdelete` envoie `ColonyDeleteOwn` puis `GetColonyInfo`, dans cet
/// ordre — `WindowTownHallDeleteAbandonColony.java:77-78`), et `refusals` est le
/// [`Refusal`] de ce module, pas celui des huttes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Button {
    /// L'id que l'XML donne et que `registerButton` lie (`AbstractWindowSkeleton.java:119`).
    pub id: &'static str,
    /// La clé du libellé ; `""` pour une image nue — la croix de `cancel`
    /// (`windowcolonyreactivate.xml:5`).
    pub label: &'static str,
    /// Les messages que le clic construit, dans l'ordre d'envoi de Java, par **type du
    /// port** (`crate::network::founding`, `crate::network::colony_settings`).
    pub sends: &'static [&'static str],
    /// Où le clic laisse la fenêtre.
    pub leaves: Leaves,
    /// Chaque façon dont le clic est refusé, dans l'ordre où Java les applique.
    pub refusals: &'static [Refusal],
}

impl Button {
    /// Un bouton qui envoie un message.
    pub const fn sending(
        id: &'static str,
        label: &'static str,
        sends: &'static [&'static str],
        leaves: Leaves,
    ) -> Button {
        Button { id, label, sends, leaves, refusals: &[] }
    }

    /// Un bouton local : il ne demande rien, il bouge un état de la fenêtre.
    pub const fn local(id: &'static str, label: &'static str, leaves: Leaves) -> Button {
        Button { id, label, sends: &[], leaves, refusals: &[] }
    }

    /// Un bouton que Java refuse dans au moins un état.
    pub const fn guarded(
        id: &'static str,
        label: &'static str,
        sends: &'static [&'static str],
        leaves: Leaves,
        refusals: &'static [Refusal],
    ) -> Button {
        Button { id, label, sends, leaves, refusals }
    }

    /// Vrai quand Java peut refuser le clic.
    pub fn is_guarded(&self) -> bool {
        !self.refusals.is_empty()
    }

    /// Vrai quand le clic n'envoie rien : il ne fait que changer l'écran ou l'état.
    pub fn is_local(&self) -> bool {
        self.sends.is_empty()
    }
}

/// Une fenêtre de cette table : sa classe, son XML, son titre, ses panneaux, ses boutons,
/// et qui l'ouvre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    /// L'id de cette table. **Pas celui de Java**, qui n'en a pas — une fenêtre s'ouvre
    /// par sa classe ou par un message.
    pub id: &'static str,
    /// La classe Java qui la construit.
    pub java: &'static str,
    /// Le XML, sous `assets/minecolonies/`.
    pub resource: &'static str,
    /// Ce que l'XML écrit en haut de la fenêtre, et que la classe ne réécrit jamais.
    pub header: Header,
    /// Qui l'ouvre : le message client d'un des deux écrans, l'hôtel de ville pour les
    /// deux autres.
    pub opener: Opener,
    /// Les panneaux que la classe remplit, ou que l'XML fige.
    pub panes: &'static [Pane],
    /// Les boutons que la classe enregistre.
    pub buttons: &'static [Button],
    /// Vrai quand la classe étend `AbstractWindowSkeleton` — donc quand elle envoie
    /// [`GENERIC_TRIGGERS`] en plus de son contenu.
    pub skeleton: bool,
}

impl Window {
    /// Le panneau de cet id, copié ; le **premier**, puisqu'un id peut revenir.
    pub fn pane(&self, id: &str) -> Option<Pane> {
        self.panes.iter().find(|pane| pane.id == id).copied()
    }

    /// Le bouton de cet id, copié.
    pub fn button(&self, id: &str) -> Option<Button> {
        self.buttons.iter().find(|button| button.id == id).copied()
    }

    /// Vrai quand la fenêtre montre au moins un panneau de ce genre.
    pub fn shows(&self, value: Value) -> bool {
        self.panes.iter().any(|pane| pane.value == value)
    }

    /// Les messages que la fenêtre peut envoyer par ses boutons, **sans doublon**, dans
    /// l'ordre des boutons puis des envois.
    pub fn sends(&self) -> Vec<&'static str> {
        let mut sent = Vec::new();
        for button in self.buttons {
            for class in button.sends {
                if !sent.contains(class) {
                    sent.push(*class);
                }
            }
        }
        sent
    }

    /// Les deux déclencheurs génériques que le squelette ajoute, ou aucun.
    pub fn generic_triggers(&self) -> &'static [&'static str] {
        if self.skeleton {
            &GENERIC_TRIGGERS
        } else {
            &[]
        }
    }
}

/// Les quatre fenêtres de ce module.
pub static WINDOWS: &[&Window] = &[
    &reactivate::WINDOW,
    &delete_abandon::WINDOW,
    &mercenary::WINDOW,
    &name_entry::WINDOW,
];

/// La fenêtre de cet id, ou [`Refusal::UnknownWindow`].
pub fn window_by_id(id: &str) -> Result<&'static Window, Refusal> {
    WINDOWS.iter().copied().find(|window| window.id == id).ok_or(Refusal::UnknownWindow)
}

/// La fenêtre de cette classe Java, ou [`Refusal::UnknownWindow`].
///
/// La jointure inverse : `crate::ui::table` nomme `WindowMainPage` par sa classe sans la
/// décrire, et `crate::ui_window_06::reactivate` nomme `WindowReactivateBuilding` — deux
/// classes qui **ne** sont pas ces quatre-là, et qui répondent donc
/// [`Refusal::UnknownWindow`] ici.
pub fn window_by_java(class: &str) -> Result<&'static Window, Refusal> {
    WINDOWS.iter().copied().find(|window| window.java == class).ok_or(Refusal::UnknownWindow)
}

/// Vrai quand `name` est un type de message **de ce port** — pas une classe Java
/// seulement nommée.
///
/// Les deux familles interrogées sont celles qui répondent à ces écrans :
/// `network::founding` (les six messages de fondation, dont quatre ici) et
/// `network::colony_settings` (le renommage et les mercenaires). Chacune publie son
/// `CENSUS`, donc la question se répond sans deviner.
pub fn message_is_known(name: &str) -> bool {
    crate::network::founding::CENSUS.iter().any(|(_, rust)| *rust == name)
        || crate::network::colony_settings::CENSUS.iter().any(|(_, rust)| *rust == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_windows_one_class_and_one_xml_each() {
        assert_eq!(WINDOWS.len(), 4);
        for (index, window) in WINDOWS.iter().enumerate() {
            assert!(window.java.starts_with("WindowTownHall"), "{} : {}", window.id, window.java);
            assert!(
                window.resource.starts_with("minecolonies:gui/townhall/")
                    && window.resource.ends_with(".xml"),
                "{} lit {}",
                window.id,
                window.resource
            );
            assert!(!window.panes.is_empty(), "{} ne montre rien", window.id);
            assert!(!window.buttons.is_empty(), "{} n'a aucun bouton", window.id);
            for other in &WINDOWS[index + 1..] {
                assert_ne!(window.id, other.id, "deux fenêtres, un id");
                assert_ne!(window.java, other.java, "deux lignes, une classe");
                assert_ne!(window.resource, other.resource, "deux fenêtres, un XML");
            }
        }
        // Quatre classes de trois mots : deux écrans sans squelette, deux avec.
        assert_eq!(WINDOWS.iter().filter(|window| window.skeleton).count(), 2);
    }

    #[test]
    fn the_seven_panes_hold_two_kinds_and_never_a_level() {
        let panes: usize = WINDOWS.iter().map(|window| window.panes.len()).sum();
        assert_eq!(panes, 7, "3 reactivate + 2 delete/abandon + 1 mercenaire + 1 nom");
        let texts = WINDOWS
            .iter()
            .flat_map(|window| window.panes.iter())
            .filter(|pane| pane.value == Value::Text)
            .count();
        assert_eq!(texts, 6, "six textes : trois titres de récit, un avertissement, une confirmation, un récit");
        let inputs = WINDOWS
            .iter()
            .flat_map(|window| window.panes.iter())
            .filter(|pane| pane.value == Value::Input)
            .count();
        assert_eq!(inputs, 1, "un seul champ de saisie dans les quatre fenêtres");
        assert_eq!(texts + inputs, panes, "deux genres de panneaux, et pas un troisième");
        // Un seul panneau n'est écrit par personne : le récit figé de l'XML mercenaire.
        let fixed: Vec<&str> = WINDOWS
            .iter()
            .flat_map(|window| window.panes.iter())
            .filter(|pane| pane.written.is_none())
            .map(|pane| pane.id)
            .collect();
        assert_eq!(fixed, [mercenary::TEXT], "windowtownhallmercenary.xml:15");
        // Aucun de ces écrans n'affiche de niveau, de progression, de stock ni d'occupation :
        // la prémisse « nom, niveau, progression, stock » ne vaut pour aucun des quatre.
        for absent in [Value::Level, Value::Progress, Value::Stock, Value::Step, Value::Occupancy] {
            assert!(
                !WINDOWS.iter().any(|window| window.shows(absent)),
                "un panneau {} est apparu",
                absent.name()
            );
        }
        // Le seul nom est le contenu du champ : un `Input`, pas un `Name`.
        let input = name_entry::WINDOW.pane(name_entry::INPUT);
        assert_eq!(input.map(|pane| pane.value), Some(Value::Input));
        assert!(!name_entry::WINDOW.shows(Value::Name));
    }

    #[test]
    fn only_the_two_skeletons_send_the_generic_triggers() {
        let skeletons: Vec<&str> = WINDOWS
            .iter()
            .filter(|window| window.skeleton)
            .map(|window| window.java)
            .collect();
        assert_eq!(
            skeletons,
            ["WindowTownHallColonyReactivate", "WindowTownHallDeleteAbandonColony"],
            "AbstractWindowSkeleton.java:96 et :196"
        );
        // `WindowTownHallMercenary` et `WindowTownHallNameEntry` étendent `BOWindow` : aucun
        // déclencheur, et aucune trace d'ouverture annoncée au serveur.
        for window in WINDOWS {
            let triggers = window.generic_triggers();
            assert_eq!(triggers.len(), if window.skeleton { 2 } else { 0 });
            for trigger in triggers {
                assert!(!window.sends().contains(trigger), "{} : le squelette les envoie, pas un bouton", window.id);
            }
        }
    }

    #[test]
    fn the_six_named_messages_are_six_port_types() {
        let mut named: Vec<&str> = WINDOWS.iter().flat_map(|window| window.sends()).collect();
        named.sort_unstable();
        assert_eq!(
            named,
            [
                "ColonyAbandonOwn",
                "ColonyDeleteOwn",
                "GetColonyInfo",
                "HireMercenary",
                "PickUpTownhall",
                "TownHallRename",
            ],
            "six types, triés"
        );
        for class in &named {
            assert!(message_is_known(*class), "{class} n'est pas un type de ce port");
        }
        // Le total des envois, doublons compris : deux boutons en envoient deux.
        let sends: usize = WINDOWS.iter().flat_map(|window| window.buttons.iter()).map(|b| b.sends.len()).sum();
        assert_eq!(sends, 7, "pickup 1 + confirmdelete 2 + confirmabandon 2 + done 1 + done 1");
        // Et une classe Java jamais portée ne passe pas.
        assert!(!message_is_known("CreateColonyMessage"));
        assert!(message_is_known("CreateColony"));
    }

    #[test]
    fn window_by_java_is_the_inverse_of_window_by_id() {
        for window in WINDOWS {
            assert_eq!(window_by_id(window.id).map(|found| found.java), Ok(window.java));
            assert_eq!(window_by_java(window.java).map(|found| found.id), Ok(window.id));
        }
        // Les deux classes que d'autres tables nomment sans les décrire ne sont pas ici.
        assert_eq!(window_by_java("WindowMainPage"), Err(Refusal::UnknownWindow));
        assert_eq!(window_by_java("WindowReactivateBuilding"), Err(Refusal::UnknownWindow));
        assert_eq!(window_by_java("WindowTownHallColonyManage"), Err(Refusal::UnknownWindow));
        assert_eq!(window_by_id("hire"), Err(Refusal::UnknownWindow), "les 13 écrans de src/ui");
    }

    #[test]
    fn a_pane_renders_as_a_src_ui_field_only_where_the_source_maps() {
        let input = match name_entry::WINDOW.pane(name_entry::INPUT) {
            Some(pane) => pane,
            None => panic!("le champ de saisie a disparu"),
        };
        let field = match input.to_ui() {
            Some(field) => field,
            None => panic!("la colonie est une source de src/ui"),
        };
        assert_eq!(field.id, "name");
        assert_eq!(field.value, Value::Input);
        assert_eq!(field.source, UiSource::Colony);
        assert_eq!(field.scope, Scope::Window);
        // Les quatre autres sources n'ont pas d'équivalent : le récit n'est pas la colonie.
        for source in [Source::Opened, Source::Story, Source::Config, Source::Input, Source::Key] {
            assert_eq!(source.to_ui(), None, "{}", source.name());
            assert!(!source.is_colony_data());
        }
    }
}
