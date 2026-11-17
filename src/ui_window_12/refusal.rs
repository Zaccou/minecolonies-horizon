//! Pourquoi un clic de ces quatre écrans ne fait rien — nommé, et dit au joueur ou pas.
//!
//! Java refuse de trois façons, et les confondre est l'erreur à ne pas faire :
//!
//! 1. il **désactive** un bouton ou ne le dessine pas (`Button.setEnabled(false)`,
//!    `WindowHutMinerModule.java:99-100`, `WindowMineGuardModule.java:165-182`) ;
//! 2. il **sort en silence** d'un gestionnaire dont la condition n'est pas remplie
//!    (`WindowHutMinerModule.java:73-77`, `WindowMineGuardModule.java:58-74`) ;
//! 3. il **parle**, une seule fois dans ces quatre fenêtres, avec une infobulle
//!    (`WindowListRecipes.java:191-195` — et c'est une infobulle de bouton grisé, pas
//!    un message).
//!
//! [`Kind::key`] répond donc `Some` **une** fois sur treize : imprimer notre nom à la place
//! d'une phrase, ou une clé là où le joueur n'entend rien, serait mentir.
//!
//! # Deux types pour une seule liste de refus
//!
//! [`Kind`] est le refus **sans ses données** : c'est ce qu'une table de boutons peut
//! porter (`Button.refusals`, une constante `static`), et c'est là que vivent le nom, la
//! clé et le pont vers `src/ui`. [`Refusal`] est le même refus **avec** la ligne, la
//! profondeur ou la position qui l'ont causé : c'est ce qu'une fonction rend, pour qu'un
//! journal dise *quelle* ligne a été refusée. Les deux ne peuvent pas diverger :
//! [`Refusal::kind`] est la seule conversion, et [`Kind::name`] la seule table de noms.
//!
//! [`Kind::ui`] fait le pont avec `src/ui` : deux de ces refus y sont **déjà** déclarés
//! (`ui::action::Refusal::WorkOrderExists`, `:CurrentLevel`) parce que `src/ui` porte
//! l'onglet de la mine. Les autres n'y ont pas de variante, et c'est pourquoi ce module a
//! la sienne — un `Action` de `src/ui` ne peut pas les nommer.

use crate::ui::action::Refusal as UiRefusal;

/// « Recette intégrée : désactive-la plutôt que de la retirer, maintiens %s si quelque
/// chose a cassé » (`com.minecolonies.coremod.gui.workerhuts.removebuiltin`,
/// `manual_en_us.json:1014`, dessinée en infobulle par
/// `WindowListRecipes.java:190-195`).
///
/// C'est la **seule** phrase de ces quatre fenêtres, et c'est l'infobulle d'un bouton
/// grisé : elle ne dit pas « refusé », elle dit « tiens Ctrl ».
pub const BUILTIN_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.removebuiltin";

/// Un refus, sans ses données : ce qu'une table de boutons peut porter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les refus de ces quatre fenêtres, un par branche de Java
pub enum Kind {
    /// La ligne demandée n'existe pas dans `levelsInfo`.
    ///
    /// Java reçoit `-1` de `getListElementIndexByPane` quand le panneau n'est pas dans la
    /// liste et le teste (`WindowHutMinerModule.java:73`) ; un index `usize` ne peut pas
    /// valoir `-1`, donc les deux cas — négatif et trop grand — sont celui-ci.
    LevelOutOfRange,
    /// Un ordre de travail de puits existe déjà pour ce niveau : `repair` est grisé
    /// (`WindowHutMinerModule.java:99`, testé à
    /// `MinerLevelManagementModuleView.java:104-115`).
    WorkOrderExists,
    /// Ce niveau est celui où le mineur travaille : `mine` est grisé
    /// (`WindowHutMinerModule.java:100`) et le gestionnaire le refuse aussi (`:73`).
    CurrentLevel,
    /// Le module n'enseigne aucun type de recette : le bouton `crafting` et le compteur
    /// sont **masqués** (`WindowListRecipes.java:85-86`), et un clic qui passerait quand
    /// même est ignoré (`:150-154`).
    RecipeAlterationForbidden,
    /// La recette vient d'un fichier de données (`getRecipeSource() != null`) et Ctrl n'est
    /// pas tenu : `remove` est grisé avec [`BUILTIN_KEY`] en infobulle
    /// (`WindowListRecipes.java:188-196`).
    BuiltinRecipeNeedsControl,
    /// Le module a atteint son plafond de recettes actives et la recette à réactiver n'est
    /// pas intégrée : le bouton `toggle` n'est **pas dessiné**
    /// (`WindowListRecipes.java:225`). La clé `...gui.recipe.full` existe
    /// (`manual_en_us.json:1018`) et n'est **pas** utilisée ici.
    RecipeLimitReached,
    /// La ligne demandée n'existe pas dans la liste des recettes. Java lève
    /// `IndexOutOfBoundsException` — `moduleView.getRecipes().get(row)`
    /// (`WindowListRecipes.java:140`) et `toggle(row)` (`CraftingModuleView.java:336`) ne
    /// vérifient rien ; le port rend un refus nommé au lieu de paniquer.
    NoSuchRow,
    /// Le déplacement ne change rien : première ligne vers le haut, dernière vers le bas,
    /// ou `switchOrder` dont l'un des deux index sort de la liste
    /// (`CraftingModuleView.java:220-240`).
    ///
    /// **Le message part quand même** : `forwardClicked` et `backwardClicked` envoient
    /// `ChangeRecipePriorityMessage` après l'appel, sans regarder si l'ordre a bougé
    /// (`WindowListRecipes.java:116`, `:129`). Ce refus est donc **local**, et le serveur
    /// reste juge.
    EdgeOfList,
    /// La mine a déjà tous les gardes qu'elle accepte à ce niveau
    /// (`WindowMineGuardModule.java:165-172`, plafond à `:122-137`) : le bouton
    /// d'affectation est grisé.
    MineAtCapacity,
    /// Ce garde garde déjà **une autre** mine : le bouton est grisé
    /// (`WindowMineGuardModule.java:178-182`) et le gestionnaire n'a pas de branche pour ce
    /// cas (`:58-74`).
    MinePostTakenElsewhere,
    /// La vue de la hutte n'est pas dans le gestionnaire client : `pullGuardsFromHut` sort
    /// **avant de vider la liste** (`WindowMineGuardModule.java:84-87`), donc la fenêtre
    /// garde les gardes de l'ouverture précédente.
    HutViewMissing,
    /// Le citoyen d'une ligne n'est pas chargé côté client. Java ajoute quand même le
    /// `null` à sa liste (`WindowMineGuardModule.java:98-101`) — `getCitizen(id)` répond
    /// `null` pour un citoyen hors de portée — puis grise le bouton de la ligne
    /// (`:185-189`) **sans écrire le nom** : le panneau garde le nom de la ligne
    /// précédente, réutilisée par `ScrollingList`.
    GuardUnknown,
    /// Le jeton d'une tâche ne désigne plus aucune requête : Java le retire de la liste
    /// **pendant `getElementCount`** (`WindowHutRequestTaskModule.java:56`), donc entre le
    /// compte et la ligne le contenu peut changer.
    TaskMissing,
}

impl Kind {
    /// Les treize, dans l'ordre de la déclaration.
    pub const ALL: [Kind; 13] = [
        Kind::LevelOutOfRange,
        Kind::WorkOrderExists,
        Kind::CurrentLevel,
        Kind::RecipeAlterationForbidden,
        Kind::BuiltinRecipeNeedsControl,
        Kind::RecipeLimitReached,
        Kind::NoSuchRow,
        Kind::EdgeOfList,
        Kind::MineAtCapacity,
        Kind::MinePostTakenElsewhere,
        Kind::HutViewMissing,
        Kind::GuardUnknown,
        Kind::TaskMissing,
    ];

    /// Un nom stable, préfixé, qui ne peut pas être pris pour une phrase.
    pub fn name(self) -> &'static str {
        match self {
            Kind::LevelOutOfRange => "ui12.refusal.level_out_of_range",
            Kind::WorkOrderExists => "ui12.refusal.work_order",
            Kind::CurrentLevel => "ui12.refusal.current_level",
            Kind::RecipeAlterationForbidden => "ui12.refusal.alteration_forbidden",
            Kind::BuiltinRecipeNeedsControl => "ui12.refusal.builtin_needs_control",
            Kind::RecipeLimitReached => "ui12.refusal.recipe_limit",
            Kind::NoSuchRow => "ui12.refusal.no_such_row",
            Kind::EdgeOfList => "ui12.refusal.edge_of_list",
            Kind::MineAtCapacity => "ui12.refusal.mine_full",
            Kind::MinePostTakenElsewhere => "ui12.refusal.mine_taken",
            Kind::HutViewMissing => "ui12.refusal.hut_view_missing",
            Kind::GuardUnknown => "ui12.refusal.guard_unknown",
            Kind::TaskMissing => "ui12.refusal.task_missing",
        }
    }

    /// La clé de traduction que le joueur lit, quand Java lui dit quelque chose.
    ///
    /// `None` n'est pas « pas de raison » : c'est « Java grise le bouton et se tait », ce
    /// qui est le cas de douze refus sur treize.
    pub fn key(self) -> Option<&'static str> {
        match self {
            Kind::BuiltinRecipeNeedsControl => Some(BUILTIN_KEY),
            Kind::LevelOutOfRange
            | Kind::WorkOrderExists
            | Kind::CurrentLevel
            | Kind::RecipeAlterationForbidden
            | Kind::RecipeLimitReached
            | Kind::NoSuchRow
            | Kind::EdgeOfList
            | Kind::MineAtCapacity
            | Kind::MinePostTakenElsewhere
            | Kind::HutViewMissing
            | Kind::GuardUnknown
            | Kind::TaskMissing => None,
        }
    }

    /// Vrai quand Java ne dit rien du tout : le bouton est grisé, ou le gestionnaire sort
    /// en silence.
    pub fn is_silent(self) -> bool {
        self.key().is_none()
    }

    /// Le refus de `src/ui` qui dit déjà la même chose, quand il existe.
    ///
    /// Deux seulement : `src/ui` porte l'onglet de la mine et y a nommé les deux refus de
    /// ligne (`ui::action::Refusal::WorkOrderExists`, `:CurrentLevel`). Un test vérifie que
    /// les deux tables ne se contredisent pas.
    pub fn ui(self) -> Option<UiRefusal> {
        match self {
            Kind::WorkOrderExists => Some(UiRefusal::WorkOrderExists),
            Kind::CurrentLevel => Some(UiRefusal::CurrentLevel),
            Kind::LevelOutOfRange
            | Kind::RecipeAlterationForbidden
            | Kind::BuiltinRecipeNeedsControl
            | Kind::RecipeLimitReached
            | Kind::NoSuchRow
            | Kind::EdgeOfList
            | Kind::MineAtCapacity
            | Kind::MinePostTakenElsewhere
            | Kind::HutViewMissing
            | Kind::GuardUnknown
            | Kind::TaskMissing => None,
        }
    }
}

/// Un refus, avec ce qui l'a causé : la ligne, la profondeur, la position.
///
/// Aucun variant ne porte de `String` : un refus se compare, se copie et se journalise, et
/// le texte qui l'explique est une clé de traduction ou notre nom, jamais l'un à la place
/// de l'autre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les treize refus de [`Kind`], avec leurs données
pub enum Refusal {
    LevelOutOfRange { row: usize, levels: usize },
    /// La profondeur est celle du couple stocké, **brute** : elle peut être négative, comme
    /// `getB()`. C'est la valeur que la recherche d'ordre de travail compare.
    WorkOrderExists { level: i32 },
    CurrentLevel { row: usize },
    RecipeAlterationForbidden,
    BuiltinRecipeNeedsControl,
    RecipeLimitReached { active: usize, max: usize },
    NoSuchRow { row: usize, rows: usize },
    EdgeOfList { row: usize, len: usize },
    MineAtCapacity { level: i32, max: i32 },
    MinePostTakenElsewhere { at: [i32; 3] },
    HutViewMissing,
    GuardUnknown { id: i32 },
    TaskMissing { id: i32 },
}

impl Refusal {
    /// Le refus sans ses données : la seule conversion entre les deux types.
    pub fn kind(self) -> Kind {
        match self {
            Refusal::LevelOutOfRange { .. } => Kind::LevelOutOfRange,
            Refusal::WorkOrderExists { .. } => Kind::WorkOrderExists,
            Refusal::CurrentLevel { .. } => Kind::CurrentLevel,
            Refusal::RecipeAlterationForbidden => Kind::RecipeAlterationForbidden,
            Refusal::BuiltinRecipeNeedsControl => Kind::BuiltinRecipeNeedsControl,
            Refusal::RecipeLimitReached { .. } => Kind::RecipeLimitReached,
            Refusal::NoSuchRow { .. } => Kind::NoSuchRow,
            Refusal::EdgeOfList { .. } => Kind::EdgeOfList,
            Refusal::MineAtCapacity { .. } => Kind::MineAtCapacity,
            Refusal::MinePostTakenElsewhere { .. } => Kind::MinePostTakenElsewhere,
            Refusal::HutViewMissing => Kind::HutViewMissing,
            Refusal::GuardUnknown { .. } => Kind::GuardUnknown,
            Refusal::TaskMissing { .. } => Kind::TaskMissing,
        }
    }

    pub fn name(self) -> &'static str {
        self.kind().name()
    }

    pub fn key(self) -> Option<&'static str> {
        self.kind().key()
    }

    pub fn is_silent(self) -> bool {
        self.kind().is_silent()
    }

    pub fn ui(self) -> Option<UiRefusal> {
        self.kind().ui()
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les treize instances portées, dans l'ordre de [`Kind::ALL`] : une conversion oubliée
    /// ne compile pas ici.
    const EVERY: [Refusal; 13] = [
        Refusal::LevelOutOfRange { row: 3, levels: 2 },
        Refusal::WorkOrderExists { level: 12 },
        Refusal::CurrentLevel { row: 0 },
        Refusal::RecipeAlterationForbidden,
        Refusal::BuiltinRecipeNeedsControl,
        Refusal::RecipeLimitReached { active: 5, max: 5 },
        Refusal::NoSuchRow { row: 9, rows: 3 },
        Refusal::EdgeOfList { row: 0, len: 4 },
        Refusal::MineAtCapacity { level: 5, max: 3 },
        Refusal::MinePostTakenElsewhere { at: [1, 2, 3] },
        Refusal::HutViewMissing,
        Refusal::GuardUnknown { id: 7 },
        Refusal::TaskMissing { id: 11 },
    ];

    #[test]
    fn every_refusal_has_its_own_namespaced_name() {
        assert_eq!(Kind::ALL.len(), 13);
        for (index, kind) in Kind::ALL.iter().enumerate() {
            assert!(kind.name().starts_with("ui12.refusal."), "{kind:?}");
            for other in &Kind::ALL[index + 1..] {
                assert_ne!(kind.name(), other.name(), "two refusals share a name");
            }
        }
    }

    #[test]
    fn the_two_types_agree_on_every_variant() {
        for (refusal, kind) in EVERY.iter().zip(Kind::ALL) {
            assert_eq!(refusal.kind(), kind, "{refusal}");
            assert_eq!(refusal.name(), kind.name());
            assert_eq!(refusal.key(), kind.key());
            assert_eq!(refusal.is_silent(), kind.is_silent());
            assert_eq!(refusal.ui(), kind.ui());
        }
        // The data a refusal carries does not change any of that.
        assert_eq!(
            Refusal::EdgeOfList { row: 0, len: 1 }.kind(),
            Refusal::EdgeOfList { row: 40, len: 2 }.kind()
        );
    }

    #[test]
    fn exactly_one_refusal_is_told_to_the_player_and_it_is_a_tooltip() {
        let keyed: Vec<&str> = Kind::ALL
            .iter()
            .filter(|kind| kind.key().is_some())
            .map(|kind| kind.name())
            .collect();
        assert_eq!(keyed, ["ui12.refusal.builtin_needs_control"]);
        assert_eq!(
            Kind::BuiltinRecipeNeedsControl.key(),
            Some("com.minecolonies.coremod.gui.workerhuts.removebuiltin")
        );
        assert_eq!(Kind::ALL.iter().filter(|kind| kind.is_silent()).count(), 12);
        // The recipe-limit key exists and this window never draws it.
        assert_ne!(BUILTIN_KEY, super::super::recipes::FULL_KEY);
    }

    #[test]
    fn only_the_two_row_refusals_of_the_mine_tab_exist_in_src_ui() {
        let mapped: Vec<&str> = Kind::ALL
            .iter()
            .filter(|kind| kind.ui().is_some())
            .map(|kind| kind.name())
            .collect();
        assert_eq!(
            mapped,
            ["ui12.refusal.work_order", "ui12.refusal.current_level"],
            "src/ui names these two on the corners of the miner tab"
        );
        assert_eq!(Kind::CurrentLevel.ui(), Some(UiRefusal::CurrentLevel));
        assert_eq!(Kind::EdgeOfList.ui(), None);
    }
}
