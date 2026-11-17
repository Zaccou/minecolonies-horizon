//! Les huit `WindowConstants` que ces quatre fenêtres lisent, avec la ligne de chacun.
//!
//! Java référence : `api/util/constant/WindowConstants.java`, 1 195 lignes. Le fichier est
//! un sac plat de chaînes partagé par toutes les GUI du mod, et **trois de ces huit
//! constantes ont une valeur que plusieurs champs portent** : quatre champs valent `name`,
//! deux valent `done`, deux valent `cancel` (la table de `crate::ui_window_01::constants`
//! compte les 24 que ses trois fenêtres lisent ; celle-ci en ajoute cinq). C'est pourquoi
//! [`Constant`] porte le **nom du champ** et pas seulement sa valeur : `INPUT_NAME:927` et
//! `LABEL_NAME:1090` sont tous deux `"name"`, et seule la fenêtre sait laquelle elle lit.
//!
//! **Le type est celui de `src/ui_window_01`, pas un second.** `Constant` est public là-bas
//! et la forme est exactement ce dont cette tranche a besoin : un champ, une valeur, une
//! ligne. La fonction de recherche, elle, est locale ([`value_of`]) parce qu'elle interroge
//! **cette** table et non celle de la tranche 1 — deux tables, deux recherches.
//!
//! **Ce que ces quatre classes **n'**importent pas est aussi instructif** :
//! `WindowTownHallMercenary` n'importe que `BUTTON_DONE` (`:19`, `:64`) et ne lit `cancel`
//! nulle part — n'importe quel bouton ferme la fenêtre (`:71`). `cancelaction` n'est pas
//! dans `WindowConstants` du tout : `WindowTownHallDeleteAbandonColony` l'enregistre avec
//! un littéral (`:51`).

pub use crate::ui_window_01::constants::Constant;

use super::refusal::Refusal;

/// Les huit constantes lues par les quatre fenêtres de ce module, **dans l'ordre du
/// fichier** (un lecteur peut suivre `WindowConstants.java` et cocher).
pub const WINDOW_CONSTANTS: &[Constant] = &[
    // `WindowTownHallDeleteAbandonColony.java:49`
    Constant { field: "BUTTON_DELETE", value: "delete", line: 162 },
    // `WindowTownHallDeleteAbandonColony.java:50`
    Constant { field: "BUTTON_ABANDON", value: "abandon", line: 167 },
    // `WindowTownHallDeleteAbandonColony.java:53`
    Constant { field: "BUTTON_CONFIRM_DELETE", value: "confirmdelete", line: 172 },
    // `WindowTownHallDeleteAbandonColony.java:54`
    Constant { field: "BUTTON_CONFIRM_ABANDON", value: "confirmabandon", line: 177 },
    // `WindowTownHallColonyReactivate.java:42`, `WindowTownHallDeleteAbandonColony.java:47`,
    // `WindowTownHallNameEntry.java:53`
    Constant { field: "BUTTON_CANCEL", value: "cancel", line: 477 },
    // `WindowTownHallDeleteAbandonColony.java:48`
    Constant { field: "BUTTON_PICKUP_BUILDING", value: "pickup", line: 594 },
    // `WindowTownHallNameEntry.java:39` (et `:47` pour la lecture)
    Constant { field: "INPUT_NAME", value: "name", line: 927 },
    // `WindowTownHallMercenary.java:64`, `WindowTownHallNameEntry.java:45`
    Constant { field: "BUTTON_DONE", value: "done", line: 934 },
];

/// La valeur du champ `field`, ou [`Refusal::UnknownConstant`].
///
/// Un `Result` et non un `Option` : le seul appelant est un test qui vérifie qu'une
/// constante citée par une fenêtre existe bien dans cette table, et un refus nommé dit
/// **laquelle** manque dans le message d'échec.
pub fn value_of(field: &str) -> Result<&'static str, Refusal> {
    WINDOW_CONSTANTS
        .iter()
        .find(|constant| constant.field == field)
        .map(|constant| constant.value)
        .ok_or(Refusal::UnknownConstant)
}

/// Les champs de **cette** table dont la valeur est `value`, dans l'ordre du fichier.
pub fn fields_for(value: &str) -> Vec<&'static str> {
    WINDOW_CONSTANTS
        .iter()
        .filter(|constant| constant.value == value)
        .map(|constant| constant.field)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_in_file_order_and_every_line_is_a_real_one() {
        assert_eq!(WINDOW_CONSTANTS.len(), 8);
        for pair in WINDOW_CONSTANTS.windows(2) {
            assert!(
                pair[0].line < pair[1].line,
                "{}:{} vient avant {}:{}",
                pair[0].field,
                pair[0].line,
                pair[1].field,
                pair[1].line
            );
        }
        for constant in WINDOW_CONSTANTS {
            // `WindowConstants.java` fait 1 195 lignes : hors de là, c'est une faute de frappe.
            assert!((1..=1195).contains(&constant.line), "{}", constant.field);
            assert!(!constant.value.is_empty(), "{} n'a pas de valeur", constant.field);
        }
    }

    #[test]
    fn the_three_shared_constants_agree_with_the_first_slice() {
        // Une seule règle, une seule valeur : la tranche 1 publie déjà ces trois champs.
        for field in ["BUTTON_CANCEL", "INPUT_NAME", "BUTTON_DONE"] {
            let mine = value_of(field).unwrap_or_else(|refusal| panic!("{field} : {refusal}"));
            assert_eq!(
                crate::ui_window_01::constants::value_of(field),
                Some(mine),
                "{field} ne vaut pas la même chose dans les deux tables"
            );
        }
        // Les cinq autres ne sont pas dans la tranche 1 : elles sont nouvelles ici.
        for field in [
            "BUTTON_DELETE",
            "BUTTON_ABANDON",
            "BUTTON_CONFIRM_DELETE",
            "BUTTON_CONFIRM_ABANDON",
            "BUTTON_PICKUP_BUILDING",
        ] {
            assert!(value_of(field).is_ok(), "{field}");
            assert_eq!(crate::ui_window_01::constants::value_of(field), None, "{field}");
        }
    }

    #[test]
    fn done_and_cancel_are_shared_by_four_windows_and_name_is_the_input() {
        // Les deux valeurs que plusieurs fenêtres de ce module lisent.
        assert_eq!(fields_for("done"), ["BUTTON_DONE"]);
        assert_eq!(fields_for("cancel"), ["BUTTON_CANCEL"]);
        // `cancel` vaut deux champs dans la table de la tranche 1 : le champ, pas la
        // valeur, dit quelle fenêtre lit quoi.
        assert!(crate::ui_window_01::constants::names_for("cancel").count() > 1);
        // Le champ de saisie de l'hôtel de ville est `INPUT_NAME`, pas `LABEL_NAME`.
        assert_eq!(value_of("INPUT_NAME"), Ok("name"));
        assert_eq!(value_of("LABEL_NAME"), Err(Refusal::UnknownConstant));
        // Une valeur que ces quatre fenêtres ne lisent pas n'est pas dans la table.
        assert!(fields_for("mercenaries").is_empty(), "BUTTON_MERCENARY:87 n'est pas un bouton d'ici");
    }
}
