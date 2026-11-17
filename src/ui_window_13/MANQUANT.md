# MANQUANT — ce que ces trois fenêtres demandent et que le port n'a pas

Ce module n'appelle **aucune** API de l'hôte. Il n'utilise que `std`, `crate::ui`
(`Field`, `Value`, `Scope`, `Source`, `Effect`), `crate::advancements::stats::Stats`,
`crate::requests::{Queue, Request, Requester}` et `crate::inventory::ItemName`. Rien
n'a été ajouté à `crates/`, et rien n'a été inventé dans le code : chaque manque
ci-dessous est nommé, et le contrat que le port prend à sa place est dit.

## 1. Un widget de liste, et un widget de liste déroulante

**Ce que je voulais faire** : décrire les **cinq** listes de ces écrans — les requêtes
d'un atelier (`layoutselectrequest.xml:9`), les statistiques d'un bâtiment
(`layoutstatsmodule.xml:17`), et les trois `quests` du journal
(`windowquestlog.xml:21`, `:34`, `:46`) — plus la liste déroulante des intervalles
(`layoutstatsmodule.xml:7`, quatre entrées).

**Ce qui existe** : `mod_api::client::widgets::Widgets`
(`crates/horizon-mod-vocab/src/client/widgets.rs:300-322`) n'a que `add_button` et
`add_text_field`, plus un `draw` (`:394`). Ni liste, ni ligne, ni défilement, ni
liste déroulante — c'est le même manque que `src/ui/MANQUANT.md` §1, et il coûte
**cinq** listes ici au lieu de sept.

**Ce que j'ai fait** : une liste est un `Pane` de `Value::Row` plus les panneaux de sa
ligne ; les quatre états de la déroulante sont `stats::Interval::ALL`, dans l'ordre de
la `LinkedHashMap` de Java (`WindowStatsModule.java:63-66`), et l'état courant est
`stats::View::interval`. L'id `intervals` n'est enregistré par personne : un test le
constate (`WindowStatsModule.java:89` ne lie que `hidezero`).

## 2. Le lien de parenté d'une requête

**Ce que je voulais faire** : porter `getOpenRequests` telle quelle
(`WindowSelectRequest.java:84-118`), c'est-à-dire remonter la chaîne des parents d'une
requête et s'arrêter sur une requête déjà retenue.

**Ce qui existe** : `crate::requests::Request` porte un id, un objet, une quantité, un
demandeur, une priorité, un état et une assignation — **pas de parent**.
`IRequest.hasParent()` et `getParent()` n'ont pas d'équivalent, et le port n'a pas non
plus de résolveur joueur ni de résolveur « qui réessaie » : `Queue` est une file plate.

**Ce que j'ai fait** : `select_request::collect` prend `parent: impl Fn(&Request) ->
Option<i32>` en paramètre. Ce n'est pas une API inventée dans `crates/`, c'est une
couture : le jour où `Request` portera son parent, la fermeture devient
`|request| request.parent()`. Les racines (`roots`) sont également fournies par
l'appelant, parce que les deux résolveurs de Java sont un `HashSet` de jetons dont
l'ordre n'est pas une règle (`:92-94`).

**Et une divergence de sûreté, assumée** : Java n'a aucune borne de profondeur, donc
deux requêtes qui se déclarent mutuellement parents et que le prédicat refuse toutes
deux font tourner le client en rond. `collect` s'arrête à `MAX_DEPTH = 64`.

## 3. Le gestionnaire de quêtes, et le registre de modèles

**Ce que je voulais faire** : lire les trois listes du journal comme Java les lit —
`getAvailableQuests()`, `getInProgressQuests()`, `getFinishedQuests()`
(`IQuestManager.java:94`, `:101`, `:108`) — et le nom de chaque quête dans
`IQuestManager.GLOBAL_SERVER_QUESTS` (`:22`).

**Ce qui existe** : rien. Le port n'a ni `IQuestInstance`, ni `IQuestTemplate`, ni
`FinishedQuest`, ni registre statique de modèles. C'est le plus gros manque de ce
module et il ne se contourne pas : une quête n'existe pas dans les données du port.

**Ce que j'ai fait** : `quest_log::Item` est exactement ce que les deux moteurs lisent
d'une entrée — un nom de modèle optionnel, un nom de donneur optionnel, et les deux
ids (`IQuestInstance.java:37`, `:50`). L'appelant le remplit. Ce qui est porté est ce
qui est **vérifiable** : le nom se refuse quand le modèle manque (là où Java lève), le
donneur vide reste une ligne normale, et le localisateur ne vise pas la même personne
selon la page. `Source::QuestTemplates` dit la source que `src/ui` ne sait pas
exprimer, et `Pane::to_ui` répond `None` pour elle plutôt que de la ranger sous
`Source::Colony`.

## 4. Le texte d'objectif d'une quête en cours

**Ce que je voulais faire** : décrire ce qu'écrit la page « en cours »
(`WindowQuestLogInProgressQuestQuestModule.java:40-50`) : un tiret, le texte de
progression de l'objectif, en or, plus une infobulle du même texte en blanc.

**Ce qui existe** : rien — il faudrait `IQuestObjectiveTemplate.getProgressText(quest,
style)` et le `Style` de Minecraft (une couleur, `ChatFormatting.GOLD`).

**Ce que j'ai fait** : le panneau `questObjective` est décrit (id, `Value::Text`,
`Source::QuestTemplates`, `windowquestlog.xml:25`) et sa valeur ne l'est pas. Le
fichier source de ce moteur n'est d'ailleurs pas dans le périmètre du brief : il est
**nommé** par `Page::module()` et pas porté.

## 5. Un jour de colonie

**Ce que je voulais faire** : filtrer les statistiques sur la période que Java calcule
(`WindowStatsModule.java:118`, `:160`) : `day - interval` à `day`.

**Ce qui existe** : `crate::advancements::stats::Stats` porte bien les trois lectures
(`total`, `in_period`, `ids`), et il documente déjà que **le jour est un paramètre,
jamais une lecture** (`src/advancements/stats.rs:8-13`). Mais `crate::colony::Colony`
n'a pas de compteur de jours, donc rien ne peut fournir `colony.getDay()`.

**Ce que j'ai fait** : `stats::Interval::count(stats, id, today)` et
`stats::View::visible(stats, today)` prennent le jour. C'est le même contrat que
`Stats::increment`, et il rend l'erreur du départ visible : « hier » couvre **deux**
jours, parce que `getStatsInPeriod` est inclusif des deux côtés
(`StatisticsManager.java:67-77`).

## 6. Une mesure de texte

**Ce que je voulais faire** : reproduire `setText` des deux moteurs, qui pose une
infobulle **seulement** quand le texte déborde de son panneau
(`WindowQuestLogAvailableQuestModule.java:54-66`).

**Ce qui existe** : `label.getRenderedTextWidth()` et `label.getWidth()` demandent la
police et la largeur du panneau — deux choses que ce module ne porte pas (ce sont des
positions et des tailles, donc du dessin).

**Ce que j'ai fait** : `quest_log::tooltip(rendered_width, pane_width)` est la règle
seule — strictement plus large, donc un texte exactement à la largeur n'a pas
d'infobulle — et l'appelant fournit les deux largeurs. La ligne de statistique, elle,
porte **toujours** son infobulle (`WindowStatsModule.java:183`) : c'est
`stats::ROW_ALWAYS_HAS_TOOLTIP`, et les deux règles sont comparées par un test.

## 7. Un registre d'objets, pour les icônes

**Ce que je voulais faire** : remplir les deux panneaux d'icône d'une ligne de requête
— `requestStack` (la pile de l'objet demandé, animée) et `deliveryImage` (l'icône du
résolveur, `WindowSelectRequest.java:170-190`).

**Ce qui existe** : `crate::inventory::ItemName` (un identifiant d'objet) et
`Stack` (identifiant + quantité). Pas de registre d'objets, donc pas d'icône, et
`request.getDisplayStacks()` de Java (une liste de `ItemStack` qui correspondent)
n'a pas d'équivalent.

**Ce que j'ai fait** : les deux panneaux sont décrits (`Value::Icon`,
`Source::Requests`) avec leur ligne XML ; `icon_index(life_count, displayed)` porte la
seule règle calculable — l'indice avance de un toutes les trente unités de temps
(`LIFE_COUNT_DIVIDER`, `WindowConstants.java:922`) et revient au début — et
`displayed` est fourni par l'appelant.

## 8. Un client, pour les deux déclencheurs

**Ce que je voulais faire** : dire ce que ces fenêtres envoient.

**Ce qui existe** : `crate::network` porte le format de messages bien réels, mais ces
trois fenêtres n'en envoient aucun (`Window.button::sends_message` répond `false` pour
les sept boutons, un test le tient). Les deux seules choses qu'elles émettent sont
génériques à BlockUI : `OpenGuiWindowTriggerMessage` à la construction
(`AbstractWindowSkeleton.java:96`) et `ClickGuiButtonTriggerMessage` à chaque clic
(`:196`). Elles sont nommées (`GENERIC_TRIGGERS`) et **pas produites**.

**Ce que j'ai fait** : `Returns` remplace le message : ce que le clic rend à la
fenêtre qui l'a ouvert. Trois valeurs — rien, la requête, ou explicitement rien — et
c'est la seule sortie réelle de ces écrans.

## 9. Un gestionnaire de surlignage

**Ce que je voulais faire** : décrire ce que fait le localisateur d'une ligne de quête
(`WindowQuestLogAvailableQuestModule.java:42-45`) : mettre un citoyen en évidence
trente secondes.

**Ce qui existe** : rien. `HighlightManager.addHighlight` et `CitizenRenderData`
demandent un rendu dans le monde, que ce port n'a pas.

**Ce que j'ai fait** : `Effect::Highlights` (le mot de `src/ui` pour « dessine un
cadre temporaire dans le monde, sans fenêtre et sans serveur »), la durée en constante
(`TRACK_SECONDS = 30`) et la clé du surlignage (`TRACK_HIGHLIGHT_KEY`). La seule
donnée manquante est l'id du citoyen, et `quest_log::tracked(page, item)` dit **lequel**
des deux ids Java choisit selon la page.
