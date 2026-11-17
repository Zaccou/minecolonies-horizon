//! Un panneau : ce qu'il porte, d'ou vient sa valeur, et la ligne qui le declare.
//!
//! **Meme forme que `src/ui`** : `(id, libelle, valeur, source, portee)` est
//! `crate::ui::field::Field`, et [`Pane::to_ui`] le rend tel quel quand la source existe
//! la-bas. Deux choses sont propres a ce module : [`Pane::declared`], le XML et la ligne
//! qui **creent** l'id, et les trois sources que `src/ui` n'a pas — le module de
//! reglages de la colonie n'est pas un module de batiment, et deux valeurs de ces quatre
//! fenetres n'existent nulle part dans un modele : elles arrivent dans le message qui
//! ouvre la fenetre.
//!
//! Java reference : `core/client/gui/townhall/` — les quatre sources du brief plus
//! `AbstractWindowTownHall`, dont les deux pages heritent, et les quatre XML sous
//! `src/main/resources/assets/minecolonies/gui/townhall/`.
//!
//! **Ce module ne porte aucun nom et aucun niveau.** La troisieme premisse du brief est
//! fausse pour ces quatre fenetres : `AbstractBuildingMainWindow` cherche `name`
//! (`:53`) et `build` (`:54`) et les **verifie avant d'ecrire** (`:135`, `:171`), donc
//! les deux sont simplement absents des quatre XML d'ici. Le seul ecran de l'hotel de
//! ville qui porte le titre et le bouton de construction est celui des actions
//! (`layoutactions.xml:6`, `:15`), que ce module ne decrit pas.

use crate::ui::field::{Field, Scope, Source as UiSource, Value};

/// Un panneau d'une des quatre fenetres de ce module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pane {
    /// L'`id` de l'XML, seule prise que la classe a sur le panneau
    /// (`findPaneOfTypeByID`, `WindowSettings.java:30`).
    pub id: &'static str,
    /// La cle du libelle dessine dans un panneau **voisin sans id**, quand il y en a un :
    /// `None` presque partout, la ligne des habitants exceptee
    /// (`layoutstats.xml:3`, un `text` sans id).
    pub label: Option<&'static str>,
    /// Ce que le panneau porte.
    pub value: Value,
    /// D'ou vient la valeur.
    pub source: Source,
    /// La fenetre, ou le panneau parent.
    pub scope: Scope,
    /// Le fichier XML et la ligne qui declarent l'`id`, 1-based.
    pub declared: (&'static str, u32),
}

impl Pane {
    /// Un panneau de la fenetre elle-meme.
    pub const fn new(
        id: &'static str,
        value: Value,
        source: Source,
        declared: (&'static str, u32),
    ) -> Pane {
        Pane { id, label: None, value, source, scope: Scope::Window, declared }
    }

    /// Un panneau de la fenetre qui porte un libelle voisin sans id.
    pub const fn labelled(
        id: &'static str,
        label: &'static str,
        value: Value,
        source: Source,
        declared: (&'static str, u32),
    ) -> Pane {
        Pane { id, label: Some(label), value, source, scope: Scope::Window, declared }
    }

    /// Un panneau **dans** un autre panneau nomme.
    ///
    /// Deux especes de parents, et la portee les separe pareillement : une ligne de
    /// liste (`citizen-stats`, `layoutstats.xml:7`) et la `view` d'un reglage
    /// (`job`, `layoutsettings.xml:9-13`). Les deux partagent l'id `trigger` et
    /// `desc` respectivement, ce qui serait une collision sans la portee
    /// (`layoutsettings.xml:22`, `:28` : deux lignes nomment leur `text` `desc`).
    pub const fn inside(
        parent: &'static str,
        id: &'static str,
        value: Value,
        source: Source,
        declared: (&'static str, u32),
    ) -> Pane {
        Pane { id, label: None, value, source, scope: Scope::Row(parent), declared }
    }

    /// Vrai quand le panneau appartient a un panneau parent plutot qu'a la fenetre.
    pub fn is_nested(&self) -> bool {
        matches!(self.scope, Scope::Row(_))
    }

    /// Le panneau sous la forme de `src/ui` ; `None` pour les trois sources qui n'y
    /// sont pas.
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

/// D'ou la valeur d'un panneau vient.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: les huit lecteurs de ces quatre fenetres, et aucun autre
pub enum Source {
    /// Le gestionnaire de statistiques de la colonie
    /// (`WindowStatsPage.java:229`, `:251`) — **de la colonie**, pas d'un batiment :
    /// l'onglet d'un batiment lit un autre gestionnaire (`BuildingStatisticsModuleView`).
    Colony,
    /// Le module de reglages **de la colonie**
    /// (`WindowSettings.java:27`, `colony.getSettings()`), cle `townhall_settings`
    /// (`BuildingModules.java:526`). Ce n'est pas un module du batiment : deux hotels
    /// de ville d'une meme colonie lisent le meme.
    Settings,
    /// Un citoyen de la vue colonie (`WindowStatsPage.java:161`).
    Citizen,
    /// Le message qui a ouvert la fenetre : le nom et la distance de la colonie la plus
    /// proche (`OpenColonyFoundingCovenantMessage.java:29`, `:40`) et le texte d'alerte
    /// (`OpenCantFoundColonyWarningMessage.java:39`, `:50`). Aucun modele ne le porte.
    Packet,
    /// Le joueur lui-meme, lu pour le nom par defaut de la colonie
    /// (`WindowTownHallColonyManage.java:54`, `mc.player.getName()`).
    Player,
    /// Le bloc de l'hotel de ville : son chemin de blueprint
    /// (`WindowTownHallColonyManage.java:83`).
    Hut,
    /// Le pack de structure selectionne, un `static` du client
    /// (`StructurePacks.selectedPack`, `WindowTownHallColonyManage.java:82`).
    StructurePack,
    /// L'horloge du monde : l'age du monde en ticks, lu pour dater le pacte
    /// (`WindowTownHallColonyManage.java:55`, `level.getGameTime()`). Le port n'a pas
    /// d'horloge (`crate::advancements::stats` prend le jour en argument pour la meme
    /// raison), donc cette valeur vient de l'appelant.
    Clock,
}

impl Source {
    /// Un nom stable, pour un log ou un test.
    pub fn name(self) -> &'static str {
        match self {
            Source::Colony => "colony",
            Source::Settings => "settings",
            Source::Citizen => "citizen",
            Source::Packet => "packet",
            Source::Player => "player",
            Source::Hut => "hut",
            Source::StructurePack => "structure_pack",
            Source::Clock => "clock",
        }
    }

    /// La source de `src/ui`, quand elle y est.
    pub fn to_ui(self) -> Option<UiSource> {
        match self {
            Source::Colony => Some(UiSource::Colony),
            Source::Settings => Some(UiSource::Module(super::settings::MODULE_KEY)),
            Source::Citizen => Some(UiSource::Citizen),
            Source::Hut => Some(UiSource::Hut),
            Source::Packet | Source::Player | Source::StructurePack | Source::Clock => None,
        }
    }

    /// Vrai quand la valeur vit dans la colonie. Le module de reglages **de la colonie**
    /// en est : c'est le seul panneau de ce module dont la valeur survit a la fermeture
    /// de la fenetre et que le serveur possede.
    pub fn is_colony_data(self) -> bool {
        matches!(self, Source::Colony | Source::Settings | Source::Citizen)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_sources_convert_to_src_ui_and_the_four_arrivals_do_not() {
        let convertible: Vec<&str> = [
            Source::Colony,
            Source::Settings,
            Source::Citizen,
            Source::Hut,
            Source::Packet,
            Source::Player,
            Source::StructurePack,
            Source::Clock,
        ]
        .iter()
        .filter(|source| source.to_ui().is_some())
        .map(|source| source.name())
        .collect();
        assert_eq!(convertible, ["colony", "settings", "citizen", "hut"]);
        // Le message, le joueur, le pack et l'horloge n'ont pas de source dans `src/ui` :
        // les ranger sous `Colony` serait faux, ils n'y vivent pas.
        assert_eq!(Source::Packet.to_ui(), None);
        assert_eq!(Source::Player.to_ui(), None);
        assert_eq!(Source::StructurePack.to_ui(), None);
        assert_eq!(Source::Clock.to_ui(), None);
        // Le module nomme est celui de la colonie, pas un module de batiment.
        assert_eq!(
            Source::Settings.to_ui(),
            Some(UiSource::Module("townhall_settings")),
            "BuildingModules.java:526"
        );
        // L'horloge n'est pas une donnee de colonie : elle ne survit a rien.
        assert!(!Source::Clock.is_colony_data());
        assert!(Source::Settings.is_colony_data());
    }

    #[test]
    fn a_pane_keeps_its_parent_and_the_line_that_declares_it() {
        let list = Pane::new(
            "citizen-stats",
            Value::Row,
            Source::Colony,
            ("layoutstats.xml", 5),
        );
        assert!(!list.is_nested());
        assert_eq!(list.declared, ("layoutstats.xml", 5));
        let row = Pane::inside(
            "citizen-stats",
            "citizensAmount",
            Value::Text,
            Source::Colony,
            ("layoutstats.xml", 7),
        );
        assert!(row.is_nested());
        assert_eq!(row.scope, Scope::Row("citizen-stats"));
        // Deux lignes de reglages nomment leur texte `desc` : la portee les separe.
        let tape = Pane::inside(
            "tape",
            "desc",
            Value::Text,
            Source::Settings,
            ("layoutsettings.xml", 28),
        );
        let enter = Pane::inside(
            "entermessages",
            "desc",
            Value::Text,
            Source::Settings,
            ("layoutsettings.xml", 22),
        );
        assert_eq!(tape.id, enter.id);
        assert_ne!(tape.scope, enter.scope);
        // Le panneau d'ici devient celui de src/ui, sauf les trois sources absentes.
        let ui = list.to_ui().expect("la colonie est une Source de src/ui");
        assert_eq!((ui.id, ui.value, ui.source), (list.id, list.value, UiSource::Colony));
        let packet = Pane::new("text3", Value::Text, Source::Packet, ("windowcolonymanagement.xml", 16));
        assert_eq!(packet.to_ui(), None);
        // La ligne des habitants porte son libelle, les autres non.
        let citizens = Pane::labelled(
            "totalCitizens",
            "com.minecolonies.coremod.gui.townhall.population.totalcitizens",
            Value::Occupancy,
            Source::Colony,
            ("layoutstats.xml", 4),
        );
        assert_eq!(citizens.label, Some("com.minecolonies.coremod.gui.townhall.population.totalcitizens"));
        assert!(citizens.value.is_computed(), "un n/max se calcule, il ne se lit pas");
        assert_eq!(list.label, None);
    }
}
