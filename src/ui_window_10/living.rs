//! La maison (et la taverne) : les résidents, la ligne « n/max », et les deux boutons.
//!
//! Java reference : `core/client/gui/huts/WindowHutLiving.java` (125 lignes),
//! `assets/minecolonies/gui/windowhuthome.xml:4-19`,
//! `core/colony/buildings/views/LivingBuildingView.java:40-43` (`getResidents`) et
//! `:69-72` (`getMax`), `core/colony/buildings/moduleviews/LivingBuildingModuleView.java:106-109`,
//! `api/util/constant/TranslationConstants.java:75` (`LEVEL_0`) et `:861`
//! (`LABEL_HOUSE_ASSIGNED_CITIZENS`), `api/util/constant/WindowConstants.java:67`
//! (`BUTTON_RECALL`).
//!
//! `src/ui` porte déjà la fenêtre (`ui::screen::HUT_LIVING_ID`) : ses cinq panneaux, ses
//! sept boutons et la maison **et** la taverne (`HomeBuildingModule.java:44`,
//! `TavernBuildingModule.java:439`). Ce fichier n'en reprend rien — il porte l'état, le
//! texte d'une ligne et les deux transitions.
//!
//! # Le seul couple « chiffres » des quatre écrans, et ce n'est pas une progression
//!
//! La ligne `assignedlabel` est écrite avec **deux arguments**, les résidents et les
//! places (`:108`) : c'est le seul endroit des quatre fenêtres de cette tranche qui
//! affiche un rapport entre deux nombres. Ce n'est **pas** une progression de
//! construction et **pas** un stock : c'est un remplissage. `src/ui` la classe
//! [`crate::ui::Value::Occupancy`] pour cette raison, et la clé est
//! [`crate::ui::OCCUPANCY_KEY`].
//!
//! Le nombre de résidents est relu **de la même liste** que les lignes (`:108` et `:86`
//! lisent tous deux `getResidents()`), donc les deux ne peuvent pas diverger. C'est
//! pourquoi [`Home::occupancy`] compte `self.residents` au lieu de tenir un compteur :
//! un compteur séparé serait un second état à garder d'accord.
//!
//! # Le texte d'une ligne n'est pas un nom
//!
//! `:95` écrit `(job.isEmpty() ? "" : translatable(job) + ": ") + name`. La ligne porte
//! donc **le métier et le nom**, et un résident sans métier n'a que son nom. `src/ui`
//! déclare ce panneau [`crate::ui::Value::Name`] ; c'est [`crate::ui::Value::Text`] qui
//! serait exact, et ce module ne réécrit pas la table pour autant — il donne
//! [`row_text`], la composition que Java fait, et le note ici.
//!
//! Deux autres choses que le source dit et qu'on n'attend pas :
//!
//! - **un résident introuvable ne vide pas la liste.** `getCitizen(id)` peut rendre
//!   `null` et Java n'écrit alors **rien** dans la ligne (`:92-96`) : la ligne reste,
//!   vide. Le port refuse par un nom ([`Refusal::UnknownResident`]) au lieu de dessiner
//!   une ligne muette, parce qu'une ligne vide est indiscernable d'un nom vide.
//! - **`recall` ne teste rien.** Il envoie `RecallCitizenHutMessage` sans regarder le
//!   niveau, les résidents ni l'état de la colonie (`:71-74`). Seul `assign` a une
//!   condition (`:115-124`).

use crate::colony::Citizen;
use crate::ui::{screen::ASSIGN_ID, Effect, OCCUPANCY_KEY};

use super::refusal::Refusal;

/// La classe Java.
pub const JAVA: &str = "WindowHutLiving";
/// Le XML (`:60`).
pub const RESOURCE: &str = "minecolonies:gui/windowhuthome.xml";

/// Le bouton d'assignation (`:31`).
pub const BUTTON_ASSIGN: &str = "assign";
/// Le bouton de rappel (`:63` ; l'id vient de `WindowConstants.java:67`,
/// `BUTTON_RECALL`).
pub const BUTTON_RECALL: &str = "recall";
/// La ligne « n/max » (`:36`).
pub const ASSIGNED_LABEL: &str = "assignedlabel";
/// La liste des résidents (`:41`).
pub const LIST_CITIZEN: &str = "assignedCitizen";
/// Le panneau de nom dans une ligne (`:95`).
pub const ROW_NAME: &str = "name";

/// Le message que `recall` envoie (`:73`) : rien n'est envoyé dans ce port, le nom est
/// tout ce qu'il y a.
pub const RECALL_MESSAGE: &str = "RecallCitizenHutMessage";
/// La fenêtre que `assign` ouvre quand la maison est construite (`:123`).
pub const ASSIGN_JAVA: &str = "WindowAssignCitizen";

/// Un résident tel qu'une ligne le lit : le nom, que le port tient déjà dans
/// [`crate::colony::Citizen`], et **le métier, que cette structure ne porte pas**.
///
/// `crate::colony::Citizen` s'arrête exprès au nom et à l'identifiant (« Java's
/// `ICitizenData` carries a job, a family, a happiness; none of that belongs to the
/// colony record »). La fenêtre, elle, a besoin du métier pour composer sa ligne, donc il
/// est ajouté ici plutôt que dans l'enregistrement de colonie.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Resident {
    /// Le citoyen : son id et son nom.
    pub citizen: Citizen,
    /// La clé du métier (`getJob()`, `ICitizenDataView.java:36`), ou `None` pour un
    /// citoyen sans emploi — Java teste `getJob().isEmpty()` (`:95`).
    pub job: Option<String>,
}

impl Resident {
    /// Un résident, avec ou sans métier.
    pub fn new(citizen: Citizen, job: Option<&str>) -> Resident {
        Resident { citizen, job: job.map(str::to_owned) }
    }

    /// Le texte de sa ligne (`:95`).
    pub fn text(&self) -> String {
        row_text(&self.citizen.name, self.job.as_deref())
    }
}

/// Le texte d'une ligne de résident, exactement comme Java le compose (`:95`).
///
/// Le métier est une **clé de langue** et Java la résout (`translatable(job).getString()`)
/// ; le port n'a pas la table de langue, donc la clé est rendue telle quelle. Un métier
/// vide vaut pas de métier — c'est le `isEmpty()` de Java.
pub fn row_text(name: &str, job: Option<&str>) -> String {
    match job {
        Some(job) if !job.is_empty() => format!("{job}: {name}"),
        _ => name.to_owned(),
    }
}

/// La ligne « n/max » (`:108`) : la clé et ses deux arguments, pas une phrase.
///
/// Java appelle `Component.translatable(LABEL_HOUSE_ASSIGNED_CITIZENS, residents, max)` :
/// une clé et deux nombres. Le port rend la même chose et laisse l'affichage à un
/// appelant qui a la table de langue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: two numbers, and the label is Java's own key
pub struct Occupancy {
    /// `buildingView.getResidents().size()`.
    pub residents: i32,
    /// `buildingView.getMax()`.
    pub places: i32,
}

impl Occupancy {
    /// La clé de la ligne ([`OCCUPANCY_KEY`], `TranslationConstants.java:861`).
    pub fn key(self) -> &'static str {
        OCCUPANCY_KEY
    }

    /// Les deux arguments, dans l'ordre où Java les passe.
    pub fn arguments(self) -> [i32; 2] {
        [self.residents, self.places]
    }

    /// La maison est-elle pleine — la condition que `WindowAssignCitizen` teste pour
    /// éteindre son bouton (`ui::action::may_assign`, `WindowAssignCitizen.java:105`).
    pub fn is_full(self) -> bool {
        self.residents >= self.places
    }
}

/// Une ligne de la liste, quand le citoyen a été trouvé.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value
pub struct Row {
    /// L'identifiant du résident (`getResidents().get(index)`).
    pub id: i32,
    /// Le texte que Java écrit dans `name` (`:95`).
    pub text: String,
}

/// La maison ou la taverne, ouverte.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own value, not a `mod_api` type
pub struct Home {
    /// `getBuildingLevel()` (`:117`).
    pub level: i32,
    /// `getResidents()` (`:86`) : des identifiants, dans l'ordre de la liste.
    pub residents: Vec<i32>,
    /// `getMax()` (`:108`), c'est-à-dire `maxInhabitants`.
    pub places: i32,
}

impl Home {
    /// L'état que la fenêtre lit à l'ouverture.
    pub fn initial(level: i32, residents: Vec<i32>, places: i32) -> Home {
        Home { level, residents, places }
    }

    /// La ligne « n/max » (`:108`).
    pub fn occupancy(&self) -> Occupancy {
        Occupancy { residents: self.residents.len() as i32, places: self.places }
    }

    /// Les lignes de la liste, une par résident, dans l'ordre de la liste.
    ///
    /// Un identifiant que `lookup` ne connaît pas rend [`Refusal::UnknownResident`] au
    /// lieu d'une ligne vide : Java n'écrit rien dans ce cas (`:92-96`), et le port dit
    /// lequel manque plutôt que de dessiner un blanc indiscernable.
    pub fn rows(&self, lookup: impl Fn(i32) -> Option<Resident>) -> Vec<Result<Row, Refusal>> {
        self.residents
            .iter()
            .map(|id| match lookup(*id) {
                Some(resident) => Ok(Row { id: *id, text: resident.text() }),
                None => Err(Refusal::UnknownResident { id: *id }),
            })
            .collect()
    }

    /// Le clic sur `assign` (`:115-124`) : le seul bouton des quatre fenêtres qui teste
    /// le niveau.
    ///
    /// À zéro, Java envoie `COM_MINECOLONIES_COREMOD_GUI_WORKERHUTS_LEVEL_0` au joueur et
    /// s'arrête (`:117-121`) ; sinon, il ouvre `WindowAssignCitizen` (`:123`).
    ///
    /// **Une précision sur les refus de `src/ui`** : la table donne deux refus à ce
    /// bouton, [`crate::ui::Refusal::NotBuilt`] et
    /// [`crate::ui::Refusal::HousingHandledElsewhere`]. Seul le premier appartient à
    /// cette fenêtre ; le second est levé par `WindowAssignCitizen`
    /// (`:301-317`), l'écran d'après. [`Home::assign`] ne peut donc rendre que
    /// `NotBuilt`, et ce n'est pas une divergence.
    pub fn assign(&self) -> Result<Effect, Refusal> {
        if self.level == 0 {
            return Err(Refusal::Port(crate::ui::Refusal::NotBuilt));
        }
        Ok(Effect::Opens(ASSIGN_ID))
    }

    /// Le clic sur `recall` (`:71-74`) : toujours le même message, jamais refusé.
    pub fn recall(&self) -> Effect {
        Effect::Sends(RECALL_MESSAGE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{screen_by_id, Refusal as UiRefusal, Value};

    fn home(residents: &[i32], places: i32) -> Home {
        Home::initial(3, residents.to_vec(), places)
    }

    fn citizen(id: i32, name: &str) -> Citizen {
        Citizen::new(id, name)
    }

    #[test]
    fn the_occupancy_pair_is_what_the_label_shows_and_it_is_not_a_build_progress() {
        let full = home(&[1, 2, 3], 3);
        assert_eq!(full.occupancy(), Occupancy { residents: 3, places: 3 });
        assert_eq!(full.occupancy().key(), OCCUPANCY_KEY);
        assert_eq!(full.occupancy().arguments(), [3, 3]);
        assert!(full.occupancy().is_full());
        let room = home(&[1], 5);
        assert_eq!(room.occupancy().arguments(), [1, 5]);
        assert!(!room.occupancy().is_full());
        // La ligne est un remplissage : `src/ui` la classe Occupancy, pas Progress.
        let screen = screen_by_id(crate::ui::screen::HUT_LIVING_ID)
            .unwrap_or_else(|refusal| panic!("{refusal}"));
        assert!(screen.shows(Value::Occupancy));
        assert!(!screen.shows(Value::Progress), "aucune maison n'affiche une progression");
        assert!(!screen.shows(Value::Stock), "ni un stock");
        assert_eq!(
            screen.field(ASSIGNED_LABEL).map(|field| field.value),
            Some(Value::Occupancy)
        );
        // Le compte vient de la même liste que les lignes, jamais d'un second champ.
        assert_eq!(full.occupancy().residents as usize, full.residents.len());
    }

    #[test]
    fn a_row_reads_job_colon_name_and_a_jobless_resident_reads_bare_name() {
        // :95 — `(job.isEmpty() ? "" : translatable(job) + ": ") + name`.
        assert_eq!(
            row_text("Anna", Some("com.minecolonies.job.builder")),
            "com.minecolonies.job.builder: Anna"
        );
        assert_eq!(row_text("Anna", None), "Anna");
        assert_eq!(row_text("Anna", Some("")), "Anna", "isEmpty() vaut pas de métier");
        let resident = Resident::new(citizen(7, "Anna"), Some("com.minecolonies.job.farmer"));
        assert_eq!(resident.text(), "com.minecolonies.job.farmer: Anna");
        // L'ordre de la liste est celui de `getResidents()`, sans tri.
        let village = home(&[9, 3, 5], 8);
        let rows = village
            .rows(|id| Some(Resident::new(citizen(id, &format!("n{id}")), None)));
        let ids: Vec<i32> = rows
            .iter()
            .filter_map(|row| row.as_ref().ok().map(|row| row.id))
            .collect();
        assert_eq!(ids, [9, 3, 5]);
        assert_eq!(row_text("n9", None), "n9");
    }

    #[test]
    fn an_unknown_resident_is_refused_by_name_instead_of_drawn_blank() {
        let village = home(&[1, 2, 3], 6);
        let rows = village.rows(|id| {
            if id == 2 {
                // `home.getColony().getCitizen(2)` a rendu `null` (:92-96).
                None
            } else {
                Some(Resident::new(citizen(id, &format!("n{id}")), None))
            }
        });
        assert_eq!(rows.len(), 3, "Java garde la ligne, il n'écrit rien dedans");
        assert!(rows[0].is_ok());
        assert_eq!(rows[1], Err(Refusal::UnknownResident { id: 2 }));
        assert!(rows[2].is_ok());
        let refusal = rows[1].as_ref().err().copied().unwrap_or_else(|| panic!("refus"));
        assert_eq!(refusal.name(), "ui10.refusal.unknown_resident");
        assert_eq!(refusal.key(), None, "le joueur ne voit rien, la ligne est vide");
        // Une liste vide ne produit aucune ligne et ne refuse rien.
        assert!(Home::initial(0, Vec::new(), 0).rows(|_| None).is_empty());
    }

    #[test]
    fn assign_refuses_an_unbuilt_home_with_javas_level_zero_key() {
        let unbuilt = Home::initial(0, Vec::new(), 0);
        let refusal = unbuilt
            .assign()
            .err()
            .unwrap_or_else(|| panic!("une maison de niveau 0 refuse l'assignation"));
        assert_eq!(refusal, Refusal::Port(UiRefusal::NotBuilt));
        // Le seul refus des quatre fenêtres qui parle au joueur (:117-121).
        assert_eq!(
            refusal.key(),
            Some("com.minecolonies.coremod.gui.workerhuts.level0")
        );
        // Construite, elle ouvre la fenêtre d'assignation — le même id que `src/ui`.
        assert_eq!(home(&[], 4).assign(), Ok(Effect::Opens(ASSIGN_ID)));
        let screen = screen_by_id(crate::ui::screen::HUT_LIVING_ID)
            .unwrap_or_else(|refusal| panic!("{refusal}"));
        assert_eq!(
            screen.action(BUTTON_ASSIGN).map(|action| action.effect),
            Some(Effect::Opens(ASSIGN_ID))
        );
        // Et le niveau n'est pas une `Gate` de l'écran : c'est une condition d'action.
        assert_eq!(screen.gate, None);
    }

    #[test]
    fn recall_sends_the_huts_message_and_never_refuses() {
        // :71-74 — aucun test de niveau, de résident ni d'état.
        for level in [0, 1, 5] {
            assert_eq!(
                Home::initial(level, Vec::new(), 0).recall(),
                Effect::Sends(RECALL_MESSAGE)
            );
        }
        assert_eq!(home(&[1, 2], 2).recall(), Effect::Sends("RecallCitizenHutMessage"));
        // La jointure : `src/ui` envoie la même classe pour ce bouton-là.
        let screen = screen_by_id(crate::ui::screen::HUT_LIVING_ID)
            .unwrap_or_else(|refusal| panic!("{refusal}"));
        assert_eq!(
            screen.action(BUTTON_RECALL).map(|action| action.effect),
            Some(Effect::Sends(RECALL_MESSAGE))
        );
        // Le rappel de la hutte d'un worker est une **autre** classe : ne pas les confondre.
        let worker = screen_by_id(crate::ui::screen::HUT_WORKER_ID)
            .unwrap_or_else(|refusal| panic!("{refusal}"));
        assert_eq!(
            worker.action("recall").map(|action| action.effect),
            Some(Effect::Sends("RecallCitizenMessage"))
        );
        assert_ne!(RECALL_MESSAGE, "RecallCitizenMessage");
    }
}
