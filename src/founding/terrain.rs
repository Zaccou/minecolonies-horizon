//! What the ground answers, and which cell of a box is in the way.
//!
//! **The module never reads the world itself**: everything it knows about a block
//! comes from [`Ground`]. A list of occupied cells is a whole world here, and a real
//! one is what an eventual `level_block_state` call
//! (`crates/horizon-mod-vocab/src/host/levels.rs:304-331`) answers later, from a
//! hook.
//!
//! Java reference: `core/network/messages/server/DirectPlaceMessage.java:117`
//! (`!world.getBlockState(pos).isAir()` and the message is dropped) and
//! `CreateColonyMessage.java:118-126` (the cell must hold a
//! `TileEntityColonyBuilding`), plus
//! `core/colony/buildings/AbstractSchematicProvider.java:266-286` for the box a
//! hut's schematic covers, which is `building::Footprint` and is reused, not
//! re-modelled.
//!
//! **Two questions and not four.** A pathfinder needs water, ladders and doors
//! (`pathfinding::WorldView`); a placement decision needs "is this cell free", and a
//! founding needs "is the town hall here". Everything else is the schematic's
//! business.

use crate::building::Footprint;

/// What this module may ask the world: is the cell empty, and does it hold the town
/// hall.
pub trait Ground {
    /// `level.getBlockState(pos).isAir()` (`DirectPlaceMessage.java:117`). What an
    /// engine counts as empty (air, a light block, an item frame) is the caller's
    /// answer to give.
    fn is_free(&self, at: [i32; 3]) -> bool;

    /// The cell holds the town hall's block **and** its colony block entity:
    /// `tileEntity instanceof TileEntityColonyBuilding` (`CreateColonyMessage.java:118-126`),
    /// which is what makes a click a founding and not a click on a tree. Provided as
    /// `false` so a terrain fake that has no hut only has one question to answer.
    fn holds_town_hall(&self, _at: [i32; 3]) -> bool {
        false
    }
}

/// A world where every cell is empty and nothing is a town hall: open air, and the
/// ground a test that does not care about blocks uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Air;

impl Ground for Air {
    fn is_free(&self, _at: [i32; 3]) -> bool {
        true
    }
}

/// The first cell of `plot` that is not free, `except` set aside.
///
/// `except` is the hut's own position: its block is already in the world when the
/// building is registered (`DirectPlaceMessage.java:137-138`,
/// `RegisteredStructureManager.java:563`), so a rule that counted it would refuse
/// every hut ever placed.
///
/// **One fixed order, x then y then z, both ends included**, so a refusal names
/// the same block on every run and a test can assert the position. Java has no
/// equivalent walk — the refusal is the port's — so the order is the port's to
/// choose, and it chooses what a nested loop writes.
pub fn first_obstacle(ground: &dyn Ground, plot: Footprint, except: [i32; 3]) -> Option<[i32; 3]> {
    let low = plot.low();
    let high = plot.high();
    for x in low[0]..=high[0] {
        for y in low[1]..=high[1] {
            for z in low[2]..=high[2] {
                let at = [x, y, z];
                if at != except && !ground.is_free(at) {
                    return Some(at);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A world with a handful of occupied cells.
    struct Blocks(Vec<[i32; 3]>);

    impl Ground for Blocks {
        fn is_free(&self, at: [i32; 3]) -> bool {
            !self.0.contains(&at)
        }
    }

    #[test]
    fn the_first_obstacle_is_the_lowest_x_then_y_then_z_and_the_hut_is_skipped() {
        let plot = Footprint::between([0, 64, 0], [2, 66, 2]);
        assert_eq!(first_obstacle(&Air, plot, [0, 64, 0]), None, "open air");
        assert_eq!(
            first_obstacle(&Blocks(vec![[2, 64, 2], [1, 66, 0]]), plot, [0, 64, 0]),
            Some([1, 66, 0]),
            "x is the outer loop"
        );
        assert_eq!(
            first_obstacle(&Blocks(vec![[0, 65, 0], [0, 64, 2]]), plot, [0, 64, 0]),
            Some([0, 64, 2]),
            "then z, at the same x: y=64 before y=65"
        );
        assert_eq!(
            first_obstacle(&Blocks(vec![[0, 64, 0]]), plot, [0, 64, 0]),
            None,
            "the hut's own cell is the block that was just placed"
        );
        assert_eq!(
            first_obstacle(&Blocks(vec![[3, 64, 0]]), plot, [0, 64, 0]),
            None,
            "outside the box is none of its business"
        );
    }
}
