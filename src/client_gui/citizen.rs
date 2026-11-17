//! Les **sept** fenetres d'un citoyen : ses onglets, ses trois barres, ses competences, sa
//! famille et ses requetes.
//!
//! Java reference: `core/client/gui/citizen/` — `AbstractWindowCitizen.java` (87 lignes,
//! les onglets), `MainWindowCitizen.java` (119, la fiche), `JobWindowCitizen.java` (22),
//! `HappinessWindowCitizen.java` (31), `FamilyWindowCitizen.java` (107),
//! `RequestWindowCitizen.java` (238) et `CitizenWindowUtils.java` (540, les barres et les
//! competences).
//!
//! **Ce module porte des regles, pas des panneaux.** Les identifiants de panneaux
//! (`WINDOW_ID_NAME`, `STATUS_ICON`, `WINDOW_ID_HEALTHBAR`) sont ceux de
//! `api/util/constant/WindowConstants.java` et ne sont cites qu'avec la regle qui les
//! remplit ; le dessin — l'icone de coeur, sa position, sa taille — reste dans les XML et
//! `TRANCHE.md` §3 M2 dit deja qu'il n'est pas porte.
//!
//! # Ce que la fiche d'un citoyen demande, et qui n'est pas un champ de `src/ui`
//!
//! | Ce que Java affiche | Ou il le lit | Ici |
//! | --- | --- | --- |
//! | nom, genre, statut | `MainWindowCitizen.java:80`, `:89-92`, `:41-53` | [`Headline`] |
//! | barre de vie | `CitizenWindowUtils.java:147-206` | [`hearts`] |
//! | barre de saturation | `:257-300` | [`saturation_icons`] |
//! | barre de bonheur | `:302-355` | [`happiness_icons`] |
//! | onze competences | `:385-405`, `Skill.java:10-150` | [`skill_pane_id`], [`skills_editable`] |
//! | famille | `FamilyWindowCitizen.java:40-107` | [`Parent`], [`partner_line`] |
//! | requetes ouvertes | `RequestWindowCitizen.java:121-146` | [`request_tokens`] |
//!
//! # Cinq choses que la source dit et qui ne se devinent pas
//!
//! 1. **L'onglet metier n'existe pas toujours.** Il est cache quand le lieu de travail
//!    n'est pas un `AbstractBuildingView` — un citoyen sans metier, ou un visiteur — **et**
//!    quand le batiment est une bibliotheque (`AbstractWindowCitizen.java:70-87`) : les
//!    etudiants n'ont pas de page de metier. L'onglet de debogage n'apparait que pour un
//!    joueur qui a le debogage actif (`:70-75`).
//! 2. **La barre de vie change de couleur selon la vie, et le libelle ne le dit pas.** Les
//!    coeurs se remplissent du plus haut palier au plus bas — bleu, vert, or, rouge
//!    (`:163-168`) — et chaque icone coute deux points : le rouge remplit jusqu'a 20, l'or
//!    mord a 21 (par sa moitie) et jusqu'a 41, le vert de 42 a 61, le bleu des 62. A 40
//!    points la barre est donc **pleine de coeurs or**, ce qui se lit comme un debut de
//!    barre alors qu'elle est pleine. Et a 19 points le libelle ecrit `9` alors que dix
//!    icones sont posees (neuf pleines et une moitie) : [`hearts`] rend les deux.
//! 3. **Le bonheur est tronque avant d'etre double.** `(int) citizen.getHappiness() * 2`
//!    (`:304`) : le cast s'applique au bonheur, pas au produit, et une valeur de 0.9
//!    s'affiche comme un bonheur de zero.
//! 4. **La saturation a trois echelles differentes** : la barre dessine
//!    `MAX_SATURATION / 6 = 10` icones (`:261`), la fiche de l'hotel de ville ecrit
//!    `/20` en dur (`WindowCitizenPage.java:147`) et la fiche du citoyen n'ecrit aucun
//!    nombre. Aucune des trois ne s'accorde avec les deux autres.
//! 5. **Le bouton de competence ne marche qu'en creatif** : les boutons `+`/`-` sont
//!    desactives hors creatif (`:388-397`), mais la fenetre envoie quand meme
//!    `AdjustSkillCitizenMessage` si un clic passe (`MainWindowCitizen.java:104-118`).

/// Le nombre d'icones des trois barres (`WindowConstants.java:760`).
pub const MAX_ICONS: i32 = 10;

/// La saturation maximale d'un citoyen (`api/colony/ICitizenData.java:30`) : **la
/// constante est celle de `citizens::needs`**, pas une seconde definition.
pub use crate::citizens::MAX_SATURATION;

/// Ce qu'une icone de saturation vaut, en points : six (`CitizenWindowUtils.java:263`,
/// `:271`).
pub const SATURATION_PER_ICON: f64 = 6.0;

/// La valeur d'une icone pleine de vie (`WindowConstants.java:770` : le rouge vaut deux).
pub const HEART_VALUE: i32 = 2;

/// Les quatre paliers de la barre de vie, dans l'ordre ou Java les consomme
/// (`CitizenWindowUtils.java:163-168`).
pub const HEART_TIERS: [HeartTier; 4] = [
    HeartTier { value: 8, previous: 6 },
    HeartTier { value: 6, previous: 4 },
    HeartTier { value: 4, previous: 2 },
    HeartTier { value: 2, previous: 0 },
];

/// Un palier de la barre de vie : sa valeur et celle du palier precedent
/// (`WindowConstants.java:770-785`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeartTier {
    /// `hpValue` du palier.
    pub value: i32,
    /// `prevHeart.hpValue` — la valeur d'une icone du palier precedent.
    pub previous: i32,
}

/// Ce que la fiche d'un citoyen affiche en tete.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : trois champs, et un quatrieme serait une nouvelle ligne
pub struct Headline {
    /// Le nom, ecrit tel quel (`MainWindowCitizen.java:80`).
    pub name: &'static str,
    /// L'icone de genre : `true` pose l'image femelle (`:89-92`).
    pub female: bool,
    /// Le statut visible, ou `None` : l'icone est alors **cachee**
    /// (`:41-47`).
    pub status: Option<&'static str>,
}

/// Les onglets d'une fenetre de citoyen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : sept onglets, un par `registerButton` de AbstractWindowCitizen
pub enum Tab {
    /// Toujours la (`AbstractWindowCitizen.java:41-43`).
    Main,
    /// Toujours la (`:44-47`).
    Request,
    /// Toujours la — et c'est un **message**, pas une fenetre (`:49-52`).
    Inventory,
    /// Toujours la (`:54-59`).
    Happiness,
    /// Toujours la (`:61-66`).
    Family,
    /// Seulement si le joueur a le debogage actif (`:70-75`).
    Debug,
    /// Seulement si le lieu de travail est un batiment et n'est pas la bibliotheque
    /// (`:77-96`).
    Job,
}

impl Tab {
    /// Les cinq onglets toujours presents, dans l'ordre de Java.
    pub const ALWAYS: [Tab; 5] =
        [Tab::Main, Tab::Request, Tab::Inventory, Tab::Happiness, Tab::Family];

    /// La cle de l'onglet, sur le modele de `registerButton("mainTab", …)`.
    pub fn id(self) -> &'static str {
        match self {
            Tab::Main => "mainTab",
            Tab::Request => "requestTab",
            Tab::Inventory => "inventoryTab",
            Tab::Happiness => "happinessTab",
            Tab::Family => "familyTab",
            Tab::Debug => "debugTab",
            Tab::Job => "jobTab",
        }
    }

    /// La cle de son icone, qui enregistre le **meme** clic (`:38-39`, `:45-46`…).
    pub fn icon(self) -> &'static str {
        match self {
            Tab::Main => "mainIcon",
            Tab::Request => "requestIcon",
            Tab::Inventory => "inventoryIcon",
            Tab::Happiness => "happinessIcon",
            Tab::Family => "familyIcon",
            Tab::Debug => "debugIcon",
            Tab::Job => "jobIcon",
        }
    }

    /// La cle de l'infobulle de l'icone (`com.minecolonies.coremod.gui.citizen.<x>`).
    pub fn tip(self) -> &'static str {
        match self {
            Tab::Main => "com.minecolonies.coremod.gui.citizen.main",
            Tab::Request => "com.minecolonies.coremod.gui.citizen.requests",
            Tab::Inventory => "com.minecolonies.coremod.gui.citizen.inventory",
            Tab::Happiness => "com.minecolonies.coremod.gui.citizen.happiness",
            Tab::Family => "com.minecolonies.coremod.gui.citizen.family",
            Tab::Debug => "com.minecolonies.coremod.debug.gui.tabicon",
            Tab::Job => "com.minecolonies.coremod.gui.citizen.job",
        }
    }

    /// La classe de fenetre que le clic ouvre, `""` pour l'onglet qui n'en ouvre aucune
    /// (`:49-51` : l'inventaire part en message, pas en fenetre).
    pub fn window_class(self) -> &'static str {
        match self {
            Tab::Main => "MainWindowCitizen",
            Tab::Request => "RequestWindowCitizen",
            Tab::Inventory => "",
            Tab::Happiness => "HappinessWindowCitizen",
            Tab::Family => "FamilyWindowCitizen",
            Tab::Debug => "DebugWindowCitizen",
            Tab::Job => "JobWindowCitizen",
        }
    }

    /// Ce que fait un clic.
    pub fn effect(self) -> Effect {
        match self {
            Tab::Inventory => Effect::Sends("OpenInventoryMessage"),
            _ => Effect::Opens(self.window_class()),
        }
    }
}

/// Ce qu'un onglet fait : la meme forme que `crate::ui::action::Effect`, en plus petit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : deux effets seulement parmi ces sept onglets
pub enum Effect {
    /// Ouvre la fenetre de cet onglet.
    Opens(&'static str),
    /// Envoie un message au serveur, par nom de classe.
    Sends(&'static str),
}

/// Les onglets d'un citoyen, dans l'ordre de Java.
///
/// Le metier vient en dernier et seulement si ses deux conditions tiennent
/// (`AbstractWindowCitizen.java:70-87` : `building instanceof AbstractBuildingView &&
/// building.getBuildingType() != ModBuildings.library.get()`).
pub fn tabs(workplace_is_building: bool, workplace_is_library: bool, debug: bool) -> Vec<Tab> {
    let mut tabs = Tab::ALWAYS.to_vec();
    if debug {
        tabs.push(Tab::Debug);
    }
    if workplace_is_building && !workplace_is_library {
        tabs.push(Tab::Job);
    }
    tabs
}

/// Les icones pleines d'une barre de vie, sa derniere icone a moitie, et le libelle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hearts {
    /// Le nombre d'icones occupees : pleines, plus la moitie s'il y en a une.
    pub filled: i32,
    /// La **derniere** icone occupee est-elle une moitie ?
    pub half: bool,
    /// Le libelle, `health / 2` en division entiere (`CitizenWindowUtils.java:148`).
    pub label: i32,
    /// L'indice dans [`HEART_TIERS`] du palier qui a pose la **premiere** icone, donc la
    /// couleur de la barre. `None` pour une vie nulle : la barre reste vide.
    pub tier: Option<usize>,
}

/// La barre de vie, palier par palier (`CitizenWindowUtils.java:147-206`).
///
/// Java dessine d'abord dix icones vides (`:153-156`), puis remplit **du plus haut
/// palier au plus bas** — bleu, vert, or, rouge (`:163-168`) — chaque icone coutant la
/// difference entre son palier et le precedent. Une icone d'un palier n'est posee que si
/// le reste depasse `precedent * 10 + 1` (`:177`), et la boucle s'arrete des que les dix
/// icones sont prises (`:194-197`).
///
/// **Deux consequences que le libelle cache.** Le nombre d'icones ne dit pas la vie : a 19
/// points la barre affiche neuf coeurs rouges **et une moitie**, soit dix icones, alors que
/// le libelle ecrit `9`. Et la couleur depend de la vie : le rouge remplit jusqu'a 20,
/// l'or mord a 21 par sa moitie puis remplit de 22 a 41, le vert de 42 a 61, le bleu des
/// 62 — donc une barre **pleine** de coeurs or a 40 points se lit comme un debut de barre.
///
/// **Et une barre peut melanger deux couleurs** : la moitie d'un palier obeit a
/// `reste > precedent * 10` **sans le `+ 1`** (`:185`), donc a 21 points la barre pose une
/// moitie **or** puis remplit le reste en rouge. [`Hearts::tier`] dit la couleur de la
/// premiere icone, pas celle du palier theorique.
pub fn hearts(health: i32) -> Hearts {
    let label = health / 2;
    let mut left = health;
    let mut filled = 0;
    let mut half = false;
    let mut tier = None;
    for (index, step) in HEART_TIERS.iter().enumerate() {
        half = false;
        while filled < MAX_ICONS && left > step.previous * MAX_ICONS + 1 {
            left -= step.value - step.previous;
            filled += 1;
            if tier.is_none() {
                tier = Some(index);
            }
        }
        // La moitie : `health % 2 == 1 && health > prev * 10`, et son icone prend la
        // meme place que l'icone precedente (`:185-192`).
        if left % 2 == 1 && filled < MAX_ICONS && left > step.previous * MAX_ICONS {
            left -= 1;
            filled += 1;
            half = true;
            if tier.is_none() {
                tier = Some(index);
            }
        }
        if filled >= MAX_ICONS {
            break;
        }
    }
    Hearts { filled, half, label, tier }
}

/// Les icones de saturation : pleines, a moitie, vides (`CitizenWindowUtils.java:257-299`).
///
/// Trois choses comptent : il y a **toujours** dix icones vides dessinees avant les
/// pleines (`:263-272`), une icone pleine vaut six points (`:273`), et la moitie
/// s'affiche des que le reste n'est pas entier (`:286`).
pub fn saturation_icons(saturation: f64) -> (i32, bool, i32) {
    let empty = (MAX_SATURATION / SATURATION_PER_ICON) as i32;
    let full = (saturation / SATURATION_PER_ICON) as i32;
    let half = (saturation / SATURATION_PER_ICON) % 1.0 > 0.0;
    (full, half, empty)
}

/// Les smileys de bonheur : pleins et a moitie (`CitizenWindowUtils.java:302-355`).
///
/// `happiness` est deja `(int) getHappiness() * 2` (`:304`), une valeur de 0 a 20. Le
/// premier palier (rouge) se remplit tant que le reste depasse `0 * 10 + 1`, donc a partir
/// de deux points.
pub fn happiness_icons(happiness: i32) -> (i32, bool) {
    let mut filled = 0;
    let mut left = happiness;
    while filled < MAX_ICONS && left > 1 {
        filled += 1;
        left -= HEART_VALUE;
    }
    (filled, left % 2 == 1)
}

/// La valeur que la fiche passe a [`happiness_icons`] : `(int) happiness * 2`
/// (`CitizenWindowUtils.java:304`).
pub fn happiness_bar_value(happiness: f64) -> i32 {
    (happiness as i32) * 2
}

/// L'identifiant du panneau d'une competence : le nom du metier en minuscules
/// (`CitizenWindowUtils.java:391`).
pub fn skill_pane_id(skill: &str) -> String {
    skill.to_lowercase()
}

/// L'identifiant du groupe de boutons d'une competence : le panneau plus `"_bts"`
/// (`CitizenWindowUtils.java:393-396`).
pub fn skill_buttons_pane_id(skill: &str) -> String {
    format!("{}_bts", skill_pane_id(skill))
}

/// Les boutons d'une competence sont actifs **en creatif seulement**
/// (`CitizenWindowUtils.java:387`, `:396`).
pub fn skills_editable(creative: bool) -> bool {
    creative
}

/// Les deux parents d'un citoyen, tels que la fenetre les ecrit
/// (`FamilyWindowCitizen.java:40-44`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : deux branches, nom ou inconnu
pub enum Parent {
    /// Le nom, ecrit tel quel.
    Named(&'static str),
    /// `com.minecolonies.coremod.gui.citizen.family.unknown`.
    Unknown,
}

/// La cle du parent inconnu (`FamilyWindowCitizen.java:43`, `:44`).
pub const PARENT_UNKNOWN_KEY: &str = "com.minecolonies.coremod.gui.citizen.family.unknown";

/// Ce que la fiche famille ecrit pour un parent vide (`:40-44`).
pub fn parent(name: &'static str) -> Parent {
    if name.is_empty() {
        Parent::Unknown
    } else {
        Parent::Named(name)
    }
}

/// Ce que la fiche famille ecrit pour un conjoint (`:46-57`).
///
/// **`"-"` quand le citoyen n'est pas dans la colonie**, pas le mot "inconnu" : Java teste
/// le resultat de `colony.getCitizen(partner)` et ecrit un tiret quand il est nul.
pub fn partner_line(name: Option<&'static str>) -> Option<&'static str> {
    name.filter(|name| !name.is_empty())
}

/// La cle des requetes ouvertes d'un citoyen, et celle des requetes de la hutte
/// (`RequestWindowCitizen.java:129`, `:141`).
///
/// Java lit **deux** entrees de la meme table : l'identifiant du citoyen, et la cle `-1`
/// qui porte les requetes adressees au batiment entier. La liste rendue est la
/// concatenation des deux, dans cet ordre.
pub const BUILDING_REQUEST_KEY: i32 = -1;

/// Les jetons de requete d'un citoyen, dans l'ordre de Java
/// (`RequestWindowCitizen.java:121-146`).
///
/// `_citizen` est l'identifiant qui sert de **cle** a la premiere lecture
/// (`:129`, `getOrDefault(citizenDataView.getId(), …)`) ; il n'apparait pas dans le
/// resultat, qui est une liste de jetons.
pub fn request_tokens(_citizen: i32, own: &[i32], building: &[i32]) -> Vec<i32> {
    let mut tokens = Vec::with_capacity(own.len() + building.len());
    tokens.extend_from_slice(own);
    tokens.extend_from_slice(building);
    tokens
}

/// Les cinq dernieres cases de l'inventaire du joueur ne sont pas prises pour une requete
/// (`RequestWindowCitizen.java:205` : `inventory.getContainerSize() - 5`, commente
/// "4 armour slots + 1 shield slot").
pub const EQUIPPED_SLOTS: usize = 5;

/// La premiere case ou Java accepte de prendre la pile : tout ce qui n'est pas equipe
/// (`RequestWindowCitizen.java:204-215`).
///
/// Rend `None` quand seules des cases equipees portent la pile — le cas ou Java envoie
/// `COM_MINECOLONIES_CANT_TAKE_EQUIPPED` au joueur.
pub fn fulfil_slot(inventory_size: usize, matching: &[usize]) -> Option<usize> {
    let usable = inventory_size.saturating_sub(EQUIPPED_SLOTS);
    matching.iter().copied().find(|slot| *slot < usable)
}

/// La quantite envoyee quand le joueur donne une pile
/// (`RequestWindowCitizen.java:234` : `isCreative ? amount : Math.min(amount, count)`).
pub fn fulfil_quantity(wanted: i32, held: i32, creative: bool) -> i32 {
    if creative {
        wanted
    } else {
        wanted.min(held)
    }
}

/// Si une requete peut etre remplie depuis cette fenetre
/// (`RequestWindowCitizen.java:156-176`).
///
/// Quatre conditions : la requete est livrable, elle n'a pas de parent ailleurs, elle est
/// dans le cache des requetes ouvertes, et le joueur a la pile (ou est en creatif).
pub fn fulfillable(
    deliverable: bool,
    has_foreign_parent: bool,
    cached: bool,
    has_item: bool,
    creative: bool,
) -> bool {
    deliverable && !has_foreign_parent && cached && (creative || has_item)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_always_tabs_are_five_and_the_two_conditional_ones_are_last() {
        assert_eq!(Tab::ALWAYS.len(), 5);
        // Sans lieu de travail, la liste est exactement les cinq onglets sans condition
        // (`AbstractWindowCitizen.java:41-66` ; le metier est saute a `:70-87`).
        assert_eq!(tabs(false, false, false), Tab::ALWAYS);
        // Avec un lieu de travail qui est un batiment et pas la bibliotheque, le metier
        // s'ajoute **en dernier** (`:72-77`).
        let mut with_job = Tab::ALWAYS.to_vec();
        with_job.push(Tab::Job);
        assert_eq!(tabs(true, false, false), with_job);
        // Le metier n'apparait que si le lieu de travail est un batiment non-bibliotheque.
        assert!(tabs(false, false, false).iter().all(|tab| *tab != Tab::Job));
        assert!(tabs(true, true, false).iter().all(|tab| *tab != Tab::Job));
        assert_eq!(tabs(true, false, false).last(), Some(&Tab::Job));
        // Le debogage vient avant le metier, et seulement s'il est actif.
        assert_eq!(tabs(true, false, true).last(), Some(&Tab::Job));
        assert_eq!(tabs(true, false, true)[5], Tab::Debug);
        assert_eq!(
            tabs(false, false, true),
            [Tab::Main, Tab::Request, Tab::Inventory, Tab::Happiness, Tab::Family, Tab::Debug]
        );
    }

    #[test]
    fn every_tab_has_an_id_an_icon_and_a_tip_and_no_two_are_alike() {
        let all = [
            Tab::Main,
            Tab::Request,
            Tab::Inventory,
            Tab::Happiness,
            Tab::Family,
            Tab::Debug,
            Tab::Job,
        ];
        for (index, tab) in all.iter().enumerate() {
            assert!(tab.id().ends_with("Tab"), "{}", tab.id());
            assert!(tab.icon().ends_with("Icon"), "{}", tab.icon());
            assert!(tab.tip().starts_with("com.minecolonies."), "{}", tab.tip());
            for other in &all[index + 1..] {
                assert_ne!(tab.id(), other.id());
                assert_ne!(tab.icon(), other.icon());
                assert_ne!(tab.tip(), other.tip());
            }
        }
    }

    #[test]
    fn the_inventory_tab_sends_a_message_and_the_others_open_their_own_window() {
        assert_eq!(Tab::Inventory.effect(), Effect::Sends("OpenInventoryMessage"));
        assert_eq!(Tab::Inventory.window_class(), "");
        // Le debogage ouvre une fenetre de debogage, pas la fiche.
        assert_eq!(Tab::Debug.effect(), Effect::Opens("DebugWindowCitizen"));
        // Les cinq autres ouvrent la classe qui porte leur nom.
        assert_eq!(Tab::Main.effect(), Effect::Opens("MainWindowCitizen"));
        assert_eq!(Tab::Request.effect(), Effect::Opens("RequestWindowCitizen"));
        assert_eq!(Tab::Happiness.effect(), Effect::Opens("HappinessWindowCitizen"));
        assert_eq!(Tab::Family.effect(), Effect::Opens("FamilyWindowCitizen"));
        assert_eq!(Tab::Job.effect(), Effect::Opens("JobWindowCitizen"));
        // Aucun onglet n'ouvre la fenetre d'un autre. L'inventaire n'en ouvre aucune : il
        // envoie `OpenInventoryMessage` (`AbstractWindowCitizen.java:49-51`), donc il est
        // le seul dont la classe est vide et il sort de ce compte.
        let mut classes: Vec<&str> = Vec::new();
        for tab in Tab::ALWAYS {
            if !tab.window_class().is_empty() {
                classes.push(tab.window_class());
            }
        }
        classes.push(Tab::Debug.window_class());
        classes.push(Tab::Job.window_class());
        assert_eq!(classes.len(), 6, "six fenetres pour sept onglets");
        let opening = [Tab::Main, Tab::Request, Tab::Inventory, Tab::Happiness, Tab::Family]
            .iter()
            .filter(|tab| !tab.window_class().is_empty())
            .count();
        assert_eq!(opening, 4, "sur les cinq onglets inconditionnels, quatre ouvrent");
        for (index, class) in classes.iter().enumerate() {
            assert!(class.ends_with("WindowCitizen"), "{class}");
            for other in &classes[index + 1..] {
                assert_ne!(class, other);
            }
        }
    }

    #[test]
    fn the_health_label_is_health_divided_by_two_and_not_rounded() {
        assert_eq!(hearts(20).label, 10);
        assert_eq!(hearts(21).label, 10, "21 / 2 vaut 10 en division entiere");
        assert_eq!(hearts(1).label, 0);
        assert_eq!(hearts(19).label, 9);
        assert_eq!(hearts(0).label, 0);
    }

    #[test]
    fn twenty_points_fill_ten_red_hearts_and_nothing_else() {
        let full = hearts(20);
        assert_eq!((full.filled, full.half), (MAX_ICONS, false));
        // Dix-huit points : neuf coeurs pleins et rien de plus.
        let nine = hearts(18);
        assert_eq!((nine.filled, nine.half), (9, false), "18 est pair");
        // Deux points : un coeur.
        assert_eq!(hearts(2).filled, 1);
        // Un point : la moitie du premier palier exige `> 0`, et le coeur plein `> 1`,
        // donc une moitie seule apparait.
        assert_eq!((hearts(1).filled, hearts(1).half), (1, true));
        assert_eq!((hearts(0).filled, hearts(0).half), (0, false));
    }

    #[test]
    fn nineteen_points_put_ten_icons_on_the_bar_and_nine_in_the_label() {
        // Le desaccord que la fenetre ne montre pas : `health / 2` d'un cote, les icones
        // de l'autre.
        let nineteen = hearts(19);
        assert_eq!(nineteen.label, 9);
        assert_eq!((nineteen.filled, nineteen.half), (MAX_ICONS, true));
        assert_ne!(i32::from(nineteen.label), nineteen.filled);
        // Et vingt points donnent le meme nombre d'icones avec un libelle different.
        assert_eq!(hearts(20).filled, hearts(19).filled);
        assert_ne!(hearts(20).label, hearts(19).label);
    }

    #[test]
    fn the_bar_colour_follows_the_health_and_a_full_bar_can_be_gold() {
        // Le rouge ne mord qu'en dessous de 21, l'or de 22 a 41, le vert de 42 a 61, le
        // bleu au-dela (`CitizenWindowUtils.java:177` : `> previous * 10 + 1`).
        assert_eq!(hearts(20).tier, Some(3), "vingt points : rouge");
        assert_eq!(hearts(40).tier, Some(2), "quarante points : OR, et la barre est pleine");
        assert_eq!(hearts(60).tier, Some(1), "soixante points : vert");
        assert_eq!(hearts(100).tier, Some(0), "cent points : bleu");
        // Chaque palier remplit la barre a dix icones : la couleur seule change.
        for health in [20, 40, 60, 100] {
            assert_eq!(hearts(health).filled, MAX_ICONS, "vie {health}");
        }
        // Une vie nulle laisse la barre vide et sans couleur.
        assert_eq!(hearts(0).tier, None);
        assert_eq!(HEART_TIERS.len(), 4);
        // Le rouge vaut une icone a deux points, comme `RED_HEART_VALUE`.
        assert_eq!(HEART_TIERS[3].value, HEART_VALUE);
        assert_eq!(HEART_TIERS[0].previous * MAX_ICONS + 1, 61);
    }

    #[test]
    fn twenty_one_points_mix_a_gold_half_heart_with_red_ones() {
        // La moitie d'un palier obeit a `reste > prev * 10` **sans** le `+ 1`
        // (`CitizenWindowUtils.java:185`) : a 21, l'or pose sa moitie avant le rouge.
        let mixed = hearts(21);
        assert_eq!(mixed.tier, Some(2), "la premiere icone est or");
        assert_eq!(mixed.filled, MAX_ICONS);
        assert!(!mixed.half, "la derniere icone est un rouge plein");
        assert_eq!(mixed.label, 10);
        // A 22, l'or a le droit a ses icones pleines.
        assert_eq!(hearts(22).tier, Some(2));
        assert!(!hearts(22).half);
    }

    #[test]
    fn saturation_draws_ten_icons_and_one_full_icon_per_six_points() {
        let (full, half, empty) = saturation_icons(0.0);
        assert_eq!((full, half, empty), (0, false, 10));
        assert_eq!(saturation_icons(6.0), (1, false, 10));
        assert_eq!(saturation_icons(30.0), (5, false, 10));
        assert_eq!(saturation_icons(MAX_SATURATION), (10, false, 10));
        // Une fraction de six allume la moitie.
        assert_eq!(saturation_icons(7.0), (1, true, 10));
        assert_eq!(saturation_icons(0.5), (0, true, 10));
        assert_eq!(saturation_icons(60.5).0, 10, "au-dela du maximum, dix icones quand meme");
    }

    #[test]
    fn happiness_is_truncated_before_it_is_doubled() {
        // `(int) happiness * 2` : 0.9 devient 0 puis 0.
        assert_eq!(happiness_bar_value(0.9), 0);
        assert_eq!(happiness_bar_value(1.0), 2);
        assert_eq!(happiness_bar_value(4.9), 8);
        assert_eq!(happiness_bar_value(10.0), 20);
        // Et la barre suit la valeur doublee.
        assert_eq!(happiness_icons(happiness_bar_value(0.9)), (0, false));
        assert_eq!(happiness_icons(2).0, 1);
        assert_eq!(happiness_icons(20).0, MAX_ICONS);
    }

    #[test]
    fn a_happiness_of_one_point_leaves_the_bar_empty() {
        // Le premier palier exige `> 0 * 10 + 1`, donc deux points.
        assert_eq!(happiness_icons(1), (0, true), "un point reste une moitie");
        assert_eq!(happiness_icons(0), (0, false));
        assert_eq!(happiness_icons(2), (1, false));
        // Dix points : cinq smileys.
        assert_eq!(happiness_icons(10).0, 5);
    }

    #[test]
    fn a_skill_pane_is_the_lowercased_skill_and_its_buttons_are_the_pane_plus_bts() {
        assert_eq!(skill_pane_id("Strength"), "strength");
        assert_eq!(skill_buttons_pane_id("Strength"), "strength_bts");
        assert_eq!(skill_pane_id("Creativity"), "creativity");
        // Les deux sont distincts, sinon le panneau de texte serait ecrase par les boutons.
        assert_ne!(skill_pane_id("Agility"), skill_buttons_pane_id("Agility"));
        assert!(skills_editable(true));
        assert!(!skills_editable(false), "hors creatif les boutons sont eteints");
    }

    #[test]
    fn an_empty_parent_name_reads_unknown_and_not_a_blank() {
        assert_eq!(parent("Anna"), Parent::Named("Anna"));
        assert_eq!(parent(""), Parent::Unknown);
        assert_ne!(Parent::Unknown, Parent::Named("Anna"));
        assert!(PARENT_UNKNOWN_KEY.starts_with("com.minecolonies.coremod.gui.citizen.family."));
    }

    #[test]
    fn a_missing_partner_is_a_dash_and_a_present_one_keeps_its_name() {
        assert_eq!(partner_line(Some("Bob")), Some("Bob"));
        assert_eq!(partner_line(None), None);
        assert_eq!(partner_line(Some("")), None, "un nom vide n'est pas un conjoint");
    }

    #[test]
    fn the_open_requests_are_the_citizens_then_the_buildings_own() {
        let own = [11, 12];
        let building = [99];
        assert_eq!(request_tokens(7, &own, &building), [11, 12, 99]);
        // Le citoyen sans requete garde celles du batiment.
        assert_eq!(request_tokens(7, &[], &building), [99]);
        assert_eq!(request_tokens(7, &own, &[]), [11, 12]);
        assert!(request_tokens(7, &[], &[]).is_empty());
        assert_eq!(BUILDING_REQUEST_KEY, -1);
    }

    #[test]
    fn a_fulfilment_never_takes_a_piece_of_armour() {
        // Taille 36 : les cinq dernieres cases sont equipees, donc 31 sont utilisables.
        assert_eq!(fulfil_slot(36, &[0, 5, 30]), Some(0));
        assert_eq!(fulfil_slot(36, &[31]), None, "31 est la premiere case equipee");
        assert_eq!(fulfil_slot(36, &[35]), None);
        assert_eq!(fulfil_slot(36, &[30]), Some(30));
        // Un inventaire plus petit que cinq cases n'a aucune case utilisable.
        assert_eq!(fulfil_slot(4, &[0]), None);
        assert_eq!(EQUIPPED_SLOTS, 5);
    }

    #[test]
    fn the_quantity_sent_is_the_wanted_amount_in_creative_and_the_held_one_otherwise() {
        assert_eq!(fulfil_quantity(16, 3, false), 3);
        assert_eq!(fulfil_quantity(16, 64, false), 16);
        assert_eq!(fulfil_quantity(16, 3, true), 16);
        assert_eq!(fulfil_quantity(16, 64, true), 16);
        assert_eq!(fulfil_quantity(16, 0, false), 0, "rien en main, rien envoye");
    }

    #[test]
    fn a_request_is_fulfillable_only_if_it_is_live_local_and_in_hand() {
        assert!(fulfillable(true, false, true, true, false));
        assert!(fulfillable(true, false, true, false, true), "le creatif se passe de l'item");
        // Quatre refus, un par condition.
        assert!(!fulfillable(false, false, true, true, true), "pas livrable");
        assert!(!fulfillable(true, true, true, true, true), "parent ailleurs");
        assert!(!fulfillable(true, false, false, true, true), "absente du cache");
        assert!(!fulfillable(true, false, true, false, false), "pas en main et pas creatif");
    }

    #[test]
    fn a_headline_without_a_status_hides_the_icon_instead_of_drawing_a_blank() {
        let shown = Headline { name: "Anna", female: true, status: Some("hungry") };
        assert_eq!(shown.status, Some("hungry"));
        assert!(shown.female);
        let bare = Headline { name: "Bob", female: false, status: None };
        assert_eq!(bare.status, None, "MainWindowCitizen.java:41-47 cache l'icone");
        assert!(!bare.female, "l'image femelle n'est posee que si isFemale");
    }
}
