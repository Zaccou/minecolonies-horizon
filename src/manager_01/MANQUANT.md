# MANQUANT — ce que ces trois gestionnaires réclament et que le port n'a pas

Écrit à la main pour ce module, comme `src/manager/MANQUANT.md`. Chaque entrée dit **ce qui
manque, d'où ça vient, ce que ça coûte**, et la sortie exacte quand elle existe.

**Deux gestionnaires du brief ne sont pas ici, et ce n'est pas un oubli** : `ColonyManager` est
déjà porté dans `src/manager/` (fichier par fichier, avec son propre `MANQUANT.md`), et la moitié
NBT de `CitizenDataManager` (`createFromNBT`, `:14-20`) est déjà portée dans
`src/persistence/roster.rs`. Les trois fichiers de ce dossier couvrent ce qui restait : le son,
les surbrillances, et la moitié réseau des vues de citoyens.

Rien de ce module n'écrit de NBT, et c'est correct : les trois états sont **transitoires côté
client** en Java aussi. `SoundManager.soundQueue` et `HighlightManager.HIGHLIGHT_ITEMS` sont des
`static` de client, et `ColonyView.citizens` vit le temps d'une session. Aucun des quatre n'est
sauvegardé.

---

## 1. Aucun hook client n'appelle ces gestionnaires

**Ce qui manque.** `SoundManager.tick` est appelé depuis `ColonyManager.onClientTick`
(`ColonyManager.java:613-629`, l'appel à `:628`) et `HighlightManager.render` depuis le rendu des
événements de monde (`WorldEventContext`). Les deux **hooks existent** dans le SDK :
`client_tick_post` (`crates/horizon-mod-vocab/src/hooks/client_tick_post.rs:1-13`, une fois par
tick client, écran-titre compris) et `render_level_stage`
(`hooks/render_level_stage.rs:1-30`, dont les lignes et les quads se dessinent à l'étape demandée).

**D'où ça vient.** `src/lib.rs` ne souscrit qu'à cinq hooks — `block_entity`,
`right_click_block`, `register_commands`, `run_mod_command`, `entity_join_level` — et il
appartient à l'orchestrateur, pas à ce module.

**Ce que ça coûte.** [`sound::SoundQueue::tick`] et [`highlight::Highlights::frame`] existent, se
testent, et **ne tournent pas** dans le jeu : la file d'un citoyen ne se vide pas, une surbrillance
ne s'éteint pas. Le jour où `client_tick_post` est souscrit, c'est une ligne — `queue.tick(level !=
None)` par client — et rien d'autre à écrire ici.

**La sortie.** Deux entrées de plus dans le `Hooks` de `src/lib.rs`, plus un état client où loger
la file et la table des surbrillances (le SDK a `mod_state` pour ça).

---

## 2. Dessiner la surbrillance n'existe pas dans ce crate

**Ce qui manque.** `IHighlightRenderData` a trois appels de rendu (`startRender`, `render`,
`stopRender`) et Java en a une dizaine d'implémentations dans
`core/client/render/worldevent/highlightmanager/` — une boîte (`TimedBoxRenderData`), une ligne,
un texte, une empreinte de pas.

**D'où ça vient.** Le port n'a aucun rendu de monde : pas de `WorldEventContext`, pas de
gestionnaire de tampons de lignes, et un mod wasm ne peut pas en fabriquer un. Le SDK expose le
hook (`render_level_stage`) et un contexte de dessin dont les **lignes et les quads** se dessinent
à l'étape demandée — donc le contrat existe, mais la géométrie d'une boîte colorée est à écrire,
et elle dépend d'un contexte que cette tranche n'a pas.

**Ce que fait le port.** Il porte **la règle** et pas le dessin : [`highlight::Step`] dit *quand*
un `startRender`, un `render` et un `stopRender` auraient lieu, dans quel ordre, et à quel tick un
highlight expire. C'est la moitié testable, et celle où Java a trois pièges (voir la note du
module).

**Ce que ça coûte.** Aucune boîte ne s'affiche. Ce qui est déjà écrit, en revanche, ne sera pas à
refaire : quand un contexte de dessin de monde existera, `frame()` donnera la liste des boîtes à
dessiner et la durée de chacune.

**La sortie.** Un contexte de dessin de monde dans le SDK (lignes et quads, couleur ARGB) plus une
souscription à `render_level_stage` — deux choses, pas une.

---

## 3. Jouer le son : `play_local_sound` oui, `player_play_sound` non

**Ce qui manque.** Java joue **côté client, à une position**, pour le joueur local :
`player.level.playSound(player, pos, event, source, volume, pitch)`
(`SoundManager.java:51-56`).

**Ce qui existe.** Deux appels, et un seul fait l'affaire :

| Appel | Côté | Ce qu'il fait | Convient ? |
| --- | --- | --- | --- |
| `play_local_sound(&SoundPlay, after)` | client | joue pour ce client, `SoundPlay::at` **à une position**, atténué par la distance (`host/effects.rs:38-48`, `client/types.rs:556-590`) | **oui**, c'est `playSound` un pour un |
| `player_play_sound(player, …)` | serveur | joue **au joueur**, sans position (`host/players.rs:917-927`) | non : une voix de citoyen doit venir de là où il est |

**Ce que fait le port.** [`sound::Playback`] rend les six valeurs que Java passe (`speaker`,
`sound_event`, `source`, `pos`, `volume`, `pitch`) et ne joue rien. La conversion est de trois
lignes chez l'appelant :

```ignore
let play = SoundPlay::at(&playback.sound_event, source, [
    f64::from(playback.pos[0]),
    f64::from(playback.pos[1]),
    f64::from(playback.pos[2]),
])
.with_volume(playback.volume)
.with_pitch(playback.pitch);
host.play_local_sound(&play, 0);
```

**Pourquoi `source` reste un entier.** C'est l'ordinal de vanilla, exactement comme le garde
`crate::network::sounds::PlaySoundForCitizen` (`sound_source: i32`). Le SDK a bien un type
(`client/types.rs:493-530`, onze variantes dans l'ordre de vanilla) ; le convertir une fois, au
moment de jouer, évite d'avoir deux endroits qui peuvent se désynchroniser sur un ordinal.

---

## 4. Le champ d'une vue de citoyen n'est pas porté ici, et c'est volontaire

**Ce qui manque.** `CitizenDataView.deserialize` lit une quarantaine de valeurs dans un ordre
précis (`core/colony/CitizenDataView.java:340-420`) : nom, sexe, entité, pause, enfant, maison,
travail, saturation, bonheur, compétences, métier, colonie, inventaire, position, options de
dialogue, quêtes, famille, statut.

**D'où ça vient.** Ce n'est pas la règle d'un gestionnaire, c'est le format d'un message ; et la
plupart de ces champs ont déjà un propriétaire dans le port (`crate::entity::data` pour les clés
du citoyen, `crate::citizens` pour les besoins, `crate::jobs` pour le métier).

**Ce que fait le port.** [`views::Views<T>`] est **générique** : il porte la question du
gestionnaire — *à qui l'update s'applique, avec ce que le client savait déjà* — et laisse le
décodage à l'appelant, sous forme de fermeture. Un test l'instancie avec le `crate::colony::Citizen`
du port, pour que le registre ne soit pas générique par défaut.

**Ce que ça coûte.** Aucune vue de citoyen réelle ne voyage : le port n'a pas de `ColonyView` à
mettre à jour, parce qu'il n'a pas de paquets de vue (même raison que `src/manager/MANQUANT.md`
§2). Ce qui est porté, c'est l'ordre des écritures et les refus — dont celui qui compte : une
donnée illisible coûte **un citoyen**, pas la colonie.

**La sortie.** Le décodeur de `CitizenDataView` dans le module qui possède les champs du citoyen,
et un `ColonyView` pour l'appeler. `Views::apply` prend alors `|previous| decode(buf, previous)`.

---

## Divergences assumées avec Java, et pourquoi

1. **Les trois maps sont ordonnées.** Java parcourt des `HashMap`
   (`SoundManager.java:23`, `HighlightManager.java:13`, `ColonyView.java:95`) : l'ordre est
   indéfini, et deux exécutions peuvent choisir deux gagnants. Le port prend l'ordre des clés.
   Conséquence dite franchement : quand six voix parlent et que le budget en avance quatre, **ce
   sont toujours les quatre mêmes** ; Java en aurait avancé quatre au hasard.
2. **Le décode d'une vue est atomique.** `createFromNetworkData` réutilise l'objet et écrit dedans
   avant la lecture qui lève (`CitizenDataManager.java:25-36`) : Java garde donc, en cas d'échec,
   une vue **à moitié réécrite** et la garde dans la map (le `put` est sauté, l'objet est déjà
   là). Le port produit la nouvelle valeur puis l'écrit : un refus laisse l'ancienne intacte. Un
   champ de moins actualisé, plutôt que quarante à moitié faux.
3. **Un lot est atomique.** La boucle des visiteurs et des animaux (`ColonyView.java:830-853`,
   `:858-880`) n'a pas de `try` : une entrée illisible jette hors du gestionnaire avec la map à
   moitié appliquée. `Views::apply_batch` décode tout, puis écrit tout — et ne vide donc même pas
   la map quand une entrée refuse.
4. **`MAX_CONCURRENT_SOUNDS` vaut 5 et le budget 4.** Le compteur de Java démarre à 1
   (`SoundManager.java:41`) et s'arrête à la constante (`:69-72`) : quatre files par tick. Le
   port **ne corrige pas** — il nomme les deux constantes (`MAX_CONCURRENT_SOUNDS`,
   `QUEUES_PER_TICK`) pour qu'un correctif soit un choix et pas un accident.
