//! Un panneau de l'une de ces quatre fenêtres : ce qu'il porte, et d'où vient sa valeur.
//!
//! **La forme est celle de `src/ui`, reprise telle quelle** : `(id, label, valeur, portée)`
//! est [`crate::ui::field::Field`], et [`Value`] comme [`Scope`] sont les types de `src/ui`.
//! [`Pane::from_ui`] / [`Pane::to_ui`] font l'aller-retour sans perdre un morceau, exactement
//! comme [`crate::ui_window_08::pane::Pane`] — **c'est la même convention, pas une
//! deuxième** : une fenêtre qui n'est pas un écran de hutte garde la forme de `src/ui` et
//! n'ouvre que sa liste de **sources**.
//!
//! | Ce que ces quatre fenêtres lisent | Fenêtre | Pourquoi `src/ui::Source` ne le dit pas |
//! | --- | --- | --- |
//! | le biome du bloc (`Minecraft.level.getBiome(pos)`, `WindowField.java:124-125`) | `WindowField` | les neuf sources de `src/ui` sont une colonie, une hutte, un module, un citoyen, un ordre de travail, une requête, un avancement, l'inventaire du client : pas le niveau |
//! | les tags de biome cultivable (`cropBiomeTags`, `:132-142`) | `WindowField` | un tag n'est pas une valeur d'un bâtiment : c'est une table du registre |
//! | le tableau des quatre largeurs (`TileEntityScarecrow.getFieldSize()`, `:184-187`) | `WindowField` | c'est l'entité du bloc, pas la hutte : `Source::Hut` est le composé sauvegardé de `crate::hut::State`, qui n'a pas de largeur de champ |
//! | l'extension de champ de la colonie (`FarmField.getSeed()`, `:157`, `:300`) | `WindowField` | une extension de bâtiment (`IBuildingExtension`) n'est ni un module ni un citoyen |
//! | les emplacements d'un conteneur (`container.slots`, `menu.craftMatrix`) | les trois `AbstractContainerScreen` | aucun des neuf `Source` de `src/ui` n'est un `container.slots` |
//! | une valeur que la classe écrit seule | `WindowField` (`dir-center`) | `ItemIcon` posé par le XML (`windowfield.xml:19`), que Java ne fait que cacher |
//!
//! Les sources que `src/ui` nomme déjà — la colonie, un citoyen, un module par clé — sont
//! [`Source::Ui`], pour qu'une clé de module ou un id de citoyen ne soit nommé qu'une fois
//! dans le crate.

use crate::ui::field::{Field, Scope, Value};

/// D'où vient la valeur d'un panneau.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: les six lecteurs que ce module ajoute, plus ceux de src/ui
pub enum Source {
    /// Un lecteur que `src/ui` nomme déjà : la colonie, un citoyen, une hutte, un module,
    /// l'inventaire du client.
    Ui(crate::ui::Source),
    /// Les emplacements d'un `AbstractContainerMenu` : `container.slots`,
    /// `menu.craftMatrix`, `menu.craftResult`.
    Container,
    /// L'entité du bloc ouvert, ici `TileEntityScarecrow` : ses quatre largeurs et sa
    /// position (`WindowField.java:184-190`).
    BlockEntity,
    /// Le biome du niveau à la position du bloc (`:124-125`) et le test
    /// d'appartenance aux tags de culture (`:134`).
    Biome,
    /// La liste des tags de biome cultivables (`cropBiomeTags`,
    /// `ModTags.java`), que la fenêtre parcourt pour écrire le climat (`:132-142`).
    Tag,
    /// L'extension de champ de la colonie (`FarmField`) : la graine choisie
    /// (`:157`, `:300`) et le bâtiment qui l'exploite (`:272`).
    Extension,
    /// Une valeur que la classe écrit elle-même, sans la demander à personne : l'icône
    /// constante du centre (`windowfield.xml:19`).
    Window,
}

impl Source {
    /// La colonie derrière le bloc (`IColonyView`, `WindowField.java:341`).
    pub const COLONY: Source = Source::Ui(crate::ui::Source::Colony);
    /// Un citoyen (`ICitizen`) : le nom de l'exploitant du champ (`:284-290`).
    pub const CITIZEN: Source = Source::Ui(crate::ui::Source::Citizen);

    /// La vue d'un module, par **clé de module** — la même chaîne que
    /// `crate::building::Module::key` sauvegarde.
    pub const fn module(key: &'static str) -> Source {
        Source::Ui(crate::ui::Source::Module(key))
    }

    /// Un nom stable, pour une ligne de log ou un test.
    pub fn name(self) -> &'static str {
        match self {
            Source::Ui(crate::ui::Source::Building) => "ui.building",
            Source::Ui(crate::ui::Source::Colony) => "ui.colony",
            Source::Ui(crate::ui::Source::Citizen) => "ui.citizen",
            Source::Ui(crate::ui::Source::Hut) => "ui.hut",
            Source::Ui(crate::ui::Source::Module(_)) => "ui.module",
            Source::Ui(crate::ui::Source::PlayerInventory) => "ui.player_inventory",
            Source::Ui(crate::ui::Source::WorkOrder) => "ui.work_order",
            Source::Ui(crate::ui::Source::Request) => "ui.request",
            Source::Ui(crate::ui::Source::Advancement) => "ui.advancement",
            Source::Container => "container",
            Source::BlockEntity => "block_entity",
            Source::Biome => "biome",
            Source::Tag => "tag",
            Source::Extension => "extension",
            Source::Window => "window",
        }
    }

    /// Vrai pour les six lecteurs que ce module a dû ajouter.
    pub fn is_ported_here(self) -> bool {
        !matches!(self, Source::Ui(_))
    }

    /// Vrai quand la valeur vit dans la colonie plutôt que dans le bloc ou la fenêtre.
    ///
    /// Les six lecteurs ajoutés répondent `false` sauf [`Source::Extension`] : une
    /// extension de champ **est** de la donnée de colonie, mais elle n'est pas une des
    /// trois formes que `src/ui` appelle « donnée de colonie » (colonie, citoyen,
    /// requête), donc elle est nommée ici et classée explicitement.
    pub fn is_colony_data(self) -> bool {
        match self {
            Source::Ui(inner) => inner.is_colony_data(),
            Source::Extension => true,
            Source::Container | Source::BlockEntity | Source::Biome | Source::Tag | Source::Window => {
                false
            }
        }
    }
}

/// Un panneau de l'une de ces quatre fenêtres.
///
/// `id` est l'attribut `id` du XML, ou — pour les trois écrans de conteneur qui n'ont pas
/// de XML — l'id que Java donne au widget qu'il crée. **Un texte que `renderLabels` dessine
/// n'a pas d'id et n'est donc pas un [`Pane`]** : le titre de `WindowCrafting` est
/// `stack.drawString(… "container.crafting" …)` (`:255`), un appel de dessin, et il est
/// décrit par [`crate::ui_window_09::Title`] au lieu d'être déguisé ici en panneau nommé.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pane {
    /// L'id du panneau, dans le XML.
    pub id: &'static str,
    /// La clé du libellé écrit à côté de cette valeur, quand il y en a un.
    pub label: Option<&'static str>,
    /// Ce que le panneau porte.
    pub value: Value,
    /// D'où vient la valeur.
    pub source: Source,
    /// Si le panneau est celui de la fenêtre ou d'une ligne de liste.
    pub scope: Scope,
}

impl Pane {
    /// Un panneau de la fenêtre, sans libellé propre.
    pub const fn new(id: &'static str, value: Value, source: Source) -> Pane {
        Pane { id, label: None, value, source, scope: Scope::Window }
    }

    /// Un panneau de la fenêtre qui porte un libellé.
    pub const fn labelled(
        id: &'static str,
        label: &'static str,
        value: Value,
        source: Source,
    ) -> Pane {
        Pane { id, label: Some(label), value, source, scope: Scope::Window }
    }

    /// Un panneau à l'intérieur d'une ligne de la liste `list`.
    pub const fn in_row(
        list: &'static str,
        id: &'static str,
        value: Value,
        source: Source,
    ) -> Pane {
        Pane { id, label: None, value, source, scope: Scope::Row(list) }
    }

    /// Le même panneau, lu sur un écran de hutte : la forme de `src/ui`, conservée.
    pub fn from_ui(field: Field) -> Pane {
        Pane {
            id: field.id,
            label: field.label,
            value: field.value,
            source: Source::Ui(field.source),
            scope: field.scope,
        }
    }

    /// Le champ de `src/ui` correspondant, quand la source est l'une des siennes.
    pub fn to_ui(&self) -> Option<Field> {
        match self.source {
            Source::Ui(inner) => Some(Field {
                id: self.id,
                label: self.label,
                value: self.value,
                source: inner,
                scope: self.scope,
            }),
            Source::Container
            | Source::BlockEntity
            | Source::Biome
            | Source::Tag
            | Source::Extension
            | Source::Window => None,
        }
    }

    /// Si le panneau appartient à une ligne plutôt qu'à la fenêtre.
    pub fn is_row_pane(&self) -> bool {
        matches!(self.scope, Scope::Row(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pane_read_off_a_hut_screen_keeps_every_field_and_can_go_back() {
        let field = Field::labelled(
            "workers",
            "a.key",
            Value::Row,
            crate::ui::Source::Module("worker"),
        );
        let pane = Pane::from_ui(field);
        assert_eq!(pane.id, "workers");
        assert_eq!(pane.label, Some("a.key"));
        assert_eq!(pane.value, Value::Row);
        assert_eq!(pane.source, Source::module("worker"));
        assert_eq!(pane.to_ui(), Some(field), "la forme de src/ui, à l'identique");
        let row = Pane::from_ui(Field::in_row(
            "workers",
            "workerName",
            Value::Name,
            crate::ui::Source::Citizen,
        ));
        assert!(row.is_row_pane());
        assert_eq!(row.scope, Scope::Row("workers"));
    }

    #[test]
    fn the_six_sources_added_here_cannot_go_back_to_src_ui() {
        for source in [
            Source::Container,
            Source::BlockEntity,
            Source::Biome,
            Source::Tag,
            Source::Extension,
            Source::Window,
        ] {
            let pane = Pane::new("x", Value::Text, source);
            assert!(pane.source.is_ported_here(), "{}", source.name());
            assert_eq!(pane.to_ui(), None, "{} n'est pas un lecteur de src/ui", source.name());
        }
        // Une extension est de la donnée de colonie, un emplacement non.
        assert!(Source::Extension.is_colony_data());
        assert!(!Source::Container.is_colony_data());
        assert!(!Source::Biome.is_colony_data());
        // Les sources de src/ui voyagent dans les deux sens.
        assert!(Pane::new("x", Value::Text, Source::CITIZEN).to_ui().is_some());
        assert!(!Source::CITIZEN.is_ported_here());
        assert!(Source::CITIZEN.is_colony_data());
    }

    #[test]
    fn every_source_has_its_own_name() {
        let all = [
            Source::Ui(crate::ui::Source::Building),
            Source::COLONY,
            Source::CITIZEN,
            Source::Ui(crate::ui::Source::Hut),
            Source::Ui(crate::ui::Source::PlayerInventory),
            Source::Ui(crate::ui::Source::WorkOrder),
            Source::Ui(crate::ui::Source::Request),
            Source::Ui(crate::ui::Source::Advancement),
            Source::module("crafting"),
            Source::Container,
            Source::BlockEntity,
            Source::Biome,
            Source::Tag,
            Source::Extension,
            Source::Window,
        ];
        for (index, source) in all.iter().enumerate() {
            assert!(!source.name().is_empty());
            for other in &all[index + 1..] {
                assert_ne!(source.name(), other.name(), "deux sources, un nom");
            }
        }
        // La clé de module ne fuit pas dans le nom : un nom, plusieurs clés.
        assert_eq!(Source::module("a").name(), Source::module("b").name());
    }
}
