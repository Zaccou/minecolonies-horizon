//! What a building asks for, and the four states this port can honour.
//!
//! Java reference: `requestable/Stack.java` for what a request *is*,
//! `request/RequestState.java` for the states, `core/entity/ai/workers/AbstractEntityAIBasic.java`
//! for the two numbers it carries.
//!
//! **The two numbers.** `Stack` holds `count` and `minCount` and clamps the second
//! to the first — `this.minCount = Math.min(minCount, count)` (`Stack.java:178`) —
//! because a minimum above the total is a request that can never be satisfied.
//! `count` is what the building wants; `minCount` is the least that already counts
//! as enough, which is how a worker decides whether to file at all:
//! `if (invCount >= minCount) return true` (`AbstractEntityAIBasic.java:1684`),
//! with `count - invCount` as the real need (`:1689-1690`).
//!
//! **The states.** `RequestState` has thirteen (`RequestState.java:13-78`) because
//! it tracks a resolver, its assignments and its clean-up. This port has no
//! resolver yet, so it keeps the four transitions it can actually make and refuses
//! the rest by name rather than pretending.
//!
//! **The quality.** A request may demand a minimum tier, and MineColonies has a
//! real one: a colony food item answers `getTier()`
//! (`api/items/IMinecoloniesFoodItem.java:12`), the top tier is 3
//! (`api/util/ItemStackUtils.java:966`), a house takes food whose tier reaches its
//! own minimum (`core/colony/buildings/views/LivingBuildingView.java:146`), and
//! that minimum is `max(0, level - 2)` (`api/util/FoodUtils.java:369-377`). That is
//! the one numeric quality rule in the Java, and the one [Quality] carries.

use crate::inventory::{
    Refusal,
    stack::{ItemName, Stack},
};

/// A quality tier, `0..=3`, where 0 is "no requirement".
///
/// The scale is MineColonies' food tier: 1 to 3, with 3 the top
/// (`ItemStackUtils.java:966`). Zero is not a tier; it is the absence of a
/// requirement, so a request that asks for nothing carries [`Self::NONE`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Quality(u8);

impl Quality {
    /// No requirement at all.
    pub const NONE: Quality = Quality(0);
    /// The highest tier a food item reports.
    pub const MAX: u8 = 3;

    /// `None` above [`Self::MAX`]: a tier the scale cannot hold is a caller bug,
    /// not a clamp, because silently lowering it would accept worse goods.
    pub fn new(tier: u8) -> Option<Self> {
        (tier <= Self::MAX).then_some(Self(tier))
    }

    /// The minimum a building of `level` demands: `max(0, level - 2)`
    /// (`api/util/FoodUtils.java:374-377`).
    ///
    /// The Java does not clamp upwards; a [Quality] cannot hold more than
    /// [`Self::MAX`], so this clamps at 3. It costs nothing in the port's own
    /// range — hut levels are 1 to 5 (`src/hut.rs`, `LEVEL_MIN`/`LEVEL_MAX`), so
    /// the highest demand is 3 anyway.
    pub fn required_for_level(level: i32) -> Self {
        let required = level.saturating_sub(2).max(0);
        Self(u8::try_from(required).unwrap_or(Self::MAX).min(Self::MAX))
    }

    pub fn tier(self) -> u8 {
        self.0
    }

    /// Whether an item of tier `available` meets this requirement.
    pub fn satisfied_by(self, available: Quality) -> bool {
        available.0 >= self.0
    }
}

/// The four states this port can move between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: a closed set the port matches without a wildcard
pub enum RequestState {
    /// Filed, and nobody has started.
    Pending,
    /// Someone is working on it, or part of it has arrived.
    InProgress,
    /// The asked quantity is in. Terminal.
    Delivered,
    /// Given up on, or impossible. Terminal.
    Failed,
}

impl RequestState {
    /// Whether a move may still happen: the two states that are not terminal.
    pub fn is_open(self) -> bool {
        matches!(self, Self::Pending | Self::InProgress)
    }

    /// Whether nothing more can happen to the request.
    pub fn is_done(self) -> bool {
        !self.is_open()
    }
}

/// One thing a building wants: the item, how many, the least that counts as
/// enough, and the quality floor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CraftingRequest {
    item: ItemName,
    count: u32,
    min_count: u32,
    min_quality: Quality,
    state: RequestState,
    delivered: u32,
}

impl CraftingRequest {
    /// `Err(EmptyQuantity)` for a zero total: a request for nothing is not a
    /// request, exactly as a stack of nothing is not a stack.
    ///
    /// `min_count` is clamped to `count` as the Java does (`Stack.java:178`).
    pub fn new(
        item: ItemName,
        count: u32,
        min_count: u32,
        min_quality: Quality,
    ) -> Result<Self, Refusal> {
        if count == 0 {
            return Err(Refusal::EmptyQuantity);
        }
        Ok(Self {
            item,
            count,
            min_count: min_count.min(count),
            min_quality,
            state: RequestState::Pending,
            delivered: 0,
        })
    }

    pub fn item(&self) -> &ItemName {
        &self.item
    }

    /// What the building wants.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// What already counts as enough.
    pub fn min_count(&self) -> u32 {
        self.min_count
    }

    pub fn min_quality(&self) -> Quality {
        self.min_quality
    }

    pub fn state(&self) -> RequestState {
        self.state
    }

    pub fn delivered(&self) -> u32 {
        self.delivered
    }

    /// What is still missing, never negative: `count - delivered`
    /// (`AbstractEntityAIBasic.java:1689`).
    pub fn outstanding(&self) -> u32 {
        self.count.saturating_sub(self.delivered)
    }

    /// Whether enough has arrived to call the need met, which is the question the
    /// Java asks before filing anything (`AbstractEntityAIBasic.java:1684`). Note
    /// this can be true while the request is still open: a building that wanted
    /// ten and settled for four has four.
    pub fn satisfied(&self) -> bool {
        self.delivered >= self.min_count
    }

    /// `Pending → InProgress`: someone starts on it without delivering anything
    /// yet (a crafting job begun). Any other starting state is refused.
    pub fn begin(&mut self) -> Result<(), Refusal> {
        match self.state {
            RequestState::Pending => {
                self.state = RequestState::InProgress;
                Ok(())
            }
            _ => Err(Refusal::InvalidState),
        }
    }

    /// Hand `stack` in. Returns how many of it the request took: never more than
    /// it was missing, and never zero — a full request is closed, so the state
    /// check refuses first.
    ///
    /// A partial hand-in leaves the request `InProgress`; the hand-in that
    /// completes it makes it `Delivered`. `Err(ItemAbsent)` for another item,
    /// `Err(QualityTooLow)` below the floor, `Err(InvalidState)` once terminal.
    pub fn deliver(&mut self, stack: &Stack, quality: Quality) -> Result<u32, Refusal> {
        if !self.state.is_open() {
            return Err(Refusal::InvalidState);
        }
        if stack.item() != &self.item {
            return Err(Refusal::ItemAbsent);
        }
        if !self.min_quality.satisfied_by(quality) {
            return Err(Refusal::QualityTooLow);
        }
        let accepted = stack.count().min(self.outstanding());
        // An open request still misses something: `Delivered` is set the moment
        // `delivered` reaches `count`, and no open state is reachable after.
        debug_assert!(accepted > 0, "an open request is missing at least one item");
        self.delivered += accepted;
        self.state = if self.delivered >= self.count {
            RequestState::Delivered
        } else {
            RequestState::InProgress
        };
        Ok(accepted)
    }

    /// Give up: `Pending` or `InProgress` become `Failed`, and a terminal request
    /// stays where it is.
    pub fn fail(&mut self) -> Result<(), Refusal> {
        if !self.state.is_open() {
            return Err(Refusal::InvalidState);
        }
        self.state = RequestState::Failed;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ladder() -> ItemName {
        ItemName::new("minecraft:ladder").expect("a valid name")
    }

    fn plank() -> ItemName {
        ItemName::new("minecraft:oak_planks").expect("a valid name")
    }

    fn stack(item: ItemName, count: u32) -> Stack {
        Stack::new(item, count).expect("a nonzero stack")
    }

    fn asking(count: u32, min_count: u32) -> CraftingRequest {
        CraftingRequest::new(ladder(), count, min_count, Quality::NONE)
            .expect("a nonzero request")
    }

    #[test]
    fn a_request_for_nothing_is_refused() {
        assert_eq!(
            CraftingRequest::new(ladder(), 0, 0, Quality::NONE),
            Err(Refusal::EmptyQuantity)
        );
    }

    #[test]
    fn the_minimum_never_exceeds_the_total() {
        let request = asking(4, 9);
        assert_eq!(request.count(), 4);
        assert_eq!(request.min_count(), 4, "clamped down to the total");
        assert_eq!(asking(10, 4).min_count(), 4, "and kept below it");
    }

    #[test]
    fn beginning_moves_pending_to_in_progress_exactly_once() {
        let mut request = asking(4, 4);
        assert_eq!(request.state(), RequestState::Pending);
        assert_eq!(request.begin(), Ok(()));
        assert_eq!(request.state(), RequestState::InProgress);
        assert_eq!(request.begin(), Err(Refusal::InvalidState));
    }

    #[test]
    fn a_full_hand_in_closes_the_request() {
        let mut request = asking(4, 2);
        let took = request
            .deliver(&stack(ladder(), 4), Quality::NONE)
            .expect("the right item, enough of it");
        assert_eq!(took, 4);
        assert_eq!(request.state(), RequestState::Delivered);
        assert_eq!(request.outstanding(), 0);
        assert!(request.satisfied());
        assert!(request.state().is_done());
    }

    #[test]
    fn a_partial_hand_in_leaves_it_open_and_takes_only_what_it_misses() {
        let mut request = asking(4, 4);
        assert_eq!(request.deliver(&stack(ladder(), 1), Quality::NONE), Ok(1));
        assert_eq!(request.state(), RequestState::InProgress);
        assert_eq!(request.outstanding(), 3);
        assert!(!request.satisfied(), "one is below the minimum of four");
        // A stack bigger than what is missing only counts for what is missing.
        assert_eq!(request.deliver(&stack(ladder(), 9), Quality::NONE), Ok(3));
        assert_eq!(request.delivered(), 4);
        assert_eq!(request.state(), RequestState::Delivered);
    }

    #[test]
    fn enough_to_satisfy_is_not_right_to_close() {
        let mut request = asking(10, 4);
        request.deliver(&stack(ladder(), 4), Quality::NONE).expect("4 of 10");
        assert!(request.satisfied(), "four is the minimum it settles for");
        assert_eq!(request.state(), RequestState::InProgress, "but six are missing");
        assert_eq!(request.outstanding(), 6);
    }

    #[test]
    fn another_item_is_refused_by_name() {
        let mut request = asking(4, 4);
        assert_eq!(
            request.deliver(&stack(plank(), 4), Quality::NONE),
            Err(Refusal::ItemAbsent)
        );
        assert_eq!(request.delivered(), 0);
    }

    #[test]
    fn a_delivery_below_the_quality_floor_is_refused_by_name() {
        let mut request =
            CraftingRequest::new(ladder(), 4, 4, Quality::new(2).expect("tier 2 exists"))
                .expect("a nonzero request");
        assert_eq!(
            request.deliver(&stack(ladder(), 4), Quality::new(1).expect("tier 1 exists")),
            Err(Refusal::QualityTooLow)
        );
        assert_eq!(
            request.deliver(&stack(ladder(), 4), Quality::new(2).expect("tier 2 exists")),
            Ok(4),
            "the floor itself is accepted"
        );
    }

    #[test]
    fn a_finished_request_takes_nothing_more() {
        for terminal in [RequestState::Delivered, RequestState::Failed] {
            let mut request = asking(4, 4);
            match terminal {
                RequestState::Delivered => {
                    request.deliver(&stack(ladder(), 4), Quality::NONE).expect("4 of 4");
                }
                _ => request.fail().expect("pending may fail"),
            }
            assert_eq!(request.state(), terminal);
            assert_eq!(request.begin(), Err(Refusal::InvalidState));
            assert_eq!(
                request.deliver(&stack(ladder(), 1), Quality::NONE),
                Err(Refusal::InvalidState)
            );
            assert_eq!(request.fail(), Err(Refusal::InvalidState));
        }
    }

    #[test]
    fn failing_keeps_what_was_already_delivered() {
        let mut request = asking(4, 4);
        request.deliver(&stack(ladder(), 2), Quality::NONE).expect("2 of 4");
        assert_eq!(request.fail(), Ok(()));
        assert_eq!(request.state(), RequestState::Failed);
        assert_eq!(request.delivered(), 2, "the two that arrived are still there");
    }

    #[test]
    fn the_quality_floor_follows_the_building_level() {
        for (level, tier) in [(-3, 0), (0, 0), (1, 0), (2, 0), (3, 1), (4, 2), (5, 3)] {
            assert_eq!(
                Quality::required_for_level(level).tier(),
                tier,
                "a level {level} building asks for tier {tier}"
            );
        }
        assert_eq!(
            Quality::required_for_level(9).tier(),
            Quality::MAX,
            "past the top tier the scale clamps instead of wrapping"
        );
        assert_eq!(Quality::new(Quality::MAX + 1), None);
        assert!(
            Quality::NONE.satisfied_by(Quality::new(1).expect("tier 1")),
            "no requirement is met by anything"
        );
        assert!(
            !Quality::new(1).expect("tier 1").satisfied_by(Quality::NONE),
            "and a requirement is not met by nothing"
        );
    }
}
