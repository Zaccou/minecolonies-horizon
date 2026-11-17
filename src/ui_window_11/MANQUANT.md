# MANQUANT — ce que ces quatre fenêtres auraient voulu et que le port n'a pas

Comme `src/ui`, ce module n'appelle **aucune** API de l'hôte : il ne dépend que de `std`,
`crate::ui`, `crate::ui_window_06::resources`, `crate::network::views::ColonyInfo`. Rien
n'a été ajouté à `crates/`. Les manques ci-dessous sont donc soit **du SDK**, soit **du
port**, et chacun est nommé plutôt que contourné.

## 1. Une vue de bâtiment cliente

**Ce que je voulais faire** : lire, pour la carte, tout ce que Java lit d'un bâtiment —
sa position, son niveau, son type, sa portée (`getRange()`), ses citoyens assignés
(`getAllAssignedCitizens()`), et surtout ses **cartes** :
`((BuildingTownHall.View) building).getMapDataList()` (`WindowColonyMap.java:249`).

**Ce qui existe** : `crate::building::Building` porte un niveau, une liste de modules
sauvegardés et une empreinte de plan, rien de plus ; `src/network/views.rs` transporte
`ColonyViewBuildingView` dont le payload est **un `Vec<u8>` opaque**, écrit par
`IBuilding.serializeToView` (voir `src/network/MANQUANT.md` §1). Il n'y a donc pas de
`getMapDataList` à appeler, ni de `getRange`.

**Ce que j'ai fait** : `map::MapData` est la **forme** de ce que la fenêtre lirait
(`center_x`, `center_z`, `scale`), et `map::tiles` décide ce qu'elle en ferait. Aucune
fonction de ce module ne prétend lire une carte qui n'existe pas.

## 2. Un `Effect` qui sache fermer

**Ce que je voulais faire** : dire ce que fait la croix de la carte et du classement
**quand on les ouvre depuis l'item** : `close()` (`WindowColonyMap.java:218`,
`WindowColonyPrestigeRanking.java:37`), c'est-à-dire fermer sans rien rouvrir.

**Ce qui existe** : les neuf variantes de `ui::action::Effect` (`src/ui/action.rs`) ; la
plus proche, `Reopens`, dit le contraire. Le manque est déjà documenté et tranché dans
`src/ui_window_06/MANQUANT.md` §2 : ce module ne rouvre pas le débat, il réutilise la
solution — `map::exit_effect(at_town_hall) -> Option<Effect>`, `None` voulant dire
« ferme ».

## 3. Une source pour l'état que la fenêtre tient elle-même

**Ce que je voulais faire** : décrire le panneau `scale` de la carte comme un `Field`,
puisqu'il a un id dans l'XML (`windowcolonymap.xml:13`) et que la classe l'écrit
(`WindowColonyMap.java:400`).

**Ce qui existe** : `ui::field::Source` a neuf variantes, toutes un endroit du modèle
(hutte, colonie, citoyen, module, inventaire, ordre de travail, requête, advancement,
bâtiment). Le zoom n'est **aucun** de ces endroits : c'est un `double` que la fenêtre tient
et que le joueur bouge à la molette. `ui_window_02` a comblé un manque voisin en ajoutant
`Item` à **sa** `Source` ; ce module n'a pas de `Source` à lui.

**Ce que j'ai fait** : `scale` n'est pas un `Field` ; c'est `map::Zoom`, avec son état
initial (`0.0`, `:178`), sa transition (`observe`, `:337-341`) et son texte
(`label`, `:400`). `map::FIELDS` ne contient donc que le panneau d'avertissement, le seul
dont la valeur vienne du modèle. Un test le dit : un `Field` est un panneau qu'on retrouve
par son id, une décoration ou un zoom est une valeur que la fenêtre calcule.

## 4. Un widget de liste, et un widget de carte

**Ce que je voulais faire** : rendre la liste du classement (`colonies`, 3 900 lignes de
`ColonyInfo` possibles) et les sept couches de la carte — une tuile de 512 pixels, des
icônes de bâtiment, des points de citoyen, des blobs de colonie.

**Ce qui existe** : `mod_api::client::widgets::Widgets`
(`crates/horizon-mod-vocab/src/client/widgets.rs:300-322`) n'a que `add_button` et
`add_text_field`. Pas de liste, pas de ligne, pas d'icône d'item, pas de boîte, pas
d'image, pas de vue zoomable. L'entrée existe (clic, glisser, molette) mais pas le rendu.

**Ce que j'ai fait** : le classement est `prestige::Ranking` (les lignes, leur ordre et
leur texte, sans panneau), la carte est `map::DECORATIONS` (sept couches, leur id, leur
valeur et leur source, sans position à l'écran).

## 5. Un gestionnaire de requêtes côté client

**Ce que je voulais faire** : l'icône d'état d'un bâtiment, qui existe quand le joueur a
des requêtes **assignées à lui** sur ce bâtiment
(`WindowColonyMap.getPlayerResolvedRequestsForBuilding`, `:704-752`), avec une ligne par
requête et un `getLongDisplayString()` pour chacune.

**Ce qui existe** : `crate::requests` (le gestionnaire du serveur) : file, résolveur,
priorités. Il n'y a pas de `IPlayerRequestResolver` côté client, et
`ColonyViewBuildingView` ne transporte pas de liste de requêtes.

**Ce que j'ai fait** : `map::resolved_marker(count)` prend le **nombre** et rend l'icône
et sa clé ; `map::marker_suffix(count)` rend `": n"`. Le compte, lui, doit venir d'ailleurs
— et cet ailleurs n'existe pas encore.

## 6. Un inventaire de joueur et un drapeau créatif

**Ce que je voulais faire** : l'onglet du builder lit **deux fois** l'inventaire du client —
une fois pour compter chaque ressource (`WindowBuilderResModule.java:93-96`), une fois par
`isCreative()` (`:77`), qui rend au joueur tout ce que la ligne demande.

**Ce qui existe** : `ui::field::Source::PlayerInventory` nomme cet inventaire, et rien ne
le fournit : le port n'a pas de client, donc pas d'inventaire à compter.

**Ce que j'ai fait** : `builder::player_amount(creative, counted, amount)` prend le compte
**déjà fait** et le drapeau ; `builder::Row::player` est ce nombre. Un test montre les deux
conséquences : en créatif la ligne passe en `HAVE_ENOUGH`, et `supplied` — qui lit
`getAvailable()` — ne bouge pas.

## 7. Un ordre de travail lisible

**Ce que je voulais faire** : le panneau `constructionName`, rempli du nom de l'ordre de
travail, retours à la ligne remplacés par des espaces
(`WindowBuilderResModule.java:136-146`), et lu seulement si `getWorkOrderId() > -1`.

**Ce qui existe** : `src/network/views.rs` a `ColonyViewWorkOrder`, avec un payload
opaque ; `crate::workorder` n'a pas de `getDisplayName()`.

**Ce que j'ai fait** : rien dans ce module. `src/ui` déclare le panneau
(`Source::WorkOrder`) ; la valeur reste à fournir par la tranche des ordres de travail, et
`builder::CONSTRUCTION` ne dit que l'id.

## 8. Un `Effect` qui distingue deux boutons d'une même porte

**Ce que je voulais faire** : dire que `prestige` et `prestigeicon` — comme `map` et
`mapicon` — sont deux dessins d'**une** porte (`WindowColonyMap.java:221-222`,
`WindowColonyPrestigeRanking.java:39-40`).

**Ce qui existe** : `Effect::Opens(id)` nomme une destination, pas un bouton ; deux
`Action` de même effet sont donc indiscernables autrement que par leur id, ce qui suffit.

**Ce que j'ai fait** : deux `Action` distinctes, même `Effect`, et un test qui vérifie que
les deux mènent au même endroit sans que les deux boutons soient confondus.

## 9. Un id d'écran hors de `src/ui`

**Ce que je voulais faire** : `Effect::Opens("map.prestige")` depuis la carte.

**Ce qui existe** : le commentaire de `Effect::Opens` dit « un écran **de cette table**,
par `crate::ui::Screen::id` », et `src/ui::screen_by_id` refuse tout autre id
(`Refusal::UnknownScreen`). Le SDK, lui, n'a aucune notion d'écran nommé (déjà noté dans
`src/ui/MANQUANT.md` §2).

**Ce que j'ai fait** : deux ids de ce module (`map.colony`, `map.prestige`), résolus par
`ui_window_11::window_by_id`, et deux tests qui prouvent que `crate::ui` les refuse **par
leur nom** au lieu de les confondre avec un écran de hutte.
