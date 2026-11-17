//! La page des citoyens : le filtre, la selection, la joie agregee.
//!
//! Java reference : `core/client/gui/townhall/WindowCitizenPage.java` (297
//! lignes) et `gui/townhall/layoutcitizens.xml`.
//!
//! **Ce que la page demande, jamais ce qu'elle dessine.** Aucune des valeurs
//! ci-dessous n'existe dans [`crate::colony::Citizen`], qui ne porte qu'un id et
//! un nom : la vue est fournie par l'appelant, qui la remplit depuis
//! `crate::citizens` (metier, besoins) et `crate::citizens::happiness`.

use crate::citizens::happiness::as_int;
use crate::jobs::Job;

use super::refusal::Refusal;

/// Le denominateur de la joie d'un citoyen : **dix**, ecrit en dur dans la page
/// (`WindowCitizenPage.java:143`).
pub const HAPPINESS_CEILING: f64 = 10.0;

/// Le denominateur de la saturation : **vingt**, ecrit en dur lui aussi
/// (`:144`).
///
/// Ce n'est pas le maximum de la saturation : [`crate::citizens::MAX_SATURATION`]
/// vaut **soixante**. La page affiche donc `60/20` pour un citoyen repu. C'est
/// l'incoherence de Java, et elle est gardee : le denominateur de l'etiquette
/// n'est pas celui du modele.
pub const SATURATION_CEILING: f64 = 20.0;

/// Un citoyen tel que la page le lit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vue<'a> {
    pub name: &'a str,
    /// `getJob()`, `None` pour un sans-emploi (`:140`).
    pub job: Option<Job>,
    pub health: f32,
    pub max_health: f32,
    pub happiness: f64,
    pub saturation: f64,
    /// `isFemale()` (`:131`).
    pub female: bool,
    /// `getPose() == Pose.SLEEPING` (`:147`).
    pub sleeping: bool,
}

impl<'a> Vue<'a> {
    /// Le texte du metier, ou `None` pour un sans-emploi.
    ///
    /// Java affiche `getJobComponent()`, un composant traduit ; le port n'a pas
    /// le libelle du sans-emploi (`MANQUANT.md` §3), donc il ne rend rien
    /// plutot que d'inventer une phrase.
    pub fn job_text(&self) -> Option<&'static str> {
        self.job.map(|job| job.display())
    }

    /// Le filtre de la case de recherche (`:103-104`) : le nom **ou** le metier,
    /// sans la casse.
    ///
    /// Java abaisse avec `Locale.US` pour echapper au defaut turc ; Rust abaisse
    /// sans locale, ce qui est la meme chose pour les noms de citoyens.
    pub fn matches(&self, filter: &str) -> bool {
        if filter.is_empty() {
            return true;
        }
        let needle = filter.to_lowercase();
        self.name.to_lowercase().contains(&needle)
            || self.job_text().is_some_and(|job| job.to_lowercase().contains(&needle))
    }
}

/// Les citoyens qui passent le filtre, tries par nom (`:106`).
pub fn filtered<'a, 'b>(citizens: &'b [Vue<'a>], filter: &str) -> Vec<&'b Vue<'a>> {
    let mut kept: Vec<&Vue<'a>> =
        citizens.iter().filter(|citizen| citizen.matches(filter)).collect();
    kept.sort_by(|left, right| left.name.cmp(right.name));
    kept
}

/// Ce que la page a de selectionne.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: zero or one citizen, as Java's nullable field
pub enum Selection {
    None,
    At(usize),
}

impl Selection {
    /// L'etat initial : le premier citoyen de la liste **non filtree**, s'il y en
    /// a un (`:71-74`).
    pub fn initial(count: usize) -> Selection {
        if count == 0 {
            Selection::None
        } else {
            Selection::At(0)
        }
    }

    /// Le clic sur une ligne (`:113-119`). Java lit la ligne dans la liste
    /// **filtree** et indexe `citizens`, qui l'est aussi : les deux listes sont
    /// la meme, donc l'indice est bon.
    pub fn select(self, index: usize, count: usize) -> Result<Selection, Refusal> {
        if index >= count {
            return Err(Refusal::CitizenNotInList { index });
        }
        Ok(Selection::At(index))
    }

    /// L'indice selectionne.
    pub fn index(self) -> Option<usize> {
        match self {
            Selection::None => None,
            Selection::At(index) => Some(index),
        }
    }

    /// Le bouton `recallone` (`:160-167`) : Java sort sans rien dire quand rien
    /// n'est selectionne.
    pub fn recall(self) -> Result<&'static str, Refusal> {
        match self {
            Selection::None => Err(Refusal::NoCitizenSelected),
            Selection::At(_) => Ok("RecallSingleCitizenMessage"),
        }
    }

    /// Vrai quand le bouton de cette ligne est eteint : celui de la ligne
    /// selectionnee (`:200-207`).
    pub fn dims(self, row: usize) -> bool {
        self.index() == Some(row)
    }
}

/// La tete d'une ligne de la liste de joie (`:255-270`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: four branches in Java, and no fifth
pub enum Face {
    Happy,
    Satisfied,
    Unsatisfied,
    Unhappy,
}

impl Face {
    /// L'icone de cette tete (`WindowConstants.java:599-602`).
    pub fn icon(self) -> &'static str {
        match self {
            Face::Happy => "minecolonies:textures/gui/happy_icon.png",
            Face::Satisfied => "minecolonies:textures/gui/satisfied_icon.png",
            Face::Unsatisfied => "minecolonies:textures/gui/unsatisfied_icon.png",
            Face::Unhappy => "minecolonies:textures/gui/unhappy_icon.png",
        }
    }
}

/// La tete que vaut un facteur moyen (`:255-270`).
///
/// L'ordre des tests est celui de Java — `> 1`, `== 1`, `> 0.75`, sinon — et un
/// `NaN` ne passe aucun des trois premiers, donc il tombe sur
/// [`Face::Unhappy`], comme dans Java.
pub fn face(value: f64) -> Face {
    if value > 1.0 {
        Face::Happy
    } else if value == 1.0 {
        Face::Satisfied
    } else if value > 0.75 {
        Face::Unsatisfied
    } else {
        Face::Unhappy
    }
}

/// La valeur d'une ligne : la somme des facteurs du modificateur, divisee par
/// le nombre de citoyens (`:249`).
///
/// `None` quand la colonie n'a personne : Java diviserait par zero et
/// dessinerait la tete d'un `NaN`. La liste est vide dans ce cas, donc la
/// branche n'est pas atteignable depuis l'ecran — elle l'est depuis
/// l'arithmetique, et c'est pourquoi elle a un nom.
pub fn row_value(sum: f64, citizens: usize) -> Option<f64> {
    if citizens == 0 {
        None
    } else {
        Some(sum / citizens as f64)
    }
}

/// Le nombre du titre de la joie (`:228-232`) : `DecimalFormat("#.#")` arrondi
/// au **plafond**, donc `5.51` s'ecrit `5.6` et `6.0` s'ecrit `6`.
pub fn title_number(overall: f64) -> String {
    if overall.is_nan() {
        // DecimalFormat ecrit "NaN".
        return "NaN".to_owned();
    }
    if !overall.is_finite() {
        return "\u{221e}".to_owned();
    }
    let tenths = (overall * 10.0).ceil() / 10.0;
    if tenths.fract() == 0.0 {
        format!("{}", tenths as i64)
    } else {
        format!("{tenths:.1}")
    }
}

/// L'icone de l'entite est-elle visible ?
///
/// **Deux conditions opposees dans Java.** `fillCitizenInfo` la **montre**
/// quand l'entite dort (`:147-152`) ; `onUpdate` la **cache** dans ce cas et la
/// montre sinon (`:281-289`). `onUpdate` tourne a chaque tick, donc c'est lui
/// qui gagne : l'icone est visible quand le citoyen **ne** dort pas. La
/// contradiction est de Java, pas du port.
pub fn entity_icon_visible(sleeping: bool) -> bool {
    !sleeping
}

/// L'etiquette courte de la vie : `(int)health + "/" + (int)maxHealth`
/// (`:142`).
pub fn short_health(health: f32, max_health: f32) -> String {
    format!("{}/{}", as_int(health as f64), as_int(max_health as f64))
}

/// L'etiquette courte de la joie, sur [`HAPPINESS_CEILING`] (`:143`).
pub fn short_happiness(happiness: f64) -> String {
    format!("{}/{}", as_int(happiness), HAPPINESS_CEILING as i32)
}

/// L'etiquette courte de la saturation, sur [`SATURATION_CEILING`] (`:144`).
pub fn short_saturation(saturation: f64) -> String {
    format!("{}/{}", as_int(saturation), SATURATION_CEILING as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vue(name: &'static str, job: Option<Job>) -> Vue<'static> {
        Vue {
            name,
            job,
            health: 20.0,
            max_health: 20.0,
            happiness: 7.0,
            saturation: 60.0,
            female: false,
            sleeping: false,
        }
    }

    #[test]
    fn the_filter_reads_the_name_and_the_job_and_keeps_the_names_ordered() {
        let people = [
            vue("Zoe", Some(Job::Builder)),
            vue("Ada", None),
            vue("Bea", Some(Job::Baker)),
        ];
        let all = filtered(&people, "");
        let names: Vec<&str> = all.iter().map(|citizen| citizen.name).collect();
        assert_eq!(names, ["Ada", "Bea", "Zoe"], "WindowCitizenPage.java:106 sorts by name");
        let bakers = filtered(&people, "BAK");
        assert_eq!(bakers.len(), 1);
        assert_eq!(bakers[0].name, "Bea");
        // The name matches too, and the search is case-blind.
        assert_eq!(filtered(&people, "a").len(), 2, "Zoe is neither named nor jobbed with an a");
        assert_eq!(filtered(&people, "zz").len(), 0);
        // Nobody jobless has a label the port can print.
        assert_eq!(people[1].job_text(), None);
    }

    #[test]
    fn the_first_citizen_starts_selected_and_the_recall_names_what_it_sends() {
        assert_eq!(Selection::initial(0), Selection::None);
        assert_eq!(Selection::initial(3), Selection::At(0));
        assert_eq!(Selection::None.recall(), Err(Refusal::NoCitizenSelected));
        assert_eq!(
            Selection::At(0).recall(),
            Ok("RecallSingleCitizenMessage"),
            "WindowCitizenPage.java:166"
        );
        assert_eq!(Selection::At(0).select(2, 3), Ok(Selection::At(2)));
        assert_eq!(Selection::At(0).select(3, 3), Err(Refusal::CitizenNotInList { index: 3 }));
        // The selected row's own button is off, and only that one.
        assert!(Selection::At(1).dims(1));
        assert!(!Selection::At(1).dims(0));
        assert!(!Selection::None.dims(0));
    }

    #[test]
    fn the_three_short_labels_truncate_and_the_saturation_denominator_is_twenty() {
        assert_eq!(short_health(17.9, 20.0), "17/20");
        assert_eq!(short_happiness(7.9), "7/10");
        // Sixty of saturation still prints over twenty: the label is not the model.
        assert_eq!(short_saturation(60.0), "60/20");
        assert_ne!(SATURATION_CEILING, crate::citizens::MAX_SATURATION);
        assert_eq!(HAPPINESS_CEILING, 10.0);
    }

    #[test]
    fn the_faces_are_read_in_javas_order_and_a_nan_falls_on_the_unhappy_one() {
        assert_eq!(face(1.01), Face::Happy);
        assert_eq!(face(1.0), Face::Satisfied);
        assert_eq!(face(0.9), Face::Unsatisfied);
        assert_eq!(face(0.75), Face::Unhappy);
        assert_eq!(face(0.0), Face::Unhappy);
        assert_eq!(face(f64::NAN), Face::Unhappy);
        // Four faces, four icons.
        let icons = [Face::Happy, Face::Satisfied, Face::Unsatisfied, Face::Unhappy];
        for (index, icon) in icons.iter().enumerate() {
            assert!(icon.icon().ends_with("_icon.png"));
            for other in &icons[index + 1..] {
                assert_ne!(icon.icon(), other.icon());
            }
        }
    }

    #[test]
    fn a_happiness_row_is_a_mean_and_a_colony_of_nobody_has_no_value() {
        assert_eq!(row_value(9.0, 3), Some(3.0));
        assert_eq!(row_value(0.0, 3), Some(0.0));
        assert_eq!(row_value(1.5, 0), None);
        // 1.5 / 2 = 0.75 exactly, and `WindowCitizenPage.java:263` tests
        // `value > 0.75` — the bound is strict, so 0.75 falls to the last branch
        // and draws UNSATISFIED's neighbour, UNHAPPY (`:267-270`).
        assert_eq!(face(row_value(1.5, 2).expect("two citizens")), Face::Unhappy);
        // One notch higher (1.6 / 2 = 0.8) is the unsatisfied face.
        assert_eq!(face(row_value(1.6, 2).expect("two citizens")), Face::Unsatisfied);
        // The title rounds up and drops a trailing zero.
        assert_eq!(title_number(5.51), "5.6");
        assert_eq!(title_number(5.5), "5.5");
        assert_eq!(title_number(6.0), "6");
        assert_eq!(title_number(0.0), "0");
        assert_eq!(title_number(f64::NAN), "NaN");
        assert!(
            !entity_icon_visible(true),
            "onUpdate wins: WindowCitizenPage.java:281-289"
        );
        assert!(entity_icon_visible(false));
    }
}
