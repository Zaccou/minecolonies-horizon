# MANQUANT — ce que ces trois fenêtres demandent et que le port n'a pas

Ce module n'appelle **aucune** API de l'hôte : comme `src/ui`, `src/jobs` et `src/colony`, c'est de la
donnée pure. Il n'utilise que `std`, `crate::ui` (ses types, pas ses valeurs) et `crate::jobs::Job`.
Rien n'a été ajouté à `crates/` ni à `Cargo.toml`. Les manques ci-dessous sont donc **du SDK ou du
port**, et chacun est nommé plutôt que contourné.

## 1. Une `Source` pour le compound d'un objet

**Ce que je voulais faire** : dire d'où vient la liste de tours de la fenêtre de ralliement, sans
mentir. Elle ne vient pas de la colonie : elle vient du tag `guardtowerpositions`
(`NbtTagConstants.java:606`) de la bannière que le joueur tient dans la main, lu par
`ItemBannerRallyGuards.getGuardTowerLocations` (`ItemBannerRallyGuards.java:333`).

**Ce qui existe** : `crate::ui::field::Source`, neuf variantes, aucune pour « le compound d'un objet
tenu ». La plus proche est `PlayerInventory` — « lu seulement pour colorer une ligne », dit sa doc.

**Ce que j'ai fait** : j'ai emprunté `Source::PlayerInventory` et j'ai écrit pourquoi, dans l'en-tête
de `rally.rs`. La bonne réponse est une dixième variante `Source::Item` dans `src/ui` ; c'est le
dossier d'un autre agent, donc je ne l'ai pas touché.

## 2. Une `Source` pour un registre

**Ce que je voulais faire** : décrire la liste des motifs de bannière, qui vient de
`BuiltInRegistries.BANNER_PATTERN.holders()` (`WindowBannerPicker.java:152`) et non d'un bâtiment, d'un
citoyen ou d'une requête.

**Ce qui existe** : rien. `Source` n'a pas de variante « registre », et le port n'a aucun accès aux
registres de jeu depuis un mod.

**Ce que j'ai fait** : la section « ce que l'écran demande » de `banner.rs` liste les trois entrées de
`Picker` et dit, en toutes lettres, que la troisième n'est pas portée. `excluded`, `visible`,
`blocked` et `scroll_bound` prennent donc un **nombre** de motifs ou un **namespace** en argument :
les règles sont là, le catalogue ne l'est pas.

## 3. `Action` ne porte qu'un effet, et son refus est une enum fermée

**Ce que je voulais faire** : décrire les boutons de la fenêtre de ralliement avec
`crate::ui::action::Action`, comme le reste du port.

**Ce qui existe** : `Action { id, label, effect, refusals }`, où `effect` est **un** `Effect` et où
`refusals` est `&'static [crate::ui::action::Refusal]`. Quatre handlers de ces trois fenêtres font deux
choses (`WindowBannerPicker.java:234-241`, `:243-249`, `WindowBannerRallyGuards.java:164-168`,
`LivingBuildingView.java:87-91`), et trois refus de ce module — `NoSuchTower`, `EmptyRallyList`,
`BaseLayerNotRemovable` — ne sont pas dans cette enum.

**Ce que j'ai fait** : `Button` = un `Effect` + un `Afterwards` (rouvrir, se fermer, rien) + les refus
de **ce** module, et `Panel` = la même forme que `Screen` avec des `Button`. C'est une deuxième forme,
et elle est justifiée par deux faits de Java, pas par un goût. Le jour où `src/ui` acceptera un second
effet et un refus extensible, ces deux types disparaissent. En attendant, `Button::action()` rend
l'`Action` équivalente **quand le bouton n'a aucun refus propre**, et un test le vérifie.

## 4. `Gate` ne porte qu'un avancement

**Ce que je voulais faire** : dire que la fenêtre de ralliement ne s'ouvre que sur une liste non vide.

**Ce qui existe** : `crate::ui::screen::Gate::Advancement(&'static str)`, une seule variante, et
`Screen.gate: Option<Gate>`.

**Ce que j'ai fait** : `rally::may_open(&towers) -> Result<(), Refusal>` avec `EmptyRallyList`. La
précondition est une **donnée**, pas un avancement (`ItemBannerRallyGuards.java:220-226`), donc elle
n'a pas sa place dans `Gate` — et la refuser par son nom vaut mieux que de tordre la variante qui
existe.

## 5. Le nombre de places d'une maison

**Ce que je voulais faire** : appeler `hire` avec les places de la maison.

**Ce qui existe** : `LivingBuildingModuleView.getMax()` rend `maxInhabitants`, un champ du module
synchronisé depuis le serveur (`LivingBuildingModuleView.java:106-109`). Le port n'a pas ce module :
`crate::building::Module` sauvegarde une **clé** et des données, pas un maximum d'habitants, et
`crate::hut::capacity` est une autre règle — un habitant par niveau, pour les **têtes d'un métier**
(`src/hut.rs:158-162`, qui le dit lui-même).

**Ce que j'ai fait** : `places: i32` est un **paramètre** de `hire`, `hire_tooltip` et `may_assign`.
Aucune des deux sources n'est substituée à l'autre : un niveau de hutte n'est pas un nombre de lits.

## 6. La maison et le lieu de travail d'un citoyen

**Ce que je voulais faire** : calculer les trois distances d'une ligne — c'est le cœur de l'écran.

**Ce qui existe** : `crate::colony::Citizen { id, name }` et rien d'autre (`src/colony/colonie.rs:61-64`).
Pas de `home`, pas de `work`, pas de métier.

**Ce que j'ai fait** : `Row { citizen, home, work }` est une **entrée** du module, pas un état du port :
`job_line`, `assigned_job_line`, `listable` et `unassigned` la prennent en argument. Le jour où
`colony::Citizen` portera ces deux positions, `Row` se construit depuis lui sans qu'une règle change.

## 7. Le drapeau de colonie

**Ce que je voulais faire** : ouvrir le sélecteur sur le drapeau enregistré.

**Ce qui existe** : `crate::colony::Colony` sauvegarde un id, un nom, un niveau, une dimension, un
centre, des citoyens, des bâtiments et des ordres (`src/colony/colonie.rs:32-39`) — **pas de drapeau**.
Côté Java, la clé est `Patterns` (`NbtTagConstants.java:473`) et le défaut est déjà une couche
BASE/BLANC (`ColonyView.java:110-112`).

**Ce que j'ai fait** : `initial_layers(&[Layer]) -> Vec<Layer>` prend le drapeau en argument et rend
les couches que le sélecteur édite ; la lecture et l'écriture du tag restent à la tranche de
persistance.

## 8. La vue de bâtiment d'une tour de garde

**Ce que je voulais faire** : lire le nombre de gardes d'une tour.

**Ce qui existe** : `Source::Building` nomme cette vue, et `src/ui/MANQUANT.md` §6 dit déjà que le port
n'a pas de vue de bâtiment cliente : `crate::building::Building` porte un type, une position et un
niveau.

**Ce que j'ai fait** : `Tower { location, guards: Option<usize> }` — `guards: None` est exactement le
cas « la tour n'existe plus » que Java dessine avec un cookie (`WindowBannerRallyGuards.java:126-134`).
Le port ne peut donc pas encore distinguer « pas encore résolu » de « détruite » ; c'est écrit ici.

## 9. `isFeatureUnlocked` est un appel HTTP

**Ce que je voulais faire** : décider si les motifs du namespace du mod sont verrouillés.

**Ce qui existe** : Java ouvre une connexion à `https://auth.minecolonies.com/api/minecraft/<uuid>/features`
sur un thread de fond et lit un booléen (`WindowMainPage.java:312-345`). Le port n'a aucun client HTTP,
et n'en aura pas : un mod wasm ne doit pas appeler un domaine privé au chargement d'un écran.

**Ce que j'ai fait** : `unlocked: bool` est un paramètre de `Picker`, `blocked` et `pattern_colour`.
Le verrou reste une règle testable ; sa source est hors du jeu, et c'est dit.

## 10. Deux choses vues en passant, à trancher par d'autres

- **`src/ui` écrit une étiquette comme un champ.** L'écran `assign` de `src/ui/screen.rs` déclare
  `Field::new("buildingassignmentmode", Value::Text, Source::Module("living"))`. Ce panneau a un `id`
  mais **aucune classe ne l'écrit** : c'est le libellé statique du bouton `mode`
  (`windowassigncitizen.xml:32`, `label="$(com.minecolonies.coremod.gui.buildingassignmentmode)"`), et
  le littéral n'apparaît nulle part dans l'arbre Java. Ma table le range dans
  `constants::ASSIGN_STATIC_LABELS`, pas dans les constantes, et un test vérifie que les huit autres
  panneaux de cet écran, eux, sont bien des constantes de `WindowConstants`. **Je n'ai pas corrigé
  `src/ui`** : ce n'est pas mon dossier.
- **Un `TODO` de Java est périmé.** `WindowBannerPicker.java:197` porte
  `// TODO: port this last vital condition` à côté de `activeLayer < layers.size() && activeLayer != 0` :
  la condition **est** écrite. Ce n'est pas un manque, c'est un commentaire qui a survécu à son
  correctif ; le port suit le code, pas le commentaire, et le dit dans `banner.rs`.
