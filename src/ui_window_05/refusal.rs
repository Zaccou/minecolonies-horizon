//! Why one of these four windows refuses a click — named, never guessed.
//!
//! Java says no in four ways and all four are refusals: a **hidden pane** (the plantation
//! field's `repair` outside a colony, `WindowPlantationField.java:112-115`; a child
//! request's `cancel`, `RequestTreeWindowModule.java:168-171`), a **button registered and
//! never implemented** (`fulfill`, `:107`, `:274-277`, `:288-292`), a **guard that returns
//! early** (`cancel` with no open request, `:313-316`), and an **exception Java catches and
//! logs** (a `response_N` naming no response, `WindowInteraction.java:177-188`).
//!
//! [`Refusal::key`] answers Java's key only where the player is told something and `None`
//! everywhere else, so a caller can never print [`Refusal::name`] where a player expects a
//! sentence. Two of the keys are **content**, not chat: the "not in colony" text and the
//! stock-limit label are painted inside the window.
//!
//! What `src/ui` already names comes back through [`Refusal::Ui`], exactly as
//! `ui::action::Refusal::HutRefuses` forwards the jobs table's: two tables cannot disagree
//! about what "not built" means if only one of them says it.

use crate::ui::action::Refusal as UiRefusal;

/// Why one of these four windows refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own closed set for four windows, and no more
pub enum Refusal {
    /// A refusal `src/ui` already names, forwarded unchanged.
    Ui(UiRefusal),
    /// The plantation field is in no colony (`WindowPlantationField.java:112-115`).
    NotInColony,
    /// No list row sits under the clicked button; Java reads index `-1` and throws
    /// (`WindowPostBoxMain.java:169-170`).
    RowNotFound,
    /// The open-request list is empty (`RequestTreeWindowModule.java:313-316`).
    NoOpenRequest,
    /// A child request is not cancellable from here: `depth <= 0` only
    /// (`RequestTreeWindowModule.java:258-268`).
    NotTopLevelRequest,
    /// Nothing here fulfils a request (`RequestTreeWindowModule.java:274-277`).
    NotFulfillable,
    /// The minimum stock has reached its ceiling (`MinimumStockModuleWindow.java:61-66`,
    /// `:93`).
    StockLimitReached,
    /// A `response_N` that names no response of the current interaction
    /// (`WindowInteraction.java:177-188`).
    NoSuchResponse,
    /// No interaction left to show, so the window closes itself (`:87-91`).
    NoInteractionLeft,
    /// A window id [`crate::ui_window_05::WINDOWS`] does not hold.
    UnknownWindow,
}

impl Refusal {
    /// A stable name, namespaced so it can never be mistaken for a sentence.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::Ui(inner) => inner.name(),
            Refusal::NotInColony => "ui5.refusal.not_in_colony",
            Refusal::RowNotFound => "ui5.refusal.row_not_found",
            Refusal::NoOpenRequest => "ui5.refusal.no_open_request",
            Refusal::NotTopLevelRequest => "ui5.refusal.not_top_level",
            Refusal::NotFulfillable => "ui5.refusal.not_fulfillable",
            Refusal::StockLimitReached => "ui5.refusal.stock_limit",
            Refusal::NoSuchResponse => "ui5.refusal.no_such_response",
            Refusal::NoInteractionLeft => "ui5.refusal.no_interaction",
            Refusal::UnknownWindow => "ui5.refusal.unknown_window",
        }
    }

    /// Java's translation key, when Java tells the player anything.
    ///
    /// `None` is not "no reason": it is "Java hides a button or logs a line", which is why
    /// a caller must not fall back to [`Self::name`] as a sentence.
    pub fn key(self) -> Option<&'static str> {
        match self {
            Refusal::Ui(inner) => inner.key(),
            // The window's own text (`windowplantationfield.xml:14`).
            Refusal::NotInColony => {
                Some("com.minecolonies.coremod.gui.workerhuts.fields.notincolony")
            }
            // The add button's label once the ceiling is reached
            // (`MinimumStockModuleWindow.java:61-66`).
            Refusal::StockLimitReached => {
                Some("com.minecolonies.coremod.gui.warehouse.limitreached")
            }
            Refusal::RowNotFound
            | Refusal::NoOpenRequest
            | Refusal::NotTopLevelRequest
            | Refusal::NotFulfillable
            | Refusal::NoSuchResponse
            | Refusal::NoInteractionLeft
            | Refusal::UnknownWindow => None,
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

impl From<UiRefusal> for Refusal {
    fn from(inner: UiRefusal) -> Refusal {
        Refusal::Ui(inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The nine this module adds; the tenth variant forwards `src/ui`'s.
    const OWN: [Refusal; 9] = [
        Refusal::NotInColony,
        Refusal::RowNotFound,
        Refusal::NoOpenRequest,
        Refusal::NotTopLevelRequest,
        Refusal::NotFulfillable,
        Refusal::StockLimitReached,
        Refusal::NoSuchResponse,
        Refusal::NoInteractionLeft,
        Refusal::UnknownWindow,
    ];

    #[test]
    fn every_refusal_has_its_own_namespaced_name() {
        for (index, refusal) in OWN.iter().enumerate() {
            assert!(refusal.name().starts_with("ui5.refusal."), "{refusal}");
            for other in &OWN[index + 1..] {
                assert_ne!(refusal.name(), other.name(), "two refusals share a name");
            }
        }
        assert_eq!(OWN.len(), 9);
    }

    #[test]
    fn only_two_are_shown_and_neither_is_a_chat_line() {
        let keyed: Vec<&str> =
            OWN.iter().filter(|r| r.key().is_some()).map(|r| r.name()).collect();
        assert_eq!(keyed.len(), 2, "Java hides a button or logs for the other seven");
        assert_eq!(
            Refusal::NotInColony.key(),
            Some("com.minecolonies.coremod.gui.workerhuts.fields.notincolony")
        );
        assert_eq!(
            Refusal::StockLimitReached.key(),
            Some("com.minecolonies.coremod.gui.warehouse.limitreached")
        );
        assert_eq!(Refusal::RowNotFound.key(), None);
        assert_eq!(Refusal::NoInteractionLeft.key(), None);
    }

    #[test]
    fn a_forwarded_refusal_keeps_the_ui_table_name_and_key() {
        assert_eq!(Refusal::Ui(UiRefusal::NotBuilt).name(), UiRefusal::NotBuilt.name());
        assert_eq!(Refusal::Ui(UiRefusal::NotBuilt).key(), UiRefusal::NotBuilt.key());
        // A silent refusal of `src/ui` stays silent through the forward.
        assert_eq!(Refusal::from(UiRefusal::NoRoom).key(), None);
        for refusal in OWN {
            assert_ne!(refusal.name(), Refusal::Ui(UiRefusal::NoRoom).name());
        }
    }
}
