# Ce qui manque a l'hotel de ville

Ce que ce module **demande** et que le port n'a pas. Chaque entree dit ce que je
voulais faire, et ce qui existe a la place. Aucune n'est comblee par du code
fictif : la valeur arrive en parametre, ou le clic est refuse par un nom.

## 1. Le verrou Patreon — un appel HTTPS, et rien d'autre

`WindowMainPage.checkFeatureUnlock` ouvre une connexion a
`https://auth.minecolonies.com/api/minecraft/<uuid>/features` dans un thread et
range la reponse dans un `static AtomicBoolean` (`WindowMainPage.java:312-349`).
Le port n'a ni client, ni reseau, ni `static` de session.

Voulu : un champ « debloque » lu par la page. Fait : `visuals(unlocked, owner,
texture_is_default)` prend le booleen en parametre. **Aucune donnee du monde ne
le contient** : c'est le seul booleen de ces quatre pages qu'aucune sauvegarde ne
peut fournir.

## 2. Le jour de la colonie

Le filtre des evenements est `event.getDay() >= colony.getDay() - interval`
(`WindowInfoPage.java:119`). `crate::colony::Colony` n'a pas de jour : il a un
id, un nom, un niveau, une dimension, un centre, des citoyens, des batiments, des
ordres et des membres.

Fait : `keeps(day, colony_day, interval)` prend le jour en parametre.

## 3. Un citoyen n'a qu'un id et un nom

`crate::colony::Citizen` porte `id` et `name`, et le dit : « Java's `ICitizenData`
carries a job, a family, a happiness; none of that belongs to the colony record ».
La page des citoyens lit huit choses de plus : metier, vie, vie maximale, joie,
saturation, sexe, position de sommeil, et les onze competences de l'infobulle.

Fait : `citizen::Vue` est la vue que l'appelant remplit, depuis
`crate::citizens` (metier, besoins) et `crate::citizens::happiness`. Manquent
encore :

- **le libelle du sans-emploi** : Java affiche `getJobComponent()`, un composant
  traduit. `Job::Placeholder` n'est pas cela (c'est un metier du registre). Donc
  `Vue::job_text` rend `None`, et un sans-emploi n'est filtre que par son nom.
- **les competences** de l'infobulle (`WindowCitizenPage.java:191-199`) : le
  port a `crate::jobs::employment` pour les niveaux, pas de quoi les attacher a
  un citoyen.

## 4. Le registre des blocs, et l'air

`addBlock` resout l'identifiant dans `ForgeRegistries.BLOCKS` et ecarte l'air
(`WindowPermissionsPage.java:542-548`) : un identifiant inconnu ne fait **rien**
et n'affiche **rien**. Le port n'a pas de registre.

Fait : `looks_like_block(text)` dit si le texte a la **forme** d'un
`ResourceLocation`. Il ne peut pas dire si le bloc existe. Une entree inconnue
sera donc acceptee ici et refusee par le serveur, la ou Java la laisse tomber en
silence — deux comportements differents, et c'est le port qui s'ecarte.

## 5. Les rangs personnalises

`Rank` est une enum fermee de cinq et [`Rank::mask`] **calcule** le `long` de
chaque rang avec le `switch` en cascade de `Permissions.java:173-219`. Java
stocke ce `long` par rang (`:569`) et en cree d'autres a la volee (`:1054-1076`).

Ce qui ne peut donc pas s'ecrire :

| Ce que la page fait | Ce qui existe |
| --- | --- |
| `buttonAddRank` — creer un rang | refus `CustomRanksUnsupported` |
| `removeRank` — supprimer un rang | refus `StockRankInitial` (les cinq sont `isInitial`, `Permissions.java:171`) |
| `rankTypePicker` — changer le type d'un rang | refus `RankBitsDerived` (le type se deduit de `is_manager`/`is_hostile`) |
| `trigger` — armer un bit | refus `RankBitsDerived` (le bit se deduit du masque) |

Lire, en revanche, marche entierement : `can_alter` est `canAlterPermission`
recopie, et `on_off` est `hasPermission(Rank, Action)`.

## 6. Le nom d'affichage d'un ordre de travail

`workOrder.getDisplayName()` est un composant construit **par le serveur** a
partir du type de travaux, du nom du batiment et du niveau cible. Le port a
`WorkOrder.building` (une chaine de type), `work` et `target_level`.

Fait : `one_line(display_name)` porte la seule regle de la page — `\n` remplace
par `": "` (`WindowInfoPage.java:292`). La composition du nom n'est pas portee.

## 7. Le nom de l'ouvrier d'un ordre

La ligne `assignee` cherche le batiment a `workOrder.getClaimedBy()` dans le
gestionnaire **client** et prend son `getWorkerName()`
(`WindowInfoPage.java:281-288`). `WorkOrder::claimed_by()` rend une position ;
il n'y a pas de nom d'ouvrier attache a un ordre.

Fait : le panneau declare `Reader::WorkOrders` et la valeur reste a fournir.

## 8. Les listes de la colonie que `Colony` ne porte pas

Aucune de ces six choses n'a de champ dans `crate::colony::Colony`, et chacune
est un panneau de ce module : les evenements de colonie (`getColonyEvents`), les
evenements de permission (`getPermissionEvents`), les blocs et positions libres
(`getFreeBlocks`, `getFreePositions`), le pack de structure
(`getStructurePack`), le style de texture et de noms (`getTextureStyleId`,
`getNameStyle`), la couleur d'equipe (`getTeamColonyColor`) et l'horodatage du
mercenaire (`getMercenaryUseTime`).

Fait : chaque panneau dit quel lecteur repond ([`Reader`]), et les regles
prennent la valeur en parametre.

## 9. Le reseau

Vingt-huit boutons, dont douze portent le **nom** d'un message de
`core/network/messages/` (`PermissionsMessage.AddPlayer`, `WorkOrderChangeMessage`,
`ColonyTextureStyleMessage`, `GiveToolMessage`, `RecallSingleCitizenMessage`,
`ChangeFreeToInteractBlockMessage`, `TeamColonyColorChangeMessage`,
`ColonyStructureStyleMessage`, `ColonyNameStyleMessage`, `PermissionsMessage.Permission`).
Aucun n'est envoye : il n'y a ni client ni gestionnaire cote serveur dans ce port.

## 10. Le widget de liste

Comme le dit `src/ui/MANQUANT.md` : le SDK n'a que `add_button` et
`add_text_field` (`crates/horizon-mod-vocab/src/client/widgets.rs`). Les **dix**
listes de ces quatre pages sont des `ScrollingList` : joie et citoyens, les deux
de la page d'information (evenements, ordres de travail), et les six de la page
des permissions (joueurs, evenements de permission, joueurs en ligne, boutons de
rang, rangs, blocs libres), plus les infobulles qui vont avec.

## 11. L'ordre de la liste de joie

Java itere une `HashMap` (`WindowCitizenPage.java:218-226`) : l'ordre des lignes
change d'une execution a l'autre. `crate::citizens::happiness` garde l'ordre
d'enregistrement, qui est stable. Le port est donc **plus** deterministe que
Java, et ce n'est pas un defaut a corriger.

## 12. Deux defauts gardes

1. `fillEventsList` inverse la liste **vivante** de la colonie a chaque
   `updateElement` (`WindowPermissionsPage.java:440`) et `addPlayerToColonyClicked`
   indexe la liste non inversee avec un indice de ligne inverse (`:203-207`) :
   le mauvais joueur est ajoute, et l'ordre des lignes se retourne a chaque
   rendu. Non reproduit : le port ne garde ni l'un ni l'autre, et le refus
   `UnknownFreeEntry` est la seule chose que la page nomme vraiment.
2. Les deux conditions opposees de l'icone de l'entite (`:147-152` contre
   `:281-289`) : `citizen::entity_icon_visible` garde celle qui gagne a
   l'execution, et la dit.
