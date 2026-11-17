//! Les lectures et écritures de tags que les quatorze block entities partagent.
//!
//! `Compound` est `BTreeMap<JavaString, Tag>`
//! (`.horizon/sdk/api/crates/horizon-core/src/nbt.rs:35`), et `Tag` la valeur NBT
//! (`:41-56`), tous deux réexportés par l'API.
//!
//! **Ce fichier est une quatrième copie de la même dizaine de fonctions**, après
//! `src/colony/nbt.rs`, `src/building/nbt.rs` et `src/requests/nbt.rs`, et pour
//! la même raison qu'eux : ces trois-là sont `mod nbt;` **privés** dans leur
//! module (`src/colony/mod.rs:28`, `src/building/mod.rs:20`,
//! `src/requests/mod.rs:41`), donc inaccessibles d'ici. Le rendre `pub(crate)`
//! est une ligne dans un fichier que cet agent n'a pas le droit de toucher ; le
//! rapport le dit plutôt que de dupliquer en silence.
//!
//! **Deux fonctions ne sont pas recopiées** : les positions viennent de
//! `crate::hut::{position, position_in}`, qui sont déjà publiques et qui portent
//! la même disposition `{x, y, z}` (`api/util/BlockPosUtil.java:190-197`). Une
//! seconde copie serait la cinquième version du même codec.
//!
//! **Lectures tolérantes, écritures strictes.** Une lecture accepte les trois
//! largeurs d'entier que NBT peut porter pour un petit nombre, pour qu'un
//! `/data merge block` écrit à la main reste lisible ; une écriture utilise
//! toujours la largeur que le modèle veut.

use mod_api::{Compound, Tag};

pub use crate::hut::{position, position_in};

/// Un entier sous `clé`, dans la largeur où il a été sauvé.
pub fn int(data: &Compound, key: &str) -> Option<i32> {
    number(data.get(&key.into())?)
}

/// L'entier que porte un tag déjà atteint.
pub fn int_of(tag: &Tag) -> Option<i32> {
    number(tag)
}

/// Un texte sous `clé`. Un tag d'une autre sorte est `None`, pas sa forme de
/// débogage : un nom de colonie n'est pas un nombre.
pub fn text(data: &Compound, key: &str) -> Option<String> {
    text_of(data.get(&key.into())?)
}

pub fn text_of(tag: &Tag) -> Option<String> {
    match tag {
        Tag::String(value) => Some(value.to_string_lossy()),
        _ => None,
    }
}

/// Un booléen sous `clé` (`CompoundTag.getBoolean`, `TileEntityBarrel.java:218`).
pub fn boolean(data: &Compound, key: &str) -> Option<bool> {
    data.get(&key.into()).and_then(boolean_of)
}

pub fn boolean_of(tag: &Tag) -> Option<bool> {
    match tag {
        Tag::Byte(value) => Some(*value != 0),
        // Un `putBoolean` écrit toujours un octet ; un `/data merge` peut écrire
        // un int, et Java le lit comme faux (`getBoolean` teste `BYTE`).
        _ => None,
    }
}

pub fn boolean_tag(value: bool) -> Tag {
    Tag::Byte(i8::from(value))
}

/// Un `float`, que NBT garde en **bits** (`Tag::Float(u32)`,
/// `.horizon/sdk/api/crates/horizon-core/src/nbt.rs:44-48`).
pub fn float(data: &Compound, key: &str) -> Option<f32> {
    match data.get(&key.into())? {
        Tag::Float(bits) => Some(f32::from_bits(*bits)),
        _ => None,
    }
}

pub fn float_tag(value: f32) -> Tag {
    Tag::Float(value.to_bits())
}

/// Un tableau d'entiers sous `clé` (`CompoundTag.getIntArray`,
/// `TileEntityScarecrow.java:96`).
pub fn int_array<'a>(data: &'a Compound, key: &str) -> Option<&'a [i32]> {
    match data.get(&key.into())? {
        Tag::IntArray(values) => Some(values.as_slice()),
        _ => None,
    }
}

pub fn int_array_tag(values: Vec<i32>) -> Tag {
    Tag::IntArray(values)
}

/// Les textes d'une liste, dans l'ordre (`AbstractTileEntityNamedGrave.java:56-64`).
/// Un élément d'une autre sorte est sauté : une ligne de tombe est une chaîne.
pub fn strings(data: &Compound, key: &str) -> Vec<String> {
    match data.get(&key.into()) {
        Some(Tag::List(items)) => items.iter().filter_map(text_of).collect(),
        _ => Vec::new(),
    }
}

pub fn string_list(lines: &[String]) -> Tag {
    Tag::List(
        lines
            .iter()
            .map(|line| Tag::String(line.as_str().into()))
            .collect(),
    )
}

/// Chaque compound de la liste sous `clé`, dans l'ordre. Une liste absente ou
/// d'une autre sorte est vide : une block entity au fichier corrompu garde ses
/// autres clés.
pub fn compounds<'a>(data: &'a Compound, key: &str) -> Vec<&'a Compound> {
    match data.get(&key.into()) {
        Some(Tag::List(items)) => items
            .iter()
            .filter_map(|item| match item {
                Tag::Compound(inner) => Some(inner),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub fn compound_list(items: Vec<Compound>) -> Tag {
    Tag::List(items.into_iter().map(Tag::Compound).collect())
}

fn number(tag: &Tag) -> Option<i32> {
    match tag {
        Tag::Int(value) => Some(*value),
        Tag::Byte(value) => Some(i32::from(*value)),
        Tag::Short(value) => Some(i32::from(*value)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_position_vient_du_codec_de_la_hutte() {
        let data = Compound::from([("anchor".into(), position([-3, 71, 1208]))]);
        assert_eq!(position_in(&data, "anchor"), Some([-3, 71, 1208]));
        assert_eq!(position_in(&Compound::new(), "anchor"), None);
    }

    #[test]
    fn un_entier_se_lit_dans_les_trois_largeurs_et_un_texte_ne_se_lit_pas_comme_un_nombre() {
        let data = Compound::from([
            ("a".into(), Tag::Byte(3)),
            ("b".into(), Tag::Short(-7)),
            ("c".into(), Tag::Int(64)),
            ("d".into(), Tag::String("64".into())),
        ]);
        assert_eq!(int(&data, "a"), Some(3));
        assert_eq!(int(&data, "b"), Some(-7));
        assert_eq!(int(&data, "c"), Some(64));
        assert_eq!(int(&data, "d"), None);
        assert_eq!(text(&data, "d"), Some("64".to_owned()));
        assert_eq!(text(&data, "c"), None);
        assert_eq!(int_of(&Tag::Int(9)), Some(9));
    }

    #[test]
    fn un_booleen_est_un_octet_et_un_float_garde_ses_bits() {
        let data = Compound::from([
            ("done".into(), boolean_tag(true)),
            ("wrong".into(), Tag::Int(1)),
            ("rotation".into(), float_tag(272.5)),
        ]);
        assert_eq!(boolean(&data, "done"), Some(true));
        assert_eq!(boolean(&data, "wrong"), None, "getBoolean ne lit qu'un BYTE");
        assert_eq!(boolean(&data, "absent"), None);
        // 272.5 n'a pas de représentation exacte en f32 : on compare la valeur relue
        // à celle que le tag a réellement encodée.
        assert_eq!(float(&data, "rotation"), Some(272.5f32));
        assert_eq!(float(&data, "done"), None);
    }

    #[test]
    fn un_tableau_d_entiers_et_une_liste_de_textes_round_trip() {
        let lines = vec!["Citizen".to_owned(), "of".to_owned(), "MineColonies".to_owned()];
        let data = Compound::from([
            ("radius".into(), int_array_tag(vec![5, 6, 7, 20])),
            ("content".into(), string_list(&lines)),
        ]);
        assert_eq!(int_array(&data, "radius"), Some(&[5, 6, 7, 20][..]));
        assert_eq!(strings(&data, "content"), lines);
        assert_eq!(strings(&Compound::new(), "content"), Vec::<String>::new());
        assert!(int_array(&data, "content").is_none());
    }

    #[test]
    fn une_liste_de_compounds_saute_ce_qui_n_est_pas_un_compound() {
        let record = Compound::from([("id".into(), Tag::Int(1))]);
        let data = Compound::from([(
            "members".into(),
            Tag::List(vec![Tag::Compound(record), Tag::Int(2)]),
        )]);
        assert_eq!(compounds(&data, "members").len(), 1);
        assert!(compounds(&Compound::new(), "members").is_empty());
        assert_eq!(
            compound_list(vec![Compound::from([("a".into(), Tag::Int(1))])]).id(),
            9,
            "une liste de records est une LIST"
        );
    }
}
