# MANQUANT — ce que les gestionnaires de `manager_06` réclament et que le port n'a pas

Écrit à la main pour ce module, comme `src/manager/MANQUANT.md` et
`src/research/MANQUANT.md`. Chaque entrée dit **ce qui manque, d'où ça vient, ce que
ça coûte**, et — quand elle existe — la sortie exacte. Aucun fichier de ce module
n'appelle l'hôte : les règles, les tables et les plans sont des valeurs, testables
sans monde. Tout ce qui suit est donc un fait qui doit venir du dehors.

---

## 1. `StatisticsManager` est déjà porté — sauf sa moitié réseau

**Ce qui manque.** `serialize` et `deserialize` sur un `FriendlyByteBuf`, avec le
`Set<String> dirtyStats` (`core/colony/managers/StatisticsManager.java:104-178`). Un
envoi partiel ne porte **qu'un jour par identifiant** (`:139`,
`dataEntry.lastIntKey()`) et un identifiant dont la carte a été vidée s'envoie avec
une taille de 0, que le lecteur interprète comme une suppression (`:164-168`).

**Où est le reste.** L'état complet — `id` → `jour` → `quantité`, l'incrément qui
s'ajoute au jour, le total, la période inclusive, `clear`, `aggregateStats` et tout le
composé `stat_manager` — est `crate::advancements::stats`
(`src/advancements/stats.rs`), avec les huit noms de `StatisticsConstants` en
constantes. `Stats::latest_day` répond déjà « quel jour un envoi partiel porte ».

**Ce que ça coûte.** Rien sur le disque, qui est complet. Il manque **le canal** : le
SDK n'expose pas d'envoi par joueur, donc `dirtyStats` n'a pas de destination et une
colonie ne peut pas synchroniser ses compteurs avec un client. `crate::network`
(`views.rs`, `colony_settings.rs`) décrit les formes des messages ; il n'y a pas de
liste d'abonnés à qui les remettre.

**La sortie.** Une primitive d'envoi par joueur (ou un hook « un joueur entre dans un
chunk ») : alors `Stats` pourrait exposer `serialize(full_sync: bool) -> Vec<(String,
Vec<(i32, i32)>)>` et le module aurait sa moitié réseau.

---

## 2. Les trois annonces de recherche n'ont pas de destinataires

**Ce qui manque.** `MessageUtils.format(...).sendTo(colony).forAllPlayers()`
(`ResearchManager.java:195`, `:245-248`, `:256-259`) et le
`SoundUtils.playSuccessSound(player, player.blockPosition())` qui le suit (`:196-199`,
`:249-252`, `:260-263`).

**Où c'est.** `autostart::Message` porte les **clés** — `RESEARCH_AVAILABLE`,
`MESSAGE_RESEARCH_STARTED`, `RESEARCH_CONCLUDED` + une variante sur trois
(`TranslationConstants.java:346`, `:400`, `:670`) — dans l'ordre où Java les
concatène. `crate::network::sounds` décrit déjà l'effet sonore.

**Ce que ça coûte.** Le joueur ne voit ni n'entend l'annonce : une recherche coûteuse
reste « disponible » silencieusement, et une recherche instantanée se termine sans
message. La règle, elle, est complète : les clés ne peuvent pas être autre chose que
celles du fichier de langue.

**La sortie.** La même primitive d'envoi par joueur que le §1.

---

## 3. `recallAllTravellingCitizens` téléporte une entité que le port n'a pas

**Ce qui manque.** `TeleportHelper.teleportCitizen(entity, world, spawnHutPos)`
(`TravellingManager.java:92`), le `citizenData.setNextRespawnPosition(
EntityUtils.getSpawnPoint(world, spawnHutPos))` du citoyen sans entité (`:87-88`), le
`citizenData.updateEntityIfNecessary()` (`:88`) et la ligne de log « has returned very
confused » (`:86`).

**Où c'est.** `travelling::Book::recall` rend un `Vec<Recall>` : l'identifiant, **où**
il va (son bâtiment de travail, sinon l'hôtel de ville, `:74-81`) et s'il avait une
entité à téléporter. La règle de destination est donc posée, l'effet ne l'est pas.

**Ce que ça coûte.** Un rappel ne déplace personne : il dit où. Un citoyen rappelé
sans entité ne reçoit pas de point de réapparition, donc s'il meurt avant de rentrer
il réapparaît là où il était.

**La sortie.** Un handle d'entité par citoyen et une primitive de téléportation dans
`host.levels()` — le SDK n'en déclare aucune
(`crates/horizon-mod-vocab/src/host/levels.rs` liste des lectures de chunk, pas des
écritures de position d'entité).

---

## 4. La génération de nom d'un enfant n'est portée nulle part

**Ce qui manque.** `CitizenData.generateName(rand, firstParentName, secondParentName,
nameFile)` (`core/colony/CitizenData.java:685-740`) : l'ordre des parties selon
`CitizenNameFile.NameOrder.EASTERN`, le nombre de parties (3 → `Prénom I. Nom`, 2 →
`Prénom Nom`, autre → `Prénom`), le choix du nom de famille pris à l'un ou l'autre
parent, et la **récurrence** de `:664-673` qui retire un nom déjà pris — récurrence
que rien ne borne dans Java.

**Où c'est.** `reproduction::Write::GenerateName { first, second }` porte les deux noms
qu'il faut au générateur et s'arrête là : le port ne fabrique pas de nom. La table des
noms elle-même est un fichier de données de la colonie (`colony.getCitizenNameFile()`),
donc un fait du dehors.

**Ce que ça coûte.** Un appelant qui applique le plan sans générateur donne à l'enfant
un nom vide. C'est visible, et préférable à un nom inventé : deux enfants homonymes
cassent `/colony citizens` et les liens de famille, qui sont indexés par nom chez
`isRelatedTo` (`:791-795`).

**La sortie.** Un port de `CitizenNameFile` + `generateName` (deux fichiers, une
table de données) et une règle de borne sur la récurrence — Java essaie indéfiniment,
le port devrait refuser après N essais et le dire.

---

## 5. L'état de famille d'un citoyen appartient à `CitizenData`, qui n'est pas porté

**Ce qui manque.** `partner`, `siblings`, `children`, `parents`, `textureSuffix`
(`CitizenData.java:98`, `:791-795`, `:1986-2001`) et les deux effets d'une assignation
(`LivingBuildingModule.onAssignment` / `onRemoval` : `setHomeBuilding` +
`calculateMaxCitizens`, `:89-100`).

**Où c'est.** `reproduction::Candidate` porte les **cinq** faits que la décision lit,
et `Write::{SetPartner, InheritSiblings, AddChild, SetParents, SetSuffix, Assign}`
nomme les écritures que l'appelant doit faire sur le vrai `CitizenData`. Aucun
`CitizenData` n'existe dans ce port : `crate::citizens` porte l'état, les besoins, la
priorité et l'emploi, pas la famille.

**Ce que ça coûte.** Les liens de famille ne survivent pas à un rechargement tant que
`CitizenData` n'a pas ses champs. `isRelatedTo` reste une fonction pure sur une copie,
donc une deuxième naissance dans la même session relit la copie de l'appelant, pas la
vérité.

**La sortie.** Les cinq champs dans `crate::citizens::state` (avec la clé NBT que Java
leur donne) : alors `Candidate` se construit depuis `CitizenData` au lieu d'être une
copie.

---

## 6. `getHouseWithSpareBed` n'a pas d'équivalent : la maison est une entrée du plan

**Ce qui manque.** La boucle de `RegisteredStructureManager.java:493-512` : un
bâtiment qui a un `LivingBuildingModule`, dont le `HiringMode` n'est **pas**
`LOCKED`, et dont `getAssignedCitizen().size() < getModuleMax()` — où `getModuleMax()`
est le **niveau du bâtiment** (`LivingBuildingModule.java:103-106`).

**Où c'est.** `crate::building::behaviour::census` (`amelioration.rs:198`) compte les
lits d'une colonie, ce qui est la même matière, mais il ne choisit pas *la* maison : le
port prend `Gate::spare_bed_house` en entrée, et `Situation::alt` pour les voisines à
moins de 50 blocs (`ReproductionManager.java:135`).

**Ce que ça coûte.** Un appelant qui répond mal place l'enfant dans une maison pleine
ou verrouillée ; le plan ne le lui reproche pas, il l'assigne.

**La sortie.** Une requête `spare_bed()` sur `crate::building` (niveau, module
`living`, mode d'embauche), alors `Gate::spare_bed_house` se calcule au lieu d'être
donné.

---

## 7. Les deux tirages de `CitizenSkillHandler` viennent du dehors

**Ce qui manque.** `colony.getCitizenManager().getRandomCitizen()`
(`CitizenSkillHandler.java:74-90`) quand un parent manque, et le
`(int) colony.getOverallHappiness()` qui plafonne le premier tirage (`:92`).

**Où c'est.** `reproduction::inherit_skills(base, models, bonus)` prend les deux
modèles et le bonus ; `Situation::happiness` porte le plafond. Un mod wasm n'a pas de
RNG (`mod_api` n'en expose aucun, cf. `src/research/MANQUANT.md` §1), donc les onze
tirages de `init(levelCap)` et le `25 + rand.nextInt(25)` sont de l'appelant.

**Ce que ça coûte.** Rien de silencieux : un appelant qui passe des zéros obtient un
enfant sans bonus, et `inherit_skills` dit pourquoi (total nul → aucun point, comme
`(int) (NaN * bonus)` en Java).

**La sortie.** Un RNG semé dans l'hôte, ou un `Draws` unique partagé par les trois
managers au lieu des deux traits séparés (`reproduction::Draws` et les paramètres
d'`autostart`).

---

## 8. Le drapeau `dirty` du gestionnaire de recherche n'a pas de destinataire

**Ce qui manque.** `markDirty()` / `isDirty()` / `sendPackets(closeSubscribers,
newSubscribers)` (`ResearchManager.java:79-113`) et son message
`ColonyViewResearchManagerViewMessage`.

**Où c'est.** `autostart::Check::dirty` dit **quand** la colonie doit être sauvée —
Java marque à la fin de chaque vérification, quoi qu'elle ait fait (`:210-211`) — et
chaque `Plan` de démarrage commence par `Write::MarkDirty` (`:220`).

**Ce que ça coûte.** Une recherche qui se termine sans que rien ne soit sauvé revient
« en cours » au rechargement, et se rejoue. Le port ne peut pas l'empêcher : c'est
l'appelant qui écrit.

**La sortie.** Le même canal que le §1, plus une écriture de la colonie dans le
`BlockEntity` de l'hôtel de ville (`crate::wiring`).
