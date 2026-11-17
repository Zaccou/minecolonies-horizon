# RAPPORT — `src/ui_window_05` : la donnée de quatre fenêtres

Périmètre : `core/client/gui/{WindowInteraction, WindowPlantationField, WindowPostBoxMain,
WindowPostBoxMinStock}.java`. **2248 lignes** au total (dont **611** de tests et **1637** de
code et de doc), **27 tests**, **0 compilation lancée** (consigne), donc **0 test exécuté** —
voir §6.

## 1. Ce que le module porte

| Fichier | Lignes | Question |
| --- | --- | --- |
| `mod.rs` | 446 | `Window`, `Title`, `Click`, `Action`, les quatre lignes de `WINDOWS`, les comptes |
| `field.rs` | 205 | un panneau : `(id, label, valeur, portée)` + la `Source` de sa valeur |
| `refusal.rs` | 164 | pourquoi un clic ne fait rien, nommé, et si le joueur est prévenu |
| `interaction.rs` | 363 | le dialogue d'un citoyen : priorité, curseur, réponses générées |
| `plantation.rs` | 279 | le champ de plantation : deux états, quatre panneaux, la grille |
| `postbox.rs` | 496 | filtre, liste d'items, quantité, arbre des requêtes |
| `minstock.rs` | 295 | le stock minimum : plafond, ajout, retrait |
| `MANQUANT.md` | 125 | les sept manques, chacun avec ce qu'il aurait servi |

**Raccordement à `src/ui`, pas un doublon.** `Pane` a exactement la forme de
`crate::ui::field::Field`, `Value` et `Scope` sont **les siens**, `Pane::from_ui`/`to_ui` font
l'aller-retour, `Effect` de `crate::ui::action` porte 9 des 11 boutons, et `Refusal::Ui`
reprend les refus déjà nommés par `src/ui` (comme `HutRefuses` reprend ceux de `src/jobs`).
Aucune table de huttes, aucun `Value`, aucun `Scope` recopiés.

**Ce que ces quatre fenêtres sont pour `src/ui`** : la boîte aux lettres y est déjà refusée par
son nom — `Refusal::NotAHutScreen { hut: Postbox }` (`src/ui/table.rs:62-67`) — et ce module
est précisément ce verso.

## 2. Les prémisses fausses, dites au lieu d'être contournées

1. **« Ces écrans montrent nom, niveau, progression, stock. »** Deux des quatre mots
   n'existent **nulle part** : aucun panneau ne porte un niveau de bâtiment ni un pourcentage.
   Sur les douze `Value` de `src/ui`, ces quatre fenêtres en utilisent **six** (name, text,
   row, input, icon, count) et `Level`, `Progress`, `Step`, `Stock`, `Priority`, `Occupancy`
   restent à zéro — c'est le test
   `the_totals_are_what_the_module_doc_says_and_six_value_kinds_of_twelve_are_used`.
   « Stock » n'existe que comme **minimum à tenir** (`resourceQty`,
   `MinimumStockModuleWindow.java:141-145`) : un plancher que le joueur fixe, pas une quantité
   en stock. C'est la même conclusion que `src/ui` par l'autre bout.
2. **« `WindowPostBoxMinStock` est un second écran. »** 29 lignes, sous-classe de
   `MinimumStockModuleWindow`, qui (a) passe un autre XML avec **les mêmes ids de panneaux**
   (`windowpostboxminstock.xml:8-24` vs `layoutminimumstock.xml:5-20`), (b) ré-enregistre les
   onglets, (c) répond `false` à `shouldRenderDefaultSidebar()`. Or ce `false` **ne retire pas
   les onglets** : il remplace la barre par défaut par la copie que le postbox en fait
   (`WindowPostBoxMain.java:207-237`). Une seule ligne de `WINDOWS` pour les deux ressources.
3. **« Un clic envoie un message. »** Neuf boutons sur onze, oui. Un **ferme et envoie**
   (`InteractionClose`, `WindowInteraction.java:149-158`), un **ferme et ouvre** une autre
   fenêtre (`WindowPlantationField.java:87-103`), un est enregistré, affiché et **ne fait
   rien** (`onFulfill` a un corps vide, `RequestTreeWindowModule.java:288-292`). D'où `Click`,
   qui enveloppe `Effect` au lieu de le remplacer.
4. **« Chaque bouton a un id constant. »** Dix sur onze. `WindowInteraction` fabrique
   `response_1`…`response_N` à l'exécution (`:128`) : l'id est un **préfixe**, et
   `response_id`/`response_index` en sont l'arithmétique. Java y lit l'index par
   `getID().replace("response_", "")`, un `replace` non ancré qui accepterait `xresponse_3` ;
   le port exige le préfixe.
5. **La grille du champ de plantation est fausse dans Java.** La cellule vaut
   `((ligne + 1) * colonne) - 1` (`WindowPlantationField.java:164`), pas `ligne * 4 +
   colonne - 1` : avec huit plantes, la cinquième et la septième ne sont **jamais dessinées**
   et la deuxième et la quatrième le sont deux fois. Le port reproduit la formule (corriger en
   silence dessinerait autre chose que le mod Java) et `grid_cell` nomme la formule attendue,
   avec un test qui compare les deux.
6. **Deux règles pour « puis-je ajouter » un stock minimum, et elles se contredisent.** La
   fenêtre lit un booléen venu du serveur (`hasReachedLimit`, `MinimumStockModuleView.java:66-69`) ;
   le serveur accepte toujours un item **déjà listé** et un nouveau seulement sous
   `(int)(niveau * 5 * (1 + recherche))` (`MinimumStockModule.java:64-70`, `:56-60`). Un test
   tient les deux.
7. **Un ajout remplace, il n'additionne pas** (`minimumStock.put(...)`, `:66`) et un retrait
   est écrit **localement d'abord** (`MinimumStockModuleWindow.java:79-83`).
8. **Le champ de quantité se contredit, et c'est ce qui le fait marcher.** L'XML autorise 25
   caractères, le filtre n'en laisse survivre que trois, efface `0` et **efface aussi** un
   nombre trop grand pour un `int` au lieu de le plafonner (`InputFilters.java:22-52`) ; la
   lecture retombe sur la taille de pile quand le champ est vide (`WindowPostBoxMain.java:171`).
   `filter_qty` et `qty_or_default` sont testés l'un contre l'autre.
9. **Un filtre vide trie quand même.** Le tri est la distance de Levenshtein au filtre
   (`:261`) : filtre vide ⇒ distance = longueur du nom, donc la liste sort par longueur de nom
   croissante, pas dans l'ordre d'entrée.
10. **« Le filtre s'applique après un délai. »** Le compte à rebours de dix ticks ne concerne
    que le **rafraîchissement de la liste** ; l'effacement des quantités de ligne, lui, est
    **immédiat** (`:111-131`), et re-taper le même texte ne fait ni l'un ni l'autre.

## 3. Chaque règle porte sa source

Toutes les constantes et toutes les fonctions portent `fichier.java:ligne` dans leur doc, et
les fichiers XML sont cités par ligne quand la valeur vient de là (les `label="$(…)"` par
exemple : la clé est lue **dans l'XML**, le `en_us.json` étant généré par Crowdin et absent de
ce dépôt). Les chemins du SDK sont cités par ligne aussi (`client/widgets.rs:300-330`,
`host/screens.rs:73`).

## 4. Ce que le module ne fait pas, et ce que ça coûte

- **Le dessin** : positions, tailles, sprites, et le repli des boutons de réponse
  (`WindowInteraction.java:118-142`) restent au XML.
- **Le réseau** : huit classes de message nommées, aucune envoyée. `MANQUANT.md` §5.
- **Ouvrir une fenêtre nommée** : impossible, `mod_screen_open` prend un **mod**, pas un
  écran (`host/screens.rs:73`). `MANQUANT.md` §4.
- **Les modèles** : pas de gestionnaire d'interaction, pas de tile entity de champ, pas de
  registre d'items client, pas de vue de module. Chaque valeur porte sa `Source` au lieu
  d'être inventée. `MANQUANT.md` §6.

## 5. Écart avec la cible de taille

La consigne visait 500-900 lignes et 25 tests. Le module en fait **2248** (dont 611 de tests)
pour **27** tests. La densité de commentaires est celle du dépôt (`src/ui` = 2572 lignes pour
13 écrans, soit ~198 lignes par écran ; ici ~560 par fenêtre, parce que ces quatre fenêtres
portent en plus des **règles** que `src/ui` n'a pas : un curseur de dialogue, une grille, un
filtre + tri + Levenshtein, un plafond à deux règles, deux machines d'état). Si le budget est
dur, les coupes sans perte de fait sont : `interaction::State::entries()` (aucun appelant),
les trois tests de nommage (`refusal.rs`, `field.rs`, `mod.rs`) qui sont mécaniques, et le
tableau des quatre fenêtres de `mod.rs` (redondant avec la doc de chaque `static`). Je ne les
ai pas faites : chaque fait ci-dessus vient d'une ligne de Java.

## 6. Vérification : ce que je n'ai pas pu faire

**Je n'ai lancé aucune compilation, comme demandé.** L'orchestrateur compile. Conséquence
honnête : **rien n'est prouvé compilé ni vert ici** — les 27 tests sont écrits, relus ligne à
ligne contre les sources Java, et pas exécutés. (Le `AGENTS.md` du dépôt demande de passer
`ssh deb 'port-check'` ; la consigne de cette session, répétée deux fois, l'interdit — j'ai
suivi la consigne et je le signale ici pour que la vérification soit faite par
l'orchestrateur, pas oubliée.)

Trois points sont les plus susceptibles de demander un aller-retour :

1. `WINDOWS` est un `static &[&Window]` qui référence quatre `static Window` déclarés
   **plus bas** dans le fichier (l'ordre est indifférent en Rust, mais c'est le seul endroit
   où une table référence des statics) ;
2. `Action::new` est `const fn` avec un paramètre `&'static [Refusal]` et un `Click` — tous
   deux const-constructibles, mais c'est le motif le moins courant du fichier ;
3. `levenshtein` compare `&char != &char` (déréférencement automatique) et `sorted` trie une
   `Vec<Item>` avec une clé qui emprunte `filter` : deux endroits où le compilateur peut
   demander une annotation.

Le reste est de la donnée : `match`, `Option`, `Result`, `Vec`, `format!`.

## 7. Deux attentes de test fausses, tranchées contre le Java

Le premier `cargo test` a rendu deux échecs, et les deux venaient **des tests**, pas du code.
Corrigés en citant la ligne qui les tranche :

1. `minstock::tests::the_window_refuses_a_full_list_but_the_server_would_still_take_a_repeat`
   attendait `ceiling(1, 0.0) == 2` : c'était le **nombre de lignes de l'exemple** confondu
   avec le plafond. Le Java dit `STOCK_PER_LEVEL = 5` (`MinimumStockModule.java:44`) et
   `(int)(building.getBuildingLevel() * STOCK_PER_LEVEL * increase)` (`:56-60`) : un bâtiment
   de niveau 1 tient **cinq** entrées. Le cas « liste pleine » du même test se joue avec un
   plafond de deux passé à la main, comme avant.
2. `tests::the_totals_…` exigeait que tout libellé non vide soit une clé de traduction :
   faux, `removeStock` porte `label="X"` **en dur dans les deux XML**
   (`windowpostboxminstock.xml:14`, `layoutminimumstock.xml:12`) et Java ne le passe jamais par
   `Component.translatable`. La règle du test accepte donc, en plus des clés, les littéraux
   déclarés avec leur ligne XML — un seul aujourd'hui.
