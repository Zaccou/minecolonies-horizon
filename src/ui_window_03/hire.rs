//! `WindowHireWorker` : la liste des citoyens assignables, ses quatre boutons par ligne,
//! et l'état qu'un clic laisse derrière lui.
//!
//! Java reference: `core/client/gui/WindowHireWorker.java` (574 lignes),
//! `assets/minecolonies/gui/windowhireworker.xml:1-43`,
//! `core/colony/buildings/moduleviews/WorkerBuildingModuleView.java:73-88` (les deux
//! `HireFireMessage`), `:148-152` (`BuildingHiringModeMessage`), `:155-162` (`canAssign`),
//! `:198-201` (`isFull`).
//!
//! **`src/ui` décrit déjà cet écran** (`ui::screen::HIRE_ID`, class `WindowHireWorker`) :
//! ses sept boutons, ses six panneaux et leurs sources. Ce fichier n'en reprend rien — il
//! porte ce que la table statique ne peut pas tenir : l'état de la sélection, la matrice
//! des boutons **par ligne**, l'ordre de la liste et les deux messages qu'un clic envoie.
//! [`crate::ui_window_03::WINDOWS`] renvoie l'enregistrement de `src/ui` plutôt que d'en
//! écrire un second.
//!
//! # La fenêtre n'embauche pas elle-même
//!
//! `doneClicked` (`:241-258`) ne parle à aucun réseau : il appelle `addCitizen` sur la vue
//! du module, et **c'est la vue qui envoie** `HireFireMessage`
//! (`WorkerBuildingModuleView.java:73-80`). Deux conséquences que le port garde :
//!
//! 1. Le clic `done` sur un citoyen qui a déjà un métier envoie **deux** fois la même
//!    classe — une fois pour l'ancien bâtiment (`:249-253`), une fois pour celui-ci
//!    (`:256`) — et l'ordre compte : l'ancien module est vidé avant que le nouveau soit
//!    rempli. Sans le premier envoi, le citoyen resterait employé deux fois.
//! 2. `pause` et `restart` passent, eux, par `WindowHireWorker` directement
//!    (`:203`, `:218`).
//!
//! # Trois conditions de Java qui ne se lisent pas comme on les attend
//!
//! - **`isFull()` n'est pas « il n'y a plus de place ».** C'est
//!   `!allowsAssignment() || assigned >= max` (`WorkerBuildingModuleView.java:200`) : un
//!   bâtiment qui n'autorise pas l'assignation est **plein** avec zéro employé.
//! - **`canAssign` d'une ligne est morte.** La liste est déjà filtrée par
//!   `canAssign` (`:354`), et la matrice de boutons reteste la même condition dans sa
//!   première branche (`:407`). Sur une ligne affichée elle est donc toujours vraie ;
//!   le port la garde parce qu'un appelant peut demander les boutons d'un citoyen qui
//!   n'est pas dans la liste.
//! - **`restart` vaut `paused`, toujours.** Il est écrit après les trois branches
//!   (`:440-447`), donc il écrase les `off()` des deux premières : une ligne « à qui on
//!   peut proposer le poste » garde quand même un bouton `restart` si le citoyen est en
//!   pause.
//!
//! Et un quatrième, plus petit : **changer de métier remet « montrer les employés » à
//! faux**, parce que `jobClicked` rappelle `setupShowEmployed` (`:298` → `:326-333`);
//! [`State::select_job`] le fait donc aussi, sans le dire au joueur.

use crate::ui::{next_hiring_mode, HiringMode, Refusal as UiRefusal};

use super::refusal::Refusal;

/// La classe Java.
pub const JAVA: &str = "WindowHireWorker";
/// Le XML, chargé à `:92`.
pub const RESOURCE: &str = "minecolonies:gui/windowhireworker.xml";

/// Le libellé de la fenêtre (`WindowConstants.java:1061`, écrit à `:135`).
pub const JOB_LABEL: &str = "jobLabel";
/// La liste des citoyens (`WindowConstants.java:994`, lue à `:96`).
pub const LIST: &str = "unemployed";
/// Le nom, dans une ligne (`WindowConstants.java:944`, écrit à `:478`).
pub const CITIZEN_LABEL: &str = "citizen";
/// La distance au logement, dans une ligne (`WindowConstants.java:1009`, `:479-495`).
pub const DISTANCE_LABEL: &str = "distance";
/// Les compétences, dans une ligne (`WindowConstants.java:1004`, `:497`).
pub const ATTRIBUTES_LABEL: &str = "attributes";

pub const DONE_BUTTON: &str = "done";
pub const FIRE_BUTTON: &str = "fire";
pub const PAUSE_BUTTON: &str = "pause";
pub const RESTART_BUTTON: &str = "restart";
/// Le bouton du mode d'embauche (`WindowConstants.java:979`, `:119`).
pub const MODE_BUTTON: &str = "mode";
/// Le basculement « montrer les employés » (`WindowConstants.java:989`, `:104`).
pub const SHOW_EMPLOYED_BUTTON: &str = "showEmployed";
pub const CANCEL_BUTTON: &str = "cancel";

/// Le libellé du mode (`windowhireworker.xml:31`).
pub const MODE_LABEL: &str = "buildingHireMode";
/// Le libellé du basculement (`windowhireworker.xml:36`).
pub const SHOW_EMPLOYED_LABEL: &str = "showEmployedLabel";
/// « oui » / « non », écrits par `showEmployedToggled` (`:315-316`).
pub const SHOW_YES: &str = "gui.yes";
pub const SHOW_NO: &str = "gui.no";

/// Le libellé du bouton `pause` quand le citoyen travaille (`TranslationConstants.java:260`).
pub const PAUSE_KEY: &str = "com.minecolonies.coremod.gui.hiring.buttonpaused";
/// Le libellé du bouton `pause` quand il est en pause (`TranslationConstants.java:262`).
pub const UNPAUSE_KEY: &str = "com.minecolonies.coremod.gui.hiring.buttonunpaused";

/// Les trois clés de la ligne `distance` — attention, elles sont en `core.gui.hiring` et
/// non en `coremod.gui.hiring` (`WindowHireWorker.java:481`, `:485`, `:489`).
pub const HOMELESS_KEY: &str = "com.minecolonies.core.gui.hiring.homeless";
pub const LIVES_HERE_KEY: &str = "com.minecolonies.core.gui.hiring.liveshere";
pub const LIVES_AT_WORK_KEY: &str = "com.minecolonies.core.gui.hiring.livesatwork";
/// La clé qui prend le nombre de blocs en argument (`:494`).
pub const DISTANCE_KEY: &str = "com.minecolonies.core.gui.hiring.distance";

/// Le message qu'un embauchement ou un licenciement envoie, **depuis la vue du module**
/// (`WorkerBuildingModuleView.java:76`, `:86`).
pub const HIRE_FIRE_MESSAGE: &str = "HireFireMessage";
/// Le message du bouton `pause` (`:218`).
pub const PAUSE_MESSAGE: &str = "PauseCitizenMessage";
/// Le message du bouton `restart` (`:203`).
pub const RESTART_MESSAGE: &str = "RestartCitizenMessage";
/// Le message du changement de mode (`WorkerBuildingModuleView.java:151`).
pub const MODE_MESSAGE: &str = "BuildingHiringModeMessage";

/// De quelle classe de vue le module hérite — trois cas, deux comportements.
///
/// Java teste `instanceof WorkerBuildingModuleView` puis exclut
/// `PupilBuildingModuleView` (`:328-330`). Les deux sont des `WorkerBuildingModuleView` :
/// `Pupil` en est un sous-type. L'énumération aplatit la hiérarchie pour que le test soit
/// une égalité et non deux `instanceof`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleKind {
    /// Un vrai `WorkerBuildingModuleView` : le basculement est actif, un citoyen déjà
    /// employé ailleurs envoie deux messages.
    Worker,
    /// `PupilBuildingModuleView` : le basculement est **désactivé** (`:329-330`).
    Pupil,
    /// Un autre module d'assignation (courier, mineur) : ni basculement, ni double envoi.
    Other,
}

/// Une vue de module d'assignation, telle que cette fenêtre la lit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    /// `getJobEntry().getKey().toString()` : l'id du bouton de métier (`:532`).
    pub job: &'static str,
    /// `getTranslationKey()` du métier, le libellé du bouton (`:526`, `:530`).
    pub label: &'static str,
    /// `getMaxInhabitants()` (`:107`).
    pub max: i32,
    /// `getAssignedCitizens()`, en ids (`:409`).
    pub assigned: Vec<i32>,
    /// `getHiringMode()` (`:168`).
    pub mode: HiringMode,
    /// La classe de la vue (voir [`ModuleKind`]).
    pub kind: ModuleKind,
    /// `buildingView.allowsAssignment()`, la moitié gauche de `isFull()`
    /// (`WorkerBuildingModuleView.java:200`).
    pub allows_assignment: bool,
    /// Vrai quand `getResearchRequirement()` est nul ou satisfait : sinon le bouton n'est
    /// **jamais ajouté** (`:515-518`).
    pub research_done: bool,
    /// `BuildingUtils.getAllowedJobs(...).test(jobEntry)` (`:106-108`).
    pub allowed: bool,
}

/// Un citoyen, tel que cette fenêtre le lit — et rien de plus.
///
/// `assignable` est le résultat de `selectedModule.canAssign(citizen)` **pour le module
/// sélectionné** : Java le recalcule à chaque changement de métier (`:300`), donc
/// l'appelant doit le recalculer aussi ([`State::select_job`] le signale).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub id: i32,
    pub name: String,
    /// `isChild()` (`:343`, `:157`).
    pub child: bool,
    /// `isPaused()` (`:437`, `:440`).
    pub paused: bool,
    /// `getColony().getTravellingManager().isTravelling(citizen)` (`:427`).
    pub travelling: bool,
    /// `getHomeBuilding()` (`:357`).
    pub home: Option<[i32; 3]>,
    /// `getWorkBuilding()` (`:247`, `:268`).
    pub work: Option<[i32; 3]>,
    /// `selectedModule.canAssign(citizen)` (`WorkerBuildingModuleView.java:155-162`).
    pub assignable: bool,
}

/// Les quatre boutons d'une ligne, et le libellé que `pause` porte quand il est écrit.
///
/// `pause_label: None` n'est pas « vide » : c'est « Java ne l'écrit pas ». Les deux
/// premières branches de `:407-422` éteignent le bouton sans toucher à son texte, et
/// BlockUI réutilise le panneau de la ligne : le libellé reste celui de la ligne
/// précédente. Le port le dit au lieu de faire semblant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowButtons {
    pub done: bool,
    pub fire: bool,
    pub pause: bool,
    pub restart: bool,
    pub pause_label: Option<&'static str>,
}

/// Ce que Java fait de la fenêtre après le clic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum After {
    /// Rien : le citoyen change, la ligne se redessine (`:219`).
    Nothing,
    /// `onOpened()` : la liste est recalculée (`:233`, `:257`, `:318`).
    Refresh,
    /// `close()` (`:204`).
    Closes,
    /// `building.openGui(false)` : la fenêtre de la hutte revient (`:147`).
    OpensHut,
}

/// Un clic et son effet : les messages envoyés, dans l'ordre, puis la fenêtre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Turn {
    pub sends: Vec<&'static str>,
    pub after: After,
}

/// Un bouton de métier, construit par `setupJobButtons` (`:510-552`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JobButton {
    pub job: &'static str,
    pub label: &'static str,
    /// Le nombre d'employés, écrit à côté du libellé seulement s'il est non nul (`:524-531`).
    pub count: usize,
    /// Faux pour le métier déjà sélectionné (`:539-546`).
    pub enabled: bool,
}

/// Où habite le citoyen, pour la ligne `distance` (`:479-495`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Where {
    Homeless,
    LivesHere,
    LivesAtWork,
    /// Le nombre de blocs, **arrondi par le bas** et non passé par le seau de 40 blocs du
    /// tri (`:494` contre `:356-374`).
    Blocks(i32),
}

impl Where {
    /// La clé de traduction, celle que la ligne écrit.
    pub const fn key(self) -> &'static str {
        match self {
            Where::Homeless => HOMELESS_KEY,
            Where::LivesHere => LIVES_HERE_KEY,
            Where::LivesAtWork => LIVES_AT_WORK_KEY,
            Where::Blocks(_) => DISTANCE_KEY,
        }
    }
}

/// L'état de la fenêtre : les modules retenus, celui qui est sélectionné, le basculement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    /// La position de la hutte ; le tri et `where_lives` la comparent aux citoyens (`:268`).
    pub at: [i32; 3],
    /// Les seuls modules qui passent le filtre du constructeur (`:107-111`).
    pub modules: Vec<Module>,
    /// L'index du module sélectionné — `0` à l'ouverture (`:116`).
    pub selected: usize,
    /// « montrer les employés », faux à l'ouverture (`:332`).
    pub show_employed: bool,
}

impl State {
    /// Le constructeur (`:90-121`), moins le dessin : filtre les modules et refuse la
    /// fenêtre s'il n'en reste aucun.
    ///
    /// Le filtre est celui de `:107-108`, **et l'ordre des deux tests compte** :
    /// `m.getMaxInhabitants() > 0 &&` est le premier conjoint, donc un module à zéro
    /// place est écarté quoi qu'il contienne — y compris quand il a déjà des employés.
    /// Le `||` de droite (`assigned` non vide, ou métier autorisé) ne peut donc jamais
    /// rattraper un `max == 0`. Un module à vider et à zéro place disparaît de cette
    /// fenêtre, et c'est Java qui le veut.
    pub fn open(at: [i32; 3], modules: Vec<Module>) -> Result<State, Refusal> {
        let kept: Vec<Module> = modules
            .into_iter()
            .filter(|module| module.max > 0 && (!module.assigned.is_empty() || module.allowed))
            .collect();
        if kept.is_empty() {
            // `:112-115` sort du constructeur, et `onOpened` ferme la fenêtre
            // (`:385-389`) : le même refus, déjà nommé par `src/ui`.
            return Err(Refusal::Port(UiRefusal::NoAssignableModule));
        }
        Ok(State { at, modules: kept, selected: 0, show_employed: false })
    }

    /// Le module sélectionné, ou [`Refusal::UnknownJob`] si l'état est vide.
    pub fn module(&self) -> Result<&Module, Refusal> {
        self.modules.get(self.selected).ok_or(Refusal::UnknownJob)
    }

    /// Les ids assignés au module sélectionné, dans l'ordre où Java les garde.
    pub fn assigned(&self) -> &[i32] {
        match self.modules.get(self.selected) {
            Some(module) => &module.assigned,
            None => &[],
        }
    }

    /// `isFull()` (`WorkerBuildingModuleView.java:198-201`) : le bâtiment n'autorise pas
    /// l'assignation, **ou** il n'y a plus de place.
    pub fn is_full(&self) -> bool {
        match self.modules.get(self.selected) {
            Some(module) => !module.allows_assignment || module.assigned.len() as i32 >= module.max,
            None => false,
        }
    }

    /// La condition de visibilité d'une ligne (`:341-344`).
    ///
    /// **Elle rend vrai pour toute ligne que [`State::rows`] a laissée passer** : c'est la
    /// même expression que le filtre. Voir la note du module.
    pub fn visible(&self, candidate: &Candidate) -> bool {
        (self.show_employed && !candidate.child) || candidate.assignable
    }

    /// L'ordre de la liste (`:266-284`) : `0` emploie ici, `3` refuse, `1` sans métier,
    /// `2` employé ailleurs. Les tests sont dans cet ordre et il compte : un citoyen déjà
    /// employé ici vaut `0` même si le module est plein.
    pub fn priority(&self, candidate: &Candidate) -> u8 {
        if candidate.work == Some(self.at) {
            0
        } else if !candidate.assignable {
            3
        } else if candidate.work.is_none() {
            1
        } else {
            2
        }
    }

    /// La distance au logement, **arrondie au multiple de 40** quand elle dépasse un seuil
    /// de 20 dans son intervalle (`:356-374`), et `100` pour un sans-abri (`:359-361`).
    pub fn distance_bucket(&self, candidate: &Candidate) -> i64 {
        match candidate.home {
            None => 100,
            Some(home) => bucket(distance_sq(home, self.at)),
        }
    }

    /// Les lignes affichées, dans l'ordre de Java : priorité, puis distance en seaux, puis
    /// nom (`:355-375`).
    pub fn rows<'a>(&self, candidates: &'a [Candidate]) -> Vec<&'a Candidate> {
        let mut kept: Vec<&Candidate> =
            candidates.iter().filter(|candidate| self.visible(candidate)).collect();
        kept.sort_by(|left, right| {
            self.priority(left)
                .cmp(&self.priority(right))
                .then_with(|| self.distance_bucket(left).cmp(&self.distance_bucket(right)))
                .then_with(|| left.name.cmp(&right.name))
        });
        kept
    }

    /// Les quatre boutons d'une ligne (`:407-447`), dans l'ordre de Java.
    pub fn row_buttons(&self, candidate: &Candidate) -> RowButtons {
        let assigned = self.assigned().contains(&candidate.id);
        let full = self.is_full();
        let mut buttons = if self.visible(candidate) && !full && !assigned {
            // :407-415 — on peut proposer le poste : `done` seul, tout le reste éteint.
            RowButtons { done: true, fire: false, pause: false, restart: false, pause_label: None }
        } else if full && !assigned {
            // :416-422 — plein : plus rien, pas même `pause`.
            RowButtons { done: false, fire: false, pause: false, restart: false, pause_label: None }
        } else {
            // :423-438 — déjà employé (ou embauchable nulle part) : `fire` sauf s'il
            // voyage, `pause` toujours, avec son libellé.
            RowButtons {
                done: false,
                fire: !candidate.travelling,
                pause: true,
                restart: false,
                pause_label: Some(if candidate.paused { UNPAUSE_KEY } else { PAUSE_KEY }),
            }
        };
        // :440-447 — écrit **après** les trois branches : `restart` suit `paused`, point.
        buttons.restart = candidate.paused;
        buttons
    }

    /// La ligne `distance` (`:479-495`), dans l'ordre des trois tests de Java.
    pub fn where_lives(&self, candidate: &Candidate) -> Where {
        match candidate.home {
            None => Where::Homeless,
            Some(home) if home == self.at => Where::LivesHere,
            Some(home) if Some(home) == candidate.work => Where::LivesAtWork,
            Some(home) => Where::Blocks((distance_sq(home, self.at) as f64).sqrt() as i32),
        }
    }

    /// Le clic sur un bouton de métier (`:291-306`) : sélectionne, et **remet le
    /// basculement à faux** parce que Java rappelle `setupShowEmployed` (`:298`).
    ///
    /// L'appelant doit ensuite recalculer `Candidate::assignable` pour le nouveau module :
    /// Java rappelle `updateCitizens()` juste après (`:300`).
    pub fn select_job(&mut self, job: &str) -> Result<(), Refusal> {
        let index = match self.modules.iter().position(|module| module.job == job) {
            Some(index) => index,
            None => return Err(Refusal::UnknownJob),
        };
        self.selected = index;
        self.show_employed = false;
        Ok(())
    }

    /// Le clic sur le mode d'embauche (`:166-181`), qui saute `LOCKED`
    /// (« only homes can be locked, not workplaces ») : c'est exactement
    /// [`crate::ui::next_hiring_mode`] avec `home == false`, donc la règle est appelée et
    /// non recopiée.
    pub fn cycle_mode(&mut self) -> Result<Turn, Refusal> {
        let index = self.selected;
        let module = match self.modules.get_mut(index) {
            Some(module) => module,
            None => return Err(Refusal::UnknownJob),
        };
        module.mode = next_hiring_mode(module.mode, false);
        // `setupSettings` écrit la clé du mode sur le bouton (`:188-191`), et
        // `setHiringMode` envoie le message (`WorkerBuildingModuleView.java:148-152`).
        Ok(Turn { sends: vec![MODE_MESSAGE], after: After::Nothing })
    }

    /// Le clic sur « montrer les employés » (`:313-319`), et le libellé qu'il laisse.
    ///
    /// Java écrit le libellé **avant** de basculer, avec l'ancienne valeur : le bouton
    /// affiche donc l'état où il est, pas l'action qu'il ferait. Le port répond le libellé
    /// d'après le nouvel état, qui est le même mot.
    pub fn toggle_show_employed(&mut self) -> &'static str {
        self.show_employed = !self.show_employed;
        if self.show_employed { SHOW_YES } else { SHOW_NO }
    }

    /// Vrai quand le basculement est utilisable (`:329-330`) : un vrai module de travail,
    /// et pas un pupille.
    pub fn can_show_employed(&self) -> bool {
        match self.modules.get(self.selected) {
            Some(module) => module.kind == ModuleKind::Worker,
            None => false,
        }
    }

    /// Les boutons de métier (`:510-552`) : un par module dont la recherche est faite,
    /// celui qui est sélectionné étant éteint.
    pub fn job_buttons(&self) -> Vec<JobButton> {
        self.modules
            .iter()
            .enumerate()
            .filter(|(_, module)| module.research_done)
            .map(|(index, module)| JobButton {
                job: module.job,
                label: module.label,
                count: module.assigned.len(),
                enabled: index != self.selected,
            })
            .collect()
    }

    /// Le bouton `done` d'une ligne (`:241-258`) : deux `HireFireMessage` quand le citoyen
    /// quitte un autre métier, un seul sinon.
    pub fn hire(&mut self, candidate: &Candidate) -> Result<Turn, Refusal> {
        if !self.visible(candidate) {
            return Err(Refusal::Port(UiRefusal::CannotAssign));
        }
        if self.assigned().contains(&candidate.id) {
            // `done` est éteint sur la ligne du citoyen déjà employé (`:409`, `:425`).
            return Err(Refusal::ButtonOff);
        }
        if self.is_full() {
            return Err(Refusal::Port(UiRefusal::NoRoom));
        }
        let worker = self.module()?.kind == ModuleKind::Worker;
        let sends = if candidate.work.is_some() && worker {
            // `:247-253` vide l'ancien module **avant** `:256` remplit le nouveau : deux
            // envois de la même classe, deux bâtiments.
            vec![HIRE_FIRE_MESSAGE, HIRE_FIRE_MESSAGE]
        } else {
            vec![HIRE_FIRE_MESSAGE]
        };
        let index = self.selected;
        if let Some(module) = self.modules.get_mut(index) {
            module.assigned.push(candidate.id);
        }
        Ok(Turn { sends, after: After::Refresh })
    }

    /// Le bouton `fire` (`:227-234`) : refuse un citoyen qui voyage (`:427-434`).
    pub fn fire(&mut self, candidate: &Candidate) -> Result<Turn, Refusal> {
        if !self.assigned().contains(&candidate.id) {
            return Err(Refusal::ButtonOff);
        }
        if candidate.travelling {
            return Err(Refusal::Port(UiRefusal::Travelling));
        }
        let index = self.selected;
        if let Some(module) = self.modules.get_mut(index) {
            module.assigned.retain(|id| *id != candidate.id);
        }
        Ok(Turn { sends: vec![HIRE_FIRE_MESSAGE], after: After::Refresh })
    }

    /// Le bouton `pause` (`:212-220`) : un message, et le citoyen que l'appelant bascule
    /// (`:219` — l'état ne possède pas les citoyens, donc il ne le bascule pas lui-même).
    pub fn pause(&self, candidate: &Candidate) -> Result<Turn, Refusal> {
        if !self.row_buttons(candidate).pause {
            return Err(Refusal::ButtonOff);
        }
        Ok(Turn { sends: vec![PAUSE_MESSAGE], after: After::Nothing })
    }

    /// Le bouton `restart` (`:198-205`) : un message, puis **la fenêtre se ferme**.
    pub fn restart(&self, candidate: &Candidate) -> Result<Turn, Refusal> {
        if !self.row_buttons(candidate).restart {
            return Err(Refusal::ButtonOff);
        }
        Ok(Turn { sends: vec![RESTART_MESSAGE], after: After::Closes })
    }

    /// La croix (`:143-149`) : sans hôtel de ville, Java ne fait **rien**.
    pub fn cancel(&self, has_town_hall: bool) -> Result<Turn, Refusal> {
        if !has_town_hall {
            return Err(Refusal::NoTownHall);
        }
        Ok(Turn { sends: Vec::new(), after: After::OpensHut })
    }
}

/// `BlockPos.distSqr`, en entiers : la comparaison est exacte avant la racine.
///
/// Les deux coordonnées passent par `i64` **avant** la soustraction : une position lue
/// dans un compound peut être n'importe quel `i32`, et `i32::MIN - i32::MAX` déborderait
/// avant d'arriver au carré.
fn distance_sq(from: [i32; 3], to: [i32; 3]) -> i64 {
    let dx = i64::from(from[0]) - i64::from(to[0]);
    let dy = i64::from(from[1]) - i64::from(to[1]);
    let dz = i64::from(from[2]) - i64::from(to[2]);
    dx * dx + dy * dy + dz * dz
}

/// Le seau de 40 blocs de `:356-374` : au-delà de 20 dans son intervalle on monte, sinon
/// on descend. Les distances sont entières, donc l'arrondi vers le bas est exact.
fn bucket(square: i64) -> i64 {
    let distance = (square as f64).sqrt();
    let rest = distance % 40.0;
    let floor = distance - rest;
    if rest > 20.0 { (floor + 40.0) as i64 } else { floor as i64 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::action::Effect;

    fn module(job: &'static str, max: i32) -> Module {
        Module {
            job,
            label: job,
            max,
            assigned: Vec::new(),
            mode: HiringMode::Default,
            kind: ModuleKind::Worker,
            allows_assignment: true,
            research_done: true,
            allowed: true,
        }
    }

    fn candidate(id: i32, name: &str, home: [i32; 3], work: Option<[i32; 3]>) -> Candidate {
        Candidate {
            id,
            name: name.to_owned(),
            child: false,
            paused: false,
            travelling: false,
            home: Some(home),
            work,
            assignable: true,
        }
    }

    fn state() -> State {
        State::open([0, 0, 0], vec![module("guard", 2)]).unwrap_or_else(|refusal| {
            panic!("le module aurait dû être retenu : {refusal}")
        })
    }

    #[test]
    fn the_constructor_filters_the_modules_and_refuses_an_empty_list() {
        // :107 — `m.getMaxInhabitants() > 0 &&` est le **premier** conjoint : un module
        // qui n'accueille personne est écarté, même avec des employés. Le `||` de `:108`
        // ne rattrape jamais un `max == 0`.
        let mut empty = module("courier", 0);
        empty.allowed = true;
        let kept = State::open([0, 0, 0], vec![module("guard", 1), empty]);
        assert_eq!(kept.map(|state| state.modules.len()), Ok(1));

        // Et le cas qui m'avait trompé : des employés ne sauvent pas un module à zéro
        // place. Mon attente d'origine (« un module à vider reste visible ») était
        // fausse, c'est le code qui avait raison.
        let mut occupied = module("courier", 0);
        occupied.assigned.push(7);
        let kept = State::open([0, 0, 0], vec![module("guard", 1), occupied]);
        assert_eq!(kept.map(|state| state.modules.len()), Ok(1), "zéro place, zéro ligne");

        // :112-115 + :385-389 — aucune place, la fenêtre se ferme : src/ui le nomme déjà.
        let mut refused = module("guard", 1);
        refused.allowed = false;
        assert_eq!(
            State::open([0, 0, 0], vec![refused]),
            Err(Refusal::Port(UiRefusal::NoAssignableModule))
        );
    }

    #[test]
    fn full_is_also_true_when_the_building_forbids_assignment() {
        // WorkerBuildingModuleView.java:200 — la moitié gauche de `||`, celle qu'on oublie.
        let mut state = state();
        state.modules[0].allows_assignment = false;
        assert!(state.is_full());
        assert_eq!(state.modules[0].assigned.len(), 0);

        state.modules[0].allows_assignment = true;
        assert!(!state.is_full());
        state.modules[0].assigned = vec![1, 2];
        assert!(state.is_full(), "deux places, deux employés");
    }

    #[test]
    fn the_four_row_buttons_follow_the_three_branches_and_the_final_override() {
        let mut state = state();
        let applicant = candidate(1, "Anna", [40, 0, 0], None);
        // :407-415 — proposable : `done` seul, et `pause` n'écrit aucun libellé.
        assert_eq!(
            state.row_buttons(&applicant),
            RowButtons { done: true, fire: false, pause: false, restart: false, pause_label: None }
        );

        // :440-447 — un candidat en pause porte `restart` même sans être employé ici.
        let mut paused = applicant.clone();
        paused.paused = true;
        assert!(state.row_buttons(&paused).restart);
        assert_eq!(state.row_buttons(&paused).pause_label, None, "pause est éteint, donc muet");

        // :416-422 — plein : plus rien.
        state.modules[0].assigned = vec![1, 2];
        let full = state.row_buttons(&candidate(3, "Bob", [0, 0, 0], None));
        assert_eq!(
            full,
            RowButtons { done: false, fire: false, pause: false, restart: false, pause_label: None }
        );

        // :423-438 — employé ici : `fire` et `pause` allumés, `done` éteint.
        let hired = candidate(1, "Anna", [40, 0, 0], Some([0, 0, 0]));
        let buttons = state.row_buttons(&hired);
        assert!(buttons.fire && buttons.pause && !buttons.done);
        assert_eq!(buttons.pause_label, Some(PAUSE_KEY));
        let mut paused_and_hired = hired.clone();
        paused_and_hired.paused = true;
        assert_eq!(state.row_buttons(&paused_and_hired).pause_label, Some(UNPAUSE_KEY));
        assert!(state.row_buttons(&paused_and_hired).restart);

        // :427-434 — un employé qui voyage ne peut pas être licencié.
        let mut travelling = hired;
        travelling.travelling = true;
        assert!(!state.row_buttons(&travelling).fire);
    }

    #[test]
    fn the_list_is_ordered_by_priority_then_by_a_forty_block_bucket_then_by_name() {
        let mut state = state();
        // Le basculement est allumé : sinon un citoyen que le module refuse ne serait
        // même pas dans la liste (`:341-344` filtre avec la même condition).
        state.show_employed = true;
        // Un citoyen déjà employé ici passe devant, même si le module refuse les autres.
        state.modules[0].assigned = vec![9];
        let here = candidate(9, "Zoe", [1000, 0, 0], Some([0, 0, 0]));
        let free = candidate(1, "Anna", [10, 0, 0], None);
        let elsewhere = candidate(2, "Bob", [10, 0, 0], Some([500, 0, 0]));
        let homeless = Candidate { home: None, ..candidate(3, "Carl", [0, 0, 0], None) };
        let refused = Candidate { assignable: false, ..candidate(4, "Dan", [0, 0, 0], None) };
        let all = vec![refused.clone(), elsewhere.clone(), homeless.clone(), free.clone(), here];
        let ordered: Vec<i32> = state.rows(&all).iter().map(|row| row.id).collect();
        // 0 = emploie ici, puis les deux sans métier par seau de distance (10 blocs avant
        // « sans abri », qui vaut 100), puis l'employé ailleurs, puis le refusé.
        assert_eq!(ordered, [9, 1, 3, 2, 4]);
        assert_eq!(state.priority(&refused), 3);
        assert_eq!(state.priority(&free), 1);
        assert_eq!(state.priority(&elsewhere), 2);
        // `Anna` à 10 blocs et `Bob` aussi : c'est la priorité qui les sépare, pas le nom.
        assert_eq!(state.distance_bucket(&free), 0);
        assert_eq!(state.distance_bucket(&homeless), 100, ":359-361");
    }

    #[test]
    fn the_bucket_moves_by_forty_around_a_twenty_threshold() {
        // :356-374 — 30 tombe au-dessus de 20 et monte à 40 ; 20 ne monte pas.
        let state = state();
        let at = |x: i32| candidate(1, "A", [x, 0, 0], None);
        assert_eq!(state.distance_bucket(&at(10)), 0);
        assert_eq!(state.distance_bucket(&at(20)), 0);
        assert_eq!(state.distance_bucket(&at(21)), 40);
        assert_eq!(state.distance_bucket(&at(60)), 40);
        assert_eq!(state.distance_bucket(&at(61)), 80);
        assert_eq!(state.distance_bucket(&at(100)), 80);
    }

    #[test]
    fn the_distance_line_is_not_the_sort_bucket() {
        // :479-495 — trois cas nommés, puis la distance brute (non arrondie).
        let state = state();
        let home = |pos: Option<[i32; 3]>, work: Option<[i32; 3]>| Candidate {
            home: pos,
            work,
            ..candidate(1, "A", [0, 0, 0], None)
        };
        assert_eq!(state.where_lives(&home(None, None)), Where::Homeless);
        assert_eq!(state.where_lives(&home(Some([0, 0, 0]), None)), Where::LivesHere);
        // « vit ici » passe avant « vit au travail » quand les deux sont vrais.
        assert_eq!(state.where_lives(&home(Some([0, 0, 0]), Some([0, 0, 0]))), Where::LivesHere);
        assert_eq!(state.where_lives(&home(Some([9, 0, 0]), Some([9, 0, 0]))), Where::LivesAtWork);
        // 30 blocs : le tri en ferait 40, la ligne dit 30.
        assert_eq!(state.where_lives(&home(Some([30, 0, 0]), None)), Where::Blocks(30));
        assert_eq!(Where::Blocks(30).key(), DISTANCE_KEY);
    }

    #[test]
    fn the_mode_cycle_skips_locked_and_the_job_switch_resets_the_toggle() {
        let mut state = State::open(
            [0, 0, 0],
            vec![module("guard", 1), module("knight", 1)],
        )
        .expect("deux modules");
        assert_eq!(state.modules[0].mode, HiringMode::Default);
        let first = state.cycle_mode();
        assert_eq!(
            first.map(|turn| turn.sends),
            Ok(vec![MODE_MESSAGE]),
            "WorkerBuildingModuleView.java:148-152"
        );
        assert_eq!(state.modules[0].mode, HiringMode::Auto);
        let _ = state.cycle_mode();
        assert_eq!(state.modules[0].mode, HiringMode::Manual);
        // :169-172 — LOCKED est sauté : on revient à DEFAULT, jamais à LOCKED.
        let _ = state.cycle_mode();
        assert_eq!(state.modules[0].mode, HiringMode::Default);

        // :313-319 — le libellé suit le nouvel état, et le changement de métier le remet.
        assert_eq!(state.toggle_show_employed(), SHOW_YES);
        assert!(state.show_employed);
        assert_eq!(state.select_job("knight"), Ok(()));
        assert!(!state.show_employed, ":298 rappelle setupShowEmployed");
        assert_eq!(state.modules[state.selected].job, "knight");
        assert_eq!(state.select_job("baker"), Err(Refusal::UnknownJob));
    }

    #[test]
    fn hiring_someone_away_from_another_job_sends_the_message_twice() {
        let mut state = state();
        let free = candidate(1, "Anna", [10, 0, 0], None);
        assert_eq!(
            state.hire(&free),
            Ok(Turn { sends: vec![HIRE_FIRE_MESSAGE], after: After::Refresh })
        );
        assert_eq!(state.assigned(), [1].as_slice());

        // :247-258 — deux envois, l'ancien poste puis le nouveau.
        let elsewhere = candidate(2, "Bob", [10, 0, 0], Some([500, 0, 0]));
        let turn = state.hire(&elsewhere).expect("une place reste");
        assert_eq!(turn.sends, vec![HIRE_FIRE_MESSAGE, HIRE_FIRE_MESSAGE]);
        assert_eq!(state.assigned(), [1, 2].as_slice());

        // Le module est plein (deux places) : refus nommé, et rien n'a bougé.
        let third = candidate(3, "Carl", [10, 0, 0], None);
        assert_eq!(state.hire(&third), Err(Refusal::Port(UiRefusal::NoRoom)));
        assert_eq!(state.assigned(), [1, 2].as_slice());

        // `fire` refuse un citoyen qui voyage, et ferme la porte au reste.
        let mut travelling = free.clone();
        travelling.travelling = true;
        assert_eq!(state.fire(&travelling), Err(Refusal::Port(UiRefusal::Travelling)));
        assert_eq!(
            state.fire(&free),
            Ok(Turn { sends: vec![HIRE_FIRE_MESSAGE], after: After::Refresh })
        );
        assert_eq!(state.assigned(), [2].as_slice());

        // `pause` et `restart` : deux messages, et `restart` referme la fenêtre.
        let paused = Candidate { paused: true, ..free };
        state.modules[0].assigned = vec![paused.id];
        assert_eq!(
            state.pause(&paused),
            Ok(Turn { sends: vec![PAUSE_MESSAGE], after: After::Nothing })
        );
        assert_eq!(
            state.restart(&paused),
            Ok(Turn { sends: vec![RESTART_MESSAGE], after: After::Closes })
        );
        // Un candidat sans `restart` est refusé nommément.
        assert_eq!(state.restart(&candidate(9, "Eve", [0, 0, 0], None)), Err(Refusal::ButtonOff));
    }

    #[test]
    fn cancel_needs_a_town_hall_and_the_job_buttons_hide_an_unresearched_job() {
        let mut state = State::open(
            [0, 0, 0],
            vec![module("guard", 1), module("knight", 1), module("baker", 1)],
        )
        .expect("trois modules");
        // :145 — sans hôtel de ville, la croix ne fait rien du tout.
        assert_eq!(state.cancel(false), Err(Refusal::NoTownHall));
        assert_eq!(
            state.cancel(true),
            Ok(Turn { sends: Vec::new(), after: After::OpensHut })
        );

        // :515-518 — la recherche manquante fait disparaître le bouton, pas le module.
        state.modules[2].research_done = false;
        state.modules[0].assigned = vec![4, 5];
        let buttons = state.job_buttons();
        assert_eq!(buttons.len(), 2);
        assert_eq!(buttons[0].count, 2, ":524-527 ajoute le compte quand il est non nul");
        assert!(!buttons[0].enabled, ":539-546 éteint le métier courant");
        assert!(buttons[1].enabled);
        assert_eq!(state.modules.len(), 3, "le module reste dans la liste des données");
    }

    #[test]
    fn the_messages_named_here_are_the_ones_src_ui_names_for_the_same_screen() {
        // La jointure : `src/ui` décrit les boutons de cet écran et nomme leurs messages ;
        // ce fichier ne peut pas nommer autre chose.
        let screen = match crate::ui::screen_by_id(crate::ui::screen::HIRE_ID) {
            Ok(screen) => screen,
            Err(refusal) => panic!("src/ui ne décrit pas hire : {refusal}"),
        };
        let expected: [(&str, &str); 4] = [
            (DONE_BUTTON, HIRE_FIRE_MESSAGE),
            (FIRE_BUTTON, HIRE_FIRE_MESSAGE),
            (PAUSE_BUTTON, PAUSE_MESSAGE),
            (RESTART_BUTTON, RESTART_MESSAGE),
        ];
        for (button, message) in expected {
            let action = match screen.action(button) {
                Some(action) => action,
                None => panic!("src/ui n'a pas de bouton {button}"),
            };
            assert_eq!(action.effect, Effect::Sends(message), "{button}");
        }
        // Et le mode a le sien, sur une action que `src/ui` décrit sans garde.
        assert_eq!(
            screen.action(MODE_BUTTON).map(|action| action.effect),
            Some(Effect::Sends(MODE_MESSAGE))
        );
    }
}
