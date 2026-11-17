//! La **donnee** de deux ecrans de MineColonies : le journal de quetes
//! (`core/client/gui/questlog/`) et la page d'alliance du tribunal
//! (`core/client/gui/townhall/WindowAlliancePage.java`).
//!
//! « Ce que l'ecran demande et ce qu'il rend » — pas le dessin. Pas de `pos`, pas de
//! `size`, pas de sprite : ceux-la sont dans les XML, et `TRANCHE.md` §3 M2 dit deja que
//! cette tranche n'en porte aucun. Ce qui est ici :
//!
//! - les **champs affiches**, avec leur type ([`crate::ui::field::Value`] pour la page
//!   d'alliance, [`quest::Line`] pour les lignes du journal) et la ligne Java qui les
//!   remplit ;
//! - les **actions**, avec le message qu'elles envoient et **chaque facon d'etre
//!   refusees**, nommee ([`Refusal`]) ;
//! - l'**etat initial** et les **transitions** quand une action passe
//!   ([`quest::Log`], [`alliance::Page::send`], [`alliance::receive`]).
//!
//! # Meme forme que `src/ui`, et pas une deuxieme
//!
//! Un panneau d'alliance est un [`crate::ui::field::Field`], une portee est un
//! [`crate::ui::field::Scope`], un effet est un [`crate::ui::action::Effect`]. Rien de
//! tout cela n'est redefini ici.
//!
//! **Un bouton, en revanche, n'est pas un `crate::ui::action::Action`**, et c'est le
//! Java qui tranche : les refus de cette page n'existent pas dans le jeu ferme de
//! `Action.refusals` (aucun ecran de hutte n'a de refus serveur de diplomatie), donc un
//! `Action` porterait zero refus et pretendrait que le clic ne peut pas echouer. [`Ally`]
//! porte donc les quatre elements lui-meme — id, cle de libelle, effet, refus nommes —
//! exactement comme `ui_window_13::Button` l'a fait pour ses huit refus.
//!
//! **`ui_window_13` decrit les panneaux du journal ; ce module decrit leurs valeurs.**
//! Les ids, les trois portees `quests` et le localisateur sont la-bas, et
//! [`quest`] les **importe** au lieu de les recopier. Ce qui est nouveau ici est ce que
//! cette table-la ne porte pas : le contenu type de chaque ligne, son encre, la regle qui
//! choisit le citoyen, et l'etat des trois listes.
//!
//! | Fichier | Question |
//! | --- | --- |
//! | [`quest`] | ce que chaque page lit, ce qu'elle rend, et ce qu'un clic surligne |
//! | [`alliance`] | trois listes, quatre boutons, cinq evenements, et un seul message |
//!
//! # Les comptes, mesures
//!
//! | Quoi | Combien | Ou |
//! | --- | --- | --- |
//! | fichiers Java du paquet `questlog/` | **6** | `ls` — le brief en nomme 4 |
//! | fenetres decrites | 2 | `WindowQuestLog`, `WindowAlliancePage` |
//! | interface de page | 1 | `WindowQuestLogQuestModule.java:13` |
//! | implementations de page | 3 | `ui_window_13::quest_log::Page` |
//! | types de ligne | **2** pour 3 pages | [`quest::Row`] |
//! | listes decrites | 6 | 3 du journal, 3 de l'alliance |
//! | panneaux de la page d'alliance | 11 (+2 vues) | [`alliance::FIELDS`] |
//! | boutons | 5 | 4 d'alliance + le localisateur |
//! | statuts de diplomatie | 3 | [`alliance::Status`] |
//! | types d'evenement | 5, dont **4** produisibles | [`alliance::EventKind`] |
//! | messages serveur nommes | **1** | `TriggerConnectionEventMessage`, 4 envois |
//! | refus nommes | 13 | [`Refusal`] |
//! | valeurs de `src/ui::Value` employees | 4 sur 12 | `Row`, `Name`, `Count`, `Text` |
//!
//! # Quatre premisses du brief que le source contredit
//!
//! 1. **« les actions (embaucher, renvoyer, prioriser) »** ne sont dans aucune des quatre
//!    sources du brief. Embaucher, renvoyer et prioriser sont les boutons de la hutte de
//!    metier, portes par `src/ui` (`Action::guarded("hire", …)`, `A_RECALL`, `A_PRIO_UP`,
//!    `A_PRIO_DOWN`). Il y a ici **cinq** boutons d'une autre espece : quatre qui envoient
//!    un message au serveur et un qui surligne un citoyen dans le monde.
//! 2. **« nom, niveau, progression, stock »** : ni niveau ni stock nulle part. La seule
//!    « progression » est le texte d'objectif d'une quete en cours
//!    (`WindowQuestLogInProgressQuestQuestModule.java:44`), ecrit par un moteur
//!    d'objectifs absent du port ; et la deuxieme valeur de la page d'alliance, que le
//!    brief ne nomme pas, est une **distance** en blocs (`WindowAlliancePage.java:179`).
//! 3. **`WindowQuestLogQuestModule` n'est pas un ecran.** C'est une interface de trois
//!    methodes dont une par defaut (`:21`, `:30`, `:37`) ; le brief la cite comme source
//!    d'ecran. Et `WindowAlliancePage` n'est pas une fenetre autonome : son XML est un
//!    **layout** monte dans le tribunal (`layoutalliance.xml:2`), atteint par l'onglet
//!    `alliances` (`AbstractWindowTownHall.java:42`).
//! 4. **La ligne « donneur » du journal ne montre pas toujours le donneur.** Sur une
//!    quete en cours, `WindowQuestLogInProgressQuestQuestModule.java:92` lit
//!    `getQuestTarget()` — « le porteur de la tache courante » (`IQuestInstance.java:93`)
//!    — sous le meme panneau et le meme prefixe que la page « disponible », qui lit
//!    `getQuestGiverId()` (`WindowQuestLogAvailableQuestModule.java:77`). C'est
//!    [`quest::Instance::holder`] contre [`quest::Instance::giver`], et un test le tient.
//!
//! # Ce qui n'est pas ici
//!
//! [`MANQUANT.md`](MANQUANT) nomme chaque manque et ce qu'il coute : le widget de liste
//! du SDK, le gestionnaire de quetes, le moteur d'objectifs, le gestionnaire de
//! connexions cote client, et le debordement `int` de `BlockPosUtil.dist`.

pub mod alliance;
pub mod quest;

pub use alliance::{Ally, Click, Connection, Event, EventKind, Panes, Status, ALLIES, FIELDS};
pub use quest::{
    cells, locator, Cell, Cells, Finished, Ink, Instance, Line, Locator, Log, Objective, Row, Tip,
};

use crate::ui_window_13::quest_log::Page as QuestPage;
use crate::ui_window_13::refusal::Refusal as Ui13;

/// Pourquoi une lecture ou un clic de ces deux ecrans ne rend rien.
///
/// **Aucun de ces refus n'a de cle de traduction, et c'est le fait, pas un oubli.** Le
/// journal ne dit rien : il rend un vide ou il leve. La page d'alliance ne dit rien non
/// plus, mais pour une autre raison — elle ne decide pas : son clic part au serveur, qui
/// abandonne en silence (`TriggerConnectionEventMessage.java:50-57`) ou se tait parce
/// qu'il n'a personne a qui repondre (`ColonyConnectionManager.java:745-748`). Un refus
/// sans cle n'est donc pas un refus sans raison : c'est un refus que le joueur ne voit
/// jamais.
///
/// Les refus que `ui_window_13` nomme deja sont **repris** ([`Refusal::Quest`]), pas
/// renommes : deux tables qui nomment le meme fait de deux facons finissent par se
/// contredire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own closed set, one variant per way nothing happens
pub enum Refusal {
    /// Un refus deja nomme par la table du journal
    /// (`crate::ui_window_13::refusal::Refusal`) : page inconnue, ligne hors bornes,
    /// modele de quete absent. [`Refusal::name`] rend le sien tel quel.
    Quest(Ui13),
    /// L'index d'objectif sort du modele :
    /// `WindowQuestLogInProgressQuestQuestModule.java:40` indexe la liste sans verifier,
    /// et un index de sauvegarde peut ne plus correspondre au modele.
    ObjectiveOutOfRange { index: i32, count: i32 },
    /// Le texte de progression manque : il vient de
    /// `IQuestObjectiveTemplate.getProgressText` (`:43`), que le port n'a pas.
    ObjectiveTextMissing { index: i32 },
    /// La page a recu le type de ligne de l'autre : les trois pages sont trois `T`
    /// (`WindowQuestLogQuestModule.java:13`).
    WrongRowKind { page: QuestPage },
    /// Le bouton n'est pas dessine dans cet etat, donc le clic n'existe pas :
    /// `WindowAlliancePage.java:182-184` et `:223`.
    NotShown,
    /// La ligne lue n'existe pas dans cette liste : Java lit
    /// `indirectConnectionData.get(-1)` (`:134`) et leve.
    ColumnRowOutOfRange { row: i32, list: &'static str },
    /// La ligne d'evenement n'existe pas : l'index inverse de `:117` et `:217` est lu
    /// sans borne.
    EventRowOutOfRange { row: i32, count: usize },
    /// Le joueur n'a pas `MANAGE_HUTS`, et le serveur sort sans rien dire
    /// (`TriggerConnectionEventMessage.java:50-51`).
    NoPermission,
    /// La colonie visee n'existe plus : `TriggerConnectionEventMessage.java:53-57` ecrit
    /// une ligne de log et s'arrete.
    TargetColonyGone,
    /// La colonie d'origine n'est ni directe ni indirecte : la fonction sort apres avoir
    /// **enregistre** l'evenement (`ColonyConnectionManager.java:732`, `:745-748`).
    OriginNotConnected,
    /// L'ordinal sauvegarde n'est pas un `DiplomacyStatus` :
    /// `ColonyConnection.java:64`, `:81` indexent `values()`.
    UnknownStatus { ordinal: i32 },
    /// L'ordinal sauvegarde n'est pas un `ConnectionEventType` :
    /// `ConnectionEvent.java:37`, `:42` indexent `values()`.
    UnknownEventKind { ordinal: i32 },
    /// Le nom d'une colonie depasse ce que `writeUtf`/`readUtf` savent porter
    /// (`ConnectionEvent.java:42`).
    NameTooLong { len: usize },
}

/// Qui decide qu'il ne se passe rien.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: trois endroits ou la decision se prend
pub enum Side {
    /// La fenetre : elle cache un bouton ou refuse de lire une ligne.
    Window,
    /// Le serveur : le message est parti et le gestionnaire l'abandonne.
    Server,
    /// La donnee : une sauvegarde porte un ordinal ou un index qui n'existe plus.
    Data,
}

impl Refusal {
    /// Un representant de chaque variante, dans l'ordre ou ce fichier les declare.
    ///
    /// `Quest` en porte un seul : la famille est ouverte, et c'est `ui_window_13` qui en
    /// tient la liste.
    pub const ALL: [Refusal; 13] = [
        Refusal::Quest(Ui13::NoRow),
        Refusal::ObjectiveOutOfRange { index: 0, count: 0 },
        Refusal::ObjectiveTextMissing { index: 0 },
        Refusal::WrongRowKind { page: QuestPage::InProgress },
        Refusal::NotShown,
        Refusal::ColumnRowOutOfRange { row: 0, list: alliance::DIRECT },
        Refusal::EventRowOutOfRange { row: 0, count: 0 },
        Refusal::NoPermission,
        Refusal::TargetColonyGone,
        Refusal::OriginNotConnected,
        Refusal::UnknownStatus { ordinal: 0 },
        Refusal::UnknownEventKind { ordinal: 0 },
        Refusal::NameTooLong { len: 0 },
    ];

    /// Un nom stable, prefixe pour ne jamais passer pour une phrase du jeu.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::Quest(inner) => inner.name(),
            Refusal::ObjectiveOutOfRange { .. } => "ui14.refusal.objective_out_of_range",
            Refusal::ObjectiveTextMissing { .. } => "ui14.refusal.objective_text_missing",
            Refusal::WrongRowKind { .. } => "ui14.refusal.wrong_row_kind",
            Refusal::NotShown => "ui14.refusal.button_hidden",
            Refusal::ColumnRowOutOfRange { .. } => "ui14.refusal.column_row",
            Refusal::EventRowOutOfRange { .. } => "ui14.refusal.event_row",
            Refusal::NoPermission => "ui14.refusal.no_permission",
            Refusal::TargetColonyGone => "ui14.refusal.target_gone",
            Refusal::OriginNotConnected => "ui14.refusal.origin_unconnected",
            Refusal::UnknownStatus { .. } => "ui14.refusal.unknown_status",
            Refusal::UnknownEventKind { .. } => "ui14.refusal.unknown_event_kind",
            Refusal::NameTooLong { .. } => "ui14.refusal.name_too_long",
        }
    }

    /// La cle de traduction que Java montre au joueur : **aucune**, pour les treize.
    ///
    /// Voir la doc du type : le journal se tait, et la page d'alliance laisse decider un
    /// serveur qui ne repond pas.
    pub fn key(self) -> Option<&'static str> {
        None
    }

    /// Qui decide.
    pub fn side(self) -> Side {
        match self {
            Refusal::Quest(inner) => match inner {
                Ui13::UnknownTemplate | Ui13::BrokenStatId | Ui13::UnknownInterval => Side::Data,
                _ => Side::Window,
            },
            Refusal::ObjectiveOutOfRange { .. }
            | Refusal::ObjectiveTextMissing { .. }
            | Refusal::UnknownStatus { .. }
            | Refusal::UnknownEventKind { .. } => Side::Data,
            Refusal::WrongRowKind { .. }
            | Refusal::NotShown
            | Refusal::ColumnRowOutOfRange { .. }
            | Refusal::EventRowOutOfRange { .. }
            | Refusal::NameTooLong { .. } => Side::Window,
            Refusal::NoPermission | Refusal::TargetColonyGone | Refusal::OriginNotConnected => {
                Side::Server
            }
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::field::Value;

    #[test]
    fn the_brief_names_four_files_and_the_quest_log_needs_six() {
        // Le paquet `questlog/` compte six fichiers : `Constants`, la fenetre, les trois
        // moteurs de page et l'interface. Le brief en nomme quatre — les deux manquants
        // sont la fenetre qui possede les pages et le moteur de la page « disponible ».
        let package = [
            "Constants",
            "WindowQuestLog",
            "WindowQuestLogQuestModule",
            "WindowQuestLogInProgressQuestQuestModule",
            "WindowQuestLogAvailableQuestModule",
            "WindowQuestLogFinishedQuestModule",
        ];
        assert_eq!(package.len(), 6);
        assert_eq!(QuestPage::ALL.len(), 3);
        // De ces six fichiers, un seul est une interface : elle n'est pas un ecran, et
        // `ui_window_13` refuse deja de la chercher comme fenetre.
        assert_eq!(
            crate::ui_window_13::window_by_java("WindowQuestLogQuestModule").map(|w| w.id),
            Err(Ui13::UnknownWindow)
        );
        // Et les deux fenetres du brief sont bien deux classes distinctes, dans deux
        // tables distinctes : `ui_window_13` porte le journal, pas l'onglet du tribunal.
        assert_eq!(
            crate::ui_window_13::window_by_java("WindowQuestLog").map(|w| w.id),
            Ok(crate::ui_window_13::QUEST_LOG_ID)
        );
        assert_eq!(
            crate::ui_window_13::window_by_java(alliance::JAVA).map(|w| w.id),
            Err(Ui13::UnknownWindow),
            "la page d'alliance n'est pas une des trois fenetres de ui_window_13"
        );
        assert_ne!(alliance::JAVA, "WindowQuestLog");
    }

    #[test]
    fn two_screens_and_one_of_them_brings_its_own_frame() {
        // Le journal est une fenetre complete ; la page d'alliance est un layout monte
        // dans le tribunal (`layoutalliance.xml:2`).
        assert!(alliance::RESOURCE.ends_with("layoutalliance.xml"));
        assert!(alliance::FRAME.ends_with("windowtownhall.xml"));
        assert_eq!(alliance::ENTRY_TAB, "alliances", "WindowConstants.java:37");
        // Le journal a une ressource a lui, et ses trois pages y vivent.
        assert_eq!(
            crate::ui_window_13::window_by_id(crate::ui_window_13::QUEST_LOG_ID).map(|w| w.resource),
            Ok("minecolonies:gui/windowquestlog.xml")
        );
        // Les deux ecrans n'ont aucun id en commun.
        assert_ne!(alliance::EVENTS, crate::ui_window_13::quest_log::LIST);
        assert_ne!(alliance::DIRECT, alliance::INDIRECT);
    }

    #[test]
    fn thirteen_refusals_and_two_thirds_of_them_are_not_the_player_s_business() {
        assert_eq!(Refusal::ALL.len(), 13);
        let mut window = 0;
        let mut server = 0;
        let mut data = 0;
        for refusal in Refusal::ALL {
            match refusal.side() {
                Side::Window => window += 1,
                Side::Server => server += 1,
                Side::Data => data += 1,
            }
        }
        // Cinq cotes fenetre, trois cotes serveur, quatre cotes donnee — plus `Quest`,
        // dont le representant est une ligne hors bornes, donc cote fenetre.
        assert_eq!((window, server, data), (6, 3, 4));
        // Les trois refus serveur sont exactement ceux du message et du gestionnaire.
        let served: Vec<&str> = Refusal::ALL
            .iter()
            .filter(|refusal| refusal.side() == Side::Server)
            .map(|refusal| refusal.name())
            .collect();
        assert_eq!(
            served,
            [
                "ui14.refusal.no_permission",
                "ui14.refusal.target_gone",
                "ui14.refusal.origin_unconnected"
            ]
        );
    }

    #[test]
    fn no_refusal_has_a_player_facing_key_and_every_name_is_namespaced() {
        for refusal in Refusal::ALL {
            assert_eq!(refusal.key(), None, "{} parle au joueur", refusal.name());
            assert!(
                refusal.name().starts_with("ui14.refusal.")
                    || refusal.name().starts_with("ui13.refusal."),
                "{} n'est pas prefixe",
                refusal.name()
            );
        }
        // Les douze noms propres sont distincts ; `Quest` rend celui de l'autre table.
        let mut names: Vec<&str> = Refusal::ALL.iter().map(|refusal| refusal.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 13, "un refus renomme en cache un autre");
        assert_eq!(Refusal::Quest(Ui13::NoRow).name(), "ui13.refusal.no_row");
        assert_eq!(Refusal::Quest(Ui13::UnknownTemplate).name(), "ui13.refusal.unknown_template");
        assert_eq!(Refusal::NotShown.to_string(), "ui14.refusal.button_hidden");
        let as_error: &dyn std::error::Error = &Refusal::NameTooLong { len: 32_768 };
        assert_eq!(as_error.to_string(), "ui14.refusal.name_too_long");
    }

    #[test]
    fn the_buttons_that_send_carry_the_refusals_that_swallow_the_message() {
        // Les quatre boutons d'alliance nomment un envoi et trois refus serveur : c'est
        // la difference avec les sept boutons de `ui_window_13`, qui n'envoient rien.
        for ally in alliance::ALLIES {
            assert_eq!(ally.effect(), crate::ui::action::Effect::Sends(alliance::MESSAGE));
            let sides: Vec<Side> = ally.refusals().iter().map(|refusal| refusal.side()).collect();
            assert!(sides.contains(&Side::Server), "{} n'a pas de refus serveur", ally.id());
            assert!(sides.contains(&Side::Window), "{} ne se cache jamais", ally.id());
        }
        // Le localisateur du journal, lui, n'envoie rien : il surligne.
        let locator = crate::ui_window_13::quest_log::AVAILABLE_BUTTONS[0];
        assert_eq!(locator.effect, crate::ui::action::Effect::Highlights);
        assert!(!locator.sends_message());
    }

    #[test]
    fn five_distinct_buttons_are_four_sends_and_one_locator_drawn_twice() {
        assert_eq!(alliance::ALLIES.len(), 4);
        // Le localisateur porte **un** id, dessine sur deux pages (« en cours » et
        // « disponible », `windowquestlog.xml:26`, `:38`) : deux boutons au comptage des
        // pages, un seul au comptage des gestes.
        let drawn: usize = crate::ui_window_13::quest_log::Page::ALL
            .iter()
            .map(|page| page.buttons().len())
            .sum();
        assert_eq!(drawn, 2, "le meme bouton dans deux listes");
        assert_eq!(1 + alliance::ALLIES.len(), 5, "un localisateur + quatre boutons");
        // Les deux fleches de page ne sont pas de ce compte : `AbstractWindowSkeleton`
        // les enregistre, pas la classe du journal (`:77-88`).
        assert_eq!(crate::ui_window_13::quest_log::SHELL_BUTTONS.len(), 2);
        // Les panneaux, eux, n'emploient que quatre des douze valeurs de `src/ui`.
        let mut values: Vec<Value> =
            alliance::FIELDS.iter().map(|field| field.value).collect();
        values.sort_unstable();
        values.dedup();
        assert_eq!(values.len(), 4, "quatre valeurs sur douze");
    }
}
