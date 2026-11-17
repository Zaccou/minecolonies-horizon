//! `WindowInteraction`: the dialogue a citizen opens, as data and as a cursor.
//!
//! Java reference: `core/client/gui/WindowInteraction.java` (191 lines, BlockUI +
//! `gui/citizen/windowinteraction.xml`), a child of `MainWindowCitizen` (`:60`).
//!
//! **Three panes of the window — plus one row pane — and only one is text the class
//! composes.** `chat` gets `citizen.getName() + ": " + handler.getInquiry(player)` (`:111`);
//! `requestItem` is an `ItemIcon` the class **hides first** (`:94`) and only a handler may show
//! (`:146`); `responseOptions` is a `Box` cleared and refilled with one button per response
//! (`:104-144`), whose label is the fourth pane. **No title, no level, no progression, no
//! stock**: the citizen's level is the parent window's, not this one's.
//!
//! # The one window whose buttons have runtime ids
//!
//! `registerButton` binds a **pane id** (`AbstractWindowSkeleton.java:130`), and the buttons
//! this window binds are the ones it built a moment earlier, named `response_` plus a
//! **1-based** index (`:36`, `:128`). A `&'static [Action]` cannot hold those, so
//! [`response_id`] and [`response_index`] stand in for the constant: the prefix is declared
//! once and the number is computed both ways, as Java does it.
//!
//! # Two rules that are easy to get wrong
//!
//! 1. **The first interaction is always shown**, even a `CHITCHAT` one: the guard is
//!    `priority <= CHITCHAT && currentInteraction > 0` (`:97`), and the second half is what
//!    makes the first dialogue visible at all ([`shown`]).
//! 2. **A chit-chat is skipped but a quest dialogue is not**: the guard ends with
//!    `!(handler instanceof QuestDialogueInteraction)` (`:97`), so a dialogue tree may hold
//!    a chit-chat priority without being swallowed.
//!
//! The list arrives already sorted by **descending** priority
//! (`CitizenDataView.java:378-379`), which is why the cursor only walks forwards.

use super::field::{Pane, Source};
use super::refusal::Refusal;
use super::{Action, Click};
use crate::ui::action::Effect;
use crate::ui::field::Value;

/// The prefix of every response button's id (`WindowInteraction.java:36`).
pub const RESPONSE_ID_PREFIX: &str = "response_";

/// The close button (`WindowConstants.java:477`); an image with a tooltip and no label
/// (`windowinteraction.xml:10-11`).
pub const CANCEL_ID: &str = "cancel";

/// That tooltip, the button's only text (`windowinteraction.xml:11`).
pub const EXIT_TIP_KEY: &str = "com.minecolonies.core.exit_interactions";

/// The pane Java writes the citizen's line into (`WindowConstants.java:1044`).
pub const CHAT_ID: &str = "chat";

/// The `Box` whose children are the response buttons (`WindowConstants.java:1054`).
pub const RESPONSE_BOX_ID: &str = "responseOptions";

/// The `ItemIcon` hidden before anything else (`:94`).
pub const REQUEST_ITEM_ID: &str = "requestItem";

/// The message sent **when the window closes**: `new InteractionClose(colonyId, citizenId,
/// dimension, inquiry)` (`:156`).
pub const CLOSE_MESSAGE: &str = "InteractionClose";

/// The message a responding handler sends (`ServerCitizenInteraction.java:175`). The
/// **window** sends nothing: the handler does, inside `onClientResponseTriggered`.
pub const RESPONSE_MESSAGE: &str = "InteractionResponse";

/// A chat priority, in Java's order, because `getPriority()` **is** the ordinal
/// (`ChatPriority.java:8-23`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: five priorities in Java, in this order
pub enum ChatPriority {
    /// Never shown.
    Hidden,
    /// Small talk.
    ChitChat,
    /// Worth a look.
    Pending,
    /// Blocks part of the day.
    Important,
    /// Stops the citizen.
    Blocking,
}

impl ChatPriority {
    /// Every priority, in declaration order.
    pub const ALL: [ChatPriority; 5] = [
        ChatPriority::Hidden,
        ChatPriority::ChitChat,
        ChatPriority::Pending,
        ChatPriority::Important,
        ChatPriority::Blocking,
    ];

    /// The number Java compares (`ChatPriority.java:20-23`).
    pub fn rank(self) -> i32 {
        self as i32
    }

    /// A stable name, for a log line or a test.
    pub fn name(self) -> &'static str {
        match self {
            ChatPriority::Hidden => "hidden",
            ChatPriority::ChitChat => "chitchat",
            ChatPriority::Pending => "pending",
            ChatPriority::Important => "important",
            ChatPriority::Blocking => "blocking",
        }
    }

    /// Whether the skip applies: `priority <= CHITCHAT` (`WindowInteraction.java:97`).
    pub fn is_small_talk(self) -> bool {
        self.rank() <= ChatPriority::ChitChat.rank()
    }
}

/// One interaction as the window sees it: how loud, whether it is a dialogue tree, whether
/// it survived the visibility filter, and how many responses it offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// `handler.getPriority().getPriority()`.
    pub priority: ChatPriority,
    /// `handler instanceof QuestDialogueInteraction`.
    pub quest: bool,
    /// `handler.isVisible(level)`, read once at open (`:78`).
    pub visible: bool,
    /// `handler.getPossibleResponses().size()` (`:113`).
    pub responses: u16,
}

impl Entry {
    /// A visible, plain interaction.
    pub const fn new(priority: ChatPriority, responses: u16) -> Entry {
        Entry { priority, quest: false, visible: true, responses }
    }

    /// A dialogue tree, which the small-talk skip never swallows.
    pub const fn quest(priority: ChatPriority, responses: u16) -> Entry {
        Entry { priority, quest: true, visible: true, responses }
    }

    /// An interaction the world no longer shows (`:78`).
    pub const fn hidden(priority: ChatPriority) -> Entry {
        Entry { priority, quest: false, visible: false, responses: 0 }
    }
}

/// Where the dialogue has got to: the list the window kept and the cursor into it — Java's
/// `interactions` and `currentInteraction` (`:46`, `:51`), and nothing else.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    entries: Vec<Entry>,
    at: usize,
}

impl State {
    /// The state at open: invisible interactions dropped, cursor settled by [`shown`]
    /// (`:78-80`).
    pub fn opened(entries: &[Entry]) -> State {
        let kept: Vec<Entry> = entries.iter().copied().filter(|e| e.visible).collect();
        let at = shown(&kept, 0).unwrap_or(kept.len());
        State { entries: kept, at }
    }

    /// The interactions the window kept.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// How many it kept.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the window opened with nothing to say.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The cursor, `None` once it has walked past the end (`:87-91`).
    pub fn at(&self) -> Option<usize> {
        if self.at < self.entries.len() { Some(self.at) } else { None }
    }

    /// The interaction on screen.
    pub fn current(&self) -> Option<&Entry> {
        self.entries.get(self.at)
    }

    /// The state after a response: `true` advances (`:177-182`), `false` re-renders the
    /// same interaction (`:183`) — a refusal of the answer, not of the click.
    pub fn respond(&self, accepted: bool) -> State {
        if !accepted {
            return self.clone();
        }
        let at = shown(&self.entries, self.at + 1).unwrap_or(self.entries.len());
        State { entries: self.entries.clone(), at }
    }

    /// Whether closing now sends `InteractionClose`: Java guards it with
    /// `currentInteraction < interactions.size()` (`:153-157`).
    pub fn closes_with_a_message(&self) -> bool {
        self.at().is_some()
    }
}

/// The first index at or after `from` the window shows, or `None` when it closes itself.
///
/// `setupInteraction`'s loop (`:85-102`) without its recursion.
pub fn shown(entries: &[Entry], from: usize) -> Option<usize> {
    let mut at = from;
    while let Some(entry) = entries.get(at) {
        if entry.priority.is_small_talk() && at > 0 && !entry.quest {
            at += 1;
            continue;
        }
        return Some(at);
    }
    None
}

/// The id of response button `index` (0-based in the call, 1-based in the id): `:112`, `:128`.
pub fn response_id(index: usize) -> String {
    format!("{RESPONSE_ID_PREFIX}{}", index + 1)
}

/// The response index an id names, `None` when it names none.
///
/// Java does `Integer.parseInt(getID().replace("response_", "")) - 1` (`:177`); the `replace`
/// is not anchored, so `xresponse_3` parses there too. This port requires the prefix.
pub fn response_index(id: &str) -> Option<usize> {
    let digits = id.strip_prefix(RESPONSE_ID_PREFIX)?;
    digits.parse::<usize>().ok()?.checked_sub(1)
}

/// What the `chat` pane reads (`:111`).
pub fn chat_line(citizen: &str, inquiry: &str) -> String {
    format!("{citizen}: {inquiry}")
}

/// Whether response `index` may be triggered.
///
/// Java checks no upper bound: it hands the index to the handler, whose
/// `getPossibleResponses().get(index)` throws and is caught (`:177-188`). This port names it.
pub fn may_respond(state: &State, index: usize) -> Result<usize, Refusal> {
    let entry = state.current().ok_or(Refusal::NoInteractionLeft)?;
    if index >= usize::from(entry.responses) {
        return Err(Refusal::NoSuchResponse);
    }
    Ok(index)
}

/// The four panes, in the order Java fills them: the line (`:111`), the item (`:94`, `:146`),
/// the response box (`:104`) and a response button's own label.
pub const PANES: &[Pane] = &[
    Pane::new(CHAT_ID, Value::Text, Source::Interaction),
    Pane::new(REQUEST_ITEM_ID, Value::Icon, Source::Interaction),
    Pane::new(RESPONSE_BOX_ID, Value::Row, Source::Interaction),
    Pane::in_row(RESPONSE_BOX_ID, RESPONSE_ID_PREFIX, Value::Text, Source::Interaction),
];

/// The close button and the family of response buttons, whose id is a **prefix** — see
/// [`response_id`].
pub const ACTIONS: &[Action] = &[
    // `registerButton(BUTTON_CANCEL, this::cancelClicked)` → `close()` (`:63`, `:66-69`),
    // and closing sends `InteractionClose` (`:149-158`).
    Action::new(CANCEL_ID, "", Click::ClosesAndSends(CLOSE_MESSAGE), &[]),
    Action::new(
        RESPONSE_ID_PREFIX,
        "",
        Click::One(Effect::Sends(RESPONSE_MESSAGE)),
        &[Refusal::NoInteractionLeft, Refusal::NoSuchResponse],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_five_priorities_keep_javas_ordinals_and_two_are_small_talk() {
        assert_eq!(ChatPriority::ALL.len(), 5);
        for (index, priority) in ChatPriority::ALL.iter().enumerate() {
            assert_eq!(priority.rank(), index as i32, "{}", priority.name());
        }
        assert_eq!(ChatPriority::Hidden.rank(), 0);
        assert_eq!(ChatPriority::Blocking.rank(), 4);
        assert!(ChatPriority::Hidden.is_small_talk());
        assert!(ChatPriority::ChitChat.is_small_talk());
        assert!(!ChatPriority::Pending.is_small_talk());
        assert!(!ChatPriority::Blocking.is_small_talk());
    }

    #[test]
    fn the_first_interaction_is_shown_and_a_later_small_talk_is_skipped() {
        // WindowInteraction.java:97 — the `currentInteraction > 0` half of the guard.
        let first =
            [Entry::new(ChatPriority::ChitChat, 2), Entry::new(ChatPriority::Pending, 1)];
        assert_eq!(State::opened(&first).at(), Some(0));
        let later = [
            Entry::new(ChatPriority::Important, 1),
            Entry::new(ChatPriority::ChitChat, 2),
            Entry::new(ChatPriority::Pending, 1),
        ];
        let state = State::opened(&later);
        assert_eq!(state.at(), Some(0));
        let next = state.respond(true);
        assert_eq!(next.at(), Some(2), "index 1 is small talk and is skipped");
        // A refused answer keeps the dialogue where it is; `true` past the end closes it.
        assert_eq!(state.respond(false), state);
        let closed = next.respond(true);
        assert_eq!(closed.at(), None);
        assert!(!closed.closes_with_a_message());
        // An invisible interaction never enters the list, so a lone hidden one opens closed.
        let hidden = [Entry::hidden(ChatPriority::Blocking), Entry::new(ChatPriority::Pending, 1)];
        assert_eq!(State::opened(&hidden).len(), 1);
        assert!(State::opened(&[Entry::hidden(ChatPriority::Pending)]).is_empty());
    }

    #[test]
    fn a_quest_dialogue_is_never_swallowed_by_the_small_talk_skip() {
        let quest =
            [Entry::new(ChatPriority::Blocking, 1), Entry::quest(ChatPriority::ChitChat, 3)];
        let state = State::opened(&quest).respond(true);
        assert_eq!(state.at(), Some(1), "a dialogue tree stays on screen");
        assert_eq!(state.current().map(|entry| entry.quest), Some(true));
        let plain =
            [Entry::new(ChatPriority::Blocking, 1), Entry::new(ChatPriority::ChitChat, 3)];
        assert_eq!(State::opened(&plain).respond(true).at(), None);
    }

    #[test]
    fn the_response_ids_carry_a_one_based_index_and_a_bad_one_is_refused_by_name() {
        assert_eq!(response_id(0), "response_1");
        assert_eq!(response_id(9), "response_10");
        assert_eq!(response_index("response_10"), Some(9));
        // Java's non-anchored `replace` would accept these; this port does not.
        assert_eq!(response_index("xresponse_3"), None);
        assert_eq!(response_index("response_"), None);
        assert_eq!(response_index("cancel"), None);
        for index in 0..12 {
            assert_eq!(response_index(&response_id(index)), Some(index));
        }
        let state = State::opened(&[Entry::new(ChatPriority::Important, 2)]);
        assert_eq!(may_respond(&state, 1), Ok(1));
        assert_eq!(may_respond(&state, 2), Err(Refusal::NoSuchResponse));
        assert_eq!(may_respond(&state.respond(true), 0), Err(Refusal::NoInteractionLeft));
        assert_eq!(chat_line("Anna", "Good day!"), "Anna: Good day!");
    }

    #[test]
    fn the_window_names_four_panes_and_two_buttons() {
        assert_eq!(PANES.len(), 4);
        assert_eq!((PANES[0].id, PANES[0].value), (CHAT_ID, Value::Text));
        assert_eq!((PANES[1].id, PANES[1].value), (REQUEST_ITEM_ID, Value::Icon));
        assert_eq!((PANES[2].id, PANES[2].value), (RESPONSE_BOX_ID, Value::Row));
        assert!(PANES[3].is_row_pane());
        assert_eq!(ACTIONS.len(), 2);
        assert_eq!(ACTIONS[0].id, CANCEL_ID);
        assert_eq!(ACTIONS[0].label, "", "an image button with a tooltip");
        // The response family is described by its prefix, never by a full id.
        assert_eq!(ACTIONS[1].id, RESPONSE_ID_PREFIX);
        assert!(response_index(ACTIONS[1].id).is_none());
    }
}
