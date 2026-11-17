//! L'état de l'écran du travailleur : la priorité qu'il tient en local, et la ligne de
//! chaque ouvrier.
//!
//! Java référence : `core/client/gui/huts/WindowHutWorkerModulePlaceholder.java:14-24`
//! — **vingt-quatre lignes, dont rien** : un constructeur qui passe le XML à
//! `AbstractWindowWorkerModuleBuilding` (`:21-24`). Tout ce que l'écran demande et rend
//! est donc dans la classe de base, `AbstractWindowWorkerModuleBuilding.java`, et c'est
//! elle que ce fichier décrit.
//!
//! **Les panneaux et les boutons ne sont pas ici.** `src/ui` les déclare déjà :
//! l'écran [`SCREEN`] y est `hut.worker`, avec ses cinq panneaux
//! (`src/ui::screen::WORKER_FIELDS`) et ses dix boutons (`HUT_ACTIONS`). Ce module
//! ajoute ce qu'aucune table statique ne peut porter : **l'état que la fenêtre tient
//! entre deux images** et **les transitions d'un clic**.
//!
//! Deux états, et un seul est vivant :
//!
//! | État | Type | Écrit par |
//! | --- | --- | --- |
//! | la priorité de livraison | [`Priority`] | `prio = buildingView.getBuildingDmPrio()` (`:78`), puis `±1` par clic |
//! | la ligne de chaque ouvrier | [`Row`] | `onOpened`, depuis les modules (`:166-174`) |
//!
//! # Ce que Java fait et qui se voit dans ce fichier
//!
//! 1. **Le `+` envoie même quand il n'incrémente pas.** Le garde est `if (prio != 10)`
//!    (`:115-118`) mais l'envoi est **hors** du garde (`:119`) : à 10, un clic envoie
//!    `ChangeDeliveryPriorityMessage(building, true)` et le serveur reçoit une demande
//!    d'augmentation qu'il ignore. [`Step::moved`] et [`Step::message`] sont deux champs
//!    distincts pour cette raison.
//! 2. **Le test est `== 10`, pas `>= 10`.** Une valeur hors bornes — relue d'un composé
//!    modifié à la main, `setPrio 12` par exemple — n'est jamais bloquée : elle monte à
//!    13, 14, et le libellé se met à écrire `13/10`.
//! 3. **Le zéro est testé, pas borné.** `prio == 0` écrit le mot « never »
//!    (`:100-104`) ; `crate::ui::field::priority_text` répond « never » pour **tout**
//!    négatif, ce qui est plus sûr que Java et pourtant différent : `Priority::text`
//!    garde le test de Java (`== 0`) pour que le libellé dise ce que Java dit.
//! 4. **La ligne de l'ouvrier n'est pas effacée quand le citoyen manque.** Le panneau
//!    d'une ligne de `ScrollingList` est **recyclé** ; l'écriture étant dans un
//!    `if (worker != null)` (`:192-197`), une ligne dont le citoyen a disparu garde le
//!    texte de la ligne précédente. [`row_text`] rend donc [`RowText::Kept`], pas du vide.
//! 5. **L'embauche ne teste pas le niveau.** `hireClicked` demande
//!    `buildingView.allowsAssignment()` (`:145-152`), un booléen **synchronisé**
//!    (`AbstractBuildingView.java:448`, `:744-747`), pas `getBuildingLevel() != 0`.
//!    `src/ui` nomme ce refus `NotBuilt` ; c'est une approximation que la ligne Java
//!    dément, et [`hire`] prend donc le drapeau, jamais le niveau.

use crate::ui::action::{Action, Effect, Refusal as UiRefusal};
use crate::ui::field::{PriorityText, PRIORITY_MAX};
use crate::ui::screen::{HIRE_ID, HUT_WORKER_ID, Screen};

/// La classe (`WindowHutWorkerModulePlaceholder.java:14`).
pub const JAVA: &str = "WindowHutWorkerModulePlaceholder";
/// La classe qui porte tout le comportement (`AbstractWindowWorkerModuleBuilding.java:33`).
pub const BASE: &str = "AbstractWindowWorkerModuleBuilding";
/// Le XML (`WindowHutWorkerModulePlaceholder.java:23`).
pub const RESOURCE: &str = "minecolonies:gui/windowhutworkerplaceholder.xml";
/// Le `layout` que ce XML inclut — une ligne, et c'est tout le corps
/// (`windowhutworkerplaceholder.xml:3`).
pub const LAYOUT: &str = "minecolonies:gui/layouthuts/layouthutpageactions.xml";
/// L'écran de `src/ui` que cette fenêtre est : la table n'est pas recopiée.
pub const SCREEN: &str = HUT_WORKER_ID;
/// La liste des ouvriers (`:43`).
pub const LIST: &str = "workers";
/// Le nom dans une ligne (`:58`).
pub const NAME: &str = "workerName";
/// Le panneau de la priorité (`:53`).
pub const PRIO: &str = "prioValue";

/// La classe du message d'une augmentation ou d'une diminution (`:119`, `:129`).
pub const PRIORITY_MESSAGE: &str = "ChangeDeliveryPriorityMessage";
/// La classe du message de rappel (`:159`).
pub const RECALL_MESSAGE: &str = "RecallCitizenMessage";
/// La classe du message de ramassage forcé (`:135`).
pub const PICKUP_MESSAGE: &str = "ForcePickupMessage";

/// L'écran de `src/ui`, pour qui veut ses panneaux sans passer par cette table.
pub fn screen() -> Option<&'static Screen> {
    crate::ui::table::screen_by_id(SCREEN).ok()
}

/// L'action de `src/ui` avec cet id de bouton.
pub fn action(id: &str) -> Option<Action> {
    screen().and_then(|screen| screen.action(id))
}

/// La priorité de livraison, telle que la fenêtre la tient.
///
/// Java garde un `int` et rien d'autre (`:78`). La valeur initiale vient du **serveur**
/// (`getBuildingDmPrio`), donc elle n'est pas bornée ici : voir la note 2 du module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Priority {
    value: i32,
}

/// Ce qu'un clic sur `+` ou `-` a fait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    /// La valeur avant le clic.
    pub before: i32,
    /// La valeur après — **égale** à `before` quand le garde a refusé de bouger.
    pub after: i32,
    /// La classe du message envoyé, qui l'est dans les deux cas (`:119`, `:129`).
    pub message: &'static str,
}

impl Step {
    /// Vrai quand la valeur affichée change.
    pub fn moved(&self) -> bool {
        self.after != self.before
    }

    /// Vrai quand Java envoie au serveur — toujours, et c'est la note 1 du module.
    pub fn sends(&self) -> bool {
        !self.message.is_empty()
    }
}

impl Priority {
    /// La valeur lue à l'ouverture (`:78`).
    pub const fn from_building(dm_prio: i32) -> Priority {
        Priority { value: dm_prio }
    }

    /// La valeur courante.
    pub const fn value(&self) -> i32 {
        self.value
    }

    // `if (prio != 10) { prio++; }` puis l'envoi, hors du garde
    // (`AbstractWindowWorkerModuleBuilding.java:115-119`).
    /// Un cran de plus.
    pub fn up(&mut self) -> Step {
        let before = self.value;
        if self.value != PRIORITY_MAX {
            self.value += 1;
        }
        Step { before, after: self.value, message: PRIORITY_MESSAGE }
    }

    // `if (prio != 0) { prio--; }` puis l'envoi, hors du garde (`:125-129`).
    /// Un cran de moins.
    pub fn down(&mut self) -> Step {
        let before = self.value;
        if self.value != 0 {
            self.value -= 1;
        }
        Step { before, after: self.value, message: PRIORITY_MESSAGE }
    }

    // `if (prio == 0)` → le mot « never », sinon `prio + "/10"`
    // (`AbstractWindowWorkerModuleBuilding.java:97-111`).
    /// Le libellé du panneau [`PRIO`], en deux composants comme Java.
    pub fn text(&self) -> PriorityText {
        if self.value == 0 {
            PriorityText::Never
        } else {
            PriorityText::OfTen(self.value)
        }
    }
}

/// Une ligne de [`LIST`] : un métier et l'id d'un citoyen.
///
/// Java construit une `List<Tuple<String, Integer>>` où la chaîne est la **clé de
/// traduction du métier**, pas son nom (`:166-174`) : c'est le module qui sait quel
/// métier il embauche, la fenêtre ne le nomme jamais.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    /// La clé de traduction du métier, lue sur le module (`:172`).
    pub job: &'static str,
    /// L'id du citoyen assigné (`:170`).
    pub citizen: i32,
}

impl Row {
    /// `job: nom` — la concaténation de Java (`:195`).
    pub fn text(&self, name: &str) -> String {
        format!("{}: {name}", self.job)
    }

    /// L'infobulle : `nom (id)` (`:196`).
    pub fn tooltip(&self, name: &str) -> String {
        format!("{name} ({})", self.citizen)
    }
}

/// Ce qu'une ligne reçoit quand on la remplit.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: Java écrit ou n'écrit pas, il n'y a pas de troisième cas
pub enum RowText {
    /// Le texte et son infobulle.
    Written { text: String, tooltip: String },
    /// Le citoyen n'est pas dans la colonie : **rien n'est écrit**, et le panneau
    /// recyclé garde la ligne précédente (`:192-197`).
    Kept,
}

/// Toutes les lignes de [`LIST`], dans l'ordre où Java les empile.
///
/// `modules` est la liste des vues de module de la hutte, chacune avec sa clé de métier
/// et ses citoyens assignés (`WorkerBuildingModuleView.getAssignedCitizens`, `:168-174`).
/// Une hutte peut en porter plusieurs : la boucle de Java est **imbriquée**, et l'ordre
/// des lignes suit donc celui des modules de `getModuleViews`.
pub fn rows(modules: &[(&'static str, &[i32])]) -> Vec<Row> {
    let mut rows = Vec::new();
    for (job, citizens) in modules {
        for citizen in *citizens {
            rows.push(Row { job, citizen: *citizen });
        }
    }
    rows
}

/// Le contenu d'une ligne, une fois le nom du citoyen connu.
pub fn row_text(row: &Row, name: Option<&str>) -> RowText {
    match name {
        Some(name) => RowText::Written { text: row.text(name), tooltip: row.tooltip(name) },
        None => RowText::Kept,
    }
}

/// Le garde de `hireClicked` (`AbstractWindowWorkerModuleBuilding.java:145-152`).
///
/// Prend `allowsAssignment()` et **pas** le niveau : voir la note 5 du module.
pub fn hire(assignable: bool) -> Result<Effect, UiRefusal> {
    if !assignable {
        return Err(UiRefusal::NotBuilt);
    }
    Ok(Effect::Opens(HIRE_ID))
}

/// `Lire` : Java n'envoie un rappel que si un ouvrier est assigné
/// (`:157-160`) — la condition est côté serveur, donc la fenêtre envoie toujours.
pub const fn recall() -> Effect {
    Effect::Sends(RECALL_MESSAGE)
}

/// `Ramasser` : le ramassage forcé, sans condition côté fenêtre (`:133-136`).
pub const fn force_pickup() -> Effect {
    Effect::Sends(PICKUP_MESSAGE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::field::PriorityText;

    #[test]
    fn the_priority_starts_from_the_server_and_moves_one_step_at_a_time() {
        let mut prio = Priority::from_building(4);
        assert_eq!(prio.value(), 4);
        let up = prio.up();
        assert_eq!((up.before, up.after), (4, 5));
        assert!(up.moved());
        assert!(up.sends());
        assert_eq!(up.message, "ChangeDeliveryPriorityMessage");
        let down = prio.down();
        assert_eq!((down.before, down.after), (5, 4));
        assert!(down.moved());
    }

    #[test]
    fn the_ten_sends_and_does_not_move_and_the_zero_does_the_same_downwards() {
        // `if (prio != 10) prio++;` puis `sendToServer(...)` hors du garde : à 10 le
        // message part quand même (`AbstractWindowWorkerModuleBuilding.java:115-119`).
        let mut top = Priority::from_building(10);
        let step = top.up();
        assert!(!step.moved());
        assert!(step.sends());
        assert_eq!(step.after, 10);
        let mut floor = Priority::from_building(0);
        let step = floor.down();
        assert!(!step.moved());
        assert!(step.sends());
        assert_eq!(step.after, 0);
        // Deux clics de plus ne descendent pas sous zéro.
        floor.down();
        assert_eq!(floor.value(), 0);
    }

    #[test]
    fn a_value_out_of_range_is_never_clamped_because_java_tests_equality() {
        // `!= 10` et non `>= 10` : une valeur montée à 12 continue de monter.
        let mut over = Priority::from_building(12);
        assert_eq!(over.up().after, 13);
        assert_eq!(over.value(), 13);
        assert_eq!(over.text(), PriorityText::OfTen(13));
        assert_eq!(over.text().ratio().as_deref(), Some("13/10"));
        // Et une valeur négative descend encore : le garde compare à zéro.
        let mut under = Priority::from_building(-3);
        assert_eq!(under.down().after, -4);
    }

    #[test]
    fn zero_reads_never_and_the_negative_does_not_in_this_window() {
        assert_eq!(Priority::from_building(0).text(), PriorityText::Never);
        assert_eq!(Priority::from_building(0).text().ratio(), None);
        assert_eq!(Priority::from_building(1).text().ratio().as_deref(), Some("1/10"));
        // Divergence assumée avec `ui::field::priority_text`, qui répond Never pour
        // tout négatif : ici c'est `== 0`, la ligne de Java.
        assert_eq!(crate::ui::field::priority_text(-1), PriorityText::Never);
        assert_eq!(Priority::from_building(-1).text(), PriorityText::OfTen(-1));
    }

    #[test]
    fn a_row_reads_the_job_key_then_the_name_and_its_tooltip_carries_the_id() {
        let row = Row { job: "com.minecolonies.job.farmer", citizen: 42 };
        assert_eq!(row.text("Alfred"), "com.minecolonies.job.farmer: Alfred");
        assert_eq!(row.tooltip("Alfred"), "Alfred (42)");
        // La clé n'est pas traduite par la fenêtre : c'est le module qui la donne.
        assert!(row.text("Alfred").starts_with(row.job));
    }

    #[test]
    fn a_missing_citizen_leaves_the_row_alone_instead_of_blanking_it() {
        let row = Row { job: "job", citizen: 1 };
        assert_eq!(
            row_text(&row, Some("Bob")),
            RowText::Written {
                text: "job: Bob".to_owned(),
                tooltip: "Bob (1)".to_owned()
            }
        );
        // `if (worker != null)` : le panneau recyclé garde la ligne précédente.
        assert_eq!(row_text(&row, None), RowText::Kept);
    }

    #[test]
    fn the_rows_follow_the_modules_then_the_citizens_of_each_module() {
        let modules: &[(&'static str, &[i32])] = &[("job.a", &[3, 1]), ("job.b", &[9])];
        let list = rows(modules);
        assert_eq!(list.len(), 3, "la boucle de Java est imbriquée (:168-174)");
        assert_eq!(list[0], Row { job: "job.a", citizen: 3 });
        assert_eq!(list[1], Row { job: "job.a", citizen: 1 });
        assert_eq!(list[2], Row { job: "job.b", citizen: 9 });
        // Aucun module : aucune ligne, et pas une ligne vide.
        assert!(rows(&[]).is_empty());
        assert!(rows(&[("job.a", &[])]).is_empty());
    }

    #[test]
    fn the_hire_button_takes_the_synced_flag_and_not_the_level() {
        // `allowsAssignment()` est un booléen de la vue (`AbstractBuildingView.java:448`).
        assert_eq!(hire(true), Ok(Effect::Opens(HIRE_ID)));
        assert_eq!(hire(false), Err(UiRefusal::NotBuilt));
        assert_eq!(UiRefusal::NotBuilt.key(), Some("com.minecolonies.coremod.gui.workerhuts.level0"));
        // Aucune ligne de Java ne teste le niveau : un appelant qui n'a que le niveau
        // ne peut pas répondre.
        assert_eq!(recall(), Effect::Sends("RecallCitizenMessage"));
        assert_eq!(force_pickup(), Effect::Sends("ForcePickupMessage"));
    }

    #[test]
    fn the_screen_is_src_uis_and_carries_its_ten_buttons() {
        let screen = screen().expect("hut.worker est un écran de src/ui");
        assert_eq!(screen.window, JAVA);
        assert_eq!(screen.resource, RESOURCE);
        assert_eq!(screen.actions.len(), 10);
        // Les dix boutons sont ceux de `src/ui` : ce module n'en ajoute aucun.
        assert_eq!(action("hire").map(|action| action.effect), Some(Effect::Opens(HIRE_ID)));
        assert_eq!(
            action("deliveryPrioUp").map(|action| action.effect),
            Some(Effect::Sends(PRIORITY_MESSAGE))
        );
        assert_eq!(action("forcePickup").map(|action| action.effect), Some(Effect::Sends(PICKUP_MESSAGE)));
        assert_eq!(action("recall").map(|action| action.effect), Some(Effect::Sends(RECALL_MESSAGE)));
        assert_eq!(action("nowhere"), None);
        // Le panneau de la priorité est celui de `src/ui`, pas un second.
        assert!(screen.field(PRIO).is_some());
        assert!(screen.field(LIST).is_some());
        assert!(screen.field(NAME).is_some());
    }
}
