# RAPPORT — la donnée de `WindowHutGuide`, `WindowHutMinPlaceholder`, `WindowHutNameEntry`, `WindowInfo`

Écrit uniquement dans `src/ui_window_04/`. `lib.rs` déclarait déjà `pub mod ui_window_04;` — je n'y ai
pas touché, ni à `Cargo.toml`, ni à `data/`, ni à `assets/`, ni au dossier d'un autre agent.

## Ce qui a été écrit

| Fichier | Lignes | Rôle |
| --- | --- | --- |
| `mod.rs` | 303 | l'en-tête et ses six prémisses fausses, `building_key`, `Owner`, `Entry`, la jointure `describe` / `describe_id` ; 4 tests |
| `refusal.rs` | 153 | les 4 refus de ces fenêtres, tous silencieux, avec la ligne qui les cache ; 3 tests |
| `minimum.rs` | 276 | `WindowHutMinPlaceholder` : la chaîne d'imbrication des deux XML, qui pose quel bouton, le titre, la visibilité du bouton `info` ; 5 tests |
| `guide.rs` | 270 | `WindowHutGuide` : les quatre panneaux et leur origine réelle, le bouton mort, le critère d'avancement, la sortie sans réarmement ; 5 tests |
| `rename.rs` | 302 | `WindowHutNameEntry` : le pré-remplissage, la coupe à quinze, la divergence UTF-16, l'effet serveur ; 4 tests |
| `info.rs` | 592 | `WindowInfo` : la règle des pages, les 18 manuels mesurés, le paginateur et ses trois transitions ; 5 tests |
| `MANQUANT.md` | 131 | les 7 manques, chacun nommé, avec ce que je voulais faire |

**1 896 lignes de Rust, 858 de commentaires portant les sources, 1 038 de code, 26 tests.** Au-dessus
des 500–900 visées, et je le dis plutôt que de le maquiller : la moitié du volume est constituée des
en-têtes et des `fichier.java:ligne`, qui sont une règle de la mission (règle 3) autant qu'une règle
du dépôt. Si tu veux couper, les en-têtes de module (`info.rs` surtout) sont la partie compressible ;
les quatre tables de constantes ne le sont pas.

## Raccordement à `src/ui` — ce que je n'ai pas décrit deux fois

**Trois des quatre fenêtres sont déjà des lignes de `src/ui`**, et ce module ne les redécrit pas :
`minimum::screen()`, `guide::screen()` et `rename::screen()` rendent le `Screen` de `src/ui` (`hut.min`,
`hut.guide`, `hut.name`), et chaque test de ces modules vérifie la ligne rendue contre le XML ou le
Java. **`WindowInfo` n'y est pas** : `src/ui` la nomme (`Effect::OpensJava("WindowInfo")`,
`ui/action.rs:436-441`) et ne la décrit pas ; c'est le seul écran que ce module décrit en entier
(`info::WINDOW`), et `mod.rs::describe` rend les deux formes sans les confondre
(`Entry::Table` / `Entry::Info`).

Réutilisé tel quel : `Screen`, `Title`, `Gate`, `Field`, `Value`, `Source`, `Scope`, `Action`,
`Effect`, `Refusal` (jamais — j'en déclare un disjoint, voir plus bas), `crate::ui::action::rename`,
`MAX_NAME_LENGTH`, `NAME_FIELD_MAX_LENGTH`, `crate::building::Kind`,
`crate::ui::table::{screen_by_id, main_screen}`, `crate::advancements::{tree, Condition, Event}`,
`crate::network::windows::ClickGuiButtonTrigger`.

**Deux jointures que ce module ferme et qui n'existaient pas :**

1. `WindowInfo` ↔ les 18 bâtiments qui ont un manuel : `info::MANUALS` est le relevé de
   `manual_en_us.json`, `manual_of` le cherche, et `minimum::open_info` rend la fenêtre ou
   `Refusal::NoInfoPage { hut }`. Un test parcourt les 54 types.
2. `WindowHutGuide` ↔ l'arbre d'avancements : `guide::clicked()` fabrique l'`Event::GuiClicked` que
   la ligne `minecolonies/check_out_guide` de `TREE` porte, et un test compare les deux — le critère
   est écrit dans trois fichiers (`DefaultAdvancementsProvider`, `advancements::tree`,
   `network::windows`) et ne dit qu'une chose.

**Un refus neuf, disjoint de `src/ui` et de `src/ui_window_02`.** Les quatre refus de ce module
(`NoInfoPage`, `GuideNotForBuilder`, `NoPages`, `UnknownWindow`) sont des boutons **cachés** ou des
`return` : Java n'écrit jamais une phrase, donc `Refusal::key()` répond `None` pour les quatre, et un
appelant ne doit pas imprimer `Refusal::name` là où le joueur attend une phrase. Aucun variant de
`src/ui::action::Refusal` n'est repris : les trois refus que ces fenêtres partageaient avec les
écrans de huttes (`NotBuilt`, `NoRoom`, `Travelling`) sont ceux des boutons `hire`/`recall`, hors
périmètre.

## Prémisses fausses, dites plutôt que contournées

1. **« les champs affichés (nom, niveau, progression, stock) » ne décrit aucune de ces quatre
   fenêtres.** Deux n'affichent **rien** de tout ça (le guide : une image et trois phrases figées ;
   `WindowInfo` : du texte paginé). Le minimum affiche un nom et un niveau
   (`AbstractBuildingMainWindow.java:171-176`) ; le renommage, un champ de saisie. La **seule**
   valeur comptée de la tranche est le numéro de page (`3/5`), et ce n'est pas une progression de
   construction : c'est un index de pagination, typé `Value::Text` et non `Value::Progress`, pour que
   le test `window_info_shows_…_and_no_progress_at_all` puisse tenir la trouvaille de `src/ui` §3.
2. **« les panneaux du guide viennent de l'avancement »** — c'est écrit dans `src/ui`
   (`Source::Advancement` sur les quatre) et c'est faux : le XML porte les textes et l'image,
   `WindowHutGuide` n'écrit aucun panneau (ses deux `registerButton` sont ses seules lignes de code),
   et l'avancement est accordé **par** le clic sur `confirm` (`DefaultAdvancementsProvider.java:176-181`)
   — s'en servir comme source serait circulaire. `guide::PANE_ORIGINS` donne l'origine réelle, et un
   test confronte les deux tables.
3. **« les quatre étendent `AbstractWindowSkeleton` »** — `WindowHutNameEntry extends BOWindow`
   (`:23`). Elle n'envoie donc ni `OpenGuiWindowTriggerMessage` ni `ClickGuiButtonTriggerMessage` :
   aucun critère d'avancement ne peut partir d'un clic dans cette fenêtre, et son `cancel` ne repasse
   pas par le `parent` du skeleton. Le renommage parle au serveur par lui-même
   (`AbstractBuildingView.java:640-644`).
4. **« le guide a deux boutons »** — Java en câble deux (`:33-34`), `windowhutguide.xml` en porte
   **un** (`:6`). `GUIDE_CLOSE` (`"close"`, `WindowConstants.java:523`) est une liaison morte, et
   `src/ui` la garde. Le test compare la ligne de `SCREENS` au fichier : c'est exactement le genre
   d'écart que cette tranche existe pour rendre visible.
5. **« le champ de renommage s'ouvre sur le nom du bâtiment »** — il s'ouvre sur le **nom
   personnalisé**, mis en minuscules et **passé à `Component.translatable`**
   (`WindowHutNameEntry.java:49`) : le nom est traité comme une clé de langue. Une hutte jamais
   renommée ouvre un champ **vide**.
6. **« le serveur coupe à quinze »** — non. `HutRenameMessage` transporte la chaîne telle quelle
   (`writeUtf`, 32 767) et `AbstractBuilding.setCustomBuildingName` la garde sans la regarder
   (`:747-750`) : la coupe est cliente, donc cosmétique et contournable. Et un renommage **renomme
   aussi toutes les commandes de travail en attente** de la hutte (`:752-763`) — un effet que ni
   `src/ui` ni la fenêtre ne mentionnent (`rename::SERVER`).

Un sixième détail de la même famille, plus petit : **Java compte des unités UTF-16 et le port des
caractères**. `String.length()` (`:59`) compte deux unités pour un emoji ; `crate::ui::action::rename`
compte un caractère. Huit emoji font seize unités — Java coupe au milieu d'une paire de substitution,
le port garde les huit. `rename::Renamed::diverges` le dit au lieu de le taire, et le test
`java_counts_utf16_units_so_an_emoji_name_diverges_instead_of_lying` le mesure. **Je n'ai pas
« corrigé » la règle de `src/ui`** : elle est dans un module qu'un autre agent possède, et la
divergence est dans la donnée, pas dans un choix de rendu.

## Vérification

`ssh deb '/home/zaccou/bin/port-check ui_window_04'` a été lancé **cinq fois** entre 00:47 et 01:23.
Mon dossier n'apparaît dans **aucune** sortie, et il y est bien apparu une fois au début (un
`super::rename` mal résolu dans `refusal.rs`, corrigé à la première passe) : les erreurs de
`src/ui_window_04` remontent donc bien dans cet outil, et leur absence est une mesure, pas une
espérance.

Le build reste **ROUGE** sur six erreurs qui ne sont pas de moi et qui n'ont pas bougé depuis
00:43 (fichiers inchangés à 01:16) :

- `src/ai_work_10/rabbit.rs:35` et `shepherd.rs:51` importent `super::state::HerdAct`, absent de
  `src/ai_work_10/state.rs` ;
- `src/ai_work_11/mod.rs:508` déclare `mod tests` une deuxième fois ;
- `src/ai_work_11/tests.rs:441`, `:445`, `:502` citent un `TradeState` qui n'existe pas.

**Conséquence, dite et non contournée : mes 26 tests n'ont pas tourné.** Tant que ces deux modules
ne compilent pas, aucun test du dépôt ne s'exécute. Je n'ai pas touché à leurs fichiers (ils ne sont
pas dans mon dossier), et je ne prétends pas qu'ils sont verts.

Deux erreurs de ma part sont tombées en route : `super::rename` au lieu de
`crate::ui_window_04::rename` dans un test (trouvée par `port-check`), et un `assert_eq!` qui
affirmait que le bouton mort du guide était dans le XML — l'inverse de la trouvaille n° 4, corrigé
contre `windowhutguide.xml:6` avant la première compilation.
