# MANQUANT — ce que les quatre fenêtres de l'hôtel de ville demandent et que le port n'a pas

Ces quatre écrans sont décrits en **donnée pure** : aucun appel d'hôte, aucun socket,
aucune horloge, aucun fichier. Ce qui suit est donc soit du SDK, soit du port — et chaque
manque dit **ce que je voulais faire**, pas seulement qu'il existe.

## 1. Un écran qui s'ouvre

**Ce que je voulais faire** : ouvrir réellement ces quatre fenêtres — `create` passe à
`WindowTownHallColonyManage`, `cancel` ferme, `done` rouvre l'hôtel de ville.

**Ce qui existe** : `ClientHost::mod_screen_open(mod_id)`
(`crates/horizon-mod-vocab/src/host/screens.rs:73`) ouvre **un** écran par mod, et
`Widgets` n'a que `add_button` et `add_text_field`
(`crates/horizon-mod-vocab/src/client/widgets.rs:300-322`). Il n'y a pas d'écran nommé,
donc pas quatre écrans.

**Ce que j'ai fait** : chaque fenêtre est un [`crate::ui_window_17::Window`] avec ses
panneaux et ses boutons, et chaque clic rend un `Out` typé. `Leaves::Closes`,
`Leaves::Reopens` et `Out::Manage` sont des **noms** : aucune ligne ne ferme ni n'ouvre
rien. Le jour où le SDK nomme un écran, c'est `Leaves` qu'on traduit, pas la table.

## 2. La vue de colonie (`IColonyView`)

**Ce que je voulais faire** : lire les trois valeurs que ces fenêtres lisent sur la
colonie — `getName()` (`WindowTownHallNameEntry.java:39`), `getCitizenCount()`
(`WindowTownHallMercenary.java:41`), `getMercenaryUseTime()`
(`ColonyView.java:1308-1312`), et poser `mercenaryLastUseTime` au clic
(`ColonyView.java:1314-1317`).

**Ce qui existe** : `crate::colony::Colony` — un enregistrement **serveur** (nom, niveau,
citoyens, bâtiments, membres). Il n'y a pas de vue cliente, et
`ColonyViewMessage` voyage en `Vec<u8>` opaque (`src/network/views.rs`, `MANQUANT.md` §1 de
ce module-là) : `mercenaryUseTime` est écrit dans ce tampon et n'en ressort pas.

**Ce que j'ai fait** : le compte des citoyens se calcule sur `Colony.citizens`, ce qui est
le même nombre (`ColonyView.java:586-590` lit `citizenCount`, que le serveur envoie depuis
son gestionnaire) ; `mercenary_use_time` et `now` sont des **paramètres** de
[`crate::ui_window_17::mercenary::ready`]. Un décodeur de `ColonyViewMessage` les
remplira sans toucher à cette règle.

## 3. La configuration serveur

**Ce que je voulais faire** : lire `allowInfiniteColonies` comme la classe le fait
(`WindowTownHallDeleteAbandonColony.java:56`).

**Ce qui existe** : rien. Le port n'a aucun lecteur de configuration : `crate::founding::Rules`
porte quatre distances, pas ce drapeau.

**Ce que j'ai fait** : [`crate::ui_window_17::delete_abandon::warning`] prend le booléen.
La branche, la clé et la présence du bouton `abandon` restent une seule décision.

## 4. Le pack de récits

**Ce que je voulais faire** : tirer un pré-nom et un récit du pack `colony_stories`, filtré
par biome (`ColonyStoryListener.java:88-97`, `:241-264`).

**Ce qui existe** : ni chargeur de ressources, ni `SimpleJsonResourceReloadListener`, ni
registre de biomes. Le pack n'est pas dans l'arbre du mod (il est généré).

**Ce que j'ai fait** : [`crate::ui_window_17::reactivate::StoryText`] et
[`crate::ui_window_17::reactivate::BiomeFilter`] reproduisent la **forme** et la
**sélection** — `All` l'emporte, un tag ou un id filtre, l'ordre du pack décide — et le
générateur est `java.util.Random` à l'identique, pour que « la même hutte raconte la même
histoire » soit vrai entre Java et le port. Les entrées, elles, sont fournies par
l'appelant. Un `resolve` avec une liste vide n'invente rien : il rend
`Refusal::NoStoryMatch` et `""`, exactement ce que fait `pickRandom`.

**Et une chose que Java ne peut pas faire** : `biome.unwrapKey().get()` (`:51`) est un
`get()` sur un `Optional`. Un biome sans clé — un biome de runtime — **fait planter la
fenêtre**. Le port ne peut pas représenter ce cas, donc il ne le refuse pas : `Biome`
exige un id.

## 5. Les portraits et le son

**Ce que je voulais faire** : dessiner jusqu'à neuf têtes de citoyen
(`WindowTownHallMercenary.java:48-58`) et jouer trois sons — `BOOK_PAGE_TURN` à
l'ouverture des deux écrans `AbstractWindowSkeleton` (`:40`) et `LIGHTNING_BOLT_THUNDER`
au clic (`:68`).

**Ce qui existe** : ni rendu d'image, ni couche de son. Les quatre suffixes
(`CitizenData.java:98`) sont une constante de ce module ; le son est une constante
([`crate::ui_window_17::mercenary::HIRE_SOUND`]).

**Ce que j'ai fait** : le **compte** est porté ([`crate::ui_window_17::mercenary::Offer`] :
`citoyens / 10 + 3` mercenaires, neuf portraits), l'image ne l'est pas. Et la tête choisie
n'est **pas reproductible**, même en Java : `ColonyConstants.rand` est un `Random` sans
graine (`ColonyConstants.java:25`), donc chaque ouverture redonne quatre têtes différentes.

## 6. Rouvrir l'hôtel de ville

**Ce que je voulais faire** : rendre le `getClientBuildingManager().getTownHall().openGui(false)`
du renommage (`WindowTownHallNameEntry.java:58-61`) et le `close()` du squelette
(`AbstractWindowSkeleton.java:254-262`).

**Ce qui existe** : `crate::colony::Colony` n'a pas de gestionnaire de bâtiments **cliente**
— un `IClientBuildingManager` n'est pas une collection de vues.

**Ce que j'ai fait** : [`crate::ui_window_17::Leaves`] est repris de
`crate::ui_window_06` et nomme les trois fins possibles ; `Refusal::NoTownHall` dit le cas
où Java ne rouvre rien. Le clic `Ignored` du renommage garde son `Leaves::Stays` : Java
sort avant la réouverture (`:53-56`).

## 7. Le pack de structure sélectionné

**Ce que je voulais faire** : compléter le cinquième argument que l'écran de réactivation
transmet — `reactivate ? "" : StructurePacks.selectedPack.getName()`
(`WindowTownHallColonyManage.java:82`).

**Ce qui existe** : `StructurePacks` est un registre de packs staturize côté client, que le
port n'a pas.

**Ce que j'ai fait** : `Handoff::pack_name()` rend `Some("")` en réactivation — la seule
branche que **cette** fenêtre décide — et `None` sinon, avec la raison écrite. Inventer un
nom de pack serait pire qu'un `None`.

## 8. Le texte composé

**Ce que je voulais faire** : rendre les trois textes du récit tels que le joueur les lit.

**Ce qui existe** : pas de couche de langue dans ce port. `Component.translatable` n'a pas
d'équivalent, donc une phrase ne peut pas être produite.

**Ce que j'ai fait** : les fonctions rendent les **clés** et leurs **arguments** — par
exemple `title` rend `(TITLE_KEY, pre_name)` — ce qui est exactement ce que le client
aurait à traduire. Les arguments du récit sont au nombre de deux
(`pre_name`, clé de biome, `:51`) et la clé du biome est calculée par
`Biome::language_key`, comme `toLanguageKey("biome")` (`:51`).

## Ce qui n'est *pas* un manque

Le tag `DEACTIVATED` de la hutte, dont dépend la vraie réactivation, **existe** dans le
port : `crate::blocks::Facts::deactivated` (`src/blocks/mod.rs:843-845`), lu par
`crate::blocks::click`. Ce module ne le consulte pas — il décrit un écran, pas un clic de
bloc — mais `Handoff::claim()` et ce drapeau disent la même chose, et c'est cette paire que
le jour serveur devra faire tenir ensemble (`CreateColonyMessage.java:131-142`).
