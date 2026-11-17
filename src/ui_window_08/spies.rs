//! `WindowsBarracksSpies` : cinq lingots d'or contre des espions, et trois façons de ne
//! pas pouvoir les payer.
//!
//! Java reference : `core/client/gui/WindowsBarracksSpies.java` (97 lignes),
//! `assets/minecolonies/gui/windowbarracksspies.xml:1-17`, et les deux bouts serveur qui
//! décident vraiment — `core/colony/buildings/workerbuildings/BuildingBarracks.java:136-159`
//! (le tick qui paie tout seul) et
//! `core/network/messages/server/colony/HireSpiesMessage.java:29-44` (le clic).
//!
//! # La fenêtre compte deux inventaires, le serveur n'en compte qu'un
//!
//! Le bouton s'ouvre si le joueur **et le rack de la caserne** totalisent cinq lingots
//! (`WindowsBarracksSpies.java:66-71`). Le serveur, lui, ne regarde que l'inventaire du
//! joueur (`HireSpiesMessage.java:38`) : un joueur qui n'a rien sur lui et cinq lingots
//! dans le rack voit un bouton actif, clique, et **rien ne se passe, sans un mot**. La
//! description de la fenêtre promet pourtant l'inverse — « mettez l'or dans l'inventaire
//! de la caserne pour les payer automatiquement » (`gui.barracks.spies.desc`) — et elle a
//! raison, mais par un autre chemin : le tick de la caserne.
//!
//! # Le bouton est presque toujours inutile, et c'est voulu par le tick
//!
//! `BuildingBarracks.onColonyTick` (`:136-159`) paie **lui-même** depuis le rack à chaque
//! tick de colonie tant que le raid dure et que les espions ne sont pas en place. Il n'y a
//! donc pas besoin de cliquer : cinq lingots dans le rack suffisent, et le bouton ne
//! devient utile que pour payer depuis l'inventaire du joueur. Le même tick remet
//! `spiesEnabled` à faux dès que le raid est fini (`:155-158`), donc le refus
//! [`Refusal::SpiesAlreadyEnabled`] n'est jamais définitif.
//!
//! # Ce que ce module ne refait pas
//!
//! Le niveau de la caserne est vérifié **avant** cette fenêtre : sous le niveau trois, le
//! bouton et son icône sont **cachés** dans l'écran de la hutte
//! (`WindowBarracksBuilding.java:92-96`), ce que `src/ui` nomme déjà
//! `Refusal::SpiesBelowLevel` sur l'action `hireSpies`. Cette fenêtre-ci ne teste aucun
//! niveau : elle est ouverte, ou elle ne l'est pas.

use crate::ui::Value;

use super::pane::{Pane, Source};
use super::refusal::Refusal;
use super::{Button, Effect, Naming};

/// La classe Java.
pub const JAVA: &str = "WindowsBarracksSpies";
/// Le XML, chargé au constructeur (`WindowsBarracksSpies.java:60`).
pub const RESOURCE: &str = "minecolonies:gui/windowbarracksspies.xml";

/// L'icône de l'or (`windowbarracksspies.xml:10`, remplie à `:63`).
pub const ICON: &str = "hireSpiesIcon";
/// Le texte `x5` (`windowbarracksspies.xml:11`, écrit à `:64`).
pub const AMOUNT: &str = "amount";
/// La description du paiement (`windowbarracksspies.xml:9`, écrite à `:75`).
pub const TEXT: &str = "text";
/// Le bouton qui paie (`windowbarracksspies.xml:12-16`, traité à `:88-94`).
pub const BUTTON_HIRE: &str = "hireSpies";
/// La croix (`windowbarracksspies.xml:7`, traitée à `:83-87`).
pub const BUTTON_CANCEL: &str = "cancel";

/// Le libellé du bouton, écrit dans le XML (`windowbarracksspies.xml:16`).
pub const HIRE_LABEL_KEY: &str = "com.minecolonies.coremod.gui.barracks.spieshire";
/// Le titre de la fenêtre de la caserne qui ouvre celle-ci : `src/ui` le porte déjà
/// (`gui/windowhutbarracks.xml:16-18`), il est cité ici pour la jointure.
pub const SPIES_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.spies";
/// La description (`TranslationConstants.java:498`, écrite à `:75`).
pub const DESCRIPTION_KEY: &str = "com.minecolonies.coremod.gui.barracks.spies.desc";
/// L'item payé.
pub const GOLD_ITEM: &str = "minecraft:gold_ingot";

/// Le prix écrit **dans la fenêtre** (`WindowsBarracksSpies.java:51`).
pub const GOLD_COST: i32 = 5;
/// Le prix que le serveur applique, et que le tick de la caserne lit
/// (`BuildingBarracks.java:65`). C'est un `public static int`, pas un `final`.
pub const SPIES_GOLD_COST: i32 = 5;
/// Le texte du panneau `amount`, écrit en dur (`:64`) : la fenêtre ne calcule pas ce
/// nombre à partir de [`GOLD_COST`].
pub const AMOUNT_TEXT: &str = "x5";
/// Le niveau de caserne sous lequel l'écran de la hutte **cache** le bouton
/// (`WindowBarracksBuilding.java:92-96`).
pub const BARRACKS_LEVEL_FOR_BUTTON: i32 = 3;

/// Les trois panneaux nommés de la fenêtre : l'icône, le `x5`, la description.
pub const FIELDS: &[Pane] = &[
    Pane::new(ICON, Value::Icon, Source::Window),
    Pane::new(AMOUNT, Value::Text, Source::Window),
    Pane::new(TEXT, Value::Text, Source::Window),
];

/// Les deux boutons. La croix n'a pas de libellé : c'est une image
/// (`windowbarracksspies.xml:7`, pas d'attribut `label`).
pub const BUTTONS: &[Button] = &[
    Button {
        id: BUTTON_CANCEL,
        label: "",
        effect: Effect::Closes,
        refusals: &[],
        naming: Naming::Xml,
    },
    Button {
        id: BUTTON_HIRE,
        label: HIRE_LABEL_KEY,
        effect: Effect::Sends("HireSpiesMessage"),
        // `:71`, dans l'ordre du `||` de Java.
        refusals: &[
            Refusal::NotRaiding,
            Refusal::NotEnoughGold { have: 0, need: GOLD_COST },
            Refusal::SpiesAlreadyEnabled,
        ],
        naming: Naming::Xml,
    },
];

/// Ce que la fenêtre lit pour décider : les deux drapeaux de la colonie et les deux
/// inventaires de lingots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    /// `colony.isRaiding()` (`:71`), la copie cliente de `RaiderManager.isRaided()`
    /// écrite à la dernière synchronisation (`ColonyView.java:364`, `:735`).
    pub raiding: bool,
    /// `colony.areSpiesEnabled()` (`:71`).
    pub spies_enabled: bool,
    /// L'or de l'inventaire du joueur (`:68`).
    pub player_gold: i32,
    /// L'or du rack de la caserne (`:69`).
    pub rack_gold: i32,
}

impl State {
    /// L'état que la fenêtre lit à l'ouverture.
    pub const fn new(
        raiding: bool,
        spies_enabled: bool,
        player_gold: i32,
        rack_gold: i32,
    ) -> State {
        State { raiding, spies_enabled, player_gold, rack_gold }
    }

    /// Le compte de la fenêtre : les deux inventaires additionnés (`:68-69`).
    pub fn gold(self) -> i32 {
        self.player_gold + self.rack_gold
    }

    /// La porte du bouton `hireSpies` (`:71`) : `!isRaiding() || gold < GOLD_COST ||
    /// areSpiesEnabled()` ferme le bouton.
    ///
    /// **Une seule de ces trois raisons est montrée** : Java appelle `disable()` et
    /// n'écrit aucun texte, d'où trois refus sans clé (voir [`Refusal::key`]).
    pub fn gate(self) -> Result<(), Refusal> {
        if !self.raiding {
            return Err(Refusal::NotRaiding);
        }
        if self.gold() < GOLD_COST {
            return Err(Refusal::NotEnoughGold { have: self.gold(), need: GOLD_COST });
        }
        if self.spies_enabled {
            return Err(Refusal::SpiesAlreadyEnabled);
        }
        Ok(())
    }

    /// L'état du bouton : Java le `disable()` exactement quand [`Self::gate`] refuse.
    pub fn button_enabled(self) -> bool {
        self.gate().is_ok()
    }

    /// Le clic sur `hireSpies` (`:88-94` puis `HireSpiesMessage.java:29-44`).
    pub fn click(self) -> Outcome {
        if let Err(refusal) = self.gate() {
            return Outcome::Refused(refusal);
        }
        if self.player_gold >= SPIES_GOLD_COST {
            Outcome::Hired
        } else {
            Outcome::IgnoredByServer
        }
    }

    /// Un tick de colonie de la caserne (`BuildingBarracks.java:136-159`), dans l'ordre
    /// de Java : raid fini d'abord, paiement automatique ensuite.
    pub fn tick(&mut self) -> Transition {
        if !self.raiding {
            if self.spies_enabled {
                self.spies_enabled = false;
                return Transition::SpiesDisabled;
            }
            return Transition::Nothing;
        }
        if self.spies_enabled || self.rack_gold < SPIES_GOLD_COST {
            return Transition::Nothing;
        }
        self.rack_gold -= SPIES_GOLD_COST;
        self.spies_enabled = true;
        Transition::PaidFromRack { spent: SPIES_GOLD_COST, left: self.rack_gold }
    }
}

/// Ce qu'un clic sur `hireSpies` obtient.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: trois issues, celles de Java
pub enum Outcome {
    /// La porte a refusé : rien n'est envoyé (`:73` a fermé le bouton).
    Refused(Refusal),
    /// Le serveur a prélevé cinq lingots chez le joueur et armé les espions
    /// (`HireSpiesMessage.java:38-43`).
    Hired,
    /// Le clic est bien parti, mais le serveur n'a pas trouvé cinq lingots **chez le
    /// joueur** : la fenêtre comptait le rack, le serveur ne le compte pas
    /// (`HireSpiesMessage.java:38`). Aucun message, aucun son.
    IgnoredByServer,
}

/// Ce qu'un tick de colonie change sans qu'aucun joueur n'ait cliqué.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les trois branches de BuildingBarracks.onColonyTick
pub enum Transition {
    /// Rien : pas de raid et pas d'espions, ou raid avec espions, ou rack trop pauvre.
    Nothing,
    /// Les espions ont été payés **depuis le rack** (`:144-153`), sans clic.
    PaidFromRack { spent: i32, left: i32 },
    /// Le raid est fini : les espions sont remis à zéro (`:155-158`).
    SpiesDisabled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_button_opens_only_during_a_raid_with_five_gold_and_no_spies_yet() {
        // Le cas qui marche.
        assert!(State::new(true, false, 5, 0).button_enabled());
        // Les trois refus, un par branche du `||` de `:71`.
        assert_eq!(State::new(false, false, 5, 0).gate(), Err(Refusal::NotRaiding));
        assert_eq!(
            State::new(true, false, 2, 2).gate(),
            Err(Refusal::NotEnoughGold { have: 4, need: 5 })
        );
        assert_eq!(
            State::new(true, true, 9, 9).gate(),
            Err(Refusal::SpiesAlreadyEnabled)
        );
        // La somme des deux inventaires suffit à ouvrir le bouton.
        assert!(State::new(true, false, 1, 4).button_enabled());
        assert_eq!(State::new(true, false, 1, 4).gold(), 5);
    }

    #[test]
    fn the_window_counts_rack_gold_and_the_server_does_not() {
        // Cinq lingots dans le rack, rien sur le joueur : la fenêtre ouvre le bouton…
        let rack_only = State::new(true, false, 0, 5);
        assert!(rack_only.button_enabled());
        // …et le clic est accepté par la fenêtre mais ignoré par le serveur.
        assert_eq!(rack_only.click(), Outcome::IgnoredByServer);
        // Le refus n'est pas nommé par la fenêtre : c'est le serveur qui n'a rien dit.
        assert_eq!(Refusal::NotEnoughGold { have: 5, need: 5 }.key(), None);
    }

    #[test]
    fn a_click_pays_from_the_player_and_never_from_the_rack() {
        let mut state = State::new(true, false, 7, 3);
        assert_eq!(state.click(), Outcome::Hired);
        // Le message ne touche pas au rack : c'est le tick qui s'en occupe.
        assert_eq!(state.rack_gold, 3);
        assert_eq!(state.player_gold, 7);
        // Un bouton fermé ne part pas.
        assert_eq!(
            State::new(false, false, 99, 99).click(),
            Outcome::Refused(Refusal::NotRaiding)
        );
        assert_eq!(
            State::new(true, true, 99, 99).click(),
            Outcome::Refused(Refusal::SpiesAlreadyEnabled)
        );
    }

    #[test]
    fn the_raid_tick_pays_from_the_rack_without_any_click() {
        // BuildingBarracks.java:144-153.
        let mut state = State::new(true, false, 0, 9);
        assert_eq!(state.tick(), Transition::PaidFromRack { spent: 5, left: 4 });
        assert!(state.spies_enabled);
        assert_eq!(state.rack_gold, 4);
        // Une fois payés, le tick ne repaie pas.
        assert_eq!(state.tick(), Transition::Nothing);
        assert_eq!(state.rack_gold, 4);
        // Quatre lingots ne suffisent pas.
        let mut poor = State::new(true, false, 0, 4);
        assert_eq!(poor.tick(), Transition::Nothing);
        assert!(!poor.spies_enabled);
    }

    #[test]
    fn the_spies_go_back_off_when_the_raid_ends() {
        // BuildingBarracks.java:155-158.
        let mut state = State::new(true, false, 0, 5);
        assert_eq!(state.tick(), Transition::PaidFromRack { spent: 5, left: 0 });
        state.raiding = false;
        assert_eq!(state.tick(), Transition::SpiesDisabled);
        assert!(!state.spies_enabled);
        // Et la remise à zéro ne se répète pas : le tick suivant ne change rien.
        assert_eq!(state.tick(), Transition::Nothing);
    }

    #[test]
    fn the_two_costs_are_two_constants_and_the_label_is_a_third_literal() {
        assert_eq!(GOLD_COST, 5, "WindowsBarracksSpies.java:51");
        assert_eq!(SPIES_GOLD_COST, 5, "BuildingBarracks.java:65");
        assert_eq!(GOLD_COST, SPIES_GOLD_COST);
        // Le `x5` n'est pas calculé : la fenêtre écrit un littéral (`:64`), donc changer
        // l'un des deux prix ne changerait pas le libellé.
        assert_eq!(AMOUNT_TEXT, "x5");
        assert_eq!(AMOUNT_TEXT, format!("x{GOLD_COST}"), "d'accord aujourd'hui, par hasard");
        // Le panneau `amount` ne dépend d'aucune vue : la classe l'écrit.
        let amount = FIELDS.iter().find(|pane| pane.id == AMOUNT);
        assert_eq!(amount.map(|pane| pane.value), Some(Value::Text));
        assert_eq!(amount.map(|pane| pane.source), Some(Source::Window));
    }
}
