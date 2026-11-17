//! The first version's numbers, against the Java they claim.
//!
//! `crate::pathfinding` was written before this module, from the same sources,
//! and mostly gets the arithmetic right. This file is the audit: for every rule
//! the first version already ported, a test that pins the value and names the
//! Java line it comes from — and, for the four places the port and the Java
//! disagree, a test that pins **both** sides so the disagreement is a fact in
//! the tree rather than a sentence in a report.
//!
//! | Rule | Where | Agrees? |
//! | --- | --- | --- |
//! | the twelve prices of `PathingOptions` | `Costs::default` | yes |
//! | a level step, a jump, a fall, a swim, a climb | `Costs::step` | yes, except ladders |
//! | the turn penalty table | `Costs::turn` | yes |
//! | `MAX_NODES`, `MAX_COST`, the jump and fall bounds | `node.rs` | yes |
//! | the node ceiling of a *range* job | `Limits::for_range` | **no**, no floor |
//! | the node ceiling of a *walk-to* job | `Limits::for_range` | **no**, the wrong formula |
//! | a door is opened by `enterGates` | `Costs::doors_open` | **no** |
//! | every climb entry costs the non-ladder price | `Costs::step` | **no**, ladders are free |
//!
//! The four disagreements are the ones this module fixes; the fix lives here,
//! the first version still has to adopt it (it is another module's file).

#[cfg(test)]
mod tests {
    use super::super::ceiling::Ceiling;
    use super::super::classify::{Barrier, passes};
    use super::super::job::{Job, Options};
    use crate::pathfinding::{
        Costs, Limits, MAX_COST, MAX_DROP, MAX_DROP_FORBIDDEN, MAX_JUMP_HEIGHT, MAX_JUMP_STEP,
        MAX_NODES, NodeKind,
    };

    #[test]
    fn the_twelve_prices_are_the_ones_pathing_options_declares() {
        let c = Costs::default();
        // PathingOptions.java:18, :23, :43, :53, :58, :63, :13.
        assert_eq!(c.jump, 2.0);
        assert_eq!(c.drop, 1.0);
        assert_eq!(c.swim, 4.0);
        assert_eq!(c.swim_enter, 24.0);
        assert_eq!(c.toggle, 2.0);
        assert_eq!(c.climb, 3.0);
        assert_eq!(MAX_COST, 25.0);
        // `computeCost` starts at the literal 1 (`AbstractPathJob.java:1030`).
        assert_eq!(c.base, 1.0);
        // The three switches, and their defaults (`:92`, `:96`, `:101`, `:125`).
        assert!(!c.can_swim && !c.enter_doors && c.enter_gates && c.can_drop);
        assert_eq!(c.turn, 0.0, "turnPenalty = 0 (:130)");
    }

    #[test]
    fn a_level_step_costs_one_a_jump_three_and_a_fall_the_cube() {
        let c = Costs::default();
        // `cost = 1`, `dY > 0` adds `jumpCost = 2` (`:1050-1056`).
        assert_eq!(c.step(NodeKind::Walk, NodeKind::Walk, 0, false), 1.0);
        assert_eq!(c.step(NodeKind::Walk, NodeKind::Walk, 1, false), 3.0);
        // `dropCost * |dY * dY * dY|` (`:1062-1065`).
        assert_eq!(c.step(NodeKind::Walk, NodeKind::Fall, -1, false), 2.0);
        assert_eq!(c.step(NodeKind::Walk, NodeKind::Fall, -2, false), 9.0);
        assert_eq!(c.step(NodeKind::Walk, NodeKind::Fall, -3, false), 28.0);
    }

    #[test]
    fn water_costs_a_surcharge_to_enter_and_a_smaller_one_to_stay() {
        let c = Costs::default();
        // `swimStart ? swimCostEnter : swimCost` (`:1106-1114`).
        assert_eq!(c.step(NodeKind::Walk, NodeKind::Swim, 0, true), 25.0);
        assert_eq!(c.step(NodeKind::Swim, NodeKind::Swim, 0, false), 5.0);
        assert_eq!(c.step(NodeKind::Walk, NodeKind::Walk, 0, true), 1.0, "dry");
    }

    #[test]
    fn a_toggle_and_a_climb_are_charged_once() {
        let c = Costs::default();
        // `traverseToggleAbleCost` when the state has an OPEN property
        // (`:1080-1083`).
        assert_eq!(c.step(NodeKind::Walk, NodeKind::Door, 0, false), 3.0);
        // `nonLadderClimbableCost` when the climb starts (`:1099-1101`).
        assert_eq!(c.step(NodeKind::Walk, NodeKind::Climb, 1, false), 6.0);
        assert_eq!(c.step(NodeKind::Climb, NodeKind::Climb, 1, false), 1.0);
    }

    #[test]
    fn the_turn_penalty_is_half_full_or_double_of_the_dot_product() {
        let p = Costs::default().with_turn_penalty(2.0);
        // `dot == 1` → 0.5P, `dot == 0` → P, else 2P (`:1791-1806`).
        assert_eq!(p.turn((0, 1), (1, 1)), 1.0, "a corner node's quarter turn");
        assert_eq!(p.turn((1, 0), (0, 1)), 2.0, "a right angle");
        assert_eq!(p.turn((1, 0), (-1, 0)), 4.0, "a u-turn");
        assert_eq!(p.turn((1, 0), (1, 0)), 0.0, "straight on");
        assert_eq!(p.turn((0, 0), (1, 0)), 0.0, "a vertical move is no turn");
    }

    #[test]
    fn the_bounds_are_the_constants_of_abstractpathjob() {
        // `MAX_NODES = 8000` (`AbstractPathJob.java:59`).
        assert_eq!(MAX_NODES, 8_000);
        // `MAX_JUMP_HEIGHT = 1.3` (`api/util/constant/PathingConstants.java:44`),
        // which at block resolution is exactly one block.
        assert_eq!(MAX_JUMP_HEIGHT, 1.3);
        assert_eq!(MAX_JUMP_STEP, 1, "1.3 blocks is one block and not two");
        // `canDrop ? 10 : 2` (`AbstractPathJob.java:1624`).
        assert_eq!(MAX_DROP, 10);
        assert_eq!(MAX_DROP_FORBIDDEN, 2);
    }

    #[test]
    fn the_range_ceiling_of_the_first_version_has_no_floor() {
        // Java: `range = Math.max(10, range)` then `min(MAX_NODES, range²)`
        // (`AbstractPathJob.java:190-193`).
        assert_eq!(Ceiling::of_range(0).base, 100, "Java");
        assert_eq!(
            Limits::for_range(0).max_nodes,
            0,
            "the first version: no floor, so a range of zero buys nothing"
        );
        assert_eq!(Ceiling::of_range(20).base, 400);
        assert_eq!(
            Limits::for_range(20).max_nodes,
            420,
            "and it adds the range"
        );
    }

    #[test]
    fn a_walk_to_job_is_not_priced_by_its_range_at_all() {
        // `PathJobMoveToLocation` calls the *corridor* constructor
        // (`:41`, `:271-283`) and then `maxNodes += range` (`:43`).
        let java = Job::MoveToLocation {
            destination: [10, 0, 0],
            range: 30,
        }
        .ceiling([0, 0, 0]);
        assert_eq!(java.base, 738, "708 from the corridor, 30 from the range");
        assert_eq!(
            Limits::for_range(30).max_nodes,
            930,
            "the first version would give this, which is a different job's cap"
        );
    }

    #[test]
    fn a_door_is_not_a_gate_and_the_first_version_says_it_is() {
        // Java asks `canEnterDoors` for a door and `canEnterGates` for a gate
        // (`AbstractPathJob.java:1445-1447`).
        assert!(
            !passes(Barrier::Door, false, true),
            "Java, with the defaults"
        );
        assert!(passes(Barrier::Gate, false, true), "Java, for a gate");
        assert!(
            Costs::default().doors_open(),
            "the first version: enterGates alone opens a door too"
        );
        // The consequence: with the defaults a citizen walks through a closed
        // door, where Java has it compute a path around.
        let options = Options::default();
        assert!(
            !options.doors && options.gates,
            "the defaults are the trigger"
        );
    }

    #[test]
    fn a_ladder_costs_nothing_to_start_in_java_and_the_price_here_it_does() {
        // Java charges the non-ladder price only when the block is *not* a
        // `LadderBlock` (`AbstractPathJob.java:1099-1101`): a vine start is 6, a
        // ladder start is 3.
        let c = Costs::default();
        assert_eq!(c.step(NodeKind::Walk, NodeKind::Climb, 1, false), 6.0);
        assert_eq!(
            1.0 + c.jump,
            3.0,
            "what Java charges for a real ladder: the step and the jump, no climb"
        );
        // The cost of keeping the first version's shape: a ladder in a mine
        // shaft is priced like a vine, so the search prefers a stair.
    }

    #[test]
    fn the_first_version_never_charges_for_walking_inside_a_shape() {
        // `walkInShapesCost = 1` is added when the cell has a collision and the
        // body is not on a colony path (`AbstractPathJob.java:1086-1087`).
        let options = Options::default();
        assert_eq!(options.in_shapes, 1.0, "PathingOptions.java:68");
        assert_eq!(
            options
                .costs()
                .step(NodeKind::Walk, NodeKind::Walk, 0, false),
            1.0,
            "and the narrowing to Costs cannot carry it: MANQUANT.md §2"
        );
    }

    #[test]
    fn the_four_cheaper_permissions_are_kept_even_where_the_search_ignores_them() {
        let pathway = Job::Pathway {
            end: [0, 0, 0],
            colony: 1,
        }
        .options();
        // `canUseRails`, `canOpenDoors`, `canClimbAdvanced`, `canPassDanger`
        // have no field in `Costs` at all (`PathingOptions.java:88`, `:106`,
        // `:110`, `:115`).
        assert!(!pathway.rails && !pathway.open_doors);
        assert!(!pathway.climb_advanced && !pathway.pass_danger);
        assert!(pathway.swim_allowed && pathway.doors && pathway.gates);
        assert!(
            pathway.costs().can_swim && pathway.costs().enter_doors,
            "the four that do have one are carried"
        );
    }
}
