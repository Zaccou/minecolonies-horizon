//! `TextureReloadListener.java`: which folder names a citizen texture pack, and the
//! index that decides.
//!
//! Java reference: `TextureReloadListener.java`, 76 lines, 3 branches, all in
//! [`super::table`]. A `SimplePreparableReloadListener` over the client's resources:
//! `prepare` scans and builds a set of **folder names**, `apply` clears the static list
//! and copies them in. `TEXTURE_PACKS` is read by the citizen screen's style picker,
//! so the only thing this listener produces is a list of names.
//!
//! **The rule is one line of string surgery** (`:42`): the third slash-separated
//! segment of a path under `textures/entity/citizen` is the pack. The three tests
//! before it are the filter — the path must contain `png` and the prefix — and the
//! "empty folder is skipped" test (`:43`) is the fourth.
//!
//! **Two things the port refuses instead of copying.**
//!
//! 1. **`split("/")[3]` is a hard index.** A path that passes the two `contains`
//!    tests always has at least four segments, so the index never throws in practice —
//!    but it *would* for `textures/entity/citizen` itself, and the Java has no guard.
//!    [`pack_of`] answers [`Refusal::TooShort`] instead of panicking; a Rust panic in a
//!    reload listener aborts the resource reload, which is strictly worse than the
//!    `ArrayIndexOutOfBoundsException` the Java throws in a `try` the caller owns.
//! 2. **`"png"` is a substring test, not a suffix test** (`:40`). A file named
//!    `citizen/foo/png_notes.txt` passes it and yields `foo`. The port keeps the test
//!    as written and says so: narrowing it to `.ends_with(".png")` would change which
//!    packs a pack author's directory produces.
//!
//! **The two phases are not an implementation detail.** `prepare` runs off-thread and
//! `apply` on the main thread (`:50-60`), so the set is built on one and copied on the
//! other; [`prepare`] returns the list and [`apply`] is the swap, which is the same
//! split without a thread.

/// The folder every citizen texture lives under (`:37`).
pub const CITIZEN_TEXTURES: &str = "textures/entity/citizen";
/// The substring Java looks for in the path (`:40`).
pub const PNG_MARKER: &str = "png";
/// The segment a pack's name is taken from (`:42`): `textures/entity/citizen/<here>/…`.
pub const PACK_SEGMENT: usize = 3;

/// Why a path yielded no pack name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The path does not name a PNG under the citizen folder (`:40`).
    NotACitizenTexture,
    /// The path has fewer than [`PACK_SEGMENT`] + 1 segments, so Java's index would
    /// throw.
    TooShort,
    /// The folder segment is empty (`:43`).
    EmptyFolder,
}

/// The pack name inside one resource path, or why there is none.
///
/// `textures/entity/citizen/female_farmer/body.png` → `female_farmer`.
pub fn pack_of(path: &str) -> Result<&str, Refusal> {
    if !path.contains(PNG_MARKER) || !path.contains(CITIZEN_TEXTURES) {
        return Err(Refusal::NotACitizenTexture);
    }
    let segments: Vec<&str> = path.split('/').collect();
    let folder = *segments.get(PACK_SEGMENT).ok_or(Refusal::TooShort)?;
    if folder.is_empty() {
        return Err(Refusal::EmptyFolder);
    }
    Ok(folder)
}

/// `prepare` (`:34-53`): every pack name the paths produce, **de-duplicated** — Java
/// collects into a `HashSet` and then a list, so the order is hash order and a
/// repeated folder appears once.
///
/// The port returns sorted names instead of hash order: the list is shown to a player,
/// and a picker whose entries move between two launches is a bug an author would
/// report as "my texture pack jumps around". The set, not the order, is what the Java
/// guarantees.
pub fn prepare<'a>(paths: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut packs: Vec<String> = Vec::new();
    for path in paths {
        if let Ok(folder) = pack_of(path) {
            let folder = folder.to_owned();
            if !packs.contains(&folder) {
                packs.push(folder);
            }
        }
    }
    packs.sort();
    packs
}

/// `apply` (`:56-60`): the list `TEXTURE_PACKS` holds after a reload.
///
/// Java clears and then `addAll`s; answering the new list and letting the caller
/// replace its reference is the same write without a `static mut`.
pub fn apply(prepared: &[String]) -> Vec<String> {
    prepared.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_folder_under_citizen_is_the_pack_name() {
        assert_eq!(
            pack_of("textures/entity/citizen/female_farmer/body.png"),
            Ok("female_farmer")
        );
        assert_eq!(
            pack_of("textures/entity/citizen/male_citizen/head.png"),
            Ok("male_citizen")
        );
    }

    #[test]
    fn a_path_outside_the_citizen_folder_is_not_a_pack() {
        assert_eq!(
            pack_of("textures/entity/zombie/zombie.png"),
            Err(Refusal::NotACitizenTexture)
        );
        assert_eq!(
            pack_of("textures/entity/citizen/female_farmer/body.json"),
            Err(Refusal::NotACitizenTexture)
        );
    }

    #[test]
    fn the_png_test_is_a_substring_test_as_in_the_java() {
        // `:40` is `res.getPath().contains("png")`, not `endsWith(".png")`.
        assert_eq!(
            pack_of("textures/entity/citizen/odd/png_notes.txt"),
            Ok("odd")
        );
    }

    #[test]
    fn a_path_too_short_for_the_java_index_is_refused_not_panicked() {
        assert_eq!(
            pack_of("textures/entity/citizenpng"),
            Err(Refusal::TooShort),
            "three segments that pass both `contains` tests: `split(\"/\")[3]` would throw"
        );
    }

    #[test]
    fn an_empty_folder_is_skipped() {
        assert_eq!(
            pack_of("textures/entity/citizen//body.png"),
            Err(Refusal::EmptyFolder)
        );
    }

    #[test]
    fn prepare_keeps_one_entry_per_folder() {
        let paths = [
            "textures/entity/citizen/male_farmer/body.png",
            "textures/entity/citizen/male_farmer/head.png",
            "textures/entity/citizen/female_farmer/body.png",
        ];
        assert_eq!(prepare(paths), vec!["female_farmer", "male_farmer"]);
    }

    #[test]
    fn prepare_ignores_the_paths_that_name_no_pack() {
        let paths = [
            "textures/entity/zombie/zombie.png",
            "textures/entity/citizen/male_farmer/body.json",
            "textures/entity/citizen//body.png",
        ];
        assert!(prepare(paths).is_empty());
    }

    #[test]
    fn prepare_is_sorted_so_the_picker_does_not_move() {
        let paths = [
            "textures/entity/citizen/zeta/body.png",
            "textures/entity/citizen/alpha/body.png",
            "textures/entity/citizen/mu/body.png",
        ];
        assert_eq!(prepare(paths), vec!["alpha", "mu", "zeta"]);
    }

    #[test]
    fn apply_is_the_whole_swap() {
        let prepared = vec!["male_farmer".to_owned()];
        assert_eq!(apply(&prepared), prepared);
        assert!(apply(&[]).is_empty(), "a reload that found nothing clears");
    }

    #[test]
    fn the_constants_are_the_java_literals() {
        assert_eq!(CITIZEN_TEXTURES, "textures/entity/citizen");
        assert_eq!(PNG_MARKER, "png");
        assert_eq!(PACK_SEGMENT, 3);
    }
}
