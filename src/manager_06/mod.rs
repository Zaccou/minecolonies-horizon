//! Three managers of Java's colony, and the fourth that was already ported.
//!
//! Java reference: `core/colony/managers/ReproductionManager.java` (266 lines),
//! `ResearchManager.java` (266), `StatisticsManager.java` (233) and
//! `TravellingManager.java` (222) — 987 lines of Java, of which this module ports
//! the rules and none of the world.
//!
//! | File | Question | Java |
//! | --- | --- | --- |
//! | `reproduction` | when a colony tries for a child, why it is refused, and in which order a birth is written | `ReproductionManager.java:38-217` |
//! | `autostart` | which research starts by itself, when, and what it writes | `ResearchManager.java:141-265` |
//! | `travelling` | who is on the road, how far along, and where a recall puts him | `TravellingManager.java:39-220` |
//! | `nbt` | the four tag reads those files share | five other copies |
//!
//! **`StatisticsManager` is already ported, twice over, and this module does not
//! repeat it.** Its state — the map `id` → `day` → `quantity`, the increment that
//! adds to a day, the total that sums the days, the inclusive period, the clear,
//! `aggregateStats` and the whole `stat_manager` compound — is
//! `crate::advancements::stats` (`src/advancements/stats.rs`, with the eight
//! `StatisticsConstants` names as constants). Its per-day reads are also
//! `crate::advancements::unlock`'s input. What *is* not there is the half that needs
//! a network: `serialize`/`deserialize` on a `FriendlyByteBuf` with the `dirtyStats`
//! set (`StatisticsManager.java:104-178`) — a partial sync carries one day per id
//! (`:139`), which [`crate::advancements::stats::Stats::latest_day`] already answers,
//! and the packet itself has no destination in a wasm mod (`MANQUANT.md` §1). So the
//! answer for the fourth manager is: written, elsewhere, and the gap is named rather
//! than duplicated.
//!
//! **The same shape in all three files, and it is the point of the module.** Every
//! manager here answers in two steps: a **refusal** named as a value, then a
//! **plan** of writes that is built only after every refusal has been answered. A
//! plan is applied through a `Sink` trait, so "nothing is written before a refusal"
//! is a test — an empty journal — and not a comment. `reproduction` and `autostart`
//! carry their plans as ordered `Write` enums with a `name()`, because the order *is*
//! the rule: Java's sixteen writes of a birth and seven of a finished research are
//! only correct in one order, and a test reads them one by one.
//!
//! **No host call, no clock, no thread, no file.** The day, the two kinds of draw
//! and the config values come in as parameters, like `crate::advancements::stats`
//! takes the day and `crate::manager` takes the claims. That is what a wasm mod
//! gets anyway, and it is what lets these rules be tested with no world.
//!
//! **What is deliberately not here**, each with its cost in [`MANQUANT.md`](MANQUANT):
//! the entity work of a recall (`TeleportHelper`, the "returned very confused" log),
//! the packets and sounds of the three research announcements, and the marriage and
//! naming state a `CitizenData` owns and this port keeps in `crate::citizens`.

pub mod autostart;
mod nbt;
pub mod reproduction;
pub mod travelling;

pub use autostart::{Check, Message, Outcome, Pending, Step, UNIVERSITY};
pub use reproduction::{
    ALT_HOUSE_REACH, CHILD_SPAWN_INTERVAL, Clock, DEFAULT_INITIAL_CITIZEN_AMOUNT, Gate,
    MIN_HOUSING_DIVISOR, MIN_TIME_BEFORE_SPAWNTRY, MIN_SIZE_FOR_REPRO, Plan, Situation, Write,
};
pub use travelling::{
    Book, Recall, Traveller, UPDATE_TRAVELING_INTERVAL,
};

/// Java's colony tick, the amount every manager's timer loses per tick
/// (`TickRateConstants.java:11`: `MAX_TICKRATE`, the interval of the colony state
/// machine).
pub const COLONY_TICK: i32 = reproduction::MAX_TICKRATE;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research::{Buildings, Progress, Tree};

    /// One journal for all three managers: it implements both plans' sinks, so a
    /// test can prove that a whole module wrote nothing.
    #[derive(Default)]
    struct Count(usize);

    impl reproduction::Sink for Count {
        fn write(&mut self, _: &reproduction::Write) {
            self.0 += 1;
        }
    }

    impl autostart::Sink for Count {
        fn write(&mut self, _: &autostart::Write) {
            self.0 += 1;
        }
    }

    /// A draw that never gets asked: the attempt it belongs to is refused before the
    /// first parent is chosen, which is the whole point of the test below.
    struct NoDraw;

    impl reproduction::Draws for NoDraw {
        fn index(&mut self, _: usize) -> usize {
            panic!("a refused attempt draws nothing");
        }

        fn coin(&mut self) -> bool {
            panic!("a refused attempt draws nothing");
        }
    }

    /// The three managers all lose the same 500 ticks per colony tick, and the
    /// travel timer's interval divides it exactly — twenty updates per colony tick,
    /// which is what makes a recall's travel time and a colony tick commensurable.
    #[test]
    fn the_colony_tick_is_five_hundred_and_divides_the_travel_interval() {
        assert_eq!(COLONY_TICK, 500);
        assert_eq!(UPDATE_TRAVELING_INTERVAL, 20);
        assert_eq!(COLONY_TICK % UPDATE_TRAVELING_INTERVAL, 0);
    }

    /// **No write before a refusal, in all three managers at once.** A colony with
    /// its move-in switch off gets no child, a negative travel time gets no
    /// traveller, and a colony with no sifter starts no research: three journals,
    /// three zeroes.
    #[test]
    fn the_three_managers_write_nothing_when_they_refuse() {
        let mut journal = Count::default();

        let situation = Situation {
            gate: reproduction::Gate {
                move_in: false,
                spare_bed_house: Some([0, 70, 0]),
                ..reproduction::Gate::default()
            },
            ..Situation::default()
        };
        let outcome = reproduction::decide(&situation, &mut NoDraw);
        assert_eq!(
            outcome.map(|plan| plan.len()),
            Err(reproduction::Refusal::MoveInOff)
        );

        let mut book = Book::new();
        assert_eq!(
            book.start_to(1, [0, 64, 0], -20),
            Err(travelling::Refusal::NegativeTravelTime {
                citizen: 1,
                ticks: -20
            })
        );
        assert!(book.is_empty());
        assert!(!book.is_dirty(), "a refused start marks no colony");

        let tree = Tree::load().expect("the shipped table loads");
        let mut pending = Pending::from_tree(&tree);
        let check = pending.check(&tree, &Progress::new(), &Buildings::new(), false, true, 0);
        assert_eq!(check.steps.len(), 4, "the four meshes get their turn");
        check.apply(&mut journal);

        assert_eq!(journal.0, 0, "not one write of the three");
        assert_eq!(pending.len(), 4, "and nothing left the waiting list");
    }
}
