//! The contract of a pose: what has to be supplied, and the box it will cover.
//!
//! Java reference: `core/placementhandlers/main/SurvivalHandler.java` (the pose
//! the build tool performs: `setRotationMirror` at `:100`, the anchor at `:101`,
//! the box at `:296-303`) and `com.ldtteam.structurize.util.RotationMirror`
//! (the eight states a blueprint can be pasted in, `:14-21`, built by
//! `of(rotation, mirror) = NONE.mirrorate(mirror).rotate(rotation)` at `:133-136`).
//!
//! **Four things, and no fifth.** A paste needs the template's id
//! ([`super::index::Entry::id`]), where it goes ([`Placing::at`]), which way it
//! faces ([`Orientation`]: a [`Turn`] and a [`Mirror`]), and where the template's
//! own origin sits relative to that position ([`Placing::anchor`]). Java calls
//! the last one `getPrimaryBlockOffset()` and reads it out of the blueprint
//! (`Blueprint.getPrimaryBlockOffset`, `:577-584`); the conversion dropped it
//! (`RAPPORT_BP2NBT.md` §2, *rien*), so here it is the caller's fact.
//!
//! **The box is not `at .. at + size - 1`.** Horizon never writes a pivot:
//! `PlaceSettings::pivot` stays `[0, 0, 0]` (`NOTES_TEMPLATE_NBT.md` §2.1,
//! measured on `processor.rs:1407-1414`), so a quarter turn pastes at
//! `at + (-z, y, x)` and the box's minimum corner is `at.x - size_z + 1`, not
//! `at`. [`Orientation::offsets`] is that transformation applied to the four
//! corners of the template, and it is what [`Placing::boxed`] is built on — so a
//! terrain check and the paste agree about which blocks are touched. Java's own
//! arithmetic (`size - 1` after the offset) is equivalent only because Java
//! re-indexes the grid when it rotates; the test below holds the horizon
//! formula, which is the one that writes blocks.
//!
//! **Why a pose is data.** Nothing here calls the host: `crate::ai::builder`
//! turns a work order into steps against `WorldFacts`, and a pose is one of the
//! facts it needs. That is what makes "does this fit" testable without a world.

use crate::ai::Pos;
use crate::building::Footprint;

use super::Refusal;
use super::index::Resolved;

/// A quarter turn, `net.minecraft.world.level.block.Rotation` as the four values
/// a save ever holds.
///
/// The integer a building stores is `TAG_ROTATION` (`api/util/constant/NbtTagConstants.java:160`),
/// 0 to 3, and `BlockPosUtil.getRotationFromRotations` turns it into one of these
/// four (`api/util/BlockPosUtil.java:1011-1024`); [`turn_from_count`] is that
/// function.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: four quarter turns, the same four vanilla has
pub enum Turn {
    /// `Rotation.NONE`.
    #[default]
    None,
    /// `Rotation.CLOCKWISE_90`.
    Cw90,
    /// `Rotation.CLOCKWISE_180`.
    Cw180,
    /// `Rotation.COUNTERCLOCKWISE_90`.
    Ccw90,
}

/// A mirror, `net.minecraft.world.level.block.Mirror`.
///
/// `RotationMirror` only ever uses [`Mirror::FrontBack`] — its eight states are
/// four turns times "mirrored or not" (`RotationMirror.java:14-21`) — but the
/// engine's `StructureTemplate::transform` handles all three
/// (`crates/horizon-world/src/worldgen/structure/template.rs:479-497`), and a
/// contract that cannot express `LeftRight` would be hiding a state the engine
/// writes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: three mirrors, the same three vanilla has
pub enum Mirror {
    /// `Mirror.NONE`.
    #[default]
    None,
    /// `Mirror.FRONT_BACK`: the x axis is flipped.
    FrontBack,
    /// `Mirror.LEFT_RIGHT`: the z axis is flipped.
    LeftRight,
}

/// Which way a template faces: a turn and a mirror, Java's `RotationMirror`.
///
/// The order is fixed and it is the engine's: the mirror is applied **first**,
/// then the turn (`RotationMirror.java:133-136`, `NONE.mirrorate(mirror).rotate(rotation)`;
/// same order in `transform`, `template.rs:485-496`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Orientation {
    /// The quarter turn.
    pub turn: Turn,
    /// The mirror, applied before the turn.
    pub mirror: Mirror,
}

impl Orientation {
    /// A pose facing `turn`, mirrored `mirror`.
    pub const fn new(turn: Turn, mirror: Mirror) -> Orientation {
        Orientation { turn, mirror }
    }

    /// Where one cell of the template lands, relative to the template's own
    /// origin: `StructureTemplate.transform(cell, mirror, rotation, [0,0,0])`
    /// (`crates/horizon-world/src/worldgen/structure/template.rs:479-497`).
    ///
    /// The pivot is always zero in Horizon (`NOTES_TEMPLATE_NBT.md` §2.1), which
    /// is why there is no pivot argument here at all.
    pub fn transform(self, cell: [i32; 3]) -> [i32; 3] {
        let [x, y, z] = cell;
        let (x, z) = match self.mirror {
            Mirror::FrontBack => (x.wrapping_neg(), z),
            Mirror::LeftRight => (x, z.wrapping_neg()),
            Mirror::None => (x, z),
        };
        match self.turn {
            Turn::None => [x, y, z],
            // `Clockwise90 => [px + pz - z, y, pz - px + x]`, pivot zero.
            Turn::Cw90 => [z.wrapping_neg(), y, x],
            // `Clockwise180 => [px + px - x, y, pz + pz - z]`, pivot zero.
            Turn::Cw180 => [x.wrapping_neg(), y, z.wrapping_neg()],
            // `Counterclockwise90 => [px - pz + z, y, px + pz - x]`, pivot zero.
            Turn::Ccw90 => [z, y, x.wrapping_neg()],
        }
    }

    /// The size of the pasted box: a quarter turn swaps x and z, a half turn
    /// does not (`Blueprint.setRotationMirrorRelative`, `Blueprints:648-661`;
    /// same table in `StructureTemplate::size`, `template.rs:471-478`).
    pub fn size(self, size: [u32; 3]) -> [u32; 3] {
        match self.turn {
            Turn::Cw90 | Turn::Ccw90 => [size[2], size[1], size[0]],
            Turn::None | Turn::Cw180 => size,
        }
    }

    /// The smallest and largest offset the pasted cells reach on each axis,
    /// relative to the template's origin.
    ///
    /// Only the four corners need transforming: the transformation is axis
    /// aligned, so the extremes of the image are the images of the extremes.
    /// The vertical axis is never touched by a turn or a mirror.
    pub fn offsets(self, size: [u32; 3]) -> ([i32; 3], [i32; 3]) {
        let far_x = size[0].saturating_sub(1) as i32;
        let far_z = size[2].saturating_sub(1) as i32;
        let far_y = size[1].saturating_sub(1) as i32;
        let mut low = [i32::MAX, 0, i32::MAX];
        let mut high = [i32::MIN, far_y, i32::MIN];
        for x in [0, far_x] {
            for z in [0, far_z] {
                let [tx, _, tz] = self.transform([x, 0, z]);
                low[0] = low[0].min(tx);
                low[2] = low[2].min(tz);
                high[0] = high[0].max(tx);
                high[2] = high[2].max(tz);
            }
        }
        (low, high)
    }

    /// The same pose, turned a further quarter: Java's `rotate(offset)`
    /// (`RotationMirror.java:104-113`), four steps and back where it started.
    pub fn rotated(self, turn: Turn) -> Orientation {
        let step = |current: Turn| match (current, turn) {
            (turn, Turn::None) => turn,
            (Turn::None, Turn::Cw90) => Turn::Cw90,
            (Turn::Cw90, Turn::Cw90) => Turn::Cw180,
            (Turn::Cw180, Turn::Cw90) => Turn::Ccw90,
            (Turn::Ccw90, Turn::Cw90) => Turn::None,
            (Turn::None, Turn::Cw180) => Turn::Cw180,
            (Turn::Cw90, Turn::Cw180) => Turn::Ccw90,
            (Turn::Cw180, Turn::Cw180) => Turn::None,
            (Turn::Ccw90, Turn::Cw180) => Turn::Cw90,
            (Turn::None, Turn::Ccw90) => Turn::Ccw90,
            (Turn::Cw90, Turn::Ccw90) => Turn::None,
            (Turn::Cw180, Turn::Ccw90) => Turn::Cw90,
            (Turn::Ccw90, Turn::Ccw90) => Turn::Cw180,
        };
        Orientation {
            turn: step(self.turn),
            mirror: self.mirror,
        }
    }
}

/// The turn a building's `rotation` tag names:
/// `BlockPosUtil.getRotationFromRotations` (`api/util/BlockPosUtil.java:1011-1024`).
///
/// The four constants are `ROTATE_ONCE`/`TWICE`/`THREE_TIMES`
/// (`api/util/constant/Constants.java:15-17`), and anything else — a corrupt save,
/// a negative — is `Rotation.NONE`, which is Java's own `default:` branch.
pub fn turn_from_count(count: i32) -> Turn {
    match count {
        1 => Turn::Cw90,
        2 => Turn::Cw180,
        3 => Turn::Ccw90,
        _ => Turn::None,
    }
}

/// The build height, inclusive at the bottom and **exclusive at the top**.
///
/// The engine's own bounds: `min_y() == -64`, `max_y() == 320`
/// (`crates/horizon-world/src/level/mod.rs:913-919`, "inclusive minimum and
/// exclusive maximum build height"). A blueprint that would write at `y == 320`
/// is refused, not clamped: half a house is worse than none.
///
/// This is a parameter and not a field of `crate::ai::WorldFacts` because the
/// facts carry what a hook observed, and no hook reports a build height yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The lowest block a paste may write.
    pub min_y: i32,
    /// The first height a paste may **not** write.
    pub max_y: i32,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            min_y: -64,
            max_y: 320,
        }
    }
}

impl Limits {
    /// Bounds given by the caller, for a level that is not the default one.
    pub const fn new(min_y: i32, max_y: i32) -> Limits {
        Limits { min_y, max_y }
    }

    /// Whether a vertical span fits: the two ends, both included, inside
    /// `min_y .. max_y`.
    pub fn holds(&self, low_y: i32, high_y: i32) -> bool {
        low_y >= self.min_y && high_y < self.max_y
    }
}

/// A pose: the template, the position, the facing and the anchor.
///
/// Distinct from [`crate::ai::Placement`], which is the *result* of one structure
/// step (`BlockPlacementResult.Result`). This one is an *input*.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placing {
    /// The template id, `minecolonies:original/fundamentals/builder3`.
    pub id: String,
    /// Where the hut block goes — Java's `blockPos`
    /// (`SurvivalHandler.java:100-101`).
    pub at: Pos,
    /// The template's primary block offset, in **unrotated** template
    /// coordinates: `getPrimaryBlockOffset()`. `[0, 0, 0]` is the template's
    /// own corner, which is all the conversion left us (`RAPPORT_BP2NBT.md` §2).
    pub anchor: [i32; 3],
    /// The facing.
    pub orientation: Orientation,
}

impl Placing {
    /// A template pasted at `at`, unrotated, anchored on its own corner.
    pub fn new(id: impl Into<String>, at: Pos) -> Placing {
        Placing {
            id: id.into(),
            at,
            anchor: [0, 0, 0],
            orientation: Orientation::default(),
        }
    }

    /// The same, for a structure the index just answered: the id comes from the
    /// file, not from the name that was asked for.
    pub fn for_structure(resolved: &Resolved<'_>, at: Pos) -> Placing {
        Placing::new(resolved.id(), at)
    }

    /// Move the anchor, in unrotated template coordinates.
    pub fn with_anchor(mut self, anchor: [i32; 3]) -> Placing {
        self.anchor = anchor;
        self
    }

    /// Face another way. An absolute setter, unlike [`Orientation::rotated`]
    /// which adds a quarter turn: `with_turn(Cw90).with_turn(Cw90)` is
    /// [`Turn::Cw90`], not [`Turn::Cw180`].
    pub fn with_turn(mut self, turn: Turn) -> Placing {
        self.orientation.turn = turn;
        self
    }

    /// Mirror. An absolute setter, like [`Self::with_turn`].
    pub fn with_mirror(mut self, mirror: Mirror) -> Placing {
        self.orientation.mirror = mirror;
        self
    }

    /// The box the paste covers, in world coordinates.
    ///
    /// `at - anchor`, then the transformed template: the engine writes each cell
    /// at `origin + transform(cell)`, and `Orientation::offsets` already holds
    /// the extremes of that image.
    pub fn boxed(&self, size: [u32; 3]) -> Footprint {
        let (low, high) = self.orientation.offsets(size);
        let anchor = self.orientation.transform(self.anchor);
        let origin = [
            self.at.x - anchor[0],
            self.at.y - anchor[1],
            self.at.z - anchor[2],
        ];
        Footprint::between(
            [origin[0] + low[0], origin[1] + low[1], origin[2] + low[2]],
            [origin[0] + high[0], origin[1] + high[1], origin[2] + high[2]],
        )
    }

    /// The box, once the terrain is known to be big enough for it.
    ///
    /// Java's own "too big" answer is the building's box against a footprint
    /// (`AbstractSchematicProvider.java:505-521`, one block of slack); here the
    /// site is handed in as a [`Footprint`] and the comparison is exact, because
    /// a caller that has the site already knows where its edges are.
    pub fn site(&self, size: [u32; 3], site: &Footprint) -> Result<Footprint, Refusal> {
        let needs = self.orientation.size(size);
        if !site.covers(needs) {
            return Err(Refusal::TooBigForSite {
                needs,
                site: site.size(),
            });
        }
        Ok(self.boxed(size))
    }

    /// The box, once the build height is known to allow it.
    pub fn within(&self, size: [u32; 3], limits: &Limits) -> Result<Footprint, Refusal> {
        let boxed = self.boxed(size);
        let low = boxed.low()[1];
        let high = boxed.high()[1];
        if limits.holds(low, high) {
            Ok(boxed)
        } else {
            Err(Refusal::OutOfWorld {
                low,
                high,
                min_y: limits.min_y,
                max_y: limits.max_y,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_transform_is_the_engines_table() {
        // `StructureTemplate::transform` with a zero pivot
        // (`template.rs:479-497`): `Clockwise90 => [-z, y, x]`,
        // `Counterclockwise90 => [z, y, -x]`, `Clockwise180 => [-x, y, -z]`,
        // the mirror applied first (`FrontBack` negates x, `LeftRight` negates z).
        let cell = [3, 7, 5];
        for (turn, expected) in [
            (Turn::None, [3, 7, 5]),
            (Turn::Cw90, [-5, 7, 3]),
            (Turn::Cw180, [-3, 7, -5]),
            (Turn::Ccw90, [5, 7, -3]),
        ] {
            assert_eq!(
                Orientation::new(turn, Mirror::None).transform(cell),
                expected,
                "{turn:?}"
            );
        }
        for (mirror, expected) in [
            (Mirror::None, [3, 7, 5]),
            (Mirror::FrontBack, [-3, 7, 5]),
            (Mirror::LeftRight, [3, 7, -5]),
        ] {
            assert_eq!(
                Orientation::new(Turn::None, mirror).transform(cell),
                expected,
                "{mirror:?}"
            );
        }
        // Mirror then turn, in that order: FrontBack on x, then Cw90 → [-z, y, -x].
        assert_eq!(
            Orientation::new(Turn::Cw90, Mirror::FrontBack).transform(cell),
            [-5, 7, -3]
        );
        assert_eq!(
            Orientation::new(Turn::Ccw90, Mirror::LeftRight).transform(cell),
            [-5, 7, -3],
            "a half turn either way is the same flip"
        );
    }

    #[test]
    fn a_quarter_turn_swaps_the_two_horizontal_axes_of_the_size() {
        // `Blueprint.setRotationMirrorRelative` (`:648-661`).
        let size = [15, 8, 13];
        assert_eq!(Orientation::new(Turn::None, Mirror::None).size(size), size);
        assert_eq!(
            Orientation::new(Turn::Cw180, Mirror::None).size(size),
            size,
            "a half turn changes no dimension"
        );
        assert_eq!(
            Orientation::new(Turn::Cw90, Mirror::None).size(size),
            [13, 8, 15]
        );
        assert_eq!(
            Orientation::new(Turn::Ccw90, Mirror::FrontBack).size(size),
            [13, 8, 15],
            "a mirror never swaps either"
        );
    }

    #[test]
    fn an_unrotated_paste_covers_at_to_at_plus_size_minus_one() {
        // `manual/fundamentals/builder1.nbt` is [15, 8, 13], measured.
        let placing = Placing::new("minecolonies:original/fundamentals/builder1", Pos::new(10, 64, -3));
        let boxed = placing.boxed([15, 8, 13]);
        assert_eq!(boxed.low(), [10, 64, -3]);
        assert_eq!(boxed.high(), [24, 71, 9]);
        assert_eq!(boxed.size(), [15, 8, 13]);
    }

    #[test]
    fn a_quarter_turn_moves_the_box_behind_the_anchor_not_in_front_of_it() {
        // The measured rule: with a zero pivot, a `clockwise_90` paste at `at`
        // occupies `at + (-z, y, x)`, so its minimum corner is
        // `(at.x - size_z + 1, at.y, at.z)` — `NOTES_TEMPLATE_NBT.md` §2.1.
        let placing = Placing::new("minecolonies:original/fundamentals/builder1", Pos::new(10, 64, -3))
            .with_turn(Turn::Cw90);
        let boxed = placing.boxed([15, 8, 13]);
        assert_eq!(boxed.low(), [10 - 13 + 1, 64, -3]);
        assert_eq!(boxed.high(), [10, 64 + 7, -3 + 15 - 1]);
        assert_eq!(boxed.size(), [13, 8, 15]);
        // The other quarter turn goes the other way round the anchor.
        let other = Placing::new("x", Pos::new(10, 64, -3)).with_turn(Turn::Ccw90);
        let boxed = other.boxed([15, 8, 13]);
        assert_eq!(boxed.low(), [10, 64, -3 - 15 + 1]);
        assert_eq!(boxed.high(), [10 + 13 - 1, 71, -3]);
    }

    #[test]
    fn a_half_turn_and_a_mirror_flip_the_box_the_way_the_engine_writes_it() {
        let at = Pos::new(10, 64, -3);
        let half = Placing::new("x", at).with_turn(Turn::Cw180).boxed([15, 8, 13]);
        assert_eq!(half.low(), [10 - 14, 64, -3 - 12]);
        assert_eq!(half.high(), [10, 71, -3]);
        // `Mirror.FRONT_BACK` negates x only, so the box runs to the -x side.
        let mirrored = Placing::new("x", at)
            .with_mirror(Mirror::FrontBack)
            .boxed([15, 8, 13]);
        assert_eq!(mirrored.low(), [10 - 14, 64, -3]);
        assert_eq!(mirrored.high(), [10, 71, -3 + 12]);
        // `Mirror.LEFT_RIGHT` negates z only.
        let left_right = Placing::new("x", at)
            .with_mirror(Mirror::LeftRight)
            .boxed([15, 8, 13]);
        assert_eq!(left_right.low(), [10, 64, -3 - 12]);
        assert_eq!(left_right.high(), [24, 71, -3]);
    }

    #[test]
    fn the_anchor_shifts_the_whole_paste() {
        // Java's `zeroPos = blockPos.subtract(getPrimaryBlockOffset())`
        // (`SurvivalHandler.java:299`), with the offset rotated like the
        // blueprint it came from.
        let at = Pos::new(10, 64, -3);
        let placing = Placing::new("x", at).with_anchor([7, 0, 6]);
        let boxed = placing.boxed([15, 8, 13]);
        assert_eq!(boxed.low(), [10 - 7, 64, -3 - 6]);
        assert_eq!(boxed.high(), [10 - 7 + 14, 64 + 7, -3 - 6 + 12]);
        // A quarter turn rotates the anchor too: [7, 0, 6] → [-6, 0, 7].
        let turned = placing.with_turn(Turn::Cw90);
        assert_eq!(turned.orientation.transform([7, 0, 6]), [-6, 0, 7]);
        let boxed = turned.boxed([15, 8, 13]);
        assert_eq!(boxed.low(), [10 + 6 - 12, 64, -3 - 7]);
        assert_eq!(boxed.high(), [10 + 6, 71, -3 - 7 + 14]);
    }

    #[test]
    fn a_terrain_too_small_for_the_blueprint_is_refused_naming_both_sizes() {
        let placing = Placing::new("minecolonies:original/fundamentals/builder1", Pos::new(0, 64, 0));
        // A 3×3 plot cannot hold a 15×8×13 builder, measured.
        let plot = Footprint::at_size([0, 64, 0], [3, 3, 3]);
        assert_eq!(
            placing.site([15, 8, 13], &plot),
            Err(Refusal::TooBigForSite {
                needs: [15, 8, 13],
                site: [3, 3, 3],
            })
        );
        // A plot that is big enough answers the box itself.
        let plot = Footprint::at_size([0, 64, 0], [40, 20, 40]);
        let boxed = placing.site([15, 8, 13], &plot).unwrap();
        assert_eq!(boxed.size(), [15, 8, 13]);
        // A quarter turn asks for the swapped box: 13 wide, 15 deep.
        assert!(placing
            .with_turn(Turn::Cw90)
            .site([15, 8, 13], &Footprint::at_size([0, 64, 0], [13, 8, 15]))
            .is_ok());
    }

    #[test]
    fn a_paste_above_the_build_height_is_refused_not_clamped() {
        // `manual/fundamentals/townhall5.nbt` is [24, 22, 18], measured: 22 tall.
        let tall = [24, 22, 18];
        let limits = Limits::default();
        assert_eq!(limits, Limits::new(-64, 320), "the engine's own bounds");
        // At y = 298 the top block is 319, the last one that fits.
        let placing = Placing::new("minecolonies:original/fundamentals/townhall5", Pos::new(0, 298, 0));
        assert!(placing.within(tall, &limits).is_ok());
        // At y = 299 the top block is 320, which the engine refuses to write.
        let too_high = Placing::new("x", Pos::new(0, 299, 0));
        assert_eq!(
            too_high.within(tall, &limits),
            Err(Refusal::OutOfWorld {
                low: 299,
                high: 320,
                min_y: -64,
                max_y: 320,
            })
        );
        // The bottom is inclusive: a one-block-floor hut at -64 is allowed.
        let low = Placing::new("x", Pos::new(0, -64, 0));
        assert!(low.within([3, 1, 3], &limits).is_ok());
        let below = Placing::new("x", Pos::new(0, -65, 0));
        assert_eq!(
            below.within([3, 1, 3], &limits),
            Err(Refusal::OutOfWorld {
                low: -65,
                high: -65,
                min_y: -64,
                max_y: 320,
            })
        );
    }

    #[test]
    fn the_rotation_tag_is_the_four_quarter_turns_and_nothing_else() {
        // `BlockPosUtil.getRotationFromRotations` (`:1011-1024`), constants
        // 1/2/3 (`Constants.java:15-17`) and Java's `default:` branch.
        assert_eq!(turn_from_count(0), Turn::None);
        assert_eq!(turn_from_count(1), Turn::Cw90);
        assert_eq!(turn_from_count(2), Turn::Cw180);
        assert_eq!(turn_from_count(3), Turn::Ccw90);
        assert_eq!(turn_from_count(4), Turn::None, "out of range is no turn");
        assert_eq!(turn_from_count(-1), Turn::None);
        assert_eq!(turn_from_count(i32::MAX), Turn::None);
    }

    #[test]
    fn four_quarter_turns_come_back_to_where_they_started() {
        // `RotationMirror.rotate` (`:104-113`) composes, and the group closes.
        let start = Orientation::new(Turn::None, Mirror::FrontBack);
        let once = start.rotated(Turn::Cw90);
        assert_eq!(once.turn, Turn::Cw90);
        assert_eq!(once.mirror, Mirror::FrontBack, "a turn does not unmirror");
        let twice = once.rotated(Turn::Cw90);
        assert_eq!(twice.turn, Turn::Cw180);
        let thrice = twice.rotated(Turn::Cw90);
        assert_eq!(thrice.turn, Turn::Ccw90);
        assert_eq!(thrice.rotated(Turn::Cw90).turn, Turn::None);
        // The long way round lands on the same place.
        assert_eq!(start.rotated(Turn::Cw180).turn, Turn::Cw180);
        assert_eq!(
            start.rotated(Turn::Cw90).rotated(Turn::Ccw90).turn,
            Turn::None
        );
        // `Turn::None` changes nothing.
        assert_eq!(start.rotated(Turn::None), start);
    }

    #[test]
    fn a_pose_takes_its_id_from_the_file_that_answered() {
        use super::super::index::{Entry, Index};
        use super::super::naming::Style;
        use crate::building::Kind;

        let mut index = Index::new();
        index.insert(Entry::parse("original", "fundamentals/tavern1", [13, 14, 15]).unwrap());
        index.insert(Entry::parse("original", "fundamentals/tavern3", [13, 14, 15]).unwrap());
        let style = Style::parse("original").unwrap();
        // Asked for level 4, the pack answers `tavern3` — and the pose names the
        // file, not the wish.
        let resolved = index.resolve(&style, Kind::Tavern, 4).unwrap();
        assert!(resolved.capped());
        let placing = Placing::for_structure(&resolved, Pos::new(0, 64, 0))
            .with_turn(turn_from_count(2));
        assert_eq!(
            placing.id,
            "minecolonies:original/fundamentals/tavern3",
            "a caller that pasted the wished-for id would paste a file that does not exist"
        );
        assert_eq!(placing.orientation.turn, Turn::Cw180);
        // The box is the answered file's size, [13, 14, 15].
        assert_eq!(placing.boxed(resolved.entry.size()).size(), [13, 14, 15]);
    }
}
