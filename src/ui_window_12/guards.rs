//! La liste des gardes qu'une mine peut employer : un nom par ligne, un bouton par ligne,
//! et un plafond qui dépend du niveau de la hutte.
//!
//! Java reference : `core/client/gui/modules/building/WindowMineGuardModule.java` (la
//! fenêtre, `:42-199`) et
//! `core/colony/buildings/moduleviews/MinerGuardAssignModuleView.java` (la vue, qui ne
//! désérialise **rien**, `:19-42`), sur `gui/layouthuts/layoutguardlist.xml` (`:6-15`).
//!
//! **Ce que cette fenêtre demande** : toutes les tours de garde de la colonie, leur
//! réglage `guard_task`, la position de mine qu'elles surveillent, et les citoyens qu'elles
//! emploient. **Ce qu'elle rend** : un nom et un bouton par garde, un avertissement quand
//! la liste est vide, et un message par clic.
//!
//! # Le module n'a pas de serveur, et ça ne se voit pas
//!
//! `MINER_GUARD_ASSIGN` est déclaré avec `null` côté serveur : `new
//! BuildingEntry.ModuleProducer<>("miner_guard_assign", null, () ->
//! MinerGuardAssignModuleView::new)` (`BuildingModules.java:505-506`). La vue ne
//! désérialise rien non plus (`MinerGuardAssignModuleView.java:19-23`). Tout ce que la
//! fenêtre affiche vient donc **d'ailleurs** : du gestionnaire de bâtiments client, qui
//! énumère les tours de garde. Une clé `miner_guard_assign` dans `src/ui::MODULE_SCREENS`
//! serait donc bien une clé de module — mais d'un module qui ne porte aucune donnée.
//!
//! # Ce que la fenêtre liste est plus large que la mine
//!
//! `pullGuardsFromHut` ne regarde pas les gardes **de cette mine** : il prend toutes les
//! tours dont le réglage `GUARD_TASK` vaut `PATROL_MINE` (`:88-90`), et n'utilise la
//! position de la mine que pour **compter** celles qui la surveillent (`:94-97`). Les
//! gardes d'une autre mine apparaissent donc dans cette liste, avec leur bouton grisé
//! (`:178-182`) — et c'est ce qui rend [`Refusal::MinePostTakenElsewhere`] nécessaire.
//!
//! # Le clic écrit trois choses dont deux sont écrasées
//!
//! `assignGuardClicked` (`:51-77`) envoie le message, écrit le libellé du bouton
//! (`:63`, `:71`) et la position de mine de la tour (`:64`, `:72`), puis appelle
//! `pullGuardsFromHut()` — qui **recalcule tout** depuis le gestionnaire client et
//! `onUpdate` le rappelle à chaque image (`:194-199`). Le libellé et le compteur locaux
//! sont donc écrasés à l'image suivante : seul le message compte, et c'est le serveur qui
//! renvoie la vérité. [`Guards::click`] modifie la tour puis rafraîchit, ce qui donne le
//! même résultat sans écrire deux fois.

use crate::colony::Citizen;
use crate::ui::action::Effect;
use crate::ui::field::{Field, Source, Value};

use super::refusal::{Kind, Refusal};

/// La clé de module de la vue (`BuildingModules.java:505-506`) — la même que
/// `crate::building::Module::key` sauvegarde. Un test vérifie que `src/ui` ne la connaît
/// pas.
pub const MODULE: &str = "miner_guard_assign";

/// La liste (`WindowMineGuardModule.java:29`).
pub const LIST: &str = "guards";
/// Le nom du garde, dans la ligne (`:159`).
pub const GUARD_NAME: &str = "guardName";
/// Le bouton de la ligne (`:30`) — un par garde, pas un par mine.
pub const ASSIGN: &str = "assignGuard";
/// L'avertissement « aucun garde » (`:106`, `:110`, `layoutguardlist.xml:6-7`).
pub const WARNING: &str = "noguardwarning";

/// Le titre (`layoutguardlist.xml:4-5`).
pub const TITLE_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.minerguards";
/// Le texte de l'avertissement (`manual_en_us.json:2235`).
pub const WARNING_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.miner.noguards";
/// Le libellé du bouton quand la mine n'est pas surveillée par ce garde (`:164`, `:180`).
pub const ASSIGN_KEY: &str = "com.minecolonies.coremod.gui.hiring.buttonassign";
/// Le libellé du bouton quand elle l'est (`:176`).
pub const UNASSIGN_KEY: &str = "com.minecolonies.coremod.gui.hiring.buttonunassign";

/// Le réglage qui fait qu'une tour de garde est dans cette liste
/// (`GuardTaskSetting.java:35`, comparé à `WindowMineGuardModule.java:89-90`).
pub const PATROL_MINE: &str = "com.minecolonies.core.guard.setting.patrol_mine";

/// Les quatre valeurs du réglage `guard_task` (`GuardTaskSetting.java:29-35`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: quatre réglages déclarés dans Java, quatre ici
pub enum GuardTask {
    /// `patrol` : la ronde par défaut.
    Patrol,
    /// `guard` : le garde reste à la tour.
    Guard,
    /// `follow` : il suit le joueur.
    Follow,
    /// `patrol_mine` : il garde une mine, et c'est la seule valeur que cette fenêtre liste.
    PatrolMine,
}

impl GuardTask {
    /// La valeur telle que le réglage la sauve (`GuardTaskSetting.java:29-35`).
    pub fn key(self) -> &'static str {
        match self {
            GuardTask::Patrol => "com.minecolonies.core.guard.setting.patrol",
            GuardTask::Guard => "com.minecolonies.core.guard.setting.guard",
            GuardTask::Follow => "com.minecolonies.core.guard.setting.follow",
            GuardTask::PatrolMine => PATROL_MINE,
        }
    }

    /// La valeur qu'une chaîne sauvegardée désigne, `None` pour une valeur inconnue.
    pub fn parse(key: &str) -> Option<GuardTask> {
        match key {
            _ if key == GuardTask::Patrol.key() => Some(GuardTask::Patrol),
            _ if key == GuardTask::Guard.key() => Some(GuardTask::Guard),
            _ if key == GuardTask::Follow.key() => Some(GuardTask::Follow),
            _ if key == GuardTask::PatrolMine.key() => Some(GuardTask::PatrolMine),
            _ => None,
        }
    }
}

/// Un emplacement de garde : le citoyen, ou son identifiant quand la vue client ne le
/// connaît pas.
///
/// Java ajoute `colony.getCitizen(citizenId)` **sans test** (`:98-101`), et cette méthode
/// rend `null` pour un citoyen hors de portée. La liste contient donc des trous, et le
/// rendu les traite à moitié : bouton grisé (`:185-189`), nom **non écrit**.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: un citoyen chargé, ou son identifiant seul
pub enum Slot {
    /// Le citoyen, tel que `crate::colony::Citizen` le porte.
    Guard(Citizen),
    /// L'identifiant d'un citoyen que la vue n'a pas : Java garde le `null`.
    Missing(i32),
}

impl Slot {
    /// L'identifiant, chargé ou non.
    pub fn id(&self) -> i32 {
        match self {
            Slot::Guard(citizen) => citizen.id,
            Slot::Missing(id) => *id,
        }
    }

    /// Le nom, quand il y en a un.
    pub fn name(&self) -> Option<&str> {
        match self {
            Slot::Guard(citizen) => Some(citizen.name.as_str()),
            Slot::Missing(_) => None,
        }
    }
}

/// Une tour de garde, vue par cette fenêtre : sa position, son réglage, la mine qu'elle
/// surveille et ses gardes.
///
/// `mine` est la position de la mine surveillée (`AbstractBuildingGuards.View.getMinePos`,
/// lu à `:58`, `:68`, `:94`), et elle appartient à la **tour**, pas au garde : trois gardes
/// d'une même tour partagent la même mine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tower {
    pub at: [i32; 3],
    pub task: GuardTask,
    pub mine: Option<[i32; 3]>,
    pub guards: Vec<Slot>,
}

impl Tower {
    pub fn new(at: [i32; 3], task: GuardTask, mine: Option<[i32; 3]>, guards: Vec<Slot>) -> Tower {
        Tower { at, task, mine, guards }
    }

    /// Cette tour est-elle dans la liste de la fenêtre ? Seul `PATROL_MINE` y entre
    /// (`:88-90`).
    pub fn is_listed(&self) -> bool {
        self.task == GuardTask::PatrolMine
    }

    /// Surveille-t-elle **cette** mine (`:94`)?
    pub fn guards_mine(&self, mine: [i32; 3]) -> bool {
        self.mine == Some(mine)
    }
}

/// Combien de gardes une mine accepte, selon le niveau de sa hutte
/// (`WindowMineGuardModule.java:122-137`).
///
/// Un `match` sur cinq valeurs et un `default` : niveau 1 et 2 → un garde, 3 et 4 → deux,
/// 5 → trois, **tout le reste, zéro compris, → aucun**. Une hutte non construite n'a donc
/// pas de garde, et le plafond n'est pas une formule.
pub fn max_guards(level: i32) -> i32 {
    match level {
        1 | 2 => 1,
        3 | 4 => 2,
        5 => 3,
        _ => 0,
    }
}

/// L'état d'un bouton de ligne, tel que `updateElement` le décide (`:161-189`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les cinq branches de Java, plus le trou
pub enum ButtonState {
    /// La tour ne surveille aucune mine et il reste de la place : « assigner »
    /// (`:162-172`).
    CanAssign,
    /// La tour surveille **cette** mine : « désassigner » (`:174-177`).
    CanUnassign,
    /// Elle ne surveille rien mais la mine est pleine : grisé (`:165-168`).
    FullOfGuards,
    /// Elle surveille une **autre** mine : grisé (`:178-182`).
    AnotherMine,
    /// Le citoyen n'est pas chargé : grisé, libellé **non écrit** (`:185-189`).
    UnknownGuard,
}

impl ButtonState {
    /// Le libellé écrit dans le bouton, `None` quand Java n'écrit rien.
    pub fn label(self) -> Option<&'static str> {
        match self {
            ButtonState::CanAssign | ButtonState::FullOfGuards | ButtonState::AnotherMine => {
                Some(ASSIGN_KEY)
            }
            ButtonState::CanUnassign => Some(UNASSIGN_KEY),
            ButtonState::UnknownGuard => None,
        }
    }

    /// Le bouton est-il cliquable ?
    pub fn is_enabled(self) -> bool {
        matches!(self, ButtonState::CanAssign | ButtonState::CanUnassign)
    }

    /// Pourquoi il ne l'est pas, quand il ne l'est pas.
    pub fn refusal(self) -> Option<Kind> {
        match self {
            ButtonState::CanAssign | ButtonState::CanUnassign => None,
            ButtonState::FullOfGuards => Some(Kind::MineAtCapacity),
            ButtonState::AnotherMine => Some(Kind::MinePostTakenElsewhere),
            ButtonState::UnknownGuard => Some(Kind::GuardUnknown),
        }
    }
}

/// Une ligne rendue : un garde et son bouton.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// L'index de la tour dans [`Guards::towers`] — Java retrouve la tour par le
    /// **bâtiment de travail du citoyen** (`:57`, `:156`), ce qui est la même chose tant
    /// qu'un citoyen n'a qu'un employeur.
    pub tower: usize,
    /// L'identifiant du garde.
    pub id: i32,
    /// Le nom, `None` pour un citoyen non chargé — et le panneau garde alors le nom de la
    /// ligne précédente (`:185-189`).
    pub name: Option<String>,
    /// L'état du bouton.
    pub button: ButtonState,
}

/// La fenêtre : la mine, le niveau de sa hutte, les tours de garde, et la liste que Java
/// garde en cache.
///
/// `pullGuardsFromHut` remplit `assignedGuards` **de zéro** à chaque appel (`:86-87`) :
/// le compteur n'est pas un état qui s'incrémente, c'est un recomptage. [`Guards::refresh`]
/// fait de même, et c'est ce qui rend le `assignedGuards--` du clic (`:73`) sans effet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Guards {
    /// La position de la mine dont on regarde les gardes (`buildingView.getPosition()`).
    hut: [i32; 3],
    /// Le niveau de la hutte : c'est lui qui fixe le plafond (`getBuildingLevel`,
    /// `:124`).
    level: i32,
    /// La hutte de la mine est-elle dans le gestionnaire client ? Sinon Java ne vide même
    /// pas la liste (`:84-87`).
    present: bool,
    /// Toutes les tours de garde connues, réglage compris.
    towers: Vec<Tower>,
    /// La liste telle que la fenêtre la dessine — Java la garde dans `guardsInfo`.
    rows: Vec<Row>,
    /// Combien de tours surveillent cette mine (`:94-97`).
    assigned: i32,
}

impl Guards {
    /// Une fenêtre qui n'a pas encore lu le gestionnaire : la liste est vide et
    /// `pullGuardsFromHut` l'a bien remplie de zéro.
    pub fn new(hut: [i32; 3], level: i32, present: bool, towers: Vec<Tower>) -> Guards {
        let mut window =
            Guards { hut, level, present, towers, rows: Vec::new(), assigned: 0 };
        let _ = window.refresh();
        window
    }

    pub fn hut(&self) -> [i32; 3] {
        self.hut
    }

    pub fn level(&self) -> i32 {
        self.level
    }

    pub fn towers(&self) -> &[Tower] {
        &self.towers
    }

    pub fn set_level(&mut self, level: i32) {
        self.level = level;
    }

    /// Vrai quand la hutte de la mine est connue du client.
    pub fn is_present(&self) -> bool {
        self.present
    }

    /// Le plafond de gardes de cette mine (`:122-137`).
    pub fn max(&self) -> i32 {
        max_guards(self.level)
    }

    /// Combien de tours surveillent cette mine.
    pub fn assigned(&self) -> i32 {
        self.assigned
    }

    /// La liste dessinée.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// L'avertissement « mets d'abord un garde en mode *Guard Mine* » est-il visible ?
    /// (`:104-111`) — il l'est quand la liste est **vide**, pas quand la mine est pleine.
    pub fn warning(&self) -> bool {
        self.rows.is_empty()
    }

    /// `pullGuardsFromHut()` (`:82-113`) : recompte les tours et reconstruit la liste.
    ///
    /// `Err(Refusal::HutViewMissing)` quand la hutte n'est pas dans le gestionnaire client :
    /// Java **sort avant de vider la liste**, donc la fenêtre continue d'afficher les gardes
    /// de l'ouverture précédente. Le port garde la même liste et le dit.
    pub fn refresh(&mut self) -> Result<(), Refusal> {
        if !self.present {
            return Err(Refusal::HutViewMissing);
        }
        self.assigned = 0;
        self.rows.clear();
        for (index, tower) in self.towers.iter().enumerate() {
            if !tower.is_listed() {
                continue;
            }
            if tower.guards_mine(self.hut) {
                self.assigned += 1;
            }
            for slot in &tower.guards {
                self.rows.push(Row {
                    tower: index,
                    id: slot.id(),
                    name: slot.name().map(|name| name.to_owned()),
                    button: self.state_of(index, slot),
                });
            }
        }
        Ok(())
    }

    /// L'état du bouton d'une ligne, dans l'ordre des tests de Java (`:161-189`).
    fn state_of(&self, index: usize, slot: &Slot) -> ButtonState {
        let tower = match self.towers.get(index) {
            Some(tower) => tower,
            None => return ButtonState::UnknownGuard,
        };
        if slot.name().is_none() {
            return ButtonState::UnknownGuard;
        }
        match tower.mine {
            Some(mine) if mine == self.hut => ButtonState::CanUnassign,
            Some(_) => ButtonState::AnotherMine,
            None if self.assigned >= self.max() => ButtonState::FullOfGuards,
            None => ButtonState::CanAssign,
        }
    }

    /// Le clic sur le bouton d'une ligne (`:51-77`).
    ///
    /// Rend la nouvelle position de mine de la tour : `Ok(Some(mine))` après une
    /// affectation, `Ok(None)` après un retrait. Java envoie `GuardSetMinePosMessage` dans
    /// les deux cas, avec la position à affecter ou sans elle
    /// (`src/network/building_modules.rs`, `GuardSetMinePos.mine_pos: Option<[i32; 3]>`).
    pub fn click(&mut self, row: usize) -> Result<Option<[i32; 3]>, Refusal> {
        let line = self
            .rows
            .get(row)
            .ok_or(Refusal::NoSuchRow { row, rows: self.rows.len() })?;
        let index = line.tower;
        let state = line.button;
        match state {
            ButtonState::CanUnassign => {
                let tower = self
                    .towers
                    .get_mut(index)
                    .ok_or(Refusal::GuardUnknown { id: line.id })?;
                tower.mine = None;
                let _ = self.refresh();
                Ok(None)
            }
            ButtonState::CanAssign => {
                // Java re-checks the cap in the handler (:60), not only in the button.
                if self.assigned >= self.max() {
                    return Err(Refusal::MineAtCapacity { level: self.level, max: self.max() });
                }
                let hut = self.hut;
                let tower = self
                    .towers
                    .get_mut(index)
                    .ok_or(Refusal::GuardUnknown { id: line.id })?;
                tower.mine = Some(hut);
                let _ = self.refresh();
                Ok(Some(hut))
            }
            ButtonState::FullOfGuards => Err(Refusal::MineAtCapacity {
                level: self.level,
                max: self.max(),
            }),
            ButtonState::AnotherMine => {
                let at = self.towers.get(index).and_then(|tower| tower.mine).unwrap_or([0, 0, 0]);
                Err(Refusal::MinePostTakenElsewhere { at })
            }
            ButtonState::UnknownGuard => Err(Refusal::GuardUnknown { id: line.id }),
        }
    }
}

/// Les trois panneaux de la fenêtre : l'avertissement, la liste et le nom de la ligne.
const F_WARNING: Field = Field::new(WARNING, Value::Text, Source::Colony);
const F_LIST: Field = Field::new(LIST, Value::Row, Source::Colony);
const F_GUARD_NAME: Field = Field::in_row(LIST, GUARD_NAME, Value::Name, Source::Citizen);

/// Les panneaux, dans l'ordre du XML.
pub const FIELDS: &[Field] = &[F_WARNING, F_LIST, F_GUARD_NAME];

/// Le seul bouton de la fenêtre, et il est **par ligne** : un garde, un bouton
/// (`:30`, `:48`).
const A_ASSIGN: super::Button = super::Button::on_row(
    ASSIGN,
    ASSIGN_KEY,
    Effect::Sends("GuardSetMinePosMessage"),
    // `:165-168` and `:178-182` disable it; `:185-189` disables it for a missing citizen.
    &[Kind::MineAtCapacity, Kind::MinePostTakenElsewhere, Kind::GuardUnknown],
);

pub const BUTTONS: &[super::Button] = &[A_ASSIGN];

#[cfg(test)]
mod tests {
    use super::*;

    fn citizen(id: i32, name: &str) -> Slot {
        Slot::Guard(Citizen::new(id, name))
    }

    fn mine() -> [i32; 3] {
        [100, 50, 100]
    }

    fn scene() -> Guards {
        let towers = vec![
            // A patrol-mine tower that watches this very mine.
            Tower::new([10, 60, 10], GuardTask::PatrolMine, Some(mine()), vec![citizen(1, "Ada")]),
            // A patrol-mine tower that watches nothing, with one unknown citizen.
            Tower::new(
                [20, 60, 20],
                GuardTask::PatrolMine,
                None,
                vec![citizen(2, "Bob"), Slot::Missing(3)],
            ),
            // A tower in plain patrol: never listed, guards or not.
            Tower::new([30, 60, 30], GuardTask::Patrol, Some(mine()), vec![citizen(4, "Cy")]),
        ];
        Guards::new(mine(), 1, true, towers)
    }

    #[test]
    fn the_panes_and_the_one_button_are_the_ones_the_xml_declares() {
        assert_eq!(FIELDS.len(), 3);
        assert_eq!(FIELDS[0].id, WARNING);
        assert_eq!(FIELDS[1].value, Value::Row);
        assert_eq!(
            FIELDS[2].scope,
            crate::ui::field::Scope::Row(LIST),
            "the name is inside a row"
        );
        assert_eq!(BUTTONS.len(), 1);
        assert!(BUTTONS[0].on_row, "assignGuard is a row button, not a window one");
        assert_eq!(BUTTONS[0].refusals.len(), 3);
        // src/ui has no line for this module: the guard list is not in its table.
        assert!(crate::ui::module_screen(MODULE).is_none());
    }

    #[test]
    fn the_cap_is_one_two_three_by_level_and_zero_below() {
        assert_eq!(max_guards(0), 0, "an unbuilt hut hires no guard");
        assert_eq!(max_guards(1), 1);
        assert_eq!(max_guards(2), 1);
        assert_eq!(max_guards(3), 2);
        assert_eq!(max_guards(4), 2);
        assert_eq!(max_guards(5), 3);
        assert_eq!(max_guards(6), 0, "above the top level, Java's default says none");
        assert_eq!(max_guards(-1), 0);
    }

    #[test]
    fn only_patrol_mine_towers_are_listed_and_a_missing_citizen_is_kept() {
        let guards = scene();
        let rows = guards.rows();
        assert_eq!(rows.len(), 3, "two guards, one hole — the patrol tower is out");
        assert_eq!(rows[0].name.as_deref(), Some("Ada"));
        assert_eq!(rows[1].name.as_deref(), Some("Bob"));
        assert_eq!(rows[2].name, None, "Java keeps the null, and renders no name");
        assert_eq!(rows[2].id, 3);
        assert_eq!(guards.assigned(), 1, "one tower watches this mine");
        assert_eq!(guards.max(), 1);
        assert!(!guards.warning());
    }

    #[test]
    fn the_three_button_states_come_from_the_mine_and_the_cap() {
        let guards = scene();
        let rows = guards.rows();
        assert_eq!(rows[0].button, ButtonState::CanUnassign);
        // The second tower watches nothing, and the mine is already held by one: level 1
        // accepts exactly one, so the button is disabled.
        assert_eq!(rows[1].button, ButtonState::FullOfGuards);
        assert_eq!(rows[2].button, ButtonState::UnknownGuard);
        assert_eq!(ButtonState::CanAssign.label(), Some(ASSIGN_KEY));
        assert_eq!(ButtonState::CanUnassign.label(), Some(UNASSIGN_KEY));
        assert_eq!(ButtonState::UnknownGuard.label(), None, "Java writes no label");
        assert!(!ButtonState::FullOfGuards.is_enabled());
        assert_eq!(
            ButtonState::FullOfGuards.refusal(),
            Some(Kind::MineAtCapacity)
        );
        assert_eq!(ButtonState::CanUnassign.refusal(), None);
        // A tower that watches another mine is disabled with the other reason.
        let elsewhere = Guards::new(
            mine(),
            5,
            true,
            vec![Tower::new(
                [40, 60, 40],
                GuardTask::PatrolMine,
                Some([1, 2, 3]),
                vec![citizen(5, "Di")],
            )],
        );
        assert_eq!(elsewhere.rows()[0].button, ButtonState::AnotherMine);
        assert_eq!(
            elsewhere.rows()[0].button.refusal(),
            Some(Kind::MinePostTakenElsewhere)
        );
    }

    #[test]
    fn clicking_assigns_this_mine_and_clicking_again_clears_it() {
        let mut guards = scene();
        // Row 0 is the guard whose tower already watches the mine: the click clears it.
        assert_eq!(guards.click(0), Ok(None));
        assert_eq!(guards.assigned(), 0);
        assert_eq!(guards.towers()[0].mine, None);
        assert_eq!(guards.rows()[0].button, ButtonState::CanAssign);
        // Clicking again points the tower at this mine.
        assert_eq!(guards.click(0), Ok(Some(mine())));
        assert_eq!(guards.towers()[0].mine, Some(mine()));
        assert_eq!(guards.assigned(), 1);
        // The cap is one: the other tower now refuses.
        assert_eq!(
            guards.click(1),
            Err(Refusal::MineAtCapacity { level: 1, max: 1 })
        );
        // A hole in the list is a named refusal too.
        assert_eq!(guards.click(2), Err(Refusal::GuardUnknown { id: 3 }));
        assert_eq!(
            guards.click(99),
            Err(Refusal::NoSuchRow { row: 99, rows: 3 })
        );
        assert!(guards.is_present());
    }

    #[test]
    fn a_missing_hut_view_leaves_the_previous_list_on_screen() {
        let mut guards = scene();
        let before = guards.rows().to_vec();
        guards.set_level(5);
        // Java returns before clearing (:84-87): the list is what the last successful pull
        // produced, and the cap has already changed.
        guards.present = false;
        assert_eq!(guards.refresh(), Err(Refusal::HutViewMissing));
        assert_eq!(guards.rows(), before.as_slice());
        assert_eq!(guards.max(), 3, "the level is read live");
        guards.present = true;
        assert_eq!(guards.refresh(), Ok(()));
        // Two guards free at level 5, so the button of the unattached tower is enabled now.
        assert_eq!(guards.rows()[1].button, ButtonState::CanAssign);
        assert_eq!(guards.warning(), false);
        let empty = Guards::new(mine(), 1, true, Vec::new());
        assert!(empty.warning(), "the noguards text is the empty-list state");
        assert_eq!(WARNING_KEY, "com.minecolonies.coremod.gui.workerhuts.miner.noguards");
    }

    #[test]
    fn the_four_guard_tasks_round_trip_through_their_keys() {
        let tasks = [
            GuardTask::Patrol,
            GuardTask::Guard,
            GuardTask::Follow,
            GuardTask::PatrolMine,
        ];
        for (index, task) in tasks.iter().enumerate() {
            assert!(task.key().starts_with("com.minecolonies.core.guard.setting."));
            assert_eq!(GuardTask::parse(task.key()), Some(*task));
            for other in &tasks[index + 1..] {
                assert_ne!(task.key(), other.key());
            }
        }
        assert_eq!(GuardTask::parse("com.minecolonies.core.guard.setting.fly"), None);
        assert_eq!(PATROL_MINE, GuardTask::PatrolMine.key());
    }
}
