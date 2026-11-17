//! `WindowQuestLog` : trois pages, une liste chacune, et un localisateur qui met un
//! citoyen en evidence trente secondes.
//!
//! Java reference : `core/client/gui/questlog/WindowQuestLog.java` (164 lignes), son
//! XML `gui/windowquestlog.xml` (54), ses moteurs de page
//! (`WindowQuestLogQuestModule.java` + trois implementations), l'item qui l'ouvre
//! (`core/items/ItemQuestLog.java:125`) et le gestionnaire qui alimente les listes
//! (`IQuestManager.java:94`, `:101`, `:108`).
//!
//! **La quatrieme source du brief n'est pas une fenetre.**
//! `WindowQuestLogAvailableQuestModule` n'a ni XML, ni titre, ni bouton : c'est un
//! `WindowQuestLogQuestModule<IQuestInstance>` (`:24`) qui remplit les lignes **d'une**
//! page, appele par le conteneur de cette page (`WindowQuestLog.java:44-46`). Le port
//! la traite comme ce qu'elle est : [`Page::Available`].
//!
//! Quatre faits que le source impose :
//!
//! 1. **Le localisateur ne vise pas toujours le donneur.** « disponible » surligne
//!    `getQuestGiverId()` (`WindowQuestLogAvailableQuestModule.java:44`), « en cours »
//!    surligne `getQuestTarget()` (`WindowQuestLogInProgressQuestQuestModule.java:59`) :
//!    une ligne peut nommer un citoyen et en surligner un autre.
//! 2. **Pas de localisateur sur « terminees »** : le XML n'y dessine pas le panneau
//!    (`windowquestlog.xml:46-50`) et le moteur n'y surcharge pas `trackQuest` — c'est
//!    le `default` vide de l'interface (`WindowQuestLogQuestModule.java:37`).
//! 3. **Le clic ne verifie pas la ligne** : `questItems.get(row)` sans borne
//!    (`WindowQuestLog.java:161`), la ou la fenetre des requetes verifie avant de lire
//!    (`WindowSelectRequest.java:134`). [`checked_row`] nomme ce qui manque.
//! 4. **Les fleches ne s'eteignent jamais** : `endless="true"` (`windowquestlog.xml:16`)
//!    et `setPage` ne coupe un bouton qu'en fin de course **non** infinie
//!    (`AbstractWindowSkeleton.java:239-246`). Ce XML n'a pas non plus de `pageNum`.

use crate::ui::action::Effect;
use crate::ui::field::{Scope, Value};

use super::pane::{Pane, Source};
use super::refusal::Refusal;
use super::{Button, By, Returns};

/// La classe de la fenetre (`WindowQuestLog.java:27`).
pub const JAVA: &str = "WindowQuestLog";

/// La classe du moteur de la page « disponible » : pas une fenetre (`:24`).
pub const AVAILABLE_JAVA: &str = "WindowQuestLogAvailableQuestModule";

/// Le XML (`WindowQuestLog.java:41`).
pub const RESOURCE: &str = "minecolonies:gui/windowquestlog.xml";

/// Le titre, dans un `text` sans id (`windowquestlog.xml:10-11`).
pub const TITLE_KEY: &str = "com.minecolonies.coremod.item.questlog.gui.title";

/// La vue a onglets (`WindowConstants.java:187`, `windowquestlog.xml:16`) : c'est elle
/// qui fait chercher deux fleches a `AbstractWindowSkeleton` (`:77-88`).
pub const SWITCH: &str = "pages";

/// L'id de liste, le meme dans les trois pages (`Constants.LIST_QUESTS`,
/// `windowquestlog.xml:21`, `:34`, `:46`).
pub const LIST: &str = "quests";

/// La page « en cours » (`windowquestlog.xml:17`, cle de `moduleMap` a `:44`).
pub const PAGE_IN_PROGRESS: &str = "pageInProgress";

/// La page « disponible » (`:30`, cle de `moduleMap` a `:45`).
pub const PAGE_AVAILABLE: &str = "pageAvailable";

/// La page « terminees » (`:42`, cle de `moduleMap` a `:46`).
pub const PAGE_FINISHED: &str = "pageFinished";

/// La ligne du nom (`Constants.LABEL_QUEST_NAME`), ecrite par les deux moteurs
/// (`WindowQuestLogAvailableQuestModule.java:37`, `WindowQuestLogFinishedQuestModule.java:31`).
pub const LABEL_QUEST_NAME: &str = "questName";

/// La ligne du donneur (`Constants.LABEL_QUEST_GIVER`), ecrite a `:38` et
/// `WindowQuestLogInProgressQuestQuestModule.java:37`.
pub const LABEL_QUEST_GIVER: &str = "questGiver";

/// La ligne de l'objectif (`Constants.LABEL_QUEST_OBJECTIVE`) : seule la page « en
/// cours » l'ecrit (`WindowQuestLogInProgressQuestQuestModule.java:40-50`).
pub const LABEL_QUEST_OBJECTIVE: &str = "questObjective";

/// La ligne du compte des repetitions (`Constants.LABEL_COMPLETED_COUNT`) : seule la
/// page « terminees » l'ecrit (`WindowQuestLogFinishedQuestModule.java:35-43`).
pub const LABEL_COMPLETED_COUNT: &str = "questCompletedCount";

/// Le bouton de ligne (`Constants.BUTTON_QUEST_LOCATOR`), lie **au niveau de la
/// fenetre** (`WindowQuestLog.java:42`) alors que le panneau est dans les lignes : le
/// clic remonte la chaine des parents, et `getListElementIndexByPane` (`:160`) retrouve
/// la ligne.
pub const BUTTON_QUEST_LOCATOR: &str = "questLocator";

/// Le prefixe du nom (`GuiTranslationConstants.java:43`).
pub const NAME_PREFIX_KEY: &str = "com.minecolonies.coremod.item.questlog.gui.name_prefix";

/// Le prefixe du donneur (`GuiTranslationConstants.java:45`).
pub const GIVER_PREFIX_KEY: &str = "com.minecolonies.coremod.item.questlog.gui.giver_prefix";

/// Le texte de l'infobulle du localisateur (`GuiTranslationConstants.java:51`).
pub const TRACK_CITIZEN_KEY: &str = "com.minecolonies.coremod.item.questlog.gui.track_citizen";

/// Le compte quand la quete se repete (`GuiTranslationConstants.java:49`).
pub const COMPLETED_MULTIPLE_KEY: &str =
    "com.minecolonies.coremod.item.questlog.gui.completed_multiple";

/// Le compte quand elle ne se repete pas (`GuiTranslationConstants.java:47`).
pub const COMPLETED_ONCE_KEY: &str = "com.minecolonies.coremod.item.questlog.gui.completed";

/// La cle du surlignage (`questlog/Constants.HIGHLIGHT_QUEST_LOG_TRACKER_KEY`).
pub const TRACK_HIGHLIGHT_KEY: &str = "questLogTracking";

/// Combien de secondes il dure (`HIGHLIGHT_QUEST_LOG_TRACKER_DURATION`, `:18`).
pub const TRACK_SECONDS: u64 = 30;

/// Les portees de ligne, composees parce que les trois listes s'appellent `quests`.
pub const IN_PROGRESS_ROWS: &str = "pageInProgress/quests";
pub const AVAILABLE_ROWS: &str = "pageAvailable/quests";
pub const FINISHED_ROWS: &str = "pageFinished/quests";

/// Ce qu'une ligne du journal surligne.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: trois pages, trois cibles
pub enum Track {
    /// Le donneur (`WindowQuestLogAvailableQuestModule.java:44`).
    Giver,
    /// La cible (`WindowQuestLogInProgressQuestQuestModule.java:59`) — pas le donneur.
    Target,
    /// Rien : la page n'a pas de localisateur.
    Nothing,
}

/// Une entree de liste, telle que les moteurs la lisent.
///
/// Quatre champs et pas un de plus : `IQuestInstance` pour les deux ids
/// (`IQuestInstance.java:37`, `:50`), `IQuestTemplate.getName()` pour le nom (`:61`), et
/// la vue colonie pour le nom du donneur. Le port n'a pas de gestionnaire de quetes
/// (`MANQUANT.md` §3) : c'est l'appelant qui remplit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item<'a> {
    /// Le nom du modele ; `None` quand il n'est pas dans le registre du client.
    pub template: Option<&'a str>,
    /// Le nom du donneur ; `None` quand le citoyen n'est pas dans la vue.
    pub giver: Option<&'a str>,
    /// `getQuestGiverId()`.
    pub giver_id: Option<i32>,
    /// `getQuestTarget()`, lu par le seul localisateur de la page « en cours ».
    pub target: Option<i32>,
}

/// Une ligne d'ecran : la cle du prefixe, et le texte qui suit.
///
/// Java ecrit `Component.translatable(prefixe).append(nom)`
/// (`WindowQuestLogAvailableQuestModule.java:37-38`) : deux morceaux, dont le port ne
/// peut traduire aucun. Les garder separes est ce qui permet a un client de le faire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line<'a> {
    /// Le prefixe, une cle du jeu.
    pub key: &'static str,
    /// Le nom a coller derriere ; `""` quand il n'y en a pas.
    pub text: &'a str,
}

/// La page « terminees » : une fois, ou n fois sur m.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les deux branches de getMaxOccurrence() > 1
pub enum Done {
    /// Le modele ne se repete pas (`WindowQuestLogFinishedQuestModule.java:41`).
    Once,
    /// Il se repete, avec les deux nombres (`:36-39`).
    Times { finished: i32, max: i32 },
}

impl Done {
    /// Le compte de cette quete finie (`:35-43`).
    ///
    /// Le test porte sur le **maximum**, pas sur le compte : un modele qui peut arriver
    /// trois fois affiche `1 / 3` des la premiere, et un modele unique affiche
    /// « terminee » meme a zero.
    pub fn of(finished: i32, max: i32) -> Done {
        if max > 1 {
            Done::Times { finished, max }
        } else {
            Done::Once
        }
    }

    /// La cle de traduction de la ligne.
    pub fn key(self) -> &'static str {
        match self {
            Done::Once => COMPLETED_ONCE_KEY,
            Done::Times { .. } => COMPLETED_MULTIPLE_KEY,
        }
    }
}

/// Une page : sa vue, ses lignes, son libelle, son moteur, ses panneaux, ses boutons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les trois cles de moduleMap
pub enum Page {
    /// `WindowQuestLog.java:44`.
    InProgress,
    /// `WindowQuestLog.java:45` — la page de la quatrieme source du brief.
    Available,
    /// `WindowQuestLog.java:46`.
    Finished,
}

impl Page {
    /// Les trois pages, dans l'ordre du `switch` (`windowquestlog.xml:17`, `:30`, `:42`) :
    /// c'est l'ordre que les fleches parcourent.
    pub const ALL: [Page; 3] = [Page::InProgress, Page::Available, Page::Finished];

    /// L'id de la `view` qui porte la page.
    pub fn view(self) -> &'static str {
        match self {
            Page::InProgress => PAGE_IN_PROGRESS,
            Page::Available => PAGE_AVAILABLE,
            Page::Finished => PAGE_FINISHED,
        }
    }

    /// La page de cet id de vue, ou [`Refusal::UnknownPage`].
    pub fn of(view: &str) -> Result<Page, Refusal> {
        Page::ALL
            .iter()
            .copied()
            .find(|page| page.view() == view)
            .ok_or(Refusal::UnknownPage)
    }

    /// La cle du libelle que le XML dessine en haut de la page (`:19`, `:32`, `:44`).
    pub fn label(self) -> &'static str {
        match self {
            Page::InProgress => "com.minecolonies.coremod.item.questlog.gui.inprogress",
            Page::Available => "com.minecolonies.coremod.item.questlog.gui.available",
            Page::Finished => "com.minecolonies.coremod.item.questlog.gui.finished",
        }
    }

    /// La portee des lignes de cette page, pour [`Pane::in_row`].
    pub fn rows(self) -> &'static str {
        match self {
            Page::InProgress => IN_PROGRESS_ROWS,
            Page::Available => AVAILABLE_ROWS,
            Page::Finished => FINISHED_ROWS,
        }
    }

    /// La classe Java qui remplit les lignes de la page.
    pub fn module(self) -> &'static str {
        match self {
            Page::InProgress => "WindowQuestLogInProgressQuestQuestModule",
            Page::Available => AVAILABLE_JAVA,
            Page::Finished => "WindowQuestLogFinishedQuestModule",
        }
    }

    /// Ce que le localisateur de la page met en evidence.
    pub fn track(self) -> Track {
        match self {
            Page::InProgress => Track::Target,
            Page::Available => Track::Giver,
            Page::Finished => Track::Nothing,
        }
    }

    /// Les panneaux de la page : sa liste, puis les panneaux de ses lignes.
    pub fn panes(self) -> &'static [Pane] {
        match self {
            Page::InProgress => IN_PROGRESS_PANES,
            Page::Available => AVAILABLE_PANES,
            Page::Finished => FINISHED_PANES,
        }
    }

    /// Les boutons de ses lignes : vide pour « terminees ».
    pub fn buttons(self) -> &'static [Button] {
        match self {
            Page::InProgress => IN_PROGRESS_BUTTONS,
            Page::Available => AVAILABLE_BUTTONS,
            Page::Finished => &[],
        }
    }
}

/// Les panneaux de « en cours » : `windowquestlog.xml:21-27`.
pub const IN_PROGRESS_PANES: &[Pane] = &[
    Pane::new(LIST, Value::Row, Source::Quests, ("windowquestlog.xml", 21)),
    Pane::in_row(IN_PROGRESS_ROWS, LABEL_QUEST_NAME, Value::Text, Source::QuestTemplates, ("windowquestlog.xml", 23)),
    Pane::in_row(IN_PROGRESS_ROWS, LABEL_QUEST_GIVER, Value::Text, Source::Citizen, ("windowquestlog.xml", 24)),
    Pane::in_row(IN_PROGRESS_ROWS, LABEL_QUEST_OBJECTIVE, Value::Text, Source::QuestTemplates, ("windowquestlog.xml", 25)),
];

/// Les panneaux de « disponible » : `windowquestlog.xml:34-39`.
pub const AVAILABLE_PANES: &[Pane] = &[
    Pane::new(LIST, Value::Row, Source::Quests, ("windowquestlog.xml", 34)),
    Pane::in_row(AVAILABLE_ROWS, LABEL_QUEST_NAME, Value::Text, Source::QuestTemplates, ("windowquestlog.xml", 36)),
    Pane::in_row(AVAILABLE_ROWS, LABEL_QUEST_GIVER, Value::Text, Source::Citizen, ("windowquestlog.xml", 37)),
];

/// Les panneaux de « terminees » : `windowquestlog.xml:46-50` — **pas de localisateur**.
pub const FINISHED_PANES: &[Pane] = &[
    Pane::new(LIST, Value::Row, Source::Quests, ("windowquestlog.xml", 46)),
    Pane::in_row(FINISHED_ROWS, LABEL_QUEST_NAME, Value::Text, Source::QuestTemplates, ("windowquestlog.xml", 48)),
    Pane::in_row(FINISHED_ROWS, LABEL_COMPLETED_COUNT, Value::Text, Source::Quests, ("windowquestlog.xml", 49)),
];

/// Le localisateur de « en cours » (`windowquestlog.xml:26`).
pub const IN_PROGRESS_BUTTONS: &[Button] = &[Button {
    id: BUTTON_QUEST_LOCATOR,
    label: "",
    effect: Effect::Highlights,
    scope: Scope::Row(IN_PROGRESS_ROWS),
    registered: By::Window,
    returns: Returns::Nothing,
    refusals: &[Refusal::NoRow],
}];

/// Le localisateur de « disponible » (`windowquestlog.xml:38`).
pub const AVAILABLE_BUTTONS: &[Button] = &[Button {
    id: BUTTON_QUEST_LOCATOR,
    label: "",
    effect: Effect::Highlights,
    scope: Scope::Row(AVAILABLE_ROWS),
    registered: By::Window,
    returns: Returns::Nothing,
    refusals: &[Refusal::NoRow],
}];

/// La fleche de gauche (`WindowConstants.java:560`).
pub const BUTTON_PREV_PAGE: &str = "prevPage";

/// La fleche de droite (`WindowConstants.java:561`).
pub const BUTTON_NEXT_PAGE: &str = "nextPage";

/// Les deux fleches de la fenetre.
///
/// `WindowQuestLog` ne les enregistre **pas** : c'est `AbstractWindowSkeleton` qui le
/// fait des qu'il trouve une vue a onglets (`:77-88`) — d'ou [`By::Base`]. Elles
/// tournent une page *relativement* (`setPage(true, -1)`, `:86-87`), d'ou
/// `Effect::Cycles`, et ne s'eteignent jamais ici (`endless="true"`).
pub const SHELL_BUTTONS: &[Button] = &[
    Button {
        id: BUTTON_PREV_PAGE,
        label: "",
        effect: Effect::Cycles,
        scope: Scope::Window,
        registered: By::Base,
        returns: Returns::Nothing,
        refusals: &[],
    },
    Button {
        id: BUTTON_NEXT_PAGE,
        label: "",
        effect: Effect::Cycles,
        scope: Scope::Window,
        registered: By::Base,
        returns: Returns::Nothing,
        refusals: &[],
    },
];

/// Le nom d'une quete, ou [`Refusal::UnknownTemplate`].
///
/// Java ne verifie pas : `GLOBAL_SERVER_QUESTS.get(quest.getId())` rend `null` pour un
/// modele que le client n'a pas recu (`:35`) et `getName()` (`:37`) leve. Le port
/// refuse, par le nom.
pub fn name_line<'a>(item: &Item<'a>) -> Result<Line<'a>, Refusal> {
    let template = item.template.ok_or(Refusal::UnknownTemplate)?;
    Ok(Line { key: NAME_PREFIX_KEY, text: template })
}

/// Le nom du donneur, **toujours** rendu : quand le citoyen n'est pas dans la vue,
/// Java concatenee `Component.empty()` (`:82`), donc la ligne ne porte que le prefixe.
/// C'est un etat normal — un donneur peut avoir quitte la colonie.
pub fn giver_line<'a>(item: &Item<'a>) -> Line<'a> {
    Line { key: GIVER_PREFIX_KEY, text: item.giver.unwrap_or("") }
}

/// L'id citoyen que le localisateur de cette page met en evidence : le donneur, la
/// cible, ou rien (`:44` contre `:58`).
pub fn tracked(page: Page, item: &Item<'_>) -> Option<i32> {
    match page.track() {
        Track::Giver => item.giver_id,
        Track::Target => item.target,
        Track::Nothing => None,
    }
}

/// La ligne de cette page, apres verification de la borne que Java oublie.
///
/// `Err(NoRow)` pour un indice negatif — `getListElementIndexByPane` rend `-1` quand le
/// panneau n'est pas dans la liste — ou hors de la liste. Java lit alors
/// `questItems.get(-1)` (`:161`) et leve.
pub fn checked_row(len: usize, row: i32) -> Result<usize, Refusal> {
    if row < 0 {
        return Err(Refusal::NoRow);
    }
    if (row as usize) < len {
        Ok(row as usize)
    } else {
        Err(Refusal::NoRow)
    }
}

/// Faut-il une infobulle ? Oui **si et seulement si** le texte deborde :
/// `getRenderedTextWidth() > getWidth()` (`WindowQuestLogAvailableQuestModule.java:59`),
/// donc un texte exactement a la largeur n'en a pas.
pub fn tooltip(rendered_width: i32, pane_width: i32) -> bool {
    rendered_width > pane_width
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les pages dont les lignes portent ce panneau.
    fn pages_with(id: &str) -> Vec<&'static str> {
        Page::ALL
            .iter()
            .filter(|page| page.panes().iter().any(|pane| pane.id == id))
            .map(|page| page.view())
            .collect()
    }

    fn item() -> Item<'static> {
        Item {
            template: Some("A Lost Sheep"),
            giver: Some("Maria"),
            giver_id: Some(4),
            target: Some(9),
        }
    }

    #[test]
    fn three_pages_ten_panes_and_one_list_id_in_three_scopes() {
        assert_eq!(Page::ALL.len(), 3);
        let mut panes = 0;
        for page in Page::ALL {
            assert_eq!(Page::of(page.view()), Ok(page));
            panes += page.panes().len();
            // Les trois listes portent le meme id et vivent dans trois portees.
            let list = page.panes()[0];
            assert_eq!((list.id, list.value, list.label), (LIST, Value::Row, None));
            assert!(page.rows().contains(LIST));
            for pane in &page.panes()[1..] {
                assert_eq!(pane.scope, Scope::Row(page.rows()), "{}", pane.id);
            }
            assert!(page.label().starts_with("com.minecolonies.coremod.item.questlog.gui."));
            // Aucun id repete dans la portee d'une page.
            for (index, pane) in page.panes().iter().enumerate() {
                assert!(!page.panes()[..index]
                    .iter()
                    .any(|other| other.id == pane.id && other.scope == pane.scope));
            }
        }
        assert_eq!(panes, 4 + 3 + 3, "1 liste + 3 lignes, 1 + 2, 1 + 2");
        let rows: Vec<&str> = Page::ALL.iter().map(|page| page.rows()).collect();
        assert_eq!(rows, [IN_PROGRESS_ROWS, AVAILABLE_ROWS, FINISHED_ROWS]);
        assert_eq!(Page::of("pageElsewhere"), Err(Refusal::UnknownPage));
        // Chaque ligne est propre a une page : l'objectif, et le compte des repetitions.
        assert_eq!(pages_with(LABEL_QUEST_OBJECTIVE), [PAGE_IN_PROGRESS]);
        assert_eq!(pages_with(LABEL_COMPLETED_COUNT), [PAGE_FINISHED]);
        assert_eq!(pages_with(LABEL_QUEST_NAME).len(), 3, "le nom est sur les trois pages");
    }

    #[test]
    fn two_pages_carry_the_locator_the_finished_page_does_not_and_two_arrows_are_the_base() {
        let carrying: Vec<&str> = Page::ALL
            .iter()
            .filter(|page| !page.buttons().is_empty())
            .map(|page| page.view())
            .collect();
        assert_eq!(carrying, [PAGE_IN_PROGRESS, PAGE_AVAILABLE]);
        assert!(Page::Finished.buttons().is_empty(), "windowquestlog.xml:46-50");
        // Le meme id sur les deux pages, deux portees de ligne.
        assert_eq!(IN_PROGRESS_BUTTONS[0].id, AVAILABLE_BUTTONS[0].id);
        assert_ne!(IN_PROGRESS_BUTTONS[0].scope, AVAILABLE_BUTTONS[0].scope);
        assert_eq!(AVAILABLE_BUTTONS[0].label, "", "une image, pas une cle");
        assert!(TRACK_CITIZEN_KEY.starts_with("com.minecolonies."));
        // La page terminee n'a pas de localisateur et son moteur ne surcharge pas
        // `trackQuest` : c'est le corps vide de l'interface qui repond, ligne 37.
        assert_eq!(Page::Finished.track(), Track::Nothing, "WindowQuestLogQuestModule.java:37");
        // Les fleches viennent de la classe mere, et ne s'eteignent jamais.
        assert_eq!(SHELL_BUTTONS.len(), 2);
        for button in SHELL_BUTTONS {
            assert_eq!(button.registered, By::Base);
            assert_eq!(button.effect, Effect::Cycles);
            assert!(!button.is_guarded(), "endless=\"true\"");
            assert_eq!(button.label_key(), None, "windowquestlog.xml:13-14");
            assert_eq!(button.scope, Scope::Window);
        }
        assert_eq!(SHELL_BUTTONS[0].id, "prevPage");
        assert_eq!(SHELL_BUTTONS[1].id, "nextPage");
    }

    #[test]
    fn the_name_needs_a_template_but_the_giver_does_not() {
        assert_eq!(
            name_line(&item()),
            Ok(Line { key: NAME_PREFIX_KEY, text: "A Lost Sheep" })
        );
        assert_eq!(giver_line(&item()), Line { key: GIVER_PREFIX_KEY, text: "Maria" });
        // Le modele absent : Java lit son nom sur un `null` (:35-37).
        let orphan = Item { template: None, ..item() };
        assert_eq!(name_line(&orphan), Err(Refusal::UnknownTemplate));
        // Le citoyen absent : la ligne porte le prefixe seul, sans refus (:77-83).
        let gone = Item { giver: None, ..item() };
        assert_eq!(giver_line(&gone).text, "");
        assert_eq!(giver_line(&gone).key, GIVER_PREFIX_KEY);
        assert_ne!(NAME_PREFIX_KEY, GIVER_PREFIX_KEY);
        assert_ne!(NAME_PREFIX_KEY, TITLE_KEY);
    }

    #[test]
    fn the_locator_does_not_always_point_at_the_giver_and_the_click_checks_the_row() {
        let item = item();
        assert_eq!(tracked(Page::Available, &item), Some(4), "le donneur");
        assert_eq!(tracked(Page::InProgress, &item), Some(9), "la cible");
        assert_ne!(tracked(Page::Available, &item), tracked(Page::InProgress, &item));
        assert_eq!(tracked(Page::Finished, &item), None);
        // Une cible inconnue ne surligne rien plutot que de surligner 0.
        assert_eq!(tracked(Page::InProgress, &Item { target: None, ..item }), None);
        assert_eq!((TRACK_SECONDS, TRACK_HIGHLIGHT_KEY), (30, "questLogTracking"));
        // Le journal ne verifie pas sa ligne (:161) ; le port le fait et le nomme.
        assert_eq!(checked_row(3, 0), Ok(0));
        assert_eq!(checked_row(3, 2), Ok(2));
        assert_eq!(checked_row(3, -1), Err(Refusal::NoRow), "getListElementIndexByPane");
        assert_eq!(checked_row(3, 3), Err(Refusal::NoRow));
        assert_eq!(checked_row(0, 0), Err(Refusal::NoRow));
        // La ou la fenetre des requetes sort sans rien rendre.
        assert_eq!(Refusal::NoRow.name(), "ui13.refusal.no_row");
        assert!(Refusal::NoRow.is_crash());
        assert!(!Refusal::RowOutOfRange.is_crash());
    }

    #[test]
    fn the_tooltip_is_strictly_wider_and_the_count_tests_the_maximum() {
        assert!(tooltip(131, 130), "WindowQuestLogAvailableQuestModule.java:59");
        assert!(!tooltip(130, 130), "exactement la largeur : pas d'infobulle");
        assert!(!tooltip(0, 130));
        // Le compte des terminees teste le maximum, pas le compte (:35-43).
        assert_eq!(Done::of(1, 3), Done::Times { finished: 1, max: 3 });
        assert_eq!(Done::of(0, 1), Done::Once, "un modele unique, meme a zero");
        assert_eq!(Done::of(4, 0), Done::Once);
        assert_eq!(Done::Times { finished: 1, max: 3 }.key(), COMPLETED_MULTIPLE_KEY);
        assert_eq!(Done::Once.key(), COMPLETED_ONCE_KEY);
        assert_ne!(COMPLETED_ONCE_KEY, COMPLETED_MULTIPLE_KEY);
    }
}
