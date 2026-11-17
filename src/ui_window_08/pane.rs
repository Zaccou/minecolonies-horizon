//! Un panneau de l'une de ces quatre fenêtres : ce qu'il porte, et d'où vient sa valeur.
//!
//! **La forme est celle de `src/ui`, reprise telle quelle** : `(id, label, valeur, portée)`
//! est [`crate::ui::field::Field`], et [`Value`] comme [`Scope`] sont les types de `src/ui`.
//! [`Pane::from_ui`] convertit un champ d'écran de hutte sans en perdre un morceau ;
//! [`Pane::to_ui`] fait le retour quand la source est une source de `src/ui`. Seule la
//! **source** a dû s'ouvrir, parce que ces quatre fenêtres lisent cinq choses qu'aucun des
//! treize écrans de `src/ui` ne lit :
//!
//! | Ce qu'elles lisent | Fenêtre | Pourquoi `src/ui::Source` ne le dit pas |
//! | --- | --- | --- |
//! | les emplacements d'un conteneur | les trois `AbstractContainerScreen` | aucun des neuf `Source` de `src/ui` n'est un `container.slots` : la brasserie en lit quatre (`WindowBrewingstandCrafting.java:116-122`), l'inventaire du citoyen `(items.size() - 36) / 9` rangées (`WindowCitizenInventory.java:82`) |
//! | le handler d'objets du bloc | `WindowsBarracksSpies` | l'or compté n'est pas un champ sauvegardé : c'est `TileEntityRack.getInventory()` (`WindowsBarracksSpies.java:66`) |
//! | le nom du bloc | `WindowBuildingInventory` | le titre est `getBlockState().getBlock().getName()` (`TileEntityColonyBuilding.java:344-347`), pas `getBuildingDisplayName()` |
//! | l'entité du niveau | `WindowCitizenInventory` | l'aperçu lit `menu.getEntity()` (`:134`), une `Optional<Entity>` du monde, pas une `ICitizenDataView` |
//! | une valeur que la classe écrit seule | `WindowsBarracksSpies` | `"x5"` est un `Component.literal` (`:64`) et l'icône `Items.GOLD_INGOT` (`:63`) : aucune vue ne les porte |
//!
//! Les sources que `src/ui` nomme déjà — la colonie, la vue de bâtiment, un citoyen,
//! l'inventaire du joueur, une vue de module par clé — sont [`Source::Ui`], pour qu'une
//! clé de module ou un id de citoyen ne soit nommé qu'une fois dans le crate.

use crate::ui::field::{Field, Scope, Value};

/// D'où vient la valeur d'un panneau.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: les cinq lecteurs que ce module ajoute, plus ceux de src/ui
pub enum Source {
    /// Un lecteur que `src/ui` nomme déjà : la colonie, la vue de bâtiment, un citoyen,
    /// une vue de module par clé, l'inventaire du client.
    Ui(crate::ui::Source),
    /// Les emplacements d'un `AbstractContainerMenu` : `container.slots`,
    /// `container.getItems()`, `container.getSize()`.
    Container,
    /// Le handler d'objets du bloc ouvert, `TileEntityRack.getInventory()` — dont
    /// `TileEntityColonyBuilding` hérite (`AbstractTileEntityColonyBuilding.java:26`),
    /// donc c'est l'inventaire de la hutte elle-même, pas une `IBuildingView`.
    Rack,
    /// Le nom du bloc (`TileEntityColonyBuilding.java:344-347`), qui est le titre de
    /// l'inventaire de bâtiment.
    Block,
    /// L'entité citoyen du niveau, lue pour l'aperçu (`WindowCitizenInventory.java:134`).
    Entity,
    /// Une valeur que la classe écrit elle-même, sans la demander à personne.
    Window,
}

impl Source {
    /// La colonie derrière le bloc (`IColonyView`) : les deux drapeaux du raid.
    pub const COLONY: Source = Source::Ui(crate::ui::Source::Colony);
    /// Un citoyen (`ICitizenDataView`), nommé par le conteneur.
    pub const CITIZEN: Source = Source::Ui(crate::ui::Source::Citizen);
    /// L'inventaire du joueur client, lu et jamais sauvegardé.
    pub const PLAYER_INVENTORY: Source = Source::Ui(crate::ui::Source::PlayerInventory);
    /// La vue de bâtiment (`IBuildingView`).
    pub const BUILDING: Source = Source::Ui(crate::ui::Source::Building);

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
            Source::Rack => "rack",
            Source::Block => "block",
            Source::Entity => "entity",
            Source::Window => "window",
        }
    }

    /// Vrai pour les cinq lecteurs que ce module a dû ajouter.
    pub fn is_ported_here(self) -> bool {
        !matches!(self, Source::Ui(_))
    }

    /// Vrai quand la valeur vit dans la colonie plutôt que dans le bloc ou la fenêtre.
    ///
    /// Les cinq lecteurs ajoutés ici répondent `false` : un emplacement, un rack, un nom
    /// de bloc et une entité sont locaux, même quand la colonie les a remplis.
    pub fn is_colony_data(self) -> bool {
        match self {
            Source::Ui(inner) => inner.is_colony_data(),
            Source::Container | Source::Rack | Source::Block | Source::Entity | Source::Window => {
                false
            }
        }
    }
}

/// Un panneau de l'une de ces quatre fenêtres.
///
/// `id` est l'attribut `id` du XML — le seul nom qu'une classe a sur un panneau, puisque
/// `findPaneOfTypeByID("amount", Text.class)` cherche exactement cette chaîne
/// (`WindowsBarracksSpies.java:64`). **Un panneau sans id n'est pas un [`Pane`]** : les
/// emplacements d'un conteneur, les textes que `renderLabels` dessine et l'aperçu d'entité
/// n'ont pas d'id, et sont donc décrits ailleurs (`brewing::Slot`, `container::Armour`) au
/// lieu d'être déguisés ici en panneaux nommés.
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
    /// `None` pour les cinq lecteurs ajoutés : un appelant qui les veut demande un
    /// lecteur que `src/ui` n'a pas.
    pub fn to_ui(&self) -> Option<Field> {
        match self.source {
            Source::Ui(inner) => Some(Field {
                id: self.id,
                label: self.label,
                value: self.value,
                source: inner,
                scope: self.scope,
            }),
            Source::Container | Source::Rack | Source::Block | Source::Entity | Source::Window => {
                None
            }
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
        let field = Field::labelled("workers", "a.key", Value::Row, crate::ui::Source::Module("worker"));
        let pane = Pane::from_ui(field);
        assert_eq!(pane.id, "workers");
        assert_eq!(pane.label, Some("a.key"));
        assert_eq!(pane.value, Value::Row);
        assert_eq!(pane.source, Source::module("worker"));
        assert_eq!(pane.to_ui(), Some(field), "src/ui's shape, kept exactly");
        // Une ligne garde sa liste.
        let row = Pane::from_ui(Field::in_row("workers", "workerName", Value::Name, crate::ui::Source::Citizen));
        assert!(row.is_row_pane());
        assert_eq!(row.scope, Scope::Row("workers"));
    }

    #[test]
    fn the_five_sources_added_here_cannot_go_back_to_src_ui() {
        for source in [
            Source::Container,
            Source::Rack,
            Source::Block,
            Source::Entity,
            Source::Window,
        ] {
            let pane = Pane::new("x", Value::Text, source);
            assert!(pane.source.is_ported_here(), "{}", source.name());
            assert!(!pane.source.is_colony_data(), "{}", source.name());
            assert_eq!(pane.to_ui(), None, "{} is not a hut screen's reader", source.name());
        }
        // Les neuf sources de `src/ui` voyagent dans les deux sens.
        assert!(Pane::new("x", Value::Text, Source::CITIZEN).to_ui().is_some());
        assert!(!Source::CITIZEN.is_ported_here());
        assert!(Source::CITIZEN.is_colony_data());
        assert!(Source::COLONY.is_colony_data());
        assert!(!Source::PLAYER_INVENTORY.is_colony_data());
    }

    #[test]
    fn every_source_has_its_own_name() {
        let all = [
            Source::BUILDING,
            Source::COLONY,
            Source::CITIZEN,
            Source::PLAYER_INVENTORY,
            Source::Ui(crate::ui::Source::Hut),
            Source::Ui(crate::ui::Source::WorkOrder),
            Source::Ui(crate::ui::Source::Request),
            Source::Ui(crate::ui::Source::Advancement),
            Source::module("brewing"),
            Source::Container,
            Source::Rack,
            Source::Block,
            Source::Entity,
            Source::Window,
        ];
        for (index, source) in all.iter().enumerate() {
            for other in &all[index + 1..] {
                assert_ne!(source.name(), other.name(), "two sources share a name");
            }
        }
        // La clé de module ne fuit pas dans le nom : un nom, plusieurs clés.
        assert_eq!(Source::module("brewing").name(), Source::module("worker").name());
    }
}
