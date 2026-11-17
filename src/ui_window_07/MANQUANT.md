# MANQUANT — ce que ces quatre fenetres demandent et que ni le port ni le SDK n'ont

Ce module n'appelle **aucune** API de l'hote : comme `src/ui`, c'est de la donnee pure —
`std` et les types de `crate::ui::field` (`Value`, `Scope`). Rien n'a ete ajoute a
`crates/`, et `cargo horizon-mod sdk diff` doit rester vide. Les manques ci-dessous sont
donc soit **du SDK**, soit **du port**, soit **du contenu** — et chacun est nomme plutot que
contourne.

## 1. Le cout par bloc de l'analyseur : portable, et hors tranche

**Ce que je voulais faire** : calculer le score complet d'un blueprint,
`SchemAnalyzerUtil.analyzeSchematic` (`core/util/SchemAnalyzerUtil.java:120-198`) — la somme
des couts de chaque bloc, puis `+ 500` par batiment, `+ 40` par bloc different, `+ volume/10`.

**Ce qui existe** : le debut de cette somme est **atteignable**.
`TagsHost::block_in_tag(block, tag) -> Option<bool>`
(`.horizon/sdk/api/crates/horizon-mod-vocab/src/host/tags.rs:21`) repond exactement a
`state.is(TagKey)`, ce dont `getScoreFor` (`:67-115`) se sert pour ses huit tags de bloc
(`minecraft:logs`, `minecraft:planks`, `minecraft:slabs`, `minecraft:banners`,
`c:storage_blocks`, `minecraft:anvil`, `c:bookshelves`, `minecraft:base_stone_nether`).

**Ce qui manque** : le **contenu**, pas l'API. Les six tags de tier
(`ModTags.tier1blocks` … `tier6blocks`) sont du contenu MineColonies que ce port n'a pas
declare, et le passage Domum Ornamentum (`isDoBlock`, `MaterialTextureData`,
`DO_NBT_TEXTURE_DATA`) demande de lire le compound NBT d'une tile entity de blueprint — le
parseur du depot sait le faire (`port-blueprint`, 9 515/9 515 blueprints lus), pas la
tranche.

**Ce que j'ai fait** : `analyzer::complexity` porte la **fin** de la formule, celle qui ne
depend d'aucun tag (`:190-198`), et prend la somme des couts de blocs en argument ; le
resultat transporte (`cost_score`) vient de l'analyse, il n'est pas recalcule.

## 2. Un panneau de texte multi-lignes, et une liste

**Ce que je voulais faire** : rendre le panneau `text` du recit, qui recoit une **liste de
composants** et non une phrase (`WindowSupplyStory.java:104`,
`setText(List<MutableComponent>)`), et la liste des ressources de l'analyseur
(`WindowSchematicAnalyzer.java:133-158`).

**Ce qui existe** : `Widgets` (`crates/horizon-mod-vocab/src/client/widgets.rs:300`) n'a que
**deux** constructeurs, `add_button` (`:316`) et `add_text_field` (`:322`), et un `draw`
(`:394`). Pas de liste, pas de ligne, pas d'icone d'item, pas de texte en lecture seule, pas
de retour a la ligne. Les hooks d'entree existent (clic, relachement, glisser, molette,
touche, caractere) : **l'entree** d'une liste est la ; c'est son rendu qui n'est pas.

**Ce que j'ai fait** : chaque panneau est un [`Pane`] de `crate::ui::field::Value` plus ses
trois champs de ligne, dans l'ordre ou Java les remplit. `src/ui/MANQUANT.md` §1 dit le meme
manque pour les sept listes des huttes ; ce module y ajoute le **texte multi-lignes**, qui
n'est pas une liste.

## 3. Une boite temporaire dans le monde

**Ce que je voulais faire** : rendre les surbrillances d'une faute de placement —
`HighlightManager.addHighlight("placement", …)`, une `TimedBoxRenderData` avec un texte, une
couleur `0x80FF0000` et une duree de soixante secondes (`WindowSupplies.java:186-192`).

**Ce qui existe** : un hook `block_highlight` (`hooks/block_highlight.rs:14`,
`contract = "cancel"`) qui ne fait qu'**annuler** le contour du bloc vise par le joueur.
Rien qui prenne une position arbitraire, une couleur, un texte et une duree.

**Ce que j'ai fait** : `supplies::Fault` porte les quatre types, leur cle
(`item.supply.error.<type>`), et les trois constantes du rendu
(`HIGHLIGHT_KEY`, `HIGHLIGHT_ARGB`, `HIGHLIGHT_SECONDS`) avec leurs lignes. Un pont vers
`HighlightManager` demanderait la primitive qui n'existe pas.

## 4. Un identifiant d'ecran nomme

**Ce que je voulais faire** : nommer une fenetre par un id que le SDK connaitrait, pour que
`Exit::OpensHere("supplies")` s'appuie sur un type du SDK et pas sur une chaine de ce
module.

**Ce qui existe** : `ClientHost::mod_screen_open(mod_id)`
(`crates/horizon-mod-vocab/src/host/screens.rs:73`) ouvre **un** ecran par mod, et
`native_mod_screen()` dit lequel est ouvert. Pas de notion d'ecran nomme, encore moins de
fenetre qui en ouvre une autre. Meme constat que `src/ui/MANQUANT.md` §2, et il coute ici un
peu plus : `WindowSupplyStory.place` **ouvre** `WindowSupplies`
(`WindowSupplyStory.java:114`), ce que `Exit::OpensHere` nomme sans pouvoir le faire.

## 5. Un apercu de structure et un catalogue de packs

**Ce que je voulais faire** : charger `decorations/supplies/<type>.blueprint` du pack
selectionne (`WindowSupplies.java:101-118`), l'afficher a une position, le tourner, le
mirrorer — et filtrer les packs qui contiennent ce fichier (`:95`).

**Ce qui existe** : le **parseur**. Le crate `port-blueprint` lit les `.blueprint`
Structurize (GZIP/INFLATE/NBT a la main, zero dependance) et a ete verifie sur 9 515/9 515
fichiers. Ce qui n'existe pas est le **client** : `StructurePacks`, `RenderingCache`,
`BlueprintPreviewData` (position, rotation, miroir) et le rendu d'un apercu. Le SDK n'a pas
de preview de structure ; c'est pourquoi `switch` et `colonyStylePicker` sortent en
`Exit::OpensJava("WindowSwitchPack")` et `selectschematic` en
`Exit::OpensJava("WindowExtendedBuildTool")`.

## 6. Le reseau, encore

**Ce que je voulais faire** : envoyer ce que les quatre fenetres envoient.

**Ce qui existe** : rien. Les quatre messages de ce lot sont des **noms** dans
[`Message`] et aucun n'est construit : `DirectPlaceMessage`
(`WindowSuggestBuildTool.java:69`), `SwitchBuildingWithToolMessage` (`:80`),
`BuildToolPlacementMessage` (`WindowSupplies.java:151`, `:170`) et
`MarkStoryReadOnItemMessage` (`WindowSupplyStory.java:113`). Aucun n'a de gestionnaire
serveur dans ce port.

## 7. Un predicat de `src/ui` qui ne couvre pas un outil (cote port)

**Ce que je voulais faire** : marquer le panneau `size` de l'analyseur comme **calcule**,
puisqu'il est bati de trois entiers (`WindowSchematicAnalyzer.java:373-377`) comme le
`stock` du builder l'est de deux.

**Ce qui existe** : `crate::ui::field::Value::is_computed` repond `true` pour
`Progress | Step | Stock | Occupancy` seulement — les quatre panneaux calcules d'une
**hutte**. Il repond donc `false` pour ce `Text`.

**Pourquoi je ne l'etends pas** : `src/ui/field.rs` est a un autre agent, et etendre le
predicat changerait le sens de trois tests de `src/ui` (`four_values_are_computed_and_eight_are_read`).
Le fait est donc ecrit ici et dans l'en-tete du module, pas dans le code d'un autre.
