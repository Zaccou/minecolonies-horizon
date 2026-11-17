//! One pane of one of these four windows: what it holds, and where its value comes from.
//!
//! **The shape is `src/ui`'s, reused as it is**: `(id, label, value, scope)` is
//! [`crate::ui::field::Field`], and [`crate::ui::field::Value`] and
//! [`crate::ui::field::Scope`] come from there unchanged — [`Pane::from_ui`] and
//! [`Pane::to_ui`] convert between the two shapes. Only the **source** grows, because these
//! four windows read three things no hut window reads:
//!
//! | What | Which window | Why `src/ui::Source` cannot say it |
//! | --- | --- | --- |
//! | an interaction handler | `WindowInteraction` | the inquiry, its responses and its priority are the handler's (`IInteractionResponseHandler.java:27-160`), not a building's |
//! | a plantation field block entity | `WindowPlantationField` | the pack name, the blueprint path, the rotation and the mirror are the **tile entity's** (`:87-103`) |
//! | the window's own state | `WindowPostBoxMain` | the filter, the delivery flag and the countdown are fields of the class (`:83`, `:88`, `:93`) |
//!
//! Everything else — building view, citizen, colony, module view by key, request, player
//! inventory — is [`Source::Ui`], so a saved module key or a citizen id is named once.

use crate::ui::field::{Field, Scope, Value};

/// Where a pane's value comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: the readers of these four windows, one variant each
pub enum Source {
    /// A source `src/ui` already names.
    Ui(crate::ui::Source),
    /// The interaction handler of the citizen being talked to
    /// (`WindowInteraction.java:96-146`).
    Interaction,
    /// The plantation field's block entity (`WindowPlantationField.java:90-102`).
    TileEntity,
    /// A value the window itself keeps (`WindowPostBoxMain.java:83`, `:88`, `:93`).
    Window,
}

impl Source {
    /// The building view, as `src/ui` names it.
    pub const BUILDING: Source = Source::Ui(crate::ui::Source::Building);
    /// A citizen's record.
    pub const CITIZEN: Source = Source::Ui(crate::ui::Source::Citizen);
    /// The colony behind the block.
    pub const COLONY: Source = Source::Ui(crate::ui::Source::Colony);
    /// A request in the colony's manager.
    pub const REQUEST: Source = Source::Ui(crate::ui::Source::Request);
    /// The **client** player's inventory, read and never saved
    /// (`WindowPostBoxMain.java:252`).
    pub const PLAYER_INVENTORY: Source = Source::Ui(crate::ui::Source::PlayerInventory);

    /// One building module's view, by **module key** — the string
    /// `crate::building::Module::key` saves (`BuildingModules.java:52-53`).
    pub const fn module(key: &'static str) -> Source {
        Source::Ui(crate::ui::Source::Module(key))
    }

    /// A stable name, for a log line or a test.
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
            Source::Interaction => "interaction",
            Source::TileEntity => "tile_entity",
            Source::Window => "window",
        }
    }

    /// Whether this is one of the three sources this module had to add.
    pub fn is_added_here(self) -> bool {
        matches!(self, Source::Interaction | Source::TileEntity | Source::Window)
    }

    /// Whether the value lives in the colony rather than in the block or the window.
    pub fn is_colony_data(self) -> bool {
        match self {
            Source::Ui(inner) => inner.is_colony_data(),
            Source::Interaction | Source::TileEntity | Source::Window => false,
        }
    }
}

/// One pane: the `(id, label, value, source, scope)` Java looks up and fills.
///
/// `id` is the XML's `id` attribute, the only handle a class has on a pane
/// (`findPaneOfTypeByID("chat", Text.class)`, `WindowInteraction.java:108`). A pane with no
/// `id` is not a pane here, which is why the plantation field's title
/// (`windowplantationfield.xml:11`) is a [`crate::ui_window_05::Title`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pane {
    /// The XML pane id.
    pub id: &'static str,
    /// The translation key of the label next to this value, when there is one.
    pub label: Option<&'static str>,
    /// What the pane holds.
    pub value: Value,
    /// Where the value comes from.
    pub source: Source,
    /// The window's pane, or a row of one of its lists.
    pub scope: Scope,
}

impl Pane {
    /// A pane of the window, with no label of its own.
    pub const fn new(id: &'static str, value: Value, source: Source) -> Pane {
        Pane { id, label: None, value, source, scope: Scope::Window }
    }

    /// A pane of the window that carries a label.
    pub const fn labelled(
        id: &'static str,
        label: &'static str,
        value: Value,
        source: Source,
    ) -> Pane {
        Pane { id, label: Some(label), value, source, scope: Scope::Window }
    }

    /// A pane inside one row of the list `list`.
    pub const fn in_row(
        list: &'static str,
        id: &'static str,
        value: Value,
        source: Source,
    ) -> Pane {
        Pane { id, label: None, value, source, scope: Scope::Row(list) }
    }

    /// The same pane, read off a hut screen's field.
    pub fn from_ui(field: Field) -> Pane {
        Pane {
            id: field.id,
            label: field.label,
            value: field.value,
            source: Source::Ui(field.source),
            scope: field.scope,
        }
    }

    /// The `src/ui` field for this pane, when `src/ui` can explain its source. `None` for
    /// the three sources this module adds.
    pub fn to_ui(&self) -> Option<Field> {
        match self.source {
            Source::Ui(inner) => Some(Field {
                id: self.id,
                label: self.label,
                value: self.value,
                source: inner,
                scope: self.scope,
            }),
            Source::Interaction | Source::TileEntity | Source::Window => None,
        }
    }

    /// Whether the pane belongs to a row.
    pub fn is_row_pane(&self) -> bool {
        matches!(self.scope, Scope::Row(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pane_read_off_a_hut_screen_keeps_every_field_and_comes_back() {
        let field =
            Field::labelled("name", "a.key", Value::Name, crate::ui::Source::Building);
        let pane = Pane::from_ui(field);
        assert_eq!(pane.id, "name");
        assert_eq!(pane.label, Some("a.key"));
        assert_eq!(pane.source, Source::BUILDING);
        assert_eq!(pane.scope, Scope::Window);
        assert_eq!(pane.to_ui(), Some(field));
        let row = Pane::from_ui(Field::in_row(
            "workers",
            "workerName",
            Value::Name,
            crate::ui::Source::Citizen,
        ));
        assert!(row.is_row_pane());
        assert_eq!(row.to_ui().map(|f| f.scope), Some(Scope::Row("workers")));
    }

    #[test]
    fn the_three_sources_this_module_adds_cannot_go_back_to_src_ui() {
        for source in [Source::Interaction, Source::TileEntity, Source::Window] {
            let pane = Pane::new("x", Value::Text, source);
            assert!(pane.source.is_added_here());
            assert!(!pane.source.is_colony_data());
            assert_eq!(pane.to_ui(), None, "{} is no hut reader", source.name());
        }
        assert!(Pane::new("x", Value::Text, Source::BUILDING).to_ui().is_some());
        assert!(!Source::BUILDING.is_added_here());
        assert!(Source::CITIZEN.is_colony_data());
        assert!(!Source::PLAYER_INVENTORY.is_colony_data());
        // The module key does not leak into the source's name: one name, many keys.
        assert_eq!(Source::module("minstock").name(), Source::module("worker").name());
        assert_eq!(Source::module("minstock").name(), "ui.module");
        assert_ne!(Source::Interaction.name(), Source::Window.name());
    }
}
