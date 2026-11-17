//! The seven block entities that draw something, and the one that draws nothing.
//!
//! Java reference: `ClientRegistryHandler.java:350-357` (the seven
//! `registerBlockEntityRenderer` calls) and the seven classes under
//! `core/client/render/` named `TileEntity*Renderer.java`.
//!
//! | Block entity | Draws | File |
//! | --- | --- | --- |
//! | `building` | **nothing** | `EmptyTileEntitySpecialRenderer.java:22-33` |
//! | `scarecrow` | a straw model, or a pumpkin one | `TileEntityScarecrowRenderer.java` |
//! | `enchanter` | a floating book | `TileEntityEnchanterRenderer.java` |
//! | `colony_flag` | a banner, pole and bar | `TileEntityColonyFlagRenderer.java` |
//! | `named_grave` | up to four lines on a plate | `TileEntityNamedGraveRenderer.java` |
//! | `deco_controller` | the block in front of it | `TileEntityDecoControllerRenderer.java` |
//! | `colony_sign` | a name and a distance, on both faces | `TileEntityColonySignRenderer.java` |
//!
//! **The hut renderer is the interesting one.** Every colony building shares one
//! empty renderer (`:350`): a hut draws through its *block model*, and the mod
//! registers a renderer only to stop the game from looking for one. A port that
//! invented a hut renderer would draw a second copy of a block that already draws.
//!
//! **Two text layouts, two sets of numbers, and they are not shared.** The grave
//! writes at 10 px per line and scales by `0.006`
//! (`TileEntityNamedGraveRenderer.java:100-107`); the sign writes at 8 px and
//! scales by `0.007` (`TileEntityColonySignRenderer.java:200-210`). Both truncate
//! to 20 characters and both are written in `0xdcdcdc00`.
//!
//! **That colour is not the light grey it reads as.** Java hands `drawInBatch` an
//! **ARGB** int, so `0xdcdcdc00` is alpha `0xdc` (220), red `0xdc`, green `0xdc`,
//! blue `0x00`: a saturated yellow at 86 % opacity, not a grey. Reading the four
//! bytes the other way round — grey `#dcdcdc` plus a trailing `00` alpha — is the
//! mistake this note exists to stop; [`text_channels`] hands back the four bytes
//! as Java reads them, and a test pins them.
//!
//! **What the port can and cannot do with the text.** The wrapping the sign does
//! at 90 px is `Font.getSplitter().splitLines` (`:157`), and Horizon *does* have
//! that call — `ClientTextClientHost::text_split`
//! (`crates/horizon-mod-vocab/src/host/client_text.rs:40-48`). What it does not
//! have is a way to read the *rendered* width of a translation in the middle of a
//! pose: `text_width` exists but takes a component, which this crate cannot build
//! from a key and an argument without a `Text` constructor. So [`sign_rows`]
//! returns the rows and the 90-px decision, and the caller with a text handle
//! splits them.

/// One registered block-entity renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: four fields, all compared
pub struct BlockRenderer {
    /// The Java class.
    pub name: &'static str,
    /// The block entity id, `minecolonies:scarecrow`.
    pub block: &'static str,
    /// What it draws, in one line, or why it draws nothing.
    pub draws: &'static str,
    /// `fichier.java:ligne` of the registration.
    pub source: &'static str,
}

/// The seven registrations, in the order `doClientStuff` makes them.
pub const BLOCK_RENDERERS: &[BlockRenderer] = &[
    BlockRenderer {
        name: "EmptyTileEntitySpecialRenderer",
        block: "minecolonies:building",
        draws: "nothing: the hut block model draws the hut",
        source: "ClientRegistryHandler.java:350",
    },
    BlockRenderer {
        name: "TileEntityScarecrowRenderer",
        block: "minecolonies:scarecrow",
        draws: "the scarecrow model, plus a vanilla lantern when the block says so",
        source: "ClientRegistryHandler.java:351",
    },
    BlockRenderer {
        name: "TileEntityEnchanterRenderer",
        block: "minecolonies:enchanter",
        draws: "the book model, bobbing and turning on the block entity's tick",
        source: "ClientRegistryHandler.java:352",
    },
    BlockRenderer {
        name: "TileEntityColonyFlagRenderer",
        block: "minecolonies:colony_flag",
        draws: "the banner's pole, bar and cloth, with the colony's patterns",
        source: "ClientRegistryHandler.java:353",
    },
    BlockRenderer {
        name: "TileEntityNamedGraveRenderer",
        block: "minecolonies:named_grave",
        draws: "up to four text lines on the grave's plate",
        source: "ClientRegistryHandler.java:354",
    },
    BlockRenderer {
        name: "TileEntityDecoControllerRenderer",
        block: "minecolonies:deco_controller",
        draws: "the block in front of it, offset to that block's shape",
        source: "ClientRegistryHandler.java:355",
    },
    BlockRenderer {
        name: "TileEntityColonySignRenderer",
        block: "minecolonies:colony_sign",
        draws: "a colony name and a distance, on both faces",
        source: "ClientRegistryHandler.java:356",
    },
];

/// The block entity id whose renderer draws nothing (`:350`).
pub const EMPTY: &str = "minecolonies:building";

/// The two scarecrow heads (`AbstractTileEntityScarecrow.getScarecrowType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: two types in `ScareCrowType`
pub enum ScarecrowHead {
    /// `ScareCrowType.PUMPKINHEAD` → `SCARECROW_A`.
    Pumpkin,
    /// Anything else → `SCARECROW_B`.
    Normal,
}

/// The texture a scarecrow wears: `SCARECROW_A` is the pumpkin head, `SCARECROW_B`
/// the plain one (`TileEntityScarecrowRenderer.java:73-78`, `:170-179`).
///
/// Both are **block atlas** sprites, not entity textures — that is why they are
/// in `block/` and why `texture_sprites` can name them.
pub fn scarecrow_texture(head: ScarecrowHead) -> &'static str {
    match head {
        ScarecrowHead::Pumpkin => "minecolonies:block/blockscarecrowpumpkin",
        ScarecrowHead::Normal => "minecolonies:block/blockscarecrownormal",
    }
}

/// `ScareCrowType.getType`'s ordinals: the saved byte is the enum's order.
pub fn scarecrow_head(ordinal: u8) -> ScarecrowHead {
    if ordinal == 0 {
        ScarecrowHead::Pumpkin
    } else {
        ScarecrowHead::Normal
    }
}

/// The y rotation a scarecrow is drawn with, in degrees: none for north, then
/// `90 * index` (`:123-136`).
pub fn scarecrow_rotation(facing: u8) -> f32 {
    match facing {
        1 => 90.0,
        2 => 180.0,
        3 => 270.0,
        _ => 0.0,
    }
}

/// Only the **lower** half draws: the upper one returns immediately
/// (`:96-100`), so a double-block scarecrow is one model, not two.
pub fn scarecrow_half_draws(upper: bool) -> bool {
    !upper
}

/// Where the scarecrow stands: block middle on x and z, 1.5 up (`:36-41`, `:104-107`).
pub const SCARECROW_AT: [f32; 3] = [0.5, 1.5, 0.5];

/// The lantern a scarecrow carries, when `BlockScarecrow.LANTERN` is set: its own
/// rotation, translate and scale (`:150-166`).
pub const SCARECROW_LANTERN: Lantern = Lantern {
    rotation_z: 180.0,
    at: [0.6, -0.6, -0.375],
    scale: [0.75, 0.65, 0.75],
};

/// The transform of the scarecrow's lantern.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: three fields, all drawn
pub struct Lantern {
    /// Degrees about z, applied first.
    pub rotation_z: f32,
    /// Where it sits, in the scarecrow's frame.
    pub at: [f32; 3],
    /// How big it is drawn.
    pub scale: [f32; 3],
}

/// The book the enchanter floats: `minecolonies:block/enchanting_table_book` on
/// the vanilla book model (`TileEntityEnchanterRenderer.java:29-36`, `:56-79`).
pub const ENCHANTER_TEXTURE: &str = "minecolonies:block/enchanting_table_book";

/// The enchanter's book position (`:59`).
pub const ENCHANTER_AT: [f32; 3] = [0.5, 0.75, 0.5];

/// The bob: `0.1 + sin(tick * 0.1) * 0.01` (`:61`).
pub fn enchanter_bob(tick: f32) -> f32 {
    0.1 + (tick * 0.1).sin() * 0.01
}

/// `Mth.lerp(partial, prev, current)` for the book's four animated values
/// (`:66-72`): the page-turn angle and the spread.
pub fn lerp(partial: f32, previous: f32, current: f32) -> f32 {
    previous + (current - previous) * partial
}

/// `flipA`/`flipB`, the two page corners, clamped (`:70-71`).
pub fn enchanter_flip(partial: f32, previous: f32, current: f32, second_page: bool) -> f32 {
    let page_flip = lerp(partial, previous, current);
    let offset = if second_page { 0.75 } else { 0.25 };
    let raw = (page_flip + offset).fract() * 1.6 - 0.3;
    raw.clamp(0.0, 1.0)
}

/// The cloth's sway on the colony flag: `xRot = (-0.0125 + 0.01 * cos(2π f)) * π`
/// with `f` a hash of the block's position and the game time, modulo 100
/// (`TileEntityColonyFlagRenderer.java:107-111`).
pub fn flag_sway(position: [i32; 3], game_time: i64, partial: f32) -> f32 {
    let hashed = i64::from(position[0]) * 7 + i64::from(position[1]) * 9 + i64::from(position[2]) * 13;
    let modulo = (hashed + game_time).rem_euclid(100) as f32;
    let fraction = (modulo + partial) / 100.0;
    (-0.0125 + 0.01 * (std::f32::consts::PI * 2.0 * fraction).cos()) * std::f32::consts::PI
}

/// The flag's own scale, `(2/3, -2/3, -2/3)`: the banner model is 2/3 the size and
/// inverted on y and z (`:100`).
pub const FLAG_SCALE: [f32; 3] = [2.0 / 3.0, -2.0 / 3.0, -2.0 / 3.0];

/// Whether a flag draws its pole: a wall banner does not (`:85-95`).
pub fn flag_has_pole(wall_banner: bool) -> bool {
    !wall_banner
}

/// A line of text one of the two renderers writes, before truncation.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: two kinds of text, one of them translated
pub enum SignText {
    /// A name, as stored.
    Literal(String),
    /// `Component.translatable("com.minecolonies.coremod.dist.blocks", distance)`,
    /// kept as the key and the argument: the port has no lang file.
    Distance(i32),
}

/// `com.minecolonies.coremod.dist.blocks`, the distance line on both the grave and
/// the sign (`TranslationConstants`, used at `TileEntityColonySignRenderer.java:137`
/// and `:186`).
pub const DISTANCE_KEY: &str = "com.minecolonies.coremod.dist.blocks";

/// One written line: the text, which line of the plate it lands on, and the
/// extra offset that stacks a second colony under the first.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: three fields, all drawn
pub struct SignRow {
    /// What is written.
    pub text: SignText,
    /// The line index, multiplied by the layout's `line_px`.
    pub line: u8,
    /// Added after the line offset, in the scaled frame.
    pub offset: i32,
}

/// The numbers one of the two text renderers uses.
#[derive(Clone, Copy, Debug, PartialEq)]
// exhaustive: six numbers, all read
pub struct TextLayout {
    /// `text.substring(0, maxSize)` with `maxSize = 20`.
    pub max_chars: usize,
    /// The line height in the scaled frame: 10 for the grave, 8 for the sign.
    pub line_px: f32,
    /// The scale applied to the pose before writing.
    pub scale: f32,
    /// The colour written, `0xdcdcdc00`.
    pub colour: u32,
    /// The width past which a name is split, `0` when the renderer never splits.
    pub wrap_px: i32,
    /// How many lines a split name may take.
    pub wrap_lines: usize,
}

/// The grave's layout (`TileEntityNamedGraveRenderer.java:100-107`): 20 characters,
/// 10 px per line, `0.006`, no wrapping.
pub const GRAVE_LAYOUT: TextLayout = TextLayout {
    max_chars: 20,
    line_px: 10.0,
    scale: 0.006,
    colour: 0xdcdc_dc00,
    wrap_px: 0,
    wrap_lines: 1,
};

/// The sign's layout (`TileEntityColonySignRenderer.java:107`, `:155-166`,
/// `:200-210`): 20 characters, 8 px per line, `0.007`, split at 90 px into two
/// lines.
pub const SIGN_LAYOUT: TextLayout = TextLayout {
    max_chars: 20,
    line_px: 8.0,
    scale: 0.007,
    colour: 0xdcdc_dc00,
    wrap_px: 90,
    wrap_lines: 2,
};

/// What the grave writes when it has no lines: `"Unknown Citizen"`
/// (`:80-83`).
pub const UNKNOWN_CITIZEN: &str = "Unknown Citizen";

/// What the sign writes when it knows no colony: `"Unknown Colony"` (`:131-135`).
pub const UNKNOWN_COLONY: &str = "Unknown Colony";

/// The grave's rows: its stored lines, or the one fallback (`:78-90`).
///
/// A grave line is `line * 10` with no offset, so the offset is always 0 here.
pub fn grave_rows(lines: &[String]) -> Vec<SignRow> {
    if lines.is_empty() {
        return vec![SignRow {
            text: SignText::Literal(String::from(UNKNOWN_CITIZEN)),
            line: 0,
            offset: 0,
        }];
    }
    lines
        .iter()
        .enumerate()
        .map(|(line, text)| SignRow {
            text: SignText::Literal(text.clone()),
            line: line as u8,
            offset: 0,
        })
        .collect()
}

/// The sign's rows on **one** face: the colony, and the connected one when the
/// sign points at another colony (`:120-146`).
///
/// `shows_target` is Java's
/// `!targetColonyName.isEmpty() && targetColonyId != cachedSignAboveColony`
/// (`:139`): a sign standing directly above the colony it talks to does not repeat
/// it.
pub fn sign_rows(
    colony: &str,
    distance: i32,
    target: Option<(&str, i32)>,
    shows_target: bool,
) -> Vec<SignRow> {
    let mut rows = Vec::new();
    if colony.is_empty() {
        rows.push(SignRow {
            text: SignText::Literal(String::from(UNKNOWN_COLONY)),
            line: 0,
            offset: 0,
        });
        rows.push(SignRow {
            text: SignText::Distance(distance),
            line: 3,
            offset: 0,
        });
        return rows;
    }
    match target {
        Some((name, target_distance)) if shows_target && !name.is_empty() => {
            // The colony on top, the neighbour 50 px lower (`:141-143`).
            rows.push(SignRow {
                text: SignText::Literal(String::from(colony)),
                line: 0,
                offset: -10,
            });
            rows.push(SignRow {
                text: SignText::Distance(distance),
                line: 3,
                offset: -10,
            });
            rows.push(SignRow {
                text: SignText::Literal(String::from(name)),
                line: 0,
                offset: -60,
            });
            rows.push(SignRow {
                text: SignText::Distance(target_distance),
                line: 3,
                offset: -60,
            });
        }
        _ => {
            // Alone, the name sits centred on the plate (`:145`).
            rows.push(SignRow {
                text: SignText::Literal(String::from(colony)),
                line: 0,
                offset: -35,
            });
            rows.push(SignRow {
                text: SignText::Distance(distance),
                line: 3,
                offset: -35,
            });
        }
    }
    rows
}

/// `substring(0, 20)` as both renderers do it (`:100-105`, `:200-205`).
///
/// Java counts UTF-16 units and would cut a surrogate pair in half; this counts
/// characters, so an emoji stays whole. No lang line is that long, and a colony
/// name could be: the difference is stated rather than hidden.
pub fn truncate(text: &str) -> &str {
    match text.char_indices().nth(TextLayout::MAX_CHARS) {
        Some((index, _)) => &text[..index],
        None => text,
    }
}

impl TextLayout {
    /// The column `substring(0, 20)` uses — named so [`truncate`] and the two
    /// layouts cannot drift.
    pub const MAX_CHARS: usize = 20;
}

/// How many faces the sign writes: both (`:78-79` calls `renderTextOnSide` twice).
pub const SIGN_FACES: usize = 2;

/// Where the colour comes from, and the one mistake to avoid.
///
/// `0xdcdcdc00` written into `Font.drawInBatch` is **ARGB**: alpha 220, red 220,
/// green 220, blue 0. A port that read it as "grey `#dcdcdc`, alpha 0" would draw a
/// colour the mod never draws. [`text_channels`] splits it the way Java does.
pub fn text_channels(argb: u32) -> (u8, u8, u8, u8) {
    let alpha = ((argb >> 24) & 0xff) as u8;
    let red = ((argb >> 16) & 0xff) as u8;
    let green = ((argb >> 8) & 0xff) as u8;
    let blue = (argb & 0xff) as u8;
    (alpha, red, green, blue)
}

/// The deco controller's offset: the block it draws is pushed by the shape of the
/// block in front, on the facing axis (`TileEntityDecoControllerRenderer.java:57-66`).
///
/// Returned as the three components, from the shape's nearest and farthest point:
/// `UP` uses `min(Y)`, `DOWN` uses `max(Y) - 1`, `NORTH` `max(Z) - 1`, `SOUTH`
/// `min(Z)`, `EAST` `min(X)`, `WEST` `max(X) - 1`.
pub fn deco_offset(facing: u8, min: [f32; 3], max: [f32; 3]) -> [f32; 3] {
    match facing {
        // 0 = DOWN, 1 = UP, 2 = NORTH, 3 = SOUTH, 4 = WEST, 5 = EAST: the vanilla
        // `Direction` ordinals Java switches on.
        1 => [0.0, min[1], 0.0],
        0 => [0.0, max[1] - 1.0, 0.0],
        2 => [0.0, 0.0, max[2] - 1.0],
        3 => [0.0, 0.0, min[2]],
        5 => [min[0], 0.0, 0.0],
        4 => [max[0] - 1.0, 0.0, 0.0],
        _ => [0.0, 0.0, 0.0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seven_block_entities_and_the_hut_one_draws_nothing() {
        assert_eq!(BLOCK_RENDERERS.len(), 7);
        let empty = BLOCK_RENDERERS
            .iter()
            .find(|row| row.block == EMPTY)
            .expect("the hut row");
        assert_eq!(empty.name, "EmptyTileEntitySpecialRenderer");
        assert!(empty.draws.starts_with("nothing"));
        // The other six all draw something.
        assert_eq!(
            BLOCK_RENDERERS
                .iter()
                .filter(|row| !row.draws.starts_with("nothing"))
                .count(),
            6
        );
        let mut blocks: Vec<&str> = BLOCK_RENDERERS.iter().map(|row| row.block).collect();
        blocks.sort_unstable();
        let mut unique = blocks.clone();
        unique.dedup();
        assert_eq!(unique.len(), 7);
    }

    #[test]
    fn the_scarecrow_has_two_heads_and_two_textures() {
        assert_eq!(
            scarecrow_texture(ScarecrowHead::Pumpkin),
            "minecolonies:block/blockscarecrowpumpkin"
        );
        assert_eq!(
            scarecrow_texture(ScarecrowHead::Normal),
            "minecolonies:block/blockscarecrownormal"
        );
        assert_ne!(
            scarecrow_texture(ScarecrowHead::Pumpkin),
            scarecrow_texture(ScarecrowHead::Normal)
        );
        assert_eq!(scarecrow_head(0), ScarecrowHead::Pumpkin);
        assert_eq!(scarecrow_head(1), ScarecrowHead::Normal);
        assert_eq!(scarecrow_head(7), ScarecrowHead::Normal);
    }

    #[test]
    fn only_the_lower_half_of_a_scarecrow_is_drawn() {
        assert!(scarecrow_half_draws(false));
        assert!(!scarecrow_half_draws(true));
        assert_eq!(SCARECROW_AT, [0.5, 1.5, 0.5]);
        assert_eq!(scarecrow_rotation(0), 0.0, "north is not rotated");
        assert_eq!(scarecrow_rotation(1), 90.0);
        assert_eq!(scarecrow_rotation(2), 180.0);
        assert_eq!(scarecrow_rotation(3), 270.0);
        assert_eq!(scarecrow_rotation(9), 0.0, "an unknown facing falls to 0");
    }

    #[test]
    fn the_scarecrow_lantern_is_three_numbers_and_a_scale() {
        assert_eq!(SCARECROW_LANTERN.rotation_z, 180.0);
        assert_eq!(SCARECROW_LANTERN.at, [0.6, -0.6, -0.375]);
        assert_eq!(SCARECROW_LANTERN.scale, [0.75, 0.65, 0.75]);
        // Not square: the lantern is squashed downwards on purpose.
        assert_ne!(SCARECROW_LANTERN.scale[1], SCARECROW_LANTERN.scale[0]);
    }

    #[test]
    fn the_enchanter_book_bobs_and_turns_pages() {
        assert!((enchanter_bob(0.0) - 0.1).abs() < 1e-6);
        // sin(π/2) = 1 at tick = π/0.1
        assert!((enchanter_bob(std::f32::consts::FRAC_PI_2 / 0.1) - 0.11).abs() < 1e-5);
        assert!((enchanter_bob(std::f32::consts::PI / 0.1) - 0.1).abs() < 1e-5);
        assert_eq!(ENCHANTER_AT, [0.5, 0.75, 0.5]);
        assert_eq!(ENCHANTER_TEXTURE, "minecolonies:block/enchanting_table_book");
        // The two page corners are half a page apart and clamped.
        let first = enchanter_flip(0.0, 0.0, 0.0, false);
        let second = enchanter_flip(0.0, 0.0, 0.0, true);
        assert!((0.0..=1.0).contains(&first));
        assert!((0.0..=1.0).contains(&second));
        assert!(first < second);
        assert_eq!(lerp(0.5, 2.0, 4.0), 3.0);
        assert_eq!(lerp(0.0, 2.0, 4.0), 2.0);
        assert_eq!(lerp(1.0, 2.0, 4.0), 4.0);
    }

    #[test]
    fn the_flag_sways_between_two_small_angles() {
        let mut lowest = f32::MAX;
        let mut highest = f32::MIN;
        for time in 0..100 {
            let sway = flag_sway([12, 70, -34], time, 0.0);
            lowest = lowest.min(sway);
            highest = highest.max(sway);
        }
        // cos reaches -1 and +1 inside the 0..0.99 sweep, so the two bounds are
        // (-0.0125 - 0.01) * π and (-0.0125 + 0.01) * π: both negative, and the
        // whole range is under a tenth of a radian.
        let low = -0.0225_f32 * std::f32::consts::PI;
        let high = -0.0025_f32 * std::f32::consts::PI;
        assert!((lowest - low).abs() < 1e-4, "{lowest} vs {low}");
        assert!((highest - high).abs() < 1e-4, "{highest} vs {high}");
        assert!(highest < 0.0, "the cloth never tilts backwards");
        // The partial tick is the fraction of the next step, so it never jumps.
        assert_ne!(flag_sway([0, 0, 0], 10, 0.5), flag_sway([0, 0, 0], 10, 0.0));
        // Two blocks with the same hash move together; 7/9/13 makes that rare.
        assert_ne!(flag_sway([0, 0, 0], 0, 0.0), flag_sway([1, 0, 0], 0, 0.0));
    }

    #[test]
    fn the_flag_loses_its_pole_on_a_wall_and_is_drawn_at_two_thirds() {
        assert!(flag_has_pole(false), "a standing banner keeps the pole");
        assert!(!flag_has_pole(true), "a wall banner does not");
        assert!((FLAG_SCALE[0] - 2.0 / 3.0).abs() < 1e-6);
        assert!(FLAG_SCALE[1] < 0.0 && FLAG_SCALE[2] < 0.0);
    }

    #[test]
    fn an_empty_grave_says_unknown_citizen_and_four_lines_fit() {
        let fallback = grave_rows(&[]);
        assert_eq!(fallback.len(), 1);
        assert_eq!(fallback[0].text, SignText::Literal(String::from("Unknown Citizen")));
        assert_eq!(fallback[0].line, 0);
        assert_eq!(fallback[0].offset, 0);

        let four: Vec<String> = ["Ann", "Bob", "Cid", "Dee"].iter().map(|s| (*s).to_string()).collect();
        let rows = grave_rows(&four);
        assert_eq!(rows.len(), 4);
        assert_eq!(
            rows.iter().map(|row| row.line).collect::<Vec<u8>>(),
            vec![0, 1, 2, 3]
        );
        assert!(rows.iter().all(|row| row.offset == 0));
    }

    #[test]
    fn a_sign_alone_is_centred_and_a_sign_with_a_neighbour_stacks_two_blocks() {
        let alone = sign_rows("Aurora", 137, None, false);
        assert_eq!(alone.len(), 2);
        assert_eq!(alone[0].offset, -35);
        assert_eq!(alone[0].text, SignText::Literal(String::from("Aurora")));
        assert_eq!(alone[1].text, SignText::Distance(137));
        assert_eq!(alone[1].line, 3);

        let pair = sign_rows("Aurora", 137, Some(("Borealis", 900)), true);
        assert_eq!(pair.len(), 4);
        assert_eq!(
            pair.iter().map(|row| row.offset).collect::<Vec<i32>>(),
            vec![-10, -10, -60, -60]
        );
        assert_eq!(pair[2].text, SignText::Literal(String::from("Borealis")));

        // The sign above the colony it names does not repeat it (`:139`).
        let same = sign_rows("Aurora", 137, Some(("Borealis", 900)), false);
        assert_eq!(same.len(), 2);
        assert_eq!(same[0].offset, -35);
    }

    #[test]
    fn an_unknown_colony_is_named_and_its_distance_goes_under_it() {
        let rows = sign_rows("", 42, Some(("Borealis", 900)), true);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].text, SignText::Literal(String::from("Unknown Colony")));
        assert_eq!(rows[0].offset, 0);
        assert_eq!(rows[1].text, SignText::Distance(42));
    }

    #[test]
    fn both_texts_stop_at_twenty_characters_and_neither_wraps_the_same_way() {
        assert_eq!(truncate("short"), "short");
        assert_eq!(truncate("0123456789012345678"), "0123456789012345678");
        assert_eq!(truncate("01234567890123456789"), "01234567890123456789");
        assert_eq!(truncate("012345678901234567890"), "01234567890123456789");
        // Characters, not UTF-16 units: a two-byte character counts once.
        assert_eq!(truncate("éééééééééééééééééééé").chars().count(), 20);

        assert_eq!(GRAVE_LAYOUT.max_chars, SIGN_LAYOUT.max_chars);
        assert_eq!(GRAVE_LAYOUT.max_chars, TextLayout::MAX_CHARS);
        assert_eq!(GRAVE_LAYOUT.colour, SIGN_LAYOUT.colour);
        assert_eq!(GRAVE_LAYOUT.line_px, 10.0);
        assert_eq!(SIGN_LAYOUT.line_px, 8.0);
        assert_ne!(GRAVE_LAYOUT.scale, SIGN_LAYOUT.scale);
        assert_eq!(GRAVE_LAYOUT.wrap_px, 0, "the grave never splits a name");
        assert_eq!(SIGN_LAYOUT.wrap_px, 90);
        assert_eq!(SIGN_LAYOUT.wrap_lines, 2);
        assert_eq!(SIGN_FACES, 2);
    }

    #[test]
    fn the_text_colour_is_read_as_argb_and_is_not_a_grey() {
        // 0xdcdcdc00: alpha 220, red 220, green 220, blue 0.
        assert_eq!(text_channels(GRAVE_LAYOUT.colour), (220, 220, 220, 0));
        assert_eq!(SIGN_LAYOUT.colour, GRAVE_LAYOUT.colour);
        // Not the colour it looks like if read as RGB + a trailing alpha.
        assert_ne!(text_channels(GRAVE_LAYOUT.colour).3, 220);
        assert_eq!(text_channels(0xffff_ffff), (255, 255, 255, 255));
        assert_eq!(text_channels(0x0000_0000), (0, 0, 0, 0));
    }

    #[test]
    fn the_deco_controller_pushes_along_the_facing_axis_only() {
        // `TileEntityDecoControllerRenderer.java:61-69` is the whole rule: the
        // offset is the shape's own bound on the facing axis, `min` for UP, SOUTH
        // and EAST (`:63`, `:66`, `:67`) and `max - 1` for DOWN, NORTH and WEST
        // (`:64`, `:65`, `:68`).
        //
        // **The numbers below are dyadic on purpose.** Java computes in `double`
        // (`VoxelShape.max` returns one) and translates by that (`:75`); this
        // crate's `BlockEntityDrawContext::translate` takes `f32`
        // (`host/draw.rs:185`). For a bound of 0.9, `max - 1` is
        // `-0.09999999999999998` in Java and `-0.100000024` here, and *neither*
        // is `-0.1`: an assertion on `-0.1` would be asserting a value no side
        // produces. A bound of 0.25 or 0.75 is exact in both widths, so the
        // strict equality below is Java's result, not a rounded one.
        let min = [0.25, 0.25, 0.5];
        let max = [0.75, 0.75, 0.75];
        assert_eq!(deco_offset(1, min, max), [0.0, 0.25, 0.0], "UP: min(Y), `:63`");
        assert_eq!(deco_offset(0, min, max), [0.0, -0.25, 0.0], "DOWN: max(Y) - 1, `:64`");
        assert_eq!(deco_offset(2, min, max), [0.0, 0.0, -0.25], "NORTH: max(Z) - 1, `:65`");
        assert_eq!(deco_offset(3, min, max), [0.0, 0.0, 0.5], "SOUTH: min(Z), `:66`");
        assert_eq!(deco_offset(5, min, max), [0.25, 0.0, 0.0], "EAST: min(X), `:67`");
        assert_eq!(deco_offset(4, min, max), [-0.25, 0.0, 0.0], "WEST: max(X) - 1, `:68`");
        // One axis moves and the other two stay zero: that is the rule. Every
        // value above is non-zero, so the count below has teeth — a direction
        // that fell into the `_` arm would move none.
        for facing in 0..=5 {
            let offset = deco_offset(facing, min, max);
            let moved = offset.iter().filter(|value| **value != 0.0).count();
            assert_eq!(moved, 1, "facing {facing} moved {moved} axes");
        }
        // The `_` arm is the port's, not Java's: `Direction` has exactly six
        // values, so Java's exhaustive switch has no default. An ordinal that is
        // not one of them must move nothing rather than guess an axis.
        assert_eq!(deco_offset(9, min, max), [0.0, 0.0, 0.0]);
    }
}
