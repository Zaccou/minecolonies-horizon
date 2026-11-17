# MANQUANT — ce que ces quatre gestionnaires réclament et que le port n'a pas

Écrit à la main pour ce module, comme `src/manager/MANQUANT.md` et
`src/manager_01/MANQUANT.md`. Chaque entrée dit **ce qui manque, d'où ça vient, ce que ça
coûte**, et la sortie exacte quand elle existe.

**Aucun des quatre fichiers n'appelle l'hôte.** Ils lisent des `Compound`, des `Value` et des
`&'static` ; tout ce qui suit est donc un fait qui doit venir du dehors, et rien ici n'est du code
fictif.

**Un gestionnaire du brief était déjà couvert pour sa moitié intéressante** : la lecture d'un
bâtiment depuis son compound est `crate::building::Building::from_compound`
(`src/building/instance.rs:373-421`) plus `crate::persistence::workshop::read`
(`src/persistence/workshop.rs:56-124`). `src/manager_02/building_data.rs` n'ajoute donc que la
question du **type** — dont l'alias `home` — et l'ordre qui fait qu'aucune écriture n'a lieu avant
un refus ; le détail est dans le rapport et dans la doc de `mod.rs`. Le seul gestionnaire réellement
absent était le **magasin** de `CustomRecipeManager`, que `src/crafting/crafter.rs:48-52` et
`src/crafting/MANQUANT.md` §6 nomment tous les deux comme « une couche au-dessus ».

---

## 1. `CustomRecipe.parseTemplate` — l'expansion d'un template

**Ce qui manque.** Un fichier `recipe-template` ne décrit pas une recette : il décrit **une recette
par membre d'un tag**, avec un filtre d'identifiants. Le Java est
`CustomRecipe.parseTemplate` (`core/colony/crafting/CustomRecipe.java:402-439`), plus ses deux
aides : `parseArrayOrStringFilter` (`:442-460`, un filtre est une sous-chaîne d'identifiant, ou une
liste de sous-chaînes, ou un booléen par défaut) et `populateTemplate` (`:463-…`, qui réécrit
`inputs`/`secondary`/`result`/`alternate-output`/`additional-output` pour un membre donné, et peut
rendre `null` pour rejeter ce membre).

**D'où ça vient.** Le service de tags **existe** : `Tags::items(&TagKey)` rend les membres d'un tag
(`crates/horizon-mod-vocab/src/world/tags.rs:45-52`) et `Tags::key` traduit un identifiant en
`TagKey` (`:27-34`). Ce qui manque est le reste de `parseTemplate` : le filtre d'identifiants
(`:442-460`), la réécriture JSON par membre (`populateTemplate`, `:463-…`, qui peut rejeter un
membre) et la construction de l'id de recette par membre (`:430`). Côté port, `src/crafting/` n'a
pas non plus de `Facts` qui répondrait à la question : l'appartenance d'un item à un tag y est
déclarée comme une question d'hôte (`Question::InTag`, `src/crafting/ingredient.rs:288`) que
personne ne répond encore.

**Ce que ça coûte.** `Store::resolve_templates` prend donc l'expansion en paramètre
(`impl FnMut(&Identifier, &Value) -> Result<Vec<CrafterRecipe>, E>`) : le magasin, ses refus et son
ordre d'écriture sont portés et testés, les recettes produites ne le sont pas. Un serveur qui
appelle `resolve_templates` avec une fonction qui refuse tout obtient zéro recette de template et
un échec nommé par template — l'état exact d'aujourd'hui, mais visible.

---

## 2. `openBuildingBrowser` et la moitié « vue » de `createViewFrom`

**Ce qui manque.** `BuildingDataManager.openBuildingBrowser` (`:97-101`) ouvre
`WindowBuildingBrowser` — une fenêtre client, donc rien à porter dans un mod sans écran. Et
`createViewFrom` (`:77-95`) fait deux choses : il lit le nom du bâtiment **dans le
`FriendlyByteBuf`**, puis il délègue le reste du tampon à `view.deserialize(networkBuffer)`. Le
port ne garde que la première moitié (`view_kind`), qui est celle qui décide si une vue existe.

**D'où ça vient.** Les vues de bâtiment sont `src/network/building_modules.rs` et
`src/network/views.rs` ; leur désérialisation réseau est un type à elles, pas à ce gestionnaire.

**Ce que ça coûte.** `view_kind` répond « cette vue a-t-elle un type connu », et le refus nommé
remplace le `null` de Java. Une vue `minecolonies:home` est refusée, comme en Java — il n'y a pas
d'alias `home` sur ce chemin (`:80-87`), contrairement au chemin de sauvegarde.

---

## 3. La forme réseau d'une extension — un **indice de registre**

**Ce qui manque.** `extensionToBuffer` écrit `buf.writeRegistryId(registry, type)`
(`BuildingExtensionDataManager.java:88-95`) et `bufferToExtension` le relit avec
`readRegistryIdSafe` (`:73-80`). Un `writeRegistryId` **forge** écrit l'**indice** de l'entrée dans
le registre du jeu, pas son identifiant — un entier qui dépend de l'ordre d'enregistrement.

**D'où ça vient.** Rien dans le SDK ne donne à un mod l'indice d'une entrée dans un registre forge :
`crates/horizon-mod-vocab/src/` n'a pas d'équivalent de `IForgeRegistry`, et le contrat de
`payload_codec` (`PayloadReader::var_int`) permet d'écrire l'entier mais pas de garantir qu'il
désigne la même entrée chez le client et le serveur. `EXTENSIONS` est dans l'ordre de
`ModBuildingExtensionsInitializer` (`:23-71`), donc l'indice *serait* la position dans cette table
si le jeu a conservé cet ordre — ce n'est pas démontrable depuis cet arbre, et ce n'est donc pas une
affirmation du port.

**Ce que ça coûte.** Aucune extension ne peut être synchronisée. Les deux payloads NBT, eux, sont
portés et font un aller-retour exact ; c'est la trame d'un octet de registre qui manque, pas la
donnée.

---

## 4. Le corps d'une recette sur le réseau — 35 champs, et un message sans layout

**Ce qui manque.** `CustomRecipe.serialize`/`deserialize`
(`core/colony/crafting/CustomRecipe.java:886-969`) et l'enveloppe
`CustomRecipeManager.serializeNetworkData` (`:384-406`). L'enveloppe est simple et portée en
esprit par `Store::ingest` : `reset()` d'abord (`:414`), `crafterNum` puis `recipeNum` par crafter
(`:416-422`), les tables de loot ensuite (`:424-434`), l'événement `CustomRecipesReloadedEvent` en
dernier (`:436-443`).

**D'où ça vient, et c'est mesuré.** Le message est **déjà déclaré sans layout** dans le port :
`src/channel/registry.rs:313` lui donne l'id 129, la classe Java et la direction `Client`, et
`rust: ""` — pas de type de port. Le corps d'une recette est `src/crafting/crafter.rs` (le modèle),
pas ce module ; sa sérialisation est le trou déclaré de ce dossier-là.

**Ce que ça coûte.** `ingest` prend des recettes et des tables **déjà décodées**. Un client ne peut
pas recevoir le magasin d'un serveur : il faut d'abord le type de port de 129, puis les 35 champs.
Un détail à ne pas oublier le jour où on l'écrit : le format Java **n'écrit pas la clé du crafter**
(`:386-394` n'écrit qu'un compte et les recettes), parce que chaque recette porte son propre champ
`crafter` — l'information est redondante des deux côtés.

---

## 5. `LootTableAnalyzer` — l'analyse, pas la table

**Ce qui manque.** `LootTableAnalyzer.toDrops` (`core/colony/crafting/LootTableAnalyzer.java:80-…`)
marche un JSON de table de butin — pools, rolls, entrées pondérées, conditions, alternatives — et en
tire une liste de [`LootDrop`] avec une probabilité, une qualité et un drapeau « conditionnel ».
C'est explicitement approximatif (`:33-38`).

**D'où ça vient.** La table elle-même **est lisible** : `Host::resource(PackType::ServerData,
"minecraft:loot_table/blocks/oak_log")` rend son JSON
(`crates/horizon-mod-vocab/src/host/resources.rs:8-22`, qui lit « les data packs du monde choisis
par-dessus les données du jeu et des mods »). Ce qui manque est l'**analyse** — pools, rolls,
entrées pondérées, conditions, alternatives, les 468 lignes de `LootTableAnalyzer` — et les deux
sources qui ne sont pas des fichiers : les tables des blocs de culture (`ModBlocks.getCrops()`,
`:324-330`) et celles des modules d'élevage et d'artisanat (`BuildingEntry.getALlModuleProducers()`,
`:332-355`).

**Ce que ça coûte.** `Store::loot_ids` porte la **première moitié** de `buildLootData` : l'ensemble
des tables — les `loot-table` des recettes, puis ce que l'appelant ajoute (les blocs de culture,
`:324-330`, les modules d'élevage et d'artisanat, `:332-355`, et la pêche, `:357-358`) — dédupliqué
en gardant le premier (`:363`). `replace_loot` porte la seconde : **vider puis remplir**
(`:360-365`). Ce qui manque est la valeur des drops, pas leur ensemble.

---

## 6. `isValidPlacement` et `FarmField.create` — deux questions au monde

**Ce qui manque.** `FarmField.isValidPlacement` lit le bloc à la position et compare
(`FarmField.java:97-104` : `blockScarecrow`), `PlantationField` en accepte deux
(`PlantationField.java:53-60`), et `FarmField.create(position, level)` lit la taille du champ sur le
`TileEntityScarecrow` (`:82-94`) — donc un état de bloc et une entité de bloc.

**D'où ça vient.** Une question à l'hôte par position — `Host::level_block_state(level, pos)`
(`crates/horizon-mod-vocab/src/host/levels.rs:304`) — et un `LevelRef`, que seuls des appelants
qui ont un monde possèdent ; c'est le rôle de la pose (`src/blocks/`).

**Ce que ça coûte.** Un champ peut être créé à une position où rien de valide ne se trouve ; le
port ne le sait pas. `FarmField::default()` ne devine pas non plus : un champ neuf a
`DEFAULT_RANGE` dans les quatre directions (`:41`, `:58`), et c'est `create` qui écrasait cette
valeur avec celle du scéarecrow.

---

## 7. Note de structure — cinq copies d'un écrivain de position

Ce n'est pas une API manquante, c'est une duplication assumée : `src/colony/nbt.rs:17-25`,
`src/building/nbt.rs:20-32`, `src/requests/nbt.rs:23-32`, le `mod nbt` en ligne de
`src/persistence/mod.rs:533-556` et maintenant `src/manager_02/nbt.rs` écrivent tous les trois mêmes
champs `x`/`y`/`z` de `BlockPosUtil.java:219-226`. La raison est la même à chaque fois — `mod nbt;`
privé dans chaque domaine — et `src/building/nbt.rs:8-12` l'a déjà écrite. Le correctif tient en une
ligne par fichier d'un autre agent (`pub(crate) mod nbt;` puis suppression des copies en trop) ; je
ne l'ai pas fait, et je le signale plutôt que de le taire.

`src/blueprint/nbt.rs` est **public**, et mes lectures passent par lui (`tag`, `as_text`,
`as_i64`) : seules les écritures sont locales.
