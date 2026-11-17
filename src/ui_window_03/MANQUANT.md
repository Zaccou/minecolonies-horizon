# MANQUANT — ce que ces quatre fenêtres auraient voulu et que le port n'a pas

Ce module n'appelle **aucune** API de l'hôte : c'est de la donnée pure, comme `src/ui`,
`src/jobs` et `src/colony`. Il n'utilise que `std`, `crate::ui` (`Field`, `Value`, `Source`,
`Scope`, `HiringMode`, `next_hiring_mode`, `next_sort`, `SORT_STAGES`, `Refusal`) et une
seule fonction d'une autre tranche, `crate::ui_window_02::browser::path_segment`, qui porte
déjà `Utils.getBlueprintLevel`. Rien n'a été ajouté à `crates/`. Les manques ci-dessous
sont donc soit **du SDK**, soit **du port**, et chacun est nommé plutôt que contourné.

## 1. `Pane.off()` / `Pane.on()` de BlockUI — la moitié du vocabulaire des refus

**Ce que je voulais faire** : dire exactement ce qu'un bouton éteint est. `WindowHireWorker`
ne désactive pas ses boutons, il les **éteint** : `off()` sur `done`, `fire`, `pause`,
`restart` (`:411-447`), et `on()` pour les rallumer.

**Ce qui existe** : rien dans l'arbre. BlockUI est un mod séparé : `Pane.off()` n'est pas
dans `/workspace/minecolonies-src`, donc je ne peux pas citer la ligne qui dit si « éteint »
veut dire *caché*, *désactivé*, ou les deux.

**Ce que j'ai fait** : [`hire::RowButtons`] porte un `bool` par bouton et sa doc dit « Java
ne peut pas le cliquer » — la seule chose que le source de MineColonies prouve. Le jour où
BlockUI entre dans l'arbre, ce fichier est le seul à relire.

## 2. `StringUtils.getLevenshteinDistance` (Apache Commons)

**Ce que je voulais faire** : trier comme `:254`, par distance d'édition entre le nom et le
filtre.

**Ce qui existe** : la **règle** est dans le source (`allItems.sort(Comparator.comparingInt(s1 ->
StringUtils.getLevenshteinDistance(...)))`), l'**implémentation** non — Apache Commons n'est
pas dans l'arbre. Deux détails de bord ne sont donc pas vérifiables ici : le comportement sur
les paires de substitution, et le départage des égalités (qui dépend ensuite du tri stable de
`List.sort`).

**Ce que j'ai fait** : [`inventory::levenshtein`] est l'algorithme classique à deux lignes, et
un test le tient sur les cas qui décident de l'ordre affiché (`Stone` à 5 de `""`,
`Redstone` à 3 de `stone`). Ce qui est **mesuré**, c'est l'ordre de la liste, pas la
bibliothèque.

## 3. Une origine « argument » pour un panneau

**Ce que je voulais faire** : dire d'où viennent `title` et `warning` de
`WindowConfirm`. Ils ne viennent d'aucun modèle : le constructeur les reçoit
(`WindowConfirm.java:21`), et ses deux appelants y mettent une clé de traduction
(`WindowBuildBuilding.java:209`) ou une phrase déjà traduite
(`ConnectionModuleWindow.java:120`).

**Ce qui existe** : `crate::ui::field::Source`, un ensemble **fermé** de neuf origines, qui
appartient à `src/ui` — une tranche qu'un autre agent écrit. Aucune ne dit « l'appelant ».

**Ce que j'ai fait** : `Source::Building`, qui nomme le cas réel de l'avertissement, et un
commentaire au-dessus de [`confirm::FIELDS`] qui dit que c'est un rapprochement. Je n'ai pas
ajouté de variante : ce serait éditer le fichier d'un autre.

## 4. Un rack, et un bâtiment qui a des conteneurs

**Ce que je voulais faire** : `WindowHutAllInventory` lit les **racks** de la hutte
(`:207-232` : `getContainers()` puis `TileEntityRack.getAllContent()`), et sa loupe redemande
à chaque rack le compte d'un item (`:128-147`, `getCount(stack, ignoreDamageValue, false)`).

**Ce qui existe** : `crate::hut::State` — niveau, embauchés, colonie, position — et rien sur
les conteneurs. Aucun `TileEntityRack` n'est porté, et `crate::inventory` ne connaît pas les
racks d'une hutte.

**Ce que j'ai fait** : [`inventory::Stack`] est une pile **déjà agrégée** (`name`, `id`,
`tooltip`, `amount`) que l'appelant fournit, et [`inventory::highlight`] ne prend que le
compte, pas le rack. Prouvé ici : la somme, le filtre, le tri et la couleur ; pas prouvé : que
la somme vienne de tous les racks.

## 5. La liste des citoyens d'une hutte

**Ce que je voulais faire** : remplir la liste de `WindowHireWorker` : nom, enfant ou non,
en pause, en déplacement, logement, lieu de travail, et `canAssign` du module.

**Ce qui existe** : `crate::colony::Citizen` — `id` et `name`, rien de plus — et
`crate::jobs::Employment`, qui compte des employés sans dire où ils habitent.

**Ce que j'ai fait** : [`hire::Candidate`] déclare les six faits que la fenêtre lit et les
reçoit de l'appelant. `assignable` est le résultat de `canAssign` **pour le module
sélectionné** : Java le recalcule à chaque changement de métier (`:300`), donc l'appelant
doit le recalculer aussi, et [`hire::State::select_job`] le dit dans sa doc.

## 6. Un envoi au serveur

**Ce que je voulais faire** : que les cinq classes nommées fassent quelque chose :
`HireFireMessage`, `PauseCitizenMessage`, `RestartCitizenMessage`,
`BuildingHiringModeMessage`, `DecorationBuildRequestMessage`.

**Ce qui existe** : aucune route « message d'un client vers un serveur » côté hôte, et le
port n'a pas de tranche réseau. `src/network` porte les **formes** des messages et leurs
identifiants (`src/channel/registry.rs:246` pour `DecorationBuildRequestMessage`), pas leur
envoi.

**Ce que j'ai fait** : [`hire::Turn`] garde l'**ordre** des envois — deux `HireFireMessage`
pour un citoyen qui change de métier, l'ancien bâtiment d'abord — parce que c'est la seule
chose qu'un futur envoi devra respecter et qu'un test peut tenir aujourd'hui.

## 7. L'état d'une session, et la touche Maj

**Ce que je voulais faire** : deux états que Java lit hors de la fenêtre — la session
(`static int sortDescriptor`, `WindowHutAllInventory.java:63`) et le clavier
(`Screen.hasShiftDown()`, `:329`).

**Ce qui existe** : rien. Le port n'a pas de « session de jeu » et pas d'état d'entrée
clavier ; le SDK a des hooks de touche et de caractère
(`crates/horizon-mod-vocab/src/client/widgets.rs:1-10`), pas un « est-ce que Maj est
enfoncée maintenant ».

**Ce que j'ai fait** : [`inventory::Session`] est un objet que l'appelant garde aussi
longtemps qu'il garde la session, et `rows(session, stacks, shift)` prend le booléen en
argument. C'est exactement ce que fait `src/ui_window_02` pour la position du joueur : une
valeur d'entrée, pas un accès au monde.

## 8. Dessiner une surbrillance dans le monde

**Ce que je voulais faire** : `locate` ajoute une **boîte** colorée sur chaque rack qui
contient l'item, avec le compte écrit dessus, pendant 60 secondes
(`WindowHutAllInventory.java:140-144`, `TimedBoxRenderData`).

**Ce qui existe** : aucun appel de rendu de boîte dans le SDK des mods. Le port n'a pas de
`HighlightManager`.

**Ce que j'ai fait** : [`inventory::highlight`] calcule la seule chose qui soit une règle —
la couleur en fonction du compte — et un test la tient sur les quatre valeurs qui comptent
(1, 32, 48, 64). La boîte elle-même, sa durée et son texte sont nommés
([`inventory::HIGHLIGHT_SECONDS`], `:143`) et pas dessinés.
