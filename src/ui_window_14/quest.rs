//! Le journal de quetes **cote donnee** : ce que chacune des trois pages lit, ce
//! qu'elle en rend, et ce qu'un clic deplace.
//!
//! Java reference : `core/client/gui/questlog/` — `WindowQuestLog.java` (164 lignes,
//! la fenetre qui possede les trois pages), `WindowQuestLogQuestModule.java` (38,
//! l'interface), et ses trois implementations `...InProgressQuestQuestModule.java`
//! (99), `...AvailableQuestModule.java` (85), `...FinishedQuestModule.java` (66).
//!
//! **La forme des panneaux est deja dans `ui_window_13`** : les ids, les portees de
//! ligne et le localisateur y sont decrits, et ce fichier ne les redecrit pas — il
//! importe `crate::ui_window_13::quest_log` et s'en sert. Ce qui est ici est ce que
//! cette table-la ne porte pas : **la valeur de chaque ligne, son type, son encre, et
//! la regle qui choisit quel citoyen la remplit.**
//!
//! # Deux types de ligne pour trois pages
//!
//! Le moteur est generique (`WindowQuestLogQuestModule.java:13`,
//! `WindowQuestLogQuestModule<T>`) et Java l'instancie **deux fois** :
//! `IQuestInstance` pour « en cours » et « disponible »
//! (`WindowQuestLogInProgressQuestQuestModule.java:24`,
//! `WindowQuestLogAvailableQuestModule.java:24`) et `FinishedQuest` pour
//! « terminees » (`WindowQuestLogFinishedQuestModule.java:20`). Une ligne n'est donc
//! pas un type unique : [`Row`] en porte deux, et [`cells`] refuse un `FinishedQuest`
//! sur une page vivante.
//!
//! # Ce que la ligne du donneur nomme vraiment
//!
//! La page « disponible » lit `getQuestGiverId()`
//! (`WindowQuestLogAvailableQuestModule.java:77`) ; la page « en cours » lit
//! `getQuestTarget()` (`WindowQuestLogInProgressQuestQuestModule.java:92`), que
//! `IQuestInstance.java:93` decrit comme « le porteur de la tache courante ». Les
//! deux pages ecrivent ce nom dans le **meme** panneau, `questGiver`, sous le meme
//! prefixe (`:38`). Sur une quete en cours, la ligne « Donneur : » affiche donc le
//! porteur de la tache, qui n'est pas forcement celui qui a donne la quete — et le
//! localisateur de la meme ligne surligne ce meme porteur (`:58`).
//!
//! # Ce que la ligne d'objectif est
//!
//! Un **litteral** de trois caracteres colle devant un texte que la fenetre ne sait
//! pas produire : `Component.literal(" - ")` puis
//! `objectiveTemplate.getProgressText(quest, or)` (`:43-46`). Ce n'est ni une
//! traduction, ni un pourcentage, ni un compteur : c'est une `Component` fabriquee par
//! le moteur d'objectifs, que le port n'a pas. L'appelant la fournit
//! ([`Objective::text`]) et son absence est refusee
//! ([`super::Refusal::ObjectiveTextMissing`]) plutot que remplacee par du vide.
//!
//! **L'infobulle de cette ligne est inconditionnelle** (`:50-53`) alors que celle du
//! nom et du compte ne s'affiche que si le texte deborde (`:57-63`, `:74-80`) —
//! c'est [`Tip`].

use crate::ui_window_13::quest_log as ql;
use crate::ui_window_13::quest_log::{Page, Track};
use crate::ui_window_13::refusal::Refusal as Ui13;

use super::Refusal;

/// Le separateur ecrit en dur devant le texte d'objectif
/// (`WindowQuestLogInProgressQuestQuestModule.java:44`). Un `literal`, donc **pas une
/// cle de traduction** : il n'y a rien a traduire, et un client qui le passerait par
/// la table de langues chercherait une cle qui n'existe pas.
pub const OBJECTIVE_SEPARATOR: &str = " - ";

/// La couleur d'un texte de ligne.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les trois encres que ces pages ecrivent
pub enum Ink {
    /// Aucun style : le defaut du panneau (`:31`, `:37-38`, `:57`).
    Plain,
    /// `ChatFormatting.GOLD` sur le texte, et sur l'infobulle du compte
    /// (`:37`, `:41`, `:46`).
    Gold,
    /// `ChatFormatting.WHITE`, **uniquement** sur l'infobulle de l'objectif (`:51`).
    White,
}

/// Quand la ligne porte une infobulle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les deux disciplines du fichier
pub enum Tip {
    /// Seulement si le texte rendu est plus large que le panneau
    /// (`WindowQuestLogInProgressQuestQuestModule.java:74`,
    /// `WindowQuestLogFinishedQuestModule.java:57`) — strictement plus large, donc un
    /// texte exactement a la largeur n'en a pas.
    Overflow,
    /// Toujours : l'objectif d'une quete en cours (`:50-53`), en blanc.
    Always,
}

impl Tip {
    /// L'encre de l'infobulle : blanche pour l'objectif (`:51`), aucune pour les trois
    /// autres, qui reprennent celle du texte.
    pub fn ink(self) -> Ink {
        match self {
            Tip::Always => Ink::White,
            Tip::Overflow => Ink::Plain,
        }
    }
}

/// Une ligne d'une ligne d'ecran, avec sa cle et son texte **separes**.
///
/// `Component.translatable(prefixe).append(texte)` (`:37-38`) est deux morceaux : le
/// port ne peut traduire aucun des deux, et les garder separes est ce qui permet a un
/// client de le faire sans redecouper une chaine deja collee.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les quatre lignes qu'une page peut rendre
pub enum Line<'a> {
    /// Le nom de la quete : prefixe + nom du modele (`:31`, `:37`).
    Name { key: &'static str, text: &'a str },
    /// Le citoyen : le donneur, ou le porteur de la tache selon la page (`:38`).
    Citizen { key: &'static str, text: &'a str },
    /// L'objectif : le litteral puis le texte de progression (`:44-48`).
    Objective { literal: &'static str, text: &'a str },
    /// Le compte des repetitions (`:35-42`), avec ses deux nombres quand le modele se
    /// repete.
    Done { key: &'static str, count: Option<(i32, i32)> },
}

impl<'a> Line<'a> {
    /// La cle de traduction que la ligne emploie, quand elle en a une.
    pub fn key(&self) -> Option<&'static str> {
        // `*self` : une ligne est `Copy`, et un motif sur `&Line` lierait `key` par
        // reference (`&&'static str`) au lieu de la copier.
        match *self {
            Line::Name { key, .. } | Line::Citizen { key, .. } | Line::Done { key, .. } => {
                Some(key)
            }
            Line::Objective { .. } => None,
        }
    }

    /// Le texte colle derriere la cle ; `""` pour un citoyen inconnu, et le litteral
    /// pour l'objectif.
    pub fn text(&self) -> &'a str {
        match *self {
            Line::Name { text, .. } | Line::Citizen { text, .. } => text,
            Line::Objective { literal, .. } => literal,
            Line::Done { .. } => "",
        }
    }
}

/// Une ligne avec son encre et sa regle d'infobulle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell<'a> {
    pub line: Line<'a>,
    pub ink: Ink,
    pub tip: Tip,
}

impl<'a> Cell<'a> {
    /// Une ligne a encre et infobulle par defaut : c'est la discipline de trois des
    /// quatre lignes (`:57-63`).
    const fn plain(line: Line<'a>) -> Cell<'a> {
        Cell { line, ink: Ink::Plain, tip: Tip::Overflow }
    }
}

/// Ce qu'une ligne d'ecran rend : deux textes, et rien d'autre.
///
/// La troisieme ligne de la page « en cours » (`questObjective`) n'existe que la ; les
/// autres n'en ont que deux, donc deux champs est la bonne forme pour les trois.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cells<'a> {
    pub name: Cell<'a>,
    pub second: Cell<'a>,
}

/// `IQuestInstance` — l'entree des pages « en cours » et « disponible ».
///
/// Tous les champs sont lus par l'une des deux pages : c'est le strict necessaire pour
/// ecrire les trois lignes moins l'objectif, et rien de plus. Aucun n'est un `Result` :
/// Java ne verifie rien a la lecture (`:35-38`, `:77`, `:92`), c'est la **construction
/// de la ligne** qui peut refuser ([`cells`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instance<'a> {
    /// `IQuestTemplate.getName()` du modele lu dans
    /// `IQuestManager.GLOBAL_SERVER_QUESTS` (`:35`) ; `None` quand le client n'a pas
    /// le modele, et Java lit alors son nom sur un `null` (`:37`).
    pub template: Option<&'a str>,
    /// `IQuestInstance.getQuestGiverId()` (`IQuestInstance.java:37`).
    pub giver_id: i32,
    /// Le nom du citoyen derriere `giver_id` ; `None` quand il a quitte la colonie
    /// (`:93-97` rend alors `Component.empty()`).
    pub giver: Option<&'a str>,
    /// `IQuestInstance.getQuestTarget()`, « le porteur de la tache courante »
    /// (`IQuestInstance.java:93`).
    pub holder_id: i32,
    /// Le nom du citoyen derriere `holder_id` (`:92`).
    pub holder: Option<&'a str>,
    /// L'objectif courant, lu seulement par la page « en cours » (`:40-48`).
    pub objective: Objective<'a>,
}

/// `IQuestObjectiveTemplate` — ce que la page « en cours » ajoute a une instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Objective<'a> {
    /// `IQuestInstance.getObjectiveIndex()` (`IQuestInstance.java:74`), que Java passe
    /// a `getObjective` sans verifier (`:40`).
    pub index: i32,
    /// `IQuestTemplate.getObjectiveCount()` (`IQuestTemplate.java:55`).
    pub count: i32,
    /// Le texte de `getProgressText(quest, style)` (`:43`) ; `None` quand l'appelant ne
    /// l'a pas — le port n'a pas le moteur d'objectifs.
    pub text: Option<&'a str>,
}

/// `FinishedQuest` (`api/quests/FinishedQuest.java:9`) — le modele et son compte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Finished<'a> {
    /// `FinishedQuest.template()` ; meme `None` que pour une instance.
    pub template: Option<&'a str>,
    /// `FinishedQuest.finishedCount()`.
    pub finished: i32,
    /// `template.getMaxOccurrence()` (`IQuestTemplate.java:32`).
    pub max: i32,
}

/// Une ligne de la page : l'un des deux types que Java instancie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: deux types pour trois pages, c'est le fait
pub enum Row<'a> {
    /// Une instance vivante, pour « en cours » et « disponible ».
    Live(Instance<'a>),
    /// Une quete finie, et **seulement** pour « terminees ».
    Done(Finished<'a>),
}

impl<'a> Row<'a> {
    /// L'instance de cette ligne, ou le refus qui dit que la page et le type ne vont
    /// pas ensemble.
    fn live(&self, page: Page) -> Result<&Instance<'a>, Refusal> {
        match self {
            Row::Live(instance) => Ok(instance),
            Row::Done(_) => Err(Refusal::WrongRowKind { page }),
        }
    }
}

/// La ligne du nom, la meme des trois pages (`:31`, `:37`) — et le premier refus du
/// fichier, celui du modele absent.
fn name<'a>(template: Option<&'a str>) -> Result<Line<'a>, Refusal> {
    Ok(Line::Name {
        key: ql::NAME_PREFIX_KEY,
        text: template.ok_or(Refusal::Quest(Ui13::UnknownTemplate))?,
    })
}

/// Les deux lignes qu'une page rend pour une ligne de liste.
///
/// Les trois pages ecrivent le nom de la meme facon (`:31`, `:37`), puis divergent :
/// « en cours » ecrit l'objectif, « disponible » le citoyen, « terminees » le compte.
pub fn cells<'a>(page: Page, row: &Row<'a>) -> Result<Cells<'a>, Refusal> {
    match page {
        Page::InProgress => {
            let instance = row.live(page)?;
            // L'ordre de Java est celui du fichier : le nom, puis le citoyen, puis
            // l'objectif (`:37-48`) ; la borne et le texte manquant sont donc testes
            // avant que quoi que ce soit soit rendu.
            let name = name(instance.template)?;
            let text = instance
                .objective
                .text
                .ok_or(Refusal::ObjectiveTextMissing { index: instance.objective.index })?;
            if instance.objective.index < 0 || instance.objective.index >= instance.objective.count {
                return Err(Refusal::ObjectiveOutOfRange {
                    index: instance.objective.index,
                    count: instance.objective.count,
                });
            }
            Ok(Cells {
                name: Cell::plain(name),
                second: Cell {
                    line: Line::Objective { literal: OBJECTIVE_SEPARATOR, text },
                    ink: Ink::Gold,
                    tip: Tip::Always,
                },
            })
        }
        Page::Available => {
            let instance = row.live(page)?;
            Ok(Cells {
                name: Cell::plain(name(instance.template)?),
                // Aucun refus pour un citoyen absent : Java rend un vide
                // (`WindowQuestLogAvailableQuestModule.java:82`).
                second: Cell::plain(Line::Citizen {
                    key: ql::GIVER_PREFIX_KEY,
                    text: instance.giver.unwrap_or(""),
                }),
            })
        }
        Page::Finished => {
            let Row::Done(finished) = row else {
                return Err(Refusal::WrongRowKind { page });
            };
            let name = name(finished.template)?;
            let done = ql::Done::of(finished.finished, finished.max);
            Ok(Cells {
                name: Cell::plain(name),
                second: Cell {
                    line: Line::Done {
                        key: done.key(),
                        // Les deux nombres ne sont portes que si le modele se repete
                        // (`:33-38`) ; `Done` decide sur le **maximum**, pas sur le
                        // compte.
                        count: match done {
                            ql::Done::Times { finished: times, max } => Some((times, max)),
                            ql::Done::Once => None,
                        },
                    },
                    ink: Ink::Gold,
                    tip: Tip::Overflow,
                },
            })
        }
    }
}

/// Le citoyen que le localisateur met en evidence, ou `Absent`.
///
/// `WindowQuestLogQuestModule.java:37` declare `trackQuest` avec un corps vide : la
/// page « terminees » ne surcharge rien, donc son localisateur — quand le XML en
/// dessinerait un — ne ferait rien. Il n'en dessine pas (`windowquestlog.xml:46-50`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: un id, ou pas de localisateur du tout
pub enum Locator {
    /// L'id passe a `CitizenRenderData` (`:59`) ; il est envoye meme quand aucun
    /// citoyen ne le porte, et le surlignage ne trouve alors rien.
    Citizen(i32),
    /// La page n'a pas de localisateur.
    Absent,
}

impl Locator {
    /// L'id a surligner.
    pub fn citizen(self) -> Option<i32> {
        match self {
            Locator::Citizen(id) => Some(id),
            Locator::Absent => None,
        }
    }
}

/// Ce que le clic du localisateur surligne, pour cette page et cette ligne.
///
/// La classification (`Giver` / `Target` / `Nothing`) est celle de
/// `ui_window_13::quest_log::Page::track` ; ce fichier y ajoute **la donnee** : quel
/// champ de l'instance elle lit, et le fait que la page « en cours » surligne le
/// porteur de la tache et non le donneur (`:58`).
pub fn locator<'a>(page: Page, row: &Row<'a>) -> Result<Locator, Refusal> {
    match page.track() {
        Track::Nothing => Ok(Locator::Absent),
        Track::Giver => Ok(Locator::Citizen(row.live(page)?.giver_id)),
        Track::Target => Ok(Locator::Citizen(row.live(page)?.holder_id)),
    }
}

/// L'etat du journal : la page courante et les trois listes.
///
/// Java ne garde qu'un `questItems` **par page** (`WindowQuestLog.java:106`) et le
/// remplace entierement a chaque rafraichissement (`:150`) : aucun diff, aucune mise a
/// jour incrementale, aucun tri. Ce type reprend la meme discipline, avec les trois
/// listes visibles d'un coup parce que le port n'a pas de conteneur par page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Log<'a> {
    page: Page,
    in_progress: Vec<Row<'a>>,
    available: Vec<Row<'a>>,
    finished: Vec<Row<'a>>,
}

impl<'a> Log<'a> {
    /// Le journal a l'ouverture : **la premiere vue du `switch`**, pas un choix de la
    /// classe. `WindowQuestLog` n'appelle jamais `setPage`, et le XML met
    /// `pageInProgress` en premier (`windowquestlog.xml:17-29`).
    pub fn new() -> Log<'a> {
        Log {
            page: Page::InProgress,
            in_progress: Vec::new(),
            available: Vec::new(),
            finished: Vec::new(),
        }
    }

    /// Remplace les trois listes, comme `onUpdate` le fait de la page courante
    /// (`WindowQuestLog.java:150`) — en une fois, pour les trois.
    pub fn reload(
        &mut self,
        in_progress: Vec<Row<'a>>,
        available: Vec<Row<'a>>,
        finished: Vec<Row<'a>>,
    ) {
        self.in_progress = in_progress;
        self.available = available;
        self.finished = finished;
    }

    /// La page affichee.
    pub fn page(&self) -> Page {
        self.page
    }

    /// Tourne la page en la nommant par l'id de vue, comme le clic du localisateur le
    /// fait (`WindowQuestLog.java:58`) : c'est cet id qui choisit le moteur qui
    /// repondra (`:59-62`).
    pub fn set_page(&mut self, view: &str) -> Result<Page, Refusal> {
        let page = Page::of(view).map_err(Refusal::Quest)?;
        self.page = page;
        Ok(page)
    }

    /// Les lignes de cette page.
    pub fn rows(&self, page: Page) -> &[Row<'a>] {
        match page {
            Page::InProgress => &self.in_progress,
            Page::Available => &self.available,
            Page::Finished => &self.finished,
        }
    }

    /// La ligne cliquee, apres la borne que Java omet
    /// (`WindowQuestLog.java:160-161`, `questItems.get(row)` sur un indice qui peut
    /// valoir `-1`).
    pub fn row(&self, page: Page, at: i32) -> Result<&Row<'a>, Refusal> {
        let index = ql::checked_row(self.rows(page).len(), at).map_err(Refusal::Quest)?;
        self.rows(page).get(index).ok_or(Refusal::Quest(Ui13::NoRow))
    }

    /// Le localisateur clique sur la page courante : `-1` en entree est un refus, pas
    /// un plantage.
    pub fn track(&self, at: i32) -> Result<Locator, Refusal> {
        let page = self.page;
        locator(page, self.row(page, at)?)
    }
}

impl Default for Log<'_> {
    fn default() -> Self {
        Log::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_window_13::quest_log::{COMPLETED_MULTIPLE_KEY, COMPLETED_ONCE_KEY};

    fn instance() -> Instance<'static> {
        Instance {
            template: Some("A Lost Sheep"),
            giver_id: 4,
            giver: Some("Maria"),
            holder_id: 9,
            holder: Some("Jonas"),
            objective: Objective { index: 1, count: 3, text: Some("2 / 5 sheep") },
        }
    }

    fn finished() -> Finished<'static> {
        Finished { template: Some("A Lost Sheep"), finished: 1, max: 3 }
    }

    fn cells_of<'a>(page: Page, row: &Row<'a>) -> Cells<'a> {
        match cells(page, row) {
            Ok(cells) => cells,
            Err(refusal) => panic!("{refusal}"),
        }
    }

    #[test]
    fn the_second_line_names_the_holder_on_the_progress_page_and_the_giver_on_the_available_one() {
        let row = Row::Live(instance());
        // « en cours » : `getQuestTarget()` (`WindowQuestLogInProgressQuestQuestModule.java:92`).
        let progress = cells_of(Page::InProgress, &row);
        assert_eq!(
            progress.second.line,
            Line::Objective { literal: OBJECTIVE_SEPARATOR, text: "2 / 5 sheep" }
        );
        assert_eq!(locator(Page::InProgress, &row), Ok(Locator::Citizen(9)), "la tache, :58");
        // « disponible » : `getQuestGiverId()` (`WindowQuestLogAvailableQuestModule.java:77`).
        let available = cells_of(Page::Available, &row);
        assert_eq!(
            available.second.line,
            Line::Citizen { key: ql::GIVER_PREFIX_KEY, text: "Maria" }
        );
        assert_eq!(locator(Page::Available, &row), Ok(Locator::Citizen(4)), "le donneur, :44");
        // Deux citoyens differents pour un meme fait : c'est le sujet de ce test.
        assert_ne!(locator(Page::Available, &row), locator(Page::InProgress, &row));
        // Et les deux lignes ne portent pas la meme cle : l'objectif n'en a aucune.
        assert_eq!(available.second.line.key(), Some(ql::GIVER_PREFIX_KEY));
        assert_eq!(progress.second.line.key(), None);
    }

    #[test]
    fn the_objective_line_is_a_literal_prefix_a_gold_text_and_an_always_tooltip() {
        let row = Row::Live(instance());
        let cells = cells_of(Page::InProgress, &row);
        assert_eq!(OBJECTIVE_SEPARATOR, " - ", "Component.literal(\" - \"), :44");
        assert_eq!(cells.second.ink, Ink::Gold, ":46");
        assert_eq!(cells.second.tip, Tip::Always, "l'infobulle de l'objectif, :50-53");
        assert_eq!(cells.second.tip.ink(), Ink::White, ":51, la seule infobulle blanche");
        assert_eq!(cells.name.tip, Tip::Overflow, "les autres, :57-63");
        assert_eq!(cells.name.tip.ink(), Ink::Plain);
        assert_eq!(cells.name.ink, Ink::Plain);
        // Un litteral n'est pas une cle : aucune table de langues ne le porte.
        assert_eq!(cells.second.line.key(), None);
        assert_eq!(cells.second.line.text(), OBJECTIVE_SEPARATOR);
    }

    #[test]
    fn the_count_line_tests_the_maximum_and_never_the_count() {
        let once = Row::Done(Finished { max: 1, ..finished() });
        let cells = cells_of(Page::Finished, &once);
        assert_eq!(cells.second.line.key(), Some(COMPLETED_ONCE_KEY));
        assert_eq!(cells.second.ink, Ink::Gold, ":41");
        assert_eq!(cells.second.tip, Tip::Overflow, ":57-63");
        let times = Row::Done(finished());
        let cells = cells_of(Page::Finished, &times);
        assert_eq!(cells.second.line.key(), Some(COMPLETED_MULTIPLE_KEY));
        match cells.second.line {
            Line::Done { count, .. } => assert_eq!(count, Some((1, 3))),
            other => panic!("{other:?}"),
        }
        // Un modele unique affiche « terminee » meme a zero finition (`:33`), parce que
        // le test porte sur `getMaxOccurrence() > 1`.
        let zero = Row::Done(Finished { template: Some("A"), finished: 0, max: 0 });
        assert_eq!(cells_of(Page::Finished, &zero).second.line.key(), Some(COMPLETED_ONCE_KEY));
        // La page « terminees » n'a pas de localisateur : le XML n'en dessine pas
        // (`windowquestlog.xml:46-50`) et le moteur ne surcharge pas `trackQuest`
        // (`WindowQuestLogQuestModule.java:37`).
        assert!(Page::Finished.buttons().is_empty());
        assert_eq!(locator(Page::Finished, &once), Ok(Locator::Absent));
    }

    #[test]
    fn a_finished_quest_is_not_an_instance_and_the_wrong_page_refuses_it() {
        let done = Row::Done(finished());
        assert_eq!(cells(Page::InProgress, &done), Err(Refusal::WrongRowKind { page: Page::InProgress }));
        assert_eq!(cells(Page::Available, &done), Err(Refusal::WrongRowKind { page: Page::Available }));
        let live = Row::Live(instance());
        assert_eq!(cells(Page::Finished, &live), Err(Refusal::WrongRowKind { page: Page::Finished }));
        assert_eq!(locator(Page::InProgress, &done), Err(Refusal::WrongRowKind { page: Page::InProgress }));
    }

    #[test]
    fn a_missing_template_is_refused_and_a_missing_citizen_is_an_empty_line() {
        let orphan = Row::Live(Instance { template: None, ..instance() });
        assert_eq!(cells(Page::Available, &orphan), Err(Refusal::Quest(Ui13::UnknownTemplate)));
        let orphan_done = Row::Done(Finished { template: None, ..finished() });
        assert_eq!(cells(Page::Finished, &orphan_done), Err(Refusal::Quest(Ui13::UnknownTemplate)));
        // Le citoyen parti n'est pas un refus : `Component.empty()` (`:82`).
        let gone = Row::Live(Instance { giver: None, ..instance() });
        let cells = cells_of(Page::Available, &gone);
        assert_eq!(cells.second.line.text(), "");
        // Son id reste surligneable, meme si aucun citoyen ne le porte (`:59`).
        assert_eq!(locator(Page::Available, &gone), Ok(Locator::Citizen(4)));
    }

    #[test]
    fn an_objective_index_out_of_the_template_is_refused_not_indexed() {
        // Java passe l'index sans verifier (`:40`) : un index de sauvegarde qui ne
        // correspond plus au modele leve.
        let bad = Row::Live(Instance {
            objective: Objective { index: 3, count: 3, text: Some("x") },
            ..instance()
        });
        assert_eq!(
            cells(Page::InProgress, &bad),
            Err(Refusal::ObjectiveOutOfRange { index: 3, count: 3 })
        );
        let negative = Row::Live(Instance {
            objective: Objective { index: -1, count: 3, text: Some("x") },
            ..instance()
        });
        assert_eq!(
            cells(Page::InProgress, &negative),
            Err(Refusal::ObjectiveOutOfRange { index: -1, count: 3 })
        );
        // Le texte manquant est un refus a part : le port n'a pas le moteur qui le
        // fabrique (`:43`).
        let silent = Row::Live(Instance {
            objective: Objective { index: 1, count: 3, text: None },
            ..instance()
        });
        assert_eq!(cells(Page::InProgress, &silent), Err(Refusal::ObjectiveTextMissing { index: 1 }));
    }

    #[test]
    fn the_log_starts_on_the_progress_page_and_a_view_id_picks_the_module() {
        let mut log = Log::new();
        assert_eq!(log.page(), Page::InProgress, "windowquestlog.xml:17, premiere vue");
        assert_eq!(log.set_page("pageFinished"), Ok(Page::Finished));
        assert_eq!(log.page(), Page::Finished);
        assert_eq!(log.set_page("pageElsewhere"), Err(Refusal::Quest(Ui13::UnknownPage)));
        assert_eq!(log.page(), Page::Finished, "un id inconnu ne tourne pas la page");
        // L'id de vue est celui que le clic lit (`WindowQuestLog.java:58`).
        assert_eq!(log.set_page(ql::PAGE_AVAILABLE), Ok(Page::Available));
    }

    #[test]
    fn the_log_reloads_all_three_lists_and_the_row_bound_is_named() {
        let mut log = Log::new();
        log.reload(
            vec![Row::Live(instance())],
            vec![Row::Live(instance()), Row::Live(instance())],
            vec![Row::Done(finished())],
        );
        assert_eq!(log.rows(Page::InProgress).len(), 1);
        assert_eq!(log.rows(Page::Available).len(), 2);
        assert_eq!(log.rows(Page::Finished).len(), 1);
        // `-1` est ce que rend `getListElementIndexByPane` hors d'une liste
        // (`WindowQuestLog.java:160`) : le port refuse au lieu de lire `get(-1)`.
        assert_eq!(log.row(Page::InProgress, -1), Err(Refusal::Quest(Ui13::NoRow)));
        assert_eq!(log.row(Page::InProgress, 1), Err(Refusal::Quest(Ui13::NoRow)));
        assert!(log.row(Page::InProgress, 0).is_ok());
        // Le localisateur suit la page courante, pas la page d'ou vient la ligne.
        assert_eq!(log.set_page("pageAvailable"), Ok(Page::Available));
        assert_eq!(log.track(0), Ok(Locator::Citizen(4)));
        assert_eq!(log.set_page("pageInProgress"), Ok(Page::InProgress));
        assert_eq!(log.track(0), Ok(Locator::Citizen(9)));
        assert_eq!(log.track(1), Err(Refusal::Quest(Ui13::NoRow)));
    }
}
