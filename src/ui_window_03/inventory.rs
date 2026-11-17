//! `WindowHutAllInventory` : tout ce que la hutte contient, un filtre, cinq tris, une
//! loupe — et un `static` que personne n'attend là.
//!
//! Java reference: `core/client/gui/WindowHutAllInventory.java` (344 lignes),
//! `assets/minecolonies/gui/windowhutallinventory.xml:1-24`,
//! `api/util/Utils.java:178-192` (`format`), `WindowConstants.java:528-552` (les cinq
//! états du tri, déjà portés par `crate::ui::next_sort`).
//!
//! **`src/ui` décrit déjà l'écran** (`ui::screen::HUT_INVENTORY_ID`) : ses sept panneaux,
//! ses trois boutons et leurs sources. Ce fichier porte l'état et les règles de lecture,
//! pas une seconde table de panneaux.
//!
//! # Le tri survit à la fenêtre, et ce n'est pas un détail
//!
//! `sortDescriptor` est `private static` (`:63`) — le commentaire de Java dit pourquoi :
//! « Intentionally static to preserve the setting for the current game session ». Deux
//! fenêtres ouvertes en même temps partagent donc leur tri, et le fermer n'y change rien.
//! [`Session`] est cet état-là ; [`State`] est celui de la fenêtre. Un port qui mettrait le
//! tri dans `State` perdrait le réglage à chaque ouverture, ce que le jeu ne fait pas.
//!
//! # La liste est triée **deux fois**, et la première ne se voit pas
//!
//! `updateResources` trie toujours par distance de Levenshtein entre le nom et le filtre
//! (`:254`), **avant** d'appliquer le tri choisi (`:256-276`). Avec un filtre vide, cette
//! distance vaut la longueur du nom : le mode « pas de tri » est donc un tri par longueur
//! de nom, pas l'ordre des racks. Et les `sort` de Java étant stables, les égalités du
//! second tri gardent cet ordre-là.
//!
//! # Le filtre cherche deux choses, dont une qui n'est pas le nom
//!
//! Le prédicat teste l'**id de description** de l'item (`item.minecraft.stone`) puis le
//! texte des infobulles (`:239-243`). Le nom n'est donc jamais testé directement : il
//! passe par l'infobulle, dont il est la première ligne. C'est ce qui fait qu'un filtre
//! « stone » trouve une pierre dont le nom est traduit en français.
//!
//! # Dix ticks de retard
//!
//! La saisie met à jour `filter` tout de suite mais ne relit les racks que **dix ticks**
//! plus tard, et chaque nouvelle frappe remet le compte à dix (`:97-104`, `:111-114`) :
//! la liste affichée est donc en retard sur le champ, et la loupe travaille sur le
//! `allItems` de ce retard-là.

use crate::ui::next_sort;

/// La classe Java.
pub const JAVA: &str = "WindowHutAllInventory";
/// Le XML, chargé à `:88`.
pub const RESOURCE: &str = "minecolonies:gui/windowhutallinventory.xml";

/// Le panneau du titre (`windowhutallinventory.xml:6`).
pub const DESC_LABEL: &str = "desc";
/// Le champ du filtre (`windowhutallinventory.xml:7`, lu à `:97`).
pub const FILTER_INPUT: &str = "names";
/// Le bouton de tri (`WindowConstants.java:552`, `:90`).
pub const SORT_BUTTON: &str = "sortStorageFilter";
/// La liste (`WindowConstants.java:548`, `:92`).
pub const LIST: &str = "allinventorylist";
/// L'icône d'une ligne (`WindowConstants.java:508`).
pub const RESOURCE_ICON: &str = "resourceIcon";
/// Le nom d'une ligne (`windowhutallinventory.xml:14`).
pub const STACK_NAME_LABEL: &str = "ressourceStackName";
/// La quantité d'une ligne (`:15`).
pub const QUANTITIES_LABEL: &str = "quantities";
/// La loupe d'une ligne (`WindowConstants.java:1078`, `:94`).
pub const LOCATE_BUTTON: &str = "locate";
/// Le retour (`WindowConstants.java:882`, `:91`).
pub const BACK_BUTTON: &str = "back";

/// Les cinq libellés du bouton de tri, dans l'ordre des états (`:180-194`).
///
/// `v^` pour « aucun tri », puis `A^`, `Av`, `1^`, `1v` : une lettre pour le nom, un
/// chiffre pour le compte, une flèche pour le sens.
pub const SORT_LABELS: [&str; 5] = ["v^", "A^", "Av", "1^", "1v"];

/// Le nombre de ticks entre la dernière frappe et la relecture (`:102`, `:111`).
pub const DEBOUNCE_TICKS: u8 = 10;

/// La longueur maximale du nom affiché (`:327`, `Math.min(17, name.length())`).
pub const NAME_LIMIT: usize = 17;

/// La clé de la clé de surbrillance (`:123`, `:140`).
pub const HIGHLIGHT_KEY: &str = "inventoryHighlight";
/// Le message que la loupe envoie au joueur (`TranslationConstants.java:636`, `:125`).
pub const LOCATING_KEY: &str = "com.minecolonies.coremod.locating";
/// La durée de la boîte surlignée (`:142`).
pub const HIGHLIGHT_SECONDS: u64 = 60;

/// L'état que Java garde **hors** de la fenêtre : le tri de la session (`:63`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Session {
    /// `sortDescriptor`, `0..=4`. `Default` est `0`, comme le champ Java.
    pub sort: u8,
}

impl Session {
    /// Une session neuve : `NO_SORT`, comme le `= 0` de Java.
    pub const fn new() -> Session {
        Session { sort: 0 }
    }

    /// Le clic sur le bouton de tri (`:162-171`) : l'état suivant, qui boucle.
    ///
    /// C'est `sortDescriptor++` puis « plus de quatre, on repart à zéro » — exactement
    /// [`crate::ui::next_sort`], donc la règle est appelée et non recopiée.
    pub fn press_sort(&mut self) -> u8 {
        self.sort = next_sort(self.sort);
        self.sort
    }

    /// Le libellé que le bouton porte dans cet état (`:180-194`).
    pub fn label(&self) -> &'static str {
        match SORT_LABELS.get(usize::from(self.sort)).copied() {
            Some(label) => label,
            // Un état hors bornes ne vient que d'un `Session` fabriqué à la main ; Java,
            // lui, imprimerait le libellé précédent (`:195-196` ne fait rien).
            None => SORT_LABELS[0],
        }
    }
}

/// L'état de la fenêtre : le filtre, et le compte à rebours de la relecture.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    /// Le filtre courant, mis à jour à chaque frappe (`:99-102`).
    pub filter: String,
    /// Les ticks qui restent avant la relecture ; `0` = rien en attente (`:102`, `:111`).
    pub pending: u8,
}

impl State {
    /// L'état d'ouverture : filtre vide, rien en attente (`:57`).
    pub fn new() -> State {
        State { filter: String::new(), pending: 0 }
    }

    /// Une frappe dans le champ (`:97-104`). Vrai quand elle a changé le filtre — donc
    /// quand Java arme le compte à rebours.
    ///
    /// Retaper le même texte ne compte pas : Java compare avant d'écrire, et ne remet le
    /// compte à dix que sur un vrai changement.
    pub fn typed(&mut self, text: &str) -> bool {
        if text == self.filter.as_str() {
            return false;
        }
        self.filter = text.to_owned();
        self.pending = DEBOUNCE_TICKS;
        true
    }

    /// Un tick de client (`:111-114`). Vrai le tick où la liste doit être relue.
    pub fn tick(&mut self) -> bool {
        if self.pending == 0 {
            return false;
        }
        self.pending -= 1;
        self.pending == 0
    }

    /// Une pile telle que la fenêtre la voit : trois textes et un compte.
    ///
    /// `id` est l'id de description de l'item (`item.minecraft.stone`), `tooltip` le texte
    /// de ses infobulles joint par des espaces (`:286-294`), `name` le nom affiché.
    ///
    /// Le filtre est comparé **déjà en minuscules** par l'appelant (Java le rabaisse à
    /// chaque ligne, `:240-243`) ; [`State::rows`] le rabaisse une fois pour toutes.
    pub fn matches(stack: &Stack, filter: &str) -> bool {
        filter.is_empty()
            || stack.id.to_lowercase().contains(filter)
            || stack.tooltip.to_lowercase().contains(filter)
    }

    /// Les lignes de la liste (`:205-279`), dans l'ordre de Java.
    ///
    /// `shift` est `Screen.hasShiftDown()` (`:329`) : le port n'a pas d'état de clavier,
    /// donc l'appelant le fournit — c'est le seul endroit où la fenêtre le lit.
    pub fn rows(&self, session: &Session, stacks: &[Stack], shift: bool) -> Vec<Row> {
        let filter = self.filter.to_lowercase();
        let mut picked: Vec<&Stack> =
            stacks.iter().filter(|stack| State::matches(stack, &filter)).collect();
        // :254 — toujours, même sans filtre : la distance à `""` est la longueur du nom.
        picked.sort_by_key(|stack| levenshtein(&stack.name, &self.filter));
        match session.sort {
            1 => picked.sort_by(|left, right| left.name.cmp(&right.name)),
            2 => picked.sort_by(|left, right| right.name.cmp(&left.name)),
            3 => picked.sort_by(|left, right| left.amount.cmp(&right.amount)),
            4 => picked.sort_by(|left, right| right.amount.cmp(&left.amount)),
            // 0 : le tri de Levenshtein reste seul.
            _ => {}
        }
        picked
            .into_iter()
            .map(|stack| Row {
                name: name_text(&stack.name),
                quantities: quantity_text(stack.amount, shift),
                amount: stack.amount,
            })
            .collect()
    }
}

/// Une pile telle que l'appelant la fournit : la fenêtre ne lit que ces quatre champs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack {
    /// `getHoverName().getString()` — ce qui s'affiche et ce qui se trie.
    pub name: String,
    /// `getDescriptionId()` — l'id de traduction, stable d'une langue à l'autre (`:240`).
    pub id: String,
    /// `getTooltipLines(...)` joint par des espaces (`:286-294`).
    pub tooltip: String,
    /// Le compte cumulé de tous les racks (`:222-232`).
    pub amount: i32,
}

/// Une ligne dessinée : les deux textes et le compte brut.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// Le nom, coupé à [`NAME_LIMIT`] (`:327`).
    pub name: String,
    /// La quantité, formatée ou brute selon la touche (`:329-336`).
    pub quantities: String,
    /// Le compte, que la loupe et l'infobulle gardent.
    pub amount: i32,
}

/// Le nom d'une ligne, coupé à dix-sept caractères (`:327`).
///
/// Java coupe par `substring`, donc en **unités UTF-16** ; le port coupe par caractères.
/// La différence ne se voit que sur un nom à caractères hors du plan de base, où Java
/// peut couper une paire de substitution en deux — ce n'est pas une règle à reproduire.
pub fn name_text(name: &str) -> String {
    name.chars().take(NAME_LIMIT).collect()
}

/// La quantité d'une ligne (`:329-336`) : abrégée, ou brute quand Maj est tenue.
pub fn quantity_text(amount: i32, shift: bool) -> String {
    if shift {
        amount.to_string()
    } else {
        format_amount(i64::from(amount))
    }
}

/// `Utils.format` (`api/util/Utils.java:178-192`) : `1200` → `1.2k`.
///
/// Un chiffre après la virgule seulement sous 100 unités du suffixe, et pas de `.0` :
/// la comparaison `(truncated / 10d) != (truncated / 10)` de Java est exactement
/// « le chiffre des unités n'est pas zéro ».
pub fn format_amount(value: i64) -> String {
    /// Les suffixes de `Utils.java:26-31`, du plus petit au plus grand.
    const SUFFIXES: [(i64, &str); 6] = [
        (1_000, "k"),
        (1_000_000, "M"),
        (1_000_000_000, "G"),
        (1_000_000_000_000, "T"),
        (1_000_000_000_000_000, "P"),
        (1_000_000_000_000_000_000, "E"),
    ];
    if value < 0 {
        return format!("-{}", format_amount(value.saturating_neg()));
    }
    if value < 1_000 {
        return value.to_string();
    }
    // `suffixes.floorEntry(value)` (`:185`) : le plus grand diviseur qui tient dessous.
    let mut chosen: Option<(i64, &str)> = None;
    for entry in SUFFIXES {
        if entry.0 <= value {
            chosen = Some(entry);
        }
    }
    let (divide_by, suffix) = match chosen {
        Some(entry) => entry,
        None => return value.to_string(),
    };
    let truncated = value / (divide_by / 10);
    if truncated < 100 && truncated % 10 != 0 {
        format!("{}.{}{}", truncated / 10, truncated % 10, suffix)
    } else {
        format!("{}{}", truncated / 10, suffix)
    }
}

/// La distance d'édition de `StringUtils.getLevenshteinDistance`, deux lignes à la fois.
///
/// Apache Commons n'est pas dans l'arbre des sources, donc il n'y a pas de ligne à citer :
/// c'est l'algorithme classique, et `MANQUANT.md` §2 dit que seule la **règle** de Java
/// (« trier par distance au filtre ») est vérifiable ici, pas l'implémentation d'origine.
pub fn levenshtein(left: &str, right: &str) -> usize {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current: Vec<usize> = vec![0; right.len() + 1];
    for (row, left_char) in left.iter().enumerate() {
        current[0] = row + 1;
        for (column, right_char) in right.iter().enumerate() {
            let cost = usize::from(left_char != right_char);
            current[column + 1] =
                (previous[column + 1] + 1).min(current[column] + 1).min(previous[column] + cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

/// La couleur d'une boîte de surbrillance pour `count` items (`:136-144`).
///
/// Rouge à un exemplaire, jaune vers 32, vert à 64 et au-delà : la composante rouge
/// décroît de `2` à `0` et la verte croît de `0` à `255`, l'alpha restant à `0x80`.
/// `None` quand il n'y a rien à montrer — Java n'ajoute alors aucune boîte (`:134`).
pub fn highlight(count: i32) -> Option<u32> {
    if count <= 0 {
        return None;
    }
    let ratio = count as f32 / 32.0;
    // `(int)` tronque vers zéro, comme le transtypage Java, et `Mth.clamp` borne ensuite.
    let red = ((255.0 * (2.0 - ratio)) as i32).clamp(0, 255) as u32;
    let green = ((255.0 * ratio) as i32).clamp(0, 255) as u32;
    Some(0x8000_0000 | (red << 16) | (green << 8))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(name: &str, id: &str, amount: i32) -> Stack {
        Stack {
            name: name.to_owned(),
            id: id.to_owned(),
            tooltip: name.to_owned(),
            amount,
        }
    }

    #[test]
    fn the_filter_is_written_at_once_but_the_list_waits_ten_ticks() {
        let mut state = State::new();
        assert!(state.typed("stone"));
        assert_eq!(state.filter, "stone");
        assert_eq!(state.pending, DEBOUNCE_TICKS);
        // Retaper la même chose ne réarme rien (`:99`).
        assert!(!state.typed("stone"));
        assert_eq!(state.pending, DEBOUNCE_TICKS);
        // Une vraie frappe repart de dix.
        for _ in 0..4 {
            assert!(!state.tick());
        }
        assert_eq!(state.pending, DEBOUNCE_TICKS - 4);
        assert!(state.typed("stones"));
        assert_eq!(state.pending, DEBOUNCE_TICKS);
        // Neuf ticks plus tard, rien ; le dixième rend la main (`:111-114`).
        for _ in 0..(DEBOUNCE_TICKS - 1) {
            assert!(!state.tick());
        }
        assert!(state.tick());
        assert!(!state.tick(), "un compte à zéro ne sonne plus");
    }

    #[test]
    fn the_sort_walks_five_labels_and_belongs_to_the_session() {
        let mut session = Session::new();
        assert_eq!(session.sort, 0);
        assert_eq!(session.label(), "v^");
        let mut seen = Vec::new();
        for _ in 0..SORT_LABELS.len() {
            seen.push(session.press_sort());
        }
        assert_eq!(seen, [1, 2, 3, 4, 0], "WindowHutAllInventory.java:163-168");
        assert_eq!(SORT_LABELS.len(), usize::from(crate::ui::SORT_STAGES));
        // Le `static` de Java (`:63`) est partagé : deux fenêtres, un seul tri.
        let mut first = State::new();
        first.typed("a");
        let second = State::new();
        assert_ne!(first, second, "les deux états de fenêtre sont distincts");
        assert_eq!(second.filter, "", "et le filtre, lui, n'est pas statique");
        assert_eq!(session.sort, 0, "le tri, lui, a survécu aux deux états");
    }

    #[test]
    fn the_filter_tests_the_translation_key_and_the_tooltip_not_the_name() {
        let mut state = State::new();
        // Un nom traduit, un id stable : « stone » passe, et c'est le but (`:239-243`).
        let stone = stack("Bloc de pierre", "item.minecraft.stone", 4);
        assert!(State::matches(&stone, "stone"));
        assert!(State::matches(&stone, "pierre"), "l'infobulle porte le nom");
        assert!(!State::matches(&stone, "granit"));
        // La casse : le filtre est mis en minuscules avant comparaison.
        assert!(State::matches(&stone, "bloc"));
        state.typed("STONE");
        assert!(State::matches(&stone, &state.filter.to_lowercase()));
        // Un nom absent de l'infobulle ne trouve rien : Java ne teste jamais le nom seul.
        let lying = Stack {
            name: "pierre".to_owned(),
            id: "item.minecraft.dirt".to_owned(),
            tooltip: "Terre".to_owned(),
            amount: 1,
        };
        assert!(!State::matches(&lying, "pierre"));
    }

    #[test]
    fn an_empty_filter_still_sorts_by_distance_so_by_name_length() {
        // :254 — la distance à `""` vaut la longueur du nom, et rien ne l'annule.
        let state = State::new();
        let session = Session::new();
        let stacks = vec![
            stack("Redstone Block", "a", 1),
            stack("Stone", "b", 1),
            stack("Cobblestone Slab", "c", 1),
        ];
        let rows = state.rows(&session, &stacks, false);
        let order: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(order[0], "Stone", "le plus court d'abord, même en mode « pas de tri »");
        assert_eq!(order[1], "Redstone Block");
        assert_eq!(order[2], "Cobblestone Slab");
        assert_eq!(levenshtein("Stone", ""), 5);
    }

    #[test]
    fn the_sort_is_stable_so_equal_keys_keep_the_distance_order() {
        let state = State::new();
        let mut session = Session::new();
        // Trois noms de longueurs 1, 1 et 3 : la distance à `""` les classe b, a, ccc —
        // et `b` avant `a` parce que la liste d'entrée le met avant (tri stable).
        let stacks =
            vec![stack("b", "id.b", 5), stack("a", "id.a", 5), stack("ccc", "id.c", 5)];
        session.sort = 1;
        let rows = state.rows(&session, &stacks, false);
        let by_name: Vec<String> = rows.iter().map(|row| row.name.clone()).collect();
        assert_eq!(by_name, ["a", "b", "ccc"]);
        // Mode 4 : tous les comptes sont égaux, donc le tri par compte ne départage rien
        // et laisse l'ordre de distance — pas l'ordre alphabétique du mode précédent.
        session.sort = 4;
        let rows = state.rows(&session, &stacks, false);
        let by_count: Vec<String> = rows.iter().map(|row| row.name.clone()).collect();
        assert_eq!(by_count, ["b", "a", "ccc"]);
    }

    #[test]
    fn a_row_cuts_its_name_at_seventeen_and_writes_the_count_two_ways() {
        assert_eq!(NAME_LIMIT, 17);
        assert_eq!(name_text("Cobblestone Slab"), "Cobblestone Slab");
        assert_eq!(name_text("Cobblestone Stairs"), "Cobblestone Stair");
        // `quantities` : abrégé, ou brut avec Maj (`:329-336`).
        assert_eq!(quantity_text(1_200, false), "1.2k");
        assert_eq!(quantity_text(1_200, true), "1200");
        assert_eq!(quantity_text(999, false), "999");
    }

    #[test]
    fn utils_format_keeps_one_decimal_only_under_a_hundred_units() {
        // :178-192 — les valeurs de la doc de Java, plus les bords.
        assert_eq!(format_amount(0), "0");
        assert_eq!(format_amount(999), "999");
        assert_eq!(format_amount(1_000), "1k");
        assert_eq!(format_amount(1_050), "1k", "1.0 ne s'écrit pas");
        assert_eq!(format_amount(1_200), "1.2k");
        assert_eq!(format_amount(13_000), "13k");
        assert_eq!(format_amount(1_500_000), "1.5M");
        assert_eq!(format_amount(-1_200), "-1.2k");
    }

    #[test]
    fn the_loupe_is_red_at_one_item_and_green_at_sixty_four() {
        // :136-144 — alpha 0x80, rouge qui décroît, vert qui croît.
        assert_eq!(highlight(0), None);
        assert_eq!(highlight(-3), None);
        assert_eq!(highlight(1), Some(0x80FF_0700));
        assert_eq!(highlight(32), Some(0x80FF_FF00));
        assert_eq!(highlight(48), Some(0x807F_FF00));
        assert_eq!(highlight(64), Some(0x8000_FF00));
        // Au-delà, le vert est borné, pas le rouge négatif.
        assert_eq!(highlight(128), Some(0x8000_FF00));
    }

    #[test]
    fn the_whole_list_is_filtered_then_sorted_then_formatted() {
        let mut state = State::new();
        let session = Session::new();
        state.typed("stone");
        let stacks = vec![
            stack("Stone", "item.minecraft.stone", 1_200),
            stack("Redstone", "item.minecraft.redstone", 40),
            stack("Dirt", "item.minecraft.dirt", 9),
        ];
        let rows = state.rows(&session, &stacks, false);
        // Le filtre garde les deux qui contiennent « stone » dans leur id ou leur nom.
        assert_eq!(rows.len(), 2);
        // `Stone` (5) est plus proche de `stone` (0) que `Redstone` (3).
        assert_eq!(rows[0].name, "Stone");
        assert_eq!(rows[0].quantities, "1.2k");
        assert_eq!(rows[1].name, "Redstone");
        assert_eq!(rows[1].amount, 40);
    }
}
