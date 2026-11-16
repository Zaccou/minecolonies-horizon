//! The real blueprint pack, as the tests read it.
//!
//! The converter's contract is a real file, not a hand-built tree: the pack
//! ships with the port (`data/minecolonies/blueprints/minecolonies/`, **9 515
//! files**, 24 MiB compressed) because it is the mod's content, so a test can
//! read the same bytes the game will. `CARGO_MANIFEST_DIR` is the crate root at
//! compile time, which is where `cargo test --lib` starts too.
//!
//! **Test-only** (`mod.rs` declares it under `#[cfg(test)]`), because the
//! production path is `Host::resource` handing over the bytes of one file at a
//! time from the server's pack stack; `std::fs` here only reads the crate's own
//! copy of that pack.

use std::path::{Path, PathBuf};

/// The pack's root inside the crate: the `minecolonies` namespace folder, so a
/// relative path reads like the `name` tag of the blueprint itself
/// (`darkoak/agriculture/husbandry/fisherman2.blueprint`).
const PACK: &str = "data/minecolonies/blueprints/minecolonies";

/// Absolute path of one blueprint of the pack.
pub fn path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(PACK)
        .join(relative)
}

/// The bytes of one blueprint, or a failure that names the missing file.
pub fn read(relative: &str) -> Vec<u8> {
    let path = path(relative);
    std::fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "blueprint pack file {} could not be read ({error}); the tests read the real \
             corpus, so a tree without data/ cannot run them",
            path.display()
        )
    })
}

/// Every blueprint of the pack, relative to [`PACK`], sorted.
///
/// Sorted on purpose: the sample below must be the same files on every run, or
/// a failure would not be reproducible.
pub fn files() -> Vec<String> {
    let root = path("");
    let mut out = Vec::new();
    walk(&root, &root, &mut out);
    out.sort();
    out
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|error| {
        panic!("blueprint pack directory {} could not be read ({error})", dir.display())
    });
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("blueprint pack entry: {error}"));
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, out);
        } else if path.to_string_lossy().ends_with(".blueprint") {
            let relative = path
                .strip_prefix(root)
                .unwrap_or_else(|error| panic!("{} is outside the pack: {error}", path.display()));
            out.push(relative.to_string_lossy().into_owned());
        }
    }
}

/// `count` files spread evenly over the whole pack, with their bytes.
///
/// A stride, not a random draw: the pack is 25 styles × ~40 building kinds, so
/// every `len / count`-th file walks all the styles and all the kinds, and the
/// list is the same on every run.
pub fn sample(count: usize) -> Vec<(String, Vec<u8>)> {
    let all = files();
    assert!(
        all.len() > count,
        "the pack holds {} blueprints, too few to sample {count}",
        all.len()
    );
    let stride = all.len() / count;
    all.iter()
        .step_by(stride.max(1))
        .take(count)
        .map(|relative| (relative.clone(), read(relative)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pack_is_where_the_tests_expect_it() {
        let all = files();
        assert_eq!(all.len(), 9_515, "the pack ships 9 515 blueprints");
        assert!(
            all.iter()
                .any(|file| file.ends_with("husbandry/fisherman2.blueprint")),
            "a known path is in the listing"
        );
        assert!(
            all.windows(2).all(|pair| pair[0] <= pair[1]),
            "the listing is sorted"
        );
    }

    #[test]
    fn one_named_file_reads_as_gzip_bytes() {
        let bytes = read("darkoak/agriculture/husbandry/fisherman2.blueprint");
        assert!(bytes.len() > 1_000);
        assert_eq!(
            &bytes[..3],
            &[0x1f, 0x8b, 0x08][..],
            "a blueprint is a gzip member"
        );
    }

    #[test]
    fn a_sample_is_spread_over_the_styles_and_never_repeats() {
        let sample = sample(24);
        assert_eq!(sample.len(), 24);
        let styles: std::collections::BTreeSet<&str> = sample
            .iter()
            .filter_map(|(relative, _)| relative.split('/').next())
            .collect();
        assert!(
            styles.len() >= 20,
            "24 files over 25 styles should touch most of them, saw {}",
            styles.len()
        );
        let mut paths: Vec<&String> = sample.iter().map(|(relative, _)| relative).collect();
        paths.dedup();
        assert_eq!(paths.len(), 24, "no file appears twice");
    }
}
