# MANQUANT — `src/ui_window_16`, les quatre fenetres de l'hotel de ville

Ce que ces fenetres demandent et que le port n'a pas. Rien de ce qui suit n'est ecrit en
code : chaque manque laisse un parametre, un `Option` ou un refus nomme.

## 1. Les trois plafonds de population viennent de trois endroits absents

`WindowStatsPage.java:85-95` combine trois nombres :

| Ce qu'il faut | Java | Ce que le port a |
| --- | --- | --- |
| la force de l'effet de recherche | `getResearchEffects().getEffectStrength(CITIZEN_CAP)` (`:89`) | rien : `crate::research` n'existe pas encore comme arbre d'effets cote client |
| la limite configuree du serveur | `MineColonies.getConfig().getServer().maxCitizenPerColony` (`:90`, `:94`) | rien : aucune configuration n'est lue par ce mod |
| la limite de logement de la colonie | `colony.getCitizenCountLimit()` (`:100`, `:102`) | rien : `crate::colony::Colony` ne porte pas ce compteur |

`stats::citizen_cap(Option<i32>, i32)` prend donc l'effet et la limite configuree en
argument, et `stats::colour` / `stats::warning` prennent la limite de logement. Une
signature qui lirait ces trois valeurs elle-meme demanderait les trois modules.

## 2. `src/ui::Value` n'a pas de valeur pour une liste deroulante

`layoutstats.xml:12-21` declare un `<dropdown id="intervals">` dont la valeur affichee est
le libelle de l'entree choisie. `crate::ui::field::Value` a douze variantes et aucune ne
dit « un choix parmi quatre ». Deux consequences, tenues dans le code :

- le panneau n'est **pas** declare (`stats::PANES` n'a pas d'entree `intervals`), et un
  test le constate (`window.pane("intervals") == None`) ;
- le choix vit dans `stats::PageState`, comme `ui_window_13::stats::View` porte le sien,
  et `stats::DROPDOWN` reste une constante nommee pour un test.

Ce qu'une treizieme variante aurait ete : `Value::Choice`, un panneau dont la valeur est
l'index choisi et dont les choix sont une liste de cles de traduction. Elle n'aurait pas
ete `is_computed()`.

## 3. Les deux compteurs de citoyens n'ont pas de source

`WindowStatsPage.java:159-171` compte les enfants (`iCitizenDataView.isChild()`) et les
chomeurs (`getJobView() == null`). `crate::colony::Citizen` (`colonie.rs:61-66`) ne porte
qu'un `id` et un `name` : ni age, ni metier, ni famille. `stats::population` prend donc
`children` et `unemployed` en argument.

Ce qu'il faudrait : la vue citoyen du client (`ICitizenDataView`), c'est-a-dire la moitie
client de `crate::colony` — un module entier, pas une fonction.

## 4. Le tableau des metiers ne sait pas quels modules partagent leur limite

`WindowStatsPage.java:135` teste `module instanceof CombinedHiringLimitModuleView`.
`crate::jobs::Employment` porte la hutte, la cle de module, le metier, les competences et
la capacite, mais **pas la classe de vue** : le port ne peut pas repondre a la question
depuis ses donnees. `stats::Module` porte donc un `combined: bool` rempli par l'appelant,
et `stats::COMBINED_VIEWS` liste les quatorze cles de `BuildingModules.java` pour que
l'appelant ait de quoi le remplir sans relire le Java.

Deux pieges notes dans le code : `CONNECTION_MODULE` (`:633`) declare ce type de vue dans
ses generiques mais fabrique une `ColonyConnectionModuleView` qui n'en herite pas, et
`CAVALRY_STABLE_WORK` (`:565-567`) fabrique la vue ordinaire.

## 5. Le son et les particules du pacte ne sont pas des donnees

`WindowTownHallColonyManage.java:73-75` declenche `VanillaParticleMessage(DRAGON_BREATH)`
en local, un `CAMPFIRE_CRACKLE` ambiant (2,5 / 0,8) et `WindowTownHallCantCreateColony`
ouvre sur `BOOK_PAGE_TURN` (`:32`, `:49`). Ce sont des effets de client : aucun panneau ne
les affiche et aucun test ne peut les tenir. Ils ne sont pas dans `founding.rs`, et c'est
volontaire.

## 6. `ISetting` au-dela d'un booleen

Les cinq reglages de l'hotel de ville sont cinq `BoolSetting`, et le module ne sait rendre
que cela : `setupHandler` cherche un `trigger` et `render` ecrit `ON`/`OFF`
(`BoolSetting.java:92`, `:103-105`). Un sixieme reglage d'un autre type — il y en a douze
sortes dans `core/colony/buildings/modules/settings/` — demanderait un `StringSetting`, un
`IntSetting`, un `BlockSetting` ou un `RecipeSetting`, chacun avec son XML
(`layoutboolsetting.xml`, `layoutstringsetting.xml`, …). `settings::Row` est donc une
enumeration fermee de cinq, et `Refusal::SettingInactive` le seul refus qu'un reglage
compose rendrait atteignable.
