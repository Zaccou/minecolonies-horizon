//! Un panneau : ce qu'il porte, d'ou vient sa valeur, et la ligne qui le declare.
//!
//! **Meme forme que `src/ui`** : `(id, libelle, valeur, source, portee)` est
//! `crate::ui::field::Field`, et [`Pane::to_ui`] le rend tel quel quand la source
//! existe la-bas. Deux choses sont propres a ce module : [`Pane::declared`], le XML et
//! la ligne qui **creent** l'id, et [`Source::QuestTemplates`], le seul lecteur que
//! `src/ui` n'a pas. Le nom d'une quete ne vient pas de la colonie mais de
//! `IQuestManager.GLOBAL_SERVER_QUESTS` (`IQuestManager.java:22`), un `static` du
//! client : [`Source::to_ui`] repond `None` plutot que de le ranger sous
//! `Source::Colony`, qui serait faux.

use crate::ui::field::{Field, Scope, Source as UiSource, Value};

/// Un panneau d'une des fenetres de ce module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pane {
    /// L'`id` de l'XML, seule prise que la classe a sur le panneau
    /// (`findPaneOfTypeByID`, `WindowSelectRequest.java:170`).
    pub id: &'static str,
    /// La cle du libelle ecrit a cote, quand il y en a un dans le meme panneau :
    /// `None` partout ici, les trois XML posant leurs titres dans un `text` sans id.
    pub label: Option<&'static str>,
    pub value: Value,
    pub source: Source,
    /// La fenetre (ou la page), ou une ligne de liste.
    pub scope: Scope,
    /// Le fichier XML et la ligne qui declarent l'`id`, 1-based.
    pub declared: (&'static str, u32),
}

impl Pane {
    /// Un panneau de la fenetre (ou de la page) elle-meme.
    pub const fn new(
        id: &'static str,
        value: Value,
        source: Source,
        declared: (&'static str, u32),
    ) -> Pane {
        Pane { id, label: None, value, source, scope: Scope::Window, declared }
    }

    /// Un panneau d'une ligne de `list`.
    ///
    /// La portee porte l'id de la liste precede de l'id de la page quand trois listes
    /// partagent un id : `windowquestlog.xml:21`, `:34` et `:46` nomment toutes trois
    /// leur liste `quests`, sans quoi trois lignes `questName` seraient un panneau.
    pub const fn in_row(
        list: &'static str,
        id: &'static str,
        value: Value,
        source: Source,
        declared: (&'static str, u32),
    ) -> Pane {
        Pane { id, label: None, value, source, scope: Scope::Row(list), declared }
    }

    /// Vrai quand le panneau appartient a une ligne de liste.
    pub fn is_row_pane(&self) -> bool {
        matches!(self.scope, Scope::Row(_))
    }

    /// Le panneau sous la forme de `src/ui` ; `None` pour [`Source::QuestTemplates`]
    /// seulement.
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
// exhaustive: les cinq lecteurs de ces quatre sources, et aucun autre
pub enum Source {
    /// Le gestionnaire de requetes de la colonie, par ses deux resolveurs
    /// (`WindowSelectRequest.java:89-94`).
    Requests,
    /// Le gestionnaire de statistiques **du batiment**
    /// (`BuildingStatisticsModuleView.java:52`, `WindowStatsModule.java:104`).
    BuildingStats,
    /// Le gestionnaire de quetes de la colonie, ses trois listes
    /// (`IQuestManager.java:94`, `:101`, `:108`).
    Quests,
    /// `IQuestManager.GLOBAL_SERVER_QUESTS` (`IQuestManager.java:22`), lu pour le nom
    /// d'une quete (`WindowQuestLogAvailableQuestModule.java:35`).
    QuestTemplates,
    /// Un citoyen de la vue colonie, par son id (`:77`).
    Citizen,
}

impl Source {
    /// Un nom stable, pour un log ou un test.
    pub fn name(self) -> &'static str {
        match self {
            Source::Requests => "requests",
            Source::BuildingStats => "building_stats",
            Source::Quests => "quests",
            Source::QuestTemplates => "quest_templates",
            Source::Citizen => "citizen",
        }
    }

    /// La source de `src/ui`, sauf le registre de modeles.
    pub fn to_ui(self) -> Option<UiSource> {
        match self {
            Source::Requests => Some(UiSource::Request),
            Source::BuildingStats => Some(UiSource::Module(super::stats::MODULE)),
            Source::Quests => Some(UiSource::Colony),
            Source::Citizen => Some(UiSource::Citizen),
            Source::QuestTemplates => None,
        }
    }

    /// Vrai quand la valeur vit dans la colonie : les statistiques sont celles d'un
    /// **batiment**, les modeles sont un `static` du client.
    pub fn is_colony_data(self) -> bool {
        matches!(self, Source::Requests | Source::Quests | Source::Citizen)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_sources_convert_to_src_ui_and_the_quest_registry_does_not() {
        let convertible: Vec<&str> = [
            Source::Requests,
            Source::BuildingStats,
            Source::Quests,
            Source::Citizen,
            Source::QuestTemplates,
        ]
        .iter()
        .filter(|source| source.to_ui().is_some())
        .map(|source| source.name())
        .collect();
        assert_eq!(convertible, ["requests", "building_stats", "quests", "citizen"]);
        assert_eq!(Source::QuestTemplates.to_ui(), None);
        assert_eq!(
            Source::BuildingStats.to_ui(),
            Some(UiSource::Module("stats_module")),
            "BuildingModules.java:63 : la conversion ne renomme pas"
        );
        assert_eq!(Source::Requests.to_ui(), Some(UiSource::Request));
        assert_eq!(Source::Quests.to_ui(), Some(UiSource::Colony));
        assert_eq!(Source::Citizen.to_ui(), Some(UiSource::Citizen));
    }

    #[test]
    fn a_pane_keeps_its_scope_and_the_line_that_declares_it() {
        let list = Pane::new("requests", Value::Row, Source::Requests, ("layoutselectrequest.xml", 9));
        assert!(!list.is_row_pane());
        let row = Pane::in_row(
            super::super::quest_log::AVAILABLE_ROWS,
            "questName",
            Value::Text,
            Source::QuestTemplates,
            ("windowquestlog.xml", 36),
        );
        assert!(row.is_row_pane());
        assert_eq!(row.declared, ("windowquestlog.xml", 36));
        // Le panneau d'ici devient celui de src/ui, sauf celui dont la source manque.
        let ui = list.to_ui().expect("les requetes sont une Source de src/ui");
        assert_eq!((ui.id, ui.value, ui.source), (list.id, list.value, UiSource::Request));
        assert_eq!(row.to_ui(), None);
        // La reponse de src/ui sur la colonie, tenue sur les cinq variantes.
        for source in [Source::Requests, Source::Quests, Source::Citizen] {
            let ui = source.to_ui().expect("convertible");
            assert_eq!(source.is_colony_data(), ui.is_colony_data(), "{}", source.name());
        }
        assert!(!Source::BuildingStats.is_colony_data());
        assert!(!Source::QuestTemplates.is_colony_data());
    }
}
