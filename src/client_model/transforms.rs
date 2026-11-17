//! The rotations a block model, an element and an item display carry.
//!
//! Java reference: `assets/minecolonies/models/block/**.json` and
//! `models/item/*.json` (216 block models, 170 item models), read by vanilla's
//! `BlockModelRotation` for a variant's `x`/`y`, by `BlockElementRotation` for an
//! element's `rotation`, and by `ItemTransform` for a model's `display`.
//!
//! | What turns | Where | Values in this mod |
//! | --- | --- | --- |
//! | the whole model, per state | a variant's `x`, `y`, `z` | multiples of 90°, four distinct `y` in 64 files and no `z` at all |
//! | one element, around a pivot | an element's `rotation` | −45, −22.5, 0, 22.5, 45 on all three axes |
//! | the item in a hand | a model's `display` | 8 contexts, and 14 files that write the 2 names vanilla dropped |
//!
//! **Measured on the 216 block models and the 170 item models** (re-run
//! `python3 src/client_model/generer_table.py --audit-modeles`): 2 105 elements,
//! **1 540 of them carrying a `rotation`**, and **912 of those are `axis: y,
//! angle: 0`** - a rotation that does nothing. Blockbench writes one for every
//! pivot an author sets, so 43 % of all elements and 59 % of the rotation records
//! are no-ops. The angles are 22.5° steps (its default grid) - 109 at −45°,
//! 168 at −22.5°, 203 at 22.5°, 132 at 45° - never the 90° the blockstate uses:
//! **the two never mix** - no blockstate names a quarter turn that an element
//! rotation would undo.
//!
//! **The convention, in one line.** A variant's turn is *negative* and applied X
//! then Y then Z about the block's centre (`hooks/standalone_models.rs:2307`,
//! "centered negative XYZ quarter-turns"; `client/types.rs:662`, the same words
//! for the standalone form). So `"y": 90` brings +X to +Z, which is the
//! clockwise turn seen from above that every blockstate in this mod relies on.
//! [`turn_point`] does that exactly, in integers: no trigonometry, so four turns
//! are the identity bit for bit.
//!
//! **What this file refuses to do**: apply `rescale` on an element rotation. One
//! element of the mod sets it (`y: 45`, 22 occurrences, the same element repeated
//! through the huts), and the factor belongs to the game's baker, not to the mod -
//! `MANQUANT.md` §3 says so rather than guessing a scale.

use mod_api::serde_json::{Map, Value};

/// The point every quarter turn turns around: the middle of the block.
pub const CENTRE: [f32; 3] = [0.5, 0.5, 0.5];

/// One of the six sides, by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: six sides, and the tables are indexed by them
pub enum Face {
    /// `-Z`.
    North,
    /// `+Z`.
    South,
    /// `+X`.
    East,
    /// `-X`.
    West,
    /// `+Y`.
    Up,
    /// `-Y`.
    Down,
}

impl Face {
    /// All six, in the order `Direction` declares them.
    pub const ALL: [Face; 6] = [
        Face::Down,
        Face::Up,
        Face::North,
        Face::South,
        Face::West,
        Face::East,
    ];

    /// The name the model JSON and the draw calls use.
    pub fn key(self) -> &'static str {
        match self {
            Face::North => "north",
            Face::South => "south",
            Face::East => "east",
            Face::West => "west",
            Face::Up => "up",
            Face::Down => "down",
        }
    }

    /// The face a key names, or `None`.
    pub fn from_key(key: &str) -> Option<Face> {
        Face::ALL.iter().copied().find(|face| face.key() == key)
    }

    /// The unit normal, which is what a quarter turn permutes.
    pub fn normal(self) -> [f32; 3] {
        match self {
            Face::North => [0.0, 0.0, -1.0],
            Face::South => [0.0, 0.0, 1.0],
            Face::East => [1.0, 0.0, 0.0],
            Face::West => [-1.0, 0.0, 0.0],
            Face::Up => [0.0, 1.0, 0.0],
            Face::Down => [0.0, -1.0, 0.0],
        }
    }

    /// The opposite face, which is the one a full turn of the block puts where
    /// this one was.
    pub fn opposite(self) -> Face {
        match self {
            Face::North => Face::South,
            Face::South => Face::North,
            Face::East => Face::West,
            Face::West => Face::East,
            Face::Up => Face::Down,
            Face::Down => Face::Up,
        }
    }
}

/// One negative quarter turn about an axis, on a vector: the exact sign swap, no
/// floating point arithmetic, so it can be applied four times and come back.
fn turn_vector(vector: [f32; 3], axis: usize, turns: u8) -> [f32; 3] {
    let [x, y, z] = vector;
    let turns = turns % 4;
    match (axis, turns) {
        (0, 1) => [x, z, -y],
        (0, 2) => [x, -y, -z],
        (0, 3) => [x, -z, y],
        (1, 1) => [-z, y, x],
        (1, 2) => [-x, y, -z],
        (1, 3) => [z, y, -x],
        (2, 1) => [y, -x, z],
        (2, 2) => [-x, -y, z],
        (2, 3) => [-y, x, z],
        _ => [x, y, z],
    }
}

/// Turn a point of the block (0..1 on each axis) by a variant's `x`, `y`, `z`.
///
/// The three are applied in X, Y, Z order, each by `-90° × turns`, around
/// [`CENTRE`]. A caller that only has `x` and `y` (every blockstate in this mod -
/// no file writes `z`) passes `turns[2] = 0` and gets the same answer.
pub fn turn_point(point: [f32; 3], turns: [u8; 3]) -> [f32; 3] {
    let mut relative = [
        point[0] - CENTRE[0],
        point[1] - CENTRE[1],
        point[2] - CENTRE[2],
    ];
    for (axis, turn) in turns.iter().enumerate() {
        relative = turn_vector(relative, axis, *turn);
    }
    [
        relative[0] + CENTRE[0],
        relative[1] + CENTRE[1],
        relative[2] + CENTRE[2],
    ]
}

/// Turn a direction (a face normal, a cull direction) by a variant's turns: the
/// same three turns, without the centring.
pub fn turn_direction(direction: [f32; 3], turns: [u8; 3]) -> [f32; 3] {
    let mut out = direction;
    for (axis, turn) in turns.iter().enumerate() {
        out = turn_vector(out, axis, *turn);
    }
    out
}

/// The face a face ends up on after a variant's turns - which is what a caller
/// needs to cull the right neighbour.
pub fn turn_face(face: Face, turns: [u8; 3]) -> Face {
    let turned = turn_direction(face.normal(), turns);
    Face::ALL
        .iter()
        .copied()
        .find(|candidate| candidate.normal() == turned)
        .unwrap_or(face)
}

/// One clockwise quarter turn of a texture coordinate inside its face.
///
/// Model UVs have `v` pointing down, so "clockwise on the sprite" is
/// `(u, v) -> (1 - v, u)`.
pub fn uv_turn(uv: [f32; 2], quarter: u8) -> [f32; 2] {
    let mut out = uv;
    for _ in 0..(quarter % 4) {
        out = [1.0 - out[1], out[0]];
    }
    out
}

/// The UV a face keeps when its variant sets `uvlock`.
///
/// Vanilla locks the texture to the world, so the sprite turns the *other* way
/// from the model: this is [`uv_turn`] by the inverse of the variant's `y` turn.
/// Only `y` is modelled, because that is the only axis any `uvlock` file of this
/// mod uses (measured: `blockconstructiontape`, `y` in 0/90/180/270, `x` 0) - an
/// `x` or `z` turn with `uvlock` is left alone and `MANQUANT.md` §2 says why.
pub fn uv_locked(uv: [f32; 2], turns: [u8; 3]) -> [f32; 2] {
    uv_turn(uv, (4 - (turns[1] % 4)) % 4)
}

/// The axis an element rotation turns about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three axes, and `index` answers for each
pub enum Axis {
    /// `"x"`.
    X,
    /// `"y"`.
    Y,
    /// `"z"`.
    Z,
}

impl Axis {
    /// The key the model JSON uses.
    pub fn key(self) -> &'static str {
        match self {
            Axis::X => "x",
            Axis::Y => "y",
            Axis::Z => "z",
        }
    }

    /// The axis a key names, or `None`.
    pub fn from_key(key: &str) -> Option<Axis> {
        match key {
            "x" => Some(Axis::X),
            "y" => Some(Axis::Y),
            "z" => Some(Axis::Z),
            _ => None,
        }
    }
}

/// An element's `rotation`: an angle, an axis, and the pivot it turns around.
///
/// Blockbench's default grid is 22.5°, and this mod sticks to it: the angles it
/// writes are ±22.5° and ±45°, plus the 0° records [`ElementRotation::is_noop`]
/// names.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: four fields, all read
pub struct ElementRotation {
    /// The angle in degrees, counter-clockwise seen from the positive end of the
    /// axis. Negative values are common in this mod.
    pub angle: f32,
    /// The axis the angle turns about.
    pub axis: Axis,
    /// The pivot, in block units: the point that does not move.
    pub origin: [f32; 3],
    /// Whether the game rescales the element so it stays inside the block.
    pub rescale: bool,
}

impl ElementRotation {
    /// Whether the rotation changes nothing: `angle` 0. 912 of the mod's 1 540
    /// rotated elements are this, because Blockbench records a pivot of its own.
    pub fn is_noop(&self) -> bool {
        self.angle == 0.0
    }

    /// Rotate a point of the block around this element's pivot.
    pub fn rotate(&self, point: [f32; 3]) -> [f32; 3] {
        let radians = self.angle.to_radians();
        let (sin, cos) = (radians.sin(), radians.cos());
        let offset = [
            point[0] - self.origin[0],
            point[1] - self.origin[1],
            point[2] - self.origin[2],
        ];
        let turned = match self.axis {
            Axis::X => [
                offset[0],
                offset[1] * cos - offset[2] * sin,
                offset[1] * sin + offset[2] * cos,
            ],
            Axis::Y => [
                offset[0] * cos + offset[2] * sin,
                offset[1],
                -offset[0] * sin + offset[2] * cos,
            ],
            Axis::Z => [
                offset[0] * cos - offset[1] * sin,
                offset[0] * sin + offset[1] * cos,
                offset[2],
            ],
        };
        [
            turned[0] + self.origin[0],
            turned[1] + self.origin[1],
            turned[2] + self.origin[2],
        ]
    }

    /// Read an element's `rotation` object.
    pub fn parse(map: &Map<String, Value>) -> Result<ElementRotation, Refusal> {
        let axis_key = map
            .get("axis")
            .and_then(Value::as_str)
            .ok_or(Refusal::Missing { key: "axis" })?;
        let axis = Axis::from_key(axis_key).ok_or(Refusal::UnknownAxis {
            key: axis_key.to_owned(),
        })?;
        let angle = number(map, "angle")?;
        let origin = vector(map, "origin")?;
        Ok(ElementRotation {
            angle,
            axis,
            origin,
            rescale: map.get("rescale").and_then(Value::as_bool).unwrap_or(false),
        })
    }
}

/// One of the eight contexts a `display` block can name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: eight contexts, indexed by `index`
pub enum Context {
    /// `thirdperson_righthand`.
    ThirdPersonRightHand,
    /// `thirdperson_lefthand`.
    ThirdPersonLeftHand,
    /// `firstperson_righthand`.
    FirstPersonRightHand,
    /// `firstperson_lefthand`.
    FirstPersonLeftHand,
    /// `ground`, the dropped item.
    Ground,
    /// `gui`, the inventory slot.
    Gui,
    /// `head`, worn on a head.
    Head,
    /// `fixed`, in an item frame.
    Fixed,
}

impl Context {
    /// The eight, in the order vanilla reads them.
    pub const ALL: [Context; 8] = [
        Context::ThirdPersonRightHand,
        Context::ThirdPersonLeftHand,
        Context::FirstPersonRightHand,
        Context::FirstPersonLeftHand,
        Context::Ground,
        Context::Gui,
        Context::Head,
        Context::Fixed,
    ];

    /// The key the model JSON uses.
    pub fn key(self) -> &'static str {
        match self {
            Context::ThirdPersonRightHand => "thirdperson_righthand",
            Context::ThirdPersonLeftHand => "thirdperson_lefthand",
            Context::FirstPersonRightHand => "firstperson_righthand",
            Context::FirstPersonLeftHand => "firstperson_lefthand",
            Context::Ground => "ground",
            Context::Gui => "gui",
            Context::Head => "head",
            Context::Fixed => "fixed",
        }
    }

    /// The context a key names, or `None`. **`thirdperson` and `firstperson` are
    /// not here**: vanilla dropped them, and 14 of this mod's item models still
    /// write them, so those items get [`Display::IDENTITY`] in every hand.
    pub fn from_key(key: &str) -> Option<Context> {
        Context::ALL.iter().copied().find(|ctx| ctx.key() == key)
    }

    /// The slot in [`Displays`] this context fills.
    pub fn index(self) -> usize {
        Context::ALL
            .iter()
            .position(|ctx| *ctx == self)
            .unwrap_or(0)
    }
}

/// The names vanilla dropped, in the order the mod uses them.
pub const LEGACY_CONTEXTS: [&str; 2] = ["thirdperson", "firstperson"];

/// One `display` entry: a rotation in degrees, a translation in sixteenths of a
/// block, and a scale.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: three vectors, and `IDENTITY` is the neutral element
pub struct Display {
    /// Degrees about X, Y and Z.
    pub rotation: [f32; 3],
    /// Sixteenths of a block, along X, Y and Z.
    pub translation: [f32; 3],
    /// A factor per axis.
    pub scale: [f32; 3],
}

impl Display {
    /// What a model gets in a context it does not name (`ItemTransform.NO_TRANSFORM`).
    pub const IDENTITY: Display = Display {
        rotation: [0.0, 0.0, 0.0],
        translation: [0.0, 0.0, 0.0],
        scale: [1.0, 1.0, 1.0],
    };

    /// Whether this entry is the identity.
    pub fn is_identity(&self) -> bool {
        *self == Display::IDENTITY
    }

    /// Place a point of the model for this display context.
    ///
    /// Vanilla's `ItemTransform` builds `translation(t/16) · scale(s) ·
    /// rotateXYZ(r)`, so a point is scaled, then turned about Z, then Y, then X,
    /// then moved. The order is the game's, kept here so a caller can compare two
    /// entries: a scale followed by a translation is not a translation followed by
    /// a scale.
    pub fn apply(&self, point: [f32; 3]) -> [f32; 3] {
        let mut out = [
            point[0] * self.scale[0],
            point[1] * self.scale[1],
            point[2] * self.scale[2],
        ];
        for axis in [Axis::Z, Axis::Y, Axis::X] {
            let radians = self.rotation[axis_index(axis)].to_radians();
            let (sin, cos) = (radians.sin(), radians.cos());
            out = rotate_about(out, axis, sin, cos);
        }
        [
            out[0] + self.translation[0] / 16.0,
            out[1] + self.translation[1] / 16.0,
            out[2] + self.translation[2] / 16.0,
        ]
    }

    /// Read one `display` entry.
    pub fn parse(map: &Map<String, Value>) -> Result<Display, Refusal> {
        let mut display = Display::IDENTITY;
        if map.contains_key("rotation") {
            display.rotation = vector(map, "rotation")?;
        }
        if map.contains_key("translation") {
            display.translation = vector(map, "translation")?;
        }
        if map.contains_key("scale") {
            display.scale = vector(map, "scale")?;
        }
        Ok(display)
    }
}

fn axis_index(axis: Axis) -> usize {
    match axis {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::Z => 2,
    }
}

fn rotate_about(point: [f32; 3], axis: Axis, sin: f32, cos: f32) -> [f32; 3] {
    let [x, y, z] = point;
    match axis {
        Axis::X => [x, y * cos - z * sin, y * sin + z * cos],
        Axis::Y => [x * cos + z * sin, y, -x * sin + z * cos],
        Axis::Z => [x * cos - y * sin, x * sin + y * cos, z],
    }
}

/// The eight slots a model's `display` fills, plus the legacy names it wrote.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: two fields, both read
pub struct Displays {
    /// One entry per [`Context`], in [`Context::ALL`] order.
    pub slots: [Display; 8],
    /// The legacy context names the file wrote, in name order (`serde_json`'s map
    /// sorts its keys). Vanilla ignores them, so the fact is recorded here instead
    /// of being silently dropped: 14 item models of this mod write `thirdperson`
    /// and `firstperson`.
    pub legacy: Vec<&'static str>,
}

impl Default for Displays {
    fn default() -> Displays {
        Displays {
            slots: [Display::IDENTITY; 8],
            legacy: Vec::new(),
        }
    }
}

impl Displays {
    /// The entry for a context: `IDENTITY` when the model does not name it.
    pub fn get(&self, context: Context) -> Display {
        self.slots[context.index()]
    }

    /// Whether every slot is the identity - the whole effect of writing only
    /// legacy names.
    pub fn all_identity(&self) -> bool {
        self.slots.iter().all(Display::is_identity)
    }

    /// Read a model's `display` object.
    pub fn parse(map: &Map<String, Value>) -> Result<Displays, Refusal> {
        let mut displays = Displays::default();
        for (key, value) in map {
            if let Some(context) = Context::from_key(key) {
                let entry = value
                    .as_object()
                    .ok_or(Refusal::NotAnObject { key: "display" })?;
                displays.slots[context.index()] = Display::parse(entry)?;
                continue;
            }
            // A legacy name is kept as a fact, not as a slot: vanilla has no slot
            // for it any more.
            if let Some(name) = LEGACY_CONTEXTS.iter().find(|name| **name == key.as_str()) {
                displays.legacy.push(name);
            }
        }
        Ok(displays)
    }
}

/// What a model file got wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: four ways, and each names its key
pub enum Refusal {
    /// A key the reader needs is absent.
    Missing { key: &'static str },
    /// A key that should hold a vector of three numbers does not.
    NotAVector { key: &'static str },
    /// An axis that is not `x`, `y` or `z`.
    UnknownAxis { key: String },
    /// A `display` entry that is not an object.
    NotAnObject { key: &'static str },
}

impl Refusal {
    /// A stable name, namespaced.
    pub fn name(&self) -> &'static str {
        match self {
            Refusal::Missing { .. } => "model.refusal.missing",
            Refusal::NotAVector { .. } => "model.refusal.not_a_vector",
            Refusal::UnknownAxis { .. } => "model.refusal.unknown_axis",
            Refusal::NotAnObject { .. } => "model.refusal.not_an_object",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::Missing { key } => write!(f, "{}: {key}", self.name()),
            Refusal::NotAVector { key } => write!(f, "{}: {key}", self.name()),
            Refusal::UnknownAxis { key } => write!(f, "{}: {key}", self.name()),
            Refusal::NotAnObject { key } => write!(f, "{}: {key}", self.name()),
        }
    }
}

impl std::error::Error for Refusal {}

fn number(map: &Map<String, Value>, key: &'static str) -> Result<f32, Refusal> {
    map.get(key)
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        .ok_or(Refusal::Missing { key })
}

fn vector(map: &Map<String, Value>, key: &'static str) -> Result<[f32; 3], Refusal> {
    let list = map
        .get(key)
        .and_then(Value::as_array)
        .ok_or(Refusal::NotAVector { key })?;
    if list.len() != 3 {
        return Err(Refusal::NotAVector { key });
    }
    let mut out = [0.0f32; 3];
    for (slot, value) in out.iter_mut().zip(list) {
        *slot = value.as_f64().ok_or(Refusal::NotAVector { key })? as f32;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::serde_json::Value;

    fn value(text: &str) -> Value {
        mod_api::serde_json::from_str(text).expect("a literal in a test is valid JSON")
    }

    fn map(text: &str) -> Map<String, Value> {
        match value(text) {
            Value::Object(map) => map,
            other => panic!("not an object: {other:?}"),
        }
    }

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        (0..3).all(|axis| (a[axis] - b[axis]).abs() < 1.0e-4)
    }

    #[test]
    fn a_quarter_turn_about_y_brings_x_to_z() {
        // `"y": 90` on a block is the clockwise turn seen from above, which is
        // what every `facing` convention of `variants.rs` is written against.
        let turned = turn_point([1.0, 0.5, 0.5], [0, 1, 0]);
        assert!(close(turned, [0.5, 0.5, 1.0]), "{turned:?}");
        assert_eq!(turn_face(Face::North, [0, 1, 0]), Face::East);
        assert_eq!(turn_face(Face::East, [0, 1, 0]), Face::South);
    }

    #[test]
    fn four_turns_are_the_identity_bit_for_bit() {
        // No trigonometry, so this is exact: the reason `turn_vector` is a match.
        let point = [0.25, 0.75, 0.125];
        assert_eq!(turn_point(point, [0, 4, 0]), point);
        assert_eq!(turn_point(point, [4, 4, 4]), point);
        // Four turns of one axis, applied one after the other, come back too. The
        // axes do not commute, so this is said per axis and not for a mix.
        for axis in 0..3 {
            let mut turned = point;
            for _ in 0..4 {
                let mut one = [0u8; 3];
                one[axis] = 1;
                turned = turn_point(turned, one);
            }
            assert_eq!(turned, point, "axis {axis}");
        }
    }

    #[test]
    fn the_block_centre_never_moves() {
        for turns in [[0, 0, 0], [1, 0, 0], [0, 3, 0], [3, 2, 1]] {
            assert_eq!(turn_point(CENTRE, turns), CENTRE);
        }
    }

    #[test]
    fn turning_the_eight_corners_is_a_permutation_of_them() {
        let corners: Vec<[f32; 3]> = (0..8)
            .map(|index| {
                [
                    if index & 1 == 0 { 0.0 } else { 1.0 },
                    if index & 2 == 0 { 0.0 } else { 1.0 },
                    if index & 4 == 0 { 0.0 } else { 1.0 },
                ]
            })
            .collect();
        for turns in [[1, 0, 0], [0, 1, 0], [0, 0, 1], [1, 2, 3]] {
            let mut seen: Vec<[f32; 3]> = corners.iter().map(|c| turn_point(*c, turns)).collect();
            seen.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
            let mut expected = corners.clone();
            expected.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
            assert_eq!(seen, expected, "{turns:?} does not keep the block's box");
        }
    }

    #[test]
    fn an_x_turn_keeps_x_and_moves_y_to_the_back() {
        // `"x": 90` tips the top towards -Z, which is what the decoration
        // controller's `facing=up` does (`decorationcontroller.json`).
        assert_eq!(turn_point([1.0, 0.5, 0.5], [1, 0, 0]), [1.0, 0.5, 0.5]);
        let top = turn_point([0.5, 1.0, 0.5], [1, 0, 0]);
        assert!(close(top, [0.5, 0.5, 0.0]), "{top:?}");
        assert_eq!(turn_face(Face::Up, [1, 0, 0]), Face::North);
        assert_eq!(turn_face(Face::Down, [3, 0, 0]), Face::North);
    }

    #[test]
    fn a_quarter_turn_maps_each_face_once_and_none_twice() {
        let mut seen: Vec<Face> = Face::ALL
            .iter()
            .map(|face| turn_face(*face, [0, 1, 0]))
            .collect();
        seen.sort_by_key(|face| face.key());
        let mut all = Face::ALL.to_vec();
        all.sort_by_key(|face| face.key());
        assert_eq!(seen, all);
        // A half turn about Y swaps the four horizontal faces with their opposite
        // and leaves up and down where they are, because the axis runs through them.
        for face in [Face::North, Face::South, Face::East, Face::West] {
            assert_eq!(turn_face(face, [0, 2, 0]), face.opposite());
        }
        assert_eq!(turn_face(Face::Up, [0, 2, 0]), Face::Up);
        assert_eq!(turn_face(Face::Down, [0, 2, 0]), Face::Down);
    }

    #[test]
    fn a_uv_turn_four_times_is_the_identity_and_uvlock_inverts_the_model() {
        let uv = [0.25, 0.75];
        assert_eq!(uv_turn(uv, 0), uv);
        assert_eq!(uv_turn(uv, 4), uv);
        assert_eq!(uv_turn(uv_turn(uv, 1), 3), uv);
        // The corner the tape relies on: a sprite turned once goes to the right.
        assert_eq!(uv_turn([0.0, 0.0], 1), [1.0, 0.0]);
        // `uvlock` turns the sprite the other way from the model.
        assert_eq!(uv_locked(uv, [0, 1, 0]), uv_turn(uv, 3));
        assert_eq!(uv_locked(uv, [0, 0, 0]), uv);
        // An `x` turn is left alone, which is the limit `MANQUANT.md` §2 names.
        assert_eq!(uv_locked(uv, [1, 0, 0]), uv);
    }

    #[test]
    fn an_element_rotation_turns_around_its_own_pivot() {
        let rotation = ElementRotation {
            angle: 22.5,
            axis: Axis::Y,
            origin: [8.0, 0.0, 8.0],
            rescale: false,
        };
        assert!(close(rotation.rotate([8.0, 4.0, 8.0]), [8.0, 4.0, 8.0]));
        // A point on +X goes towards -Z by a counter-clockwise turn seen from +Y,
        // which is the right-handed rotation the game's baker uses.
        let turned = rotation.rotate([12.0, 0.0, 8.0]);
        let (sin, cos) = (22.5f32.to_radians().sin(), 22.5f32.to_radians().cos());
        assert!(
            close(turned, [8.0 + 4.0 * cos, 0.0, 8.0 - 4.0 * sin]),
            "{turned:?}"
        );
    }

    #[test]
    fn an_angle_of_zero_is_the_noop_nine_hundred_and_twelve_elements_are() {
        let rotation = ElementRotation {
            angle: 0.0,
            axis: Axis::Y,
            origin: [8.0, 8.0, 8.0],
            rescale: false,
        };
        assert!(rotation.is_noop());
        assert_eq!(rotation.rotate([1.0, 2.0, 3.0]), [1.0, 2.0, 3.0]);
        // The measured shapes of this mod's angles: 22.5° steps, never a quarter.
        for angle in [-45.0, -22.5, 0.0, 22.5, 45.0] {
            assert!(!ElementRotation { angle, ..rotation }.is_noop() || angle == 0.0);
        }
    }

    #[test]
    fn an_element_rotation_is_read_from_the_model_json() {
        // `blockhutbuilder.json`'s first element, verbatim.
        let rotation = ElementRotation::parse(&map(
            r#"{"angle": -22.5, "axis": "y", "origin": [8, 7.5, 8]}"#,
        ))
        .expect("valid");
        assert_eq!(rotation.angle, -22.5);
        assert_eq!(rotation.axis, Axis::Y);
        assert_eq!(rotation.origin, [8.0, 7.5, 8.0]);
        assert!(!rotation.rescale);
        // The one element of the mod that rescales.
        let rescaling = ElementRotation::parse(&map(
            r#"{"angle": 45, "axis": "y", "origin": [8, 8, 8], "rescale": true}"#,
        ))
        .expect("valid");
        assert!(rescaling.rescale);
        assert_eq!(
            ElementRotation::parse(&map(r#"{"angle": 0, "axis": "w", "origin": [0,0,0]}"#))
                .expect_err("no such axis")
                .name(),
            "model.refusal.unknown_axis"
        );
    }

    #[test]
    fn a_display_entry_defaults_to_the_identity_and_reads_each_field() {
        assert_eq!(Displays::default().get(Context::Gui), Display::IDENTITY);
        let displays = Displays::parse(&map(
            r#"{"gui": {"rotation": [20, -45, 0], "translation": [0, -2.5, 0], "scale": [0.42, 0.42, 0.42]}}"#,
        ))
        .expect("valid");
        let gui = displays.get(Context::Gui);
        assert_eq!(gui.rotation, [20.0, -45.0, 0.0]);
        assert_eq!(gui.translation, [0.0, -2.5, 0.0]);
        assert_eq!(gui.scale, [0.42, 0.42, 0.42]);
        assert!(!gui.is_identity());
        assert!(displays.get(Context::Head).is_identity());
    }

    #[test]
    fn the_translation_of_a_display_entry_is_in_sixteenths_and_scale_comes_first() {
        let display = Display {
            rotation: [0.0, 0.0, 0.0],
            translation: [0.0, 8.0, 0.0],
            scale: [2.0, 2.0, 2.0],
        };
        let placed = display.apply([0.25, 0.0, 0.0]);
        assert!(close(placed, [0.5, 0.5, 0.0]), "{placed:?}");
        // Scale then translate, never the other way: 2 * 0.25 + 8/16.
        assert!((placed[0] - (0.5)).abs() < 1.0e-4);
        assert!((placed[1] - 0.5).abs() < 1.0e-4);
    }

    #[test]
    fn a_rotation_of_ninety_degrees_in_a_display_entry_is_a_real_turn() {
        let display = Display {
            rotation: [0.0, 90.0, 0.0],
            ..Display::IDENTITY
        };
        let turned = display.apply([1.0, 0.0, 0.0]);
        assert!(close(turned, [0.0, 0.0, -1.0]), "{turned:?}");
    }

    #[test]
    fn the_fourteen_item_models_that_write_a_legacy_context_keep_it_as_a_fact() {
        // `caliper.json`, `compost.json`, the three scepters, `supplychestdeployer`
        // and the rest: vanilla has no `thirdperson` slot any more, so the entry is
        // recorded and the item is drawn with the identity matrix.
        let displays = Displays::parse(&map(
            r#"{"thirdperson": {"rotation": [-90, 0, 0], "translation": [0, 1, -3], "scale": [0.55, 0.55, 0.55]},
               "firstperson": {"rotation": [0, -135, 25], "translation": [0, 4, 2], "scale": [1.7, 1.7, 1.7]}}"#,
        ))
        .expect("valid");
        assert_eq!(displays.legacy, vec!["firstperson", "thirdperson"]);
        assert!(displays.all_identity(), "no modern slot is filled");
        // A modern set is not empty, which is the difference the 14 files pay for.
        let modern = Displays::parse(&map(
            r#"{"thirdperson_righthand": {"rotation": [45, -45, 0]}}"#,
        ))
        .expect("valid");
        assert!(!modern.all_identity());
        assert!(modern.legacy.is_empty());
    }

    #[test]
    fn the_eight_contexts_round_trip_each_way() {
        assert_eq!(Context::ALL.len(), 8);
        for context in Context::ALL {
            assert_eq!(Context::from_key(context.key()), Some(context));
            assert_eq!(Context::ALL[context.index()], context);
        }
        assert_eq!(Context::from_key("thirdperson"), None);
        assert_eq!(
            Context::from_key("firstperson_lefthand"),
            Some(Context::FirstPersonLeftHand)
        );
        let mut seen: Vec<usize> = Context::ALL.iter().map(|ctx| ctx.index()).collect();
        seen.sort_unstable();
        assert_eq!(seen, vec![0, 1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn the_six_faces_round_trip_and_pair_into_opposites() {
        assert_eq!(Face::ALL.len(), 6);
        for face in Face::ALL {
            assert_eq!(Face::from_key(face.key()), Some(face));
            assert_eq!(face.opposite().opposite(), face);
            assert_ne!(face.opposite(), face);
        }
        assert_eq!(Face::from_key("sideways"), None);
        assert_eq!(Face::North.normal(), [0.0, 0.0, -1.0]);
    }

    #[test]
    fn a_display_entry_that_is_not_an_object_is_refused() {
        assert_eq!(
            Displays::parse(&map(r#"{"gui": "big"}"#))
                .expect_err("not an object")
                .name(),
            "model.refusal.not_an_object"
        );
        assert_eq!(
            Display::parse(&map(r#"{"rotation": [1, 2]}"#))
                .expect_err("two numbers")
                .name(),
            "model.refusal.not_a_vector"
        );
    }
}
