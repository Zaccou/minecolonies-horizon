# MANQUANT — ce que ces quatre fenêtres auraient voulu et que le port n'a pas

Ce module n'appelle **aucune** API de l'hôte : comme `src/ui`, c'est de la donnée pure.
Il n'utilise que `std`, `crate::ui`, `crate::colony` et `crate::blocks` (pour lire les
variantes du rack). Rien n'a été ajouté à `crates/`. Les manques ci-dessous sont donc
soit **du SDK**, soit **du port** — et chacun est nommé plutôt que contourné.

## 1. Une grille de cases

**Ce que je voulais faire** : décrire ce que le rack dessine vraiment — 27 à 108 cases en
`rows × columns`, plus les 36 cases du joueur (`ContainerRack.java:103-139`), avec les
deux textes de titre.

**Ce qui existe** : `mod_api::client::widgets::Widgets`
(`crates/horizon-mod-vocab/src/client/widgets.rs:300-322`) n'a que **deux**
constructeurs, `add_button` et `add_text_field`. Il n'y a ni image, ni case, ni grille,
ni texte libre. Les entrées sont couvertes (clic, relâchement, glisser, molette, touche,
caractère, `widgets.rs:1-10`) : c'est le rendu qui manque.

**Ce que j'ai fait** : [`rack::Grid`] porte la géométrie que Java calcule
(`WindowRack.java:87-96`) et [`rack::WINDOW`] déclare **zéro panneau et zéro bouton**,
parce que c'est la vérité du fichier. Un pont vers `Widgets` demanderait le widget
manquant, pas une ligne de plus dans cette tranche.

## 2. Deux textes qui n'ont pas de panneau

**Ce que je voulais faire** : décrire les deux seuls textes du rack — son titre et
« Inventaire » — comme les autres écrans décrivent leurs panneaux.

**Ce qui existe** : `crate::ui::Field` exige un `id`, et **ces deux textes n'en ont pas** :
`WindowRack.java:105-106` appelle `drawString` deux fois, sans
`findPaneOfTypeByID` et sans XML. Un `Field` leur inventerait un id que BlockUI n'a
jamais cherché.

**Ce que j'ai fait** : deux constantes de position — [`rack::TITLE_POS`] et
[`rack::inventory_title_pos`] — et `rack::WINDOW.fields` **vide**. Un test vérifie que le
compte reste zéro : c'est un fait du source, pas un oubli.

## 3. Le métier d'un citoyen

**Ce que je voulais faire** : écrire la ligne d'un résident, qui porte **son métier et son
nom** (`WindowHutLiving.java:95`).

**Ce qui existe** : `crate::colony::Citizen` s'arrête à `id` et `name`, et le dit :
« Java's `ICitizenData` carries a job, a family, a happiness; none of that belongs to the
colony record » (`src/colony/colonie.rs:57-64`). `crate::ui::Source::Citizen` envoie donc
une ligne de résident vers un enregistrement qui n'a pas la moitié de son texte.

**Ce que j'ai fait** : [`living::Resident`], qui **enveloppe** `Citizen` et ajoute le
métier, plutôt qu'un second type de citoyen ou un champ ajouté à l'enregistrement de
colonie. Le nom reste celui de `Citizen` ; il n'y a pas deux noms possibles.

## 4. La fenêtre des espions

**Ce que je voulais faire** : décrire ce que le bouton `hireSpies` ouvre.

**Ce qui existe** : `WindowBarracksBuilding.java:106` ouvre `WindowsBarracksSpies`, une
classe de `core/client/gui/` qui n'est ni une fenêtre de hutte ni une ligne de `src/ui`.
Ce module la nomme par `crate::ui::Effect::OpensJava` et s'arrête là.

**Ce que j'ai fait** : rien de plus, et c'est délibéré — c'est une cinquième fenêtre, pas
un panneau de la caserne. Elle demanderait sa propre tranche (`WindowsBarracksSpies`,
`WindowHireWorker` pour les espions, et le message de recrutement associé).

## 5. Une table de langue

**Ce que je voulais faire** : rendre les phrases que ces fenêtres affichent — les trois
paliers de distance (`TranslationConstants.java:187-191`), les onze directions
(`:11-31`), le libellé « n/max » (`:861`), le nom du métier d'un résident.

**Ce qui existe** : le port n'a **pas** de résolution de clés de langue côté mod, et les
fichiers `en_us.json` de MineColonies ne sont pas dans l'arbre de référence
(`assets/minecolonies/lang/` ne contient que `manual_en_us.json`).

**Ce que j'ai fait** : les clés, jamais les phrases. [`barracks::SpawnRow::keys`],
[`living::Occupancy::key`] et [`living::row_text`] rendent des clés et des compositions
littérales (`"n: "`), pas des traductions. Inventer « assez près, au nord-ouest »
donnerait un texte faux dans les onze langues du mod ; une clé ne ment jamais.

## 6. Une table d'assets

**Ce que je voulais faire** : vérifier à l'exécution que la texture calculée par
`WindowRack.getCorrectTextureForSlots` (`:136-146`) existe.

**Ce qui existe** : `assets/minecolonies/` ne porte que des `blockstates`, `items`,
`models` et `textures/block` — aucune texture de `gui/`, et aucune API du port pour
résoudre un chemin d'asset.

**Ce que j'ai fait** : [`rack::ROW_TEXTURES`] est **mesuré** dans l'arbre Java
(`assets/minecolonies/textures/gui/gui81.png`, `gui90`, `gui99`, `gui108`, plus
`generic_108.png`), et [`rack::texture_for_rows`] rend
[`refusal::Refusal::MissingRowTexture`] pour tout le reste. Un test balaie les vingt
combinaisons réellement atteignables et prouve qu'aucune n'y tombe — la formule de Java
est juste sur l'espace des tailles possibles, elle ne l'est pas au-delà.
