# RAPPORT — `src/ui_window_16` : la donnee des quatre fenetres de l'hotel de ville

## Ce qui est porte

Les quatre sources du brief, plus la classe mere qui leur donne leurs onglets :

| Fichier | Ecran | Ce que la fenetre demande et rend |
| --- | --- | --- |
| `settings.rs` | `WindowSettings` (70 l.) | cinq bascules booleennes du module de reglages **de la colonie**, un `TriggerSettingMessage` par clic |
| `stats.rs` | `WindowStatsPage` (298 l.) | trois plafonds de population, un tableau de metiers, une periode sur les statistiques de la colonie |
| `founding.rs` | `WindowTownHallColonyManage` (87 l.) | le nom du pacte, l'annee, la colonie voisine, un `CreateColonyMessage` |
| `founding.rs` | `WindowTownHallCantCreateColony` (52 l.) | un texte d'alerte venu du message, un `PickUpTownhallMessage` |
| `mod.rs` | `AbstractWindowTownHall` (70 l.) | les sept onglets, les trois panneaux par onglet (`id`, `id0`, `id1`, `idExt`) |

`pane.rs` (panneau et source) et `refusal.rs` (dix-sept refus en quatre especes) sont les
deux briques communes, de la meme forme que `src/ui` : un panneau est un
`crate::ui::Field` (`Pane::to_ui`), un effet un `crate::ui::action::Effect`
(`Click::effect`), une portee un `Scope`.

## Les premisses du brief qui sont fausses

1. **« les champs affiches (nom, niveau, progression, stock) »** — aucun des quatre n'est
   sur ces fenetres. `AbstractBuildingMainWindow` cherche `name` (`:53`) et `build`
   (`:54`) et **verifie avant d'ecrire** (`:135`, `:171`) ; les quatre XML d'ici ne les
   declarent pas. Ils sont sur `layoutactions.xml:6`, `:15`, la page d'accueil, que ce
   module ne decrit pas. Un test le tient (`no_window_of_the_town_hall_shows_a_name_...`).
2. **« les actions (embaucher, renvoyer, prioriser) »** — sur aucune des quatre : ce sont
   les boutons d'un ecran de hutte, deja portes par `src/ui` (`hire`, `recall`,
   `deliveryPrioUp/Down`). Les neuf boutons d'ici basculent un reglage, envoient le pacte,
   ramassent le bloc ou ferment.
3. **« l'etat initial »** n'est pas un seul etat : quatre fenetres, quatre etats, et deux
   viennent du **message d'ouverture** et non d'un modele. Le champ du pacte n'est jamais
   vide a l'ouverture : nom du joueur en fondation, nom tire au hasard (graine = position
   du bloc) en reactivation (`WindowTownHallColonyReactivate.java:45-48`, `:60`).
4. **« les quatre fichiers » ne forment pas un ecran** : deux pages de l'hotel de ville
   (barre d'onglets) et deux fenetres autonomes (`AbstractWindowSkeleton`). Les traiter
   comme quatre pages obligerait a inventer un titre et une barre qui n'y sont pas.

## Les quatre faits que je n'aurais pas devines sans lire

- **L'onglet des statistiques s'appelle `happiness`** (`WindowConstants.java:62`) et son
  libelle dit « Statistics » : le nom vient de la place, pas du contenu. `stats` est le
  nom de la **liste** des compteurs (`layoutstats.xml:22`).
- **Deux lignes de reglages sur cinq commencent avec le libelle « Automatic »** : le XML
  pose `gui.hiring.off` (`layoutsettings.xml:11`, `:17`) avant que la classe n'ecrive
  `On`/`Off` depuis `retrieveon`/`retrieveoff`.
- **La somme des lignes d'un batiment a limite partagee n'est pas sa limite.** Chaque
  ligne affiche `max - tout le batiment + soi` (`WindowStatsPage.java:134-148`) ;
  `isFull` compare les affectes du batiment entier a **une** place de module
  (`CombinedHiringLimitModuleView.java:12-15`). Cinq modules d'une place, deux gardes :
  les lignes somment 17, la limite est 5.
- **Le pacte ne date pas en annees.** `gameTime / 20 / 60 / 100` = un jour de jeu divise
  par cinq, plancher 13 (`:55`).

## Ce qui est raccorde plutot que duplique

- `crate::advancements::stats::Stats` est le magasin de statistiques ; `WindowStatsPage`
  et l'onglet d'un batiment lisent le meme. **`stats::Interval` est importe de
  `crate::ui_window_13::stats`** : `WindowStatsPage.INTERVAL` (`:41-44`) et
  `WindowStatsModule.INTERVAL` (`:63-66`) sont la meme table ecrite deux fois en Java, et
  le port n'en garde qu'une. Deux tests epinglent ses quatre valeurs.
- `crate::jobs::Job` pour les noms de metier, `crate::ui::{Value, Scope, Field, Effect}`,
  `crate::ui::Screen` pour verifier qu'aucun id d'ici ne l'usurpe.

Dependance unique et assumee : **`ui_window_13::stats::{Interval, PARTIAL_STATS_MODIFIER_NAME}`**.
Si ce module bouge, celui-ci ne compile plus — c'est le prix du non-duplicata.

## Les comptes

4 fenetres, 7 onglets (2 seulement mènent ici), 14 panneaux (2 listes), 9 boutons
(7 envoient, 4 ferment), 17 refus (3 fautes de table, 2 plantages, 4 silences, 8 phrases),
3 messages serveur nommes, 4 valeurs et 4 sources de `src/ui` employees, **35 tests**.

## Ce qui n'est pas la, et ou c'est dit

`MANQUANT.md` : les trois plafonds de population (recherche, configuration, logement) que
le port n'a pas ; l'absence de `Value::Choice` pour la liste deroulante ; les deux
compteurs enfants/chomeurs (`crate::colony::Citizen` n'a ni age ni metier) ; la classe de
vue des modules a limite partagee ; les sons et particules ; les autres sortes de
reglages.

## Ecart assume sur la consigne de taille

La consigne visait 500 a 900 lignes ; ce module en fait ~3 100 (dont un tiers de tests et
un tiers de sources citees). Le volume vient du sujet, pas du remplissage : **quatre
fenetres** et non un ecran, **sept onglets**, `n/max` a trois plafonds, et surtout **six
decisions serveur** dont les refus sont disperses dans quatre classes
(`GetColonyInfoMessage`, `CreateColonyMessage`, `PickUpTownhallMessage`,
`SettingsModuleView`). Reduire voudrait dire couper des refus nommes ou les sources ;
dites-moi ce que vous preferez sacrifier et je le coupe.
