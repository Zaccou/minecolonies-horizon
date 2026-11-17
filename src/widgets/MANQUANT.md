# MANQUANT — ce que la couche widget demande et que ce SDK ne sait pas dessiner

Mesuré contre le SDK épinglé de ce dépôt : `.horizon/sdk/sdk.json` →
`build.commit = db7638b30c531c7f8004fdc5b6946de468db7aac`, `build.date = 2026-11-15`,
`game = 26.3`. Toute ligne citée ci-dessous est dans
`.horizon/sdk/api/crates/horizon-mod-vocab/`.

**Deux prémisses du brief sont fausses pour ce build** et structurant la moitié de ce
fichier : `text_lines` (#4490) et les boutons à sprites (#4491) **ne sont pas** dans ce
SDK. Les deux contrats existent bien, mais sur d'autres branches
(`/workspace/RAPPORT_4490.md`, `/workspace/RAPPORT_4491.md`) ; aucun code de
`src/widgets/` ne les appelle.

---

## 1. Un label qui doit être coupé ou replié — `text_lines` absent

**Voulu.** `AbstractTextElement.recalcTextRendering` découpe chaque paragraphe à la
largeur de la boîte avec `Font.split`, puis `recalcPreparedTextBox` garde une ligne sans
`wrap` et autant que la hauteur le permet avec
(`/workspace/blockui-src/src/main/java/com/ldtteam/blockui/controls/AbstractTextElement.java:211-213`,
`:232-241`, `:266-271`).

**Ce que le SDK offre.** Rien pour découper un composant :

- `DrawContext` n'a aucune méthode de mise en lignes — l'inventaire complet va de `size`
  à `widget_text_field` (`src/host/draw.rs:26-172`) ;
- `DrawContext::text` prend **une** chaîne et une position (`src/host/draw.rs:61`), donc
  pas de découpe ;
- `text_width` mesure une ligne, pas un bloc (`:87`).

`text_lines` (#4490) ajouterait
`text_lines(&Text, max_width, align, line_spacing) -> Result<TextLines, TextFormError>`
(`/workspace/RAPPORT_4490.md` §« Ce que j'expose »). Il n'est pas là.

**Ce que le module fait.** `Gap::WrappedLabel` : un label qui demande `wrap` ne dessine
rien et le dit (`adapt.rs`, `label_ops`). Un label **sans** `wrap` dont la ligne dépasse
la boîte est dessiné **entier**, débordant, avec `Gap::CutLine { width, room }` — Java
l'aurait coupé.

**Ce que ça coûte, mesuré sur les XML de MineColonies**
(`/workspace/minecolonies-src/src/main/resources/assets/minecolonies/gui/`, 94 fichiers) :
**11 balises `<text wrap="true">`**, sur **342** balises `<text>`. Peu nombreuses, mais ce
sont les textes longs (aides, descriptions) — ceux qui débordent le plus visiblement.

## 2. Un bouton sur une texture de mod — `ButtonSprites` absent

**Voulu.** 226 des **271** balises `<button>` portent un `source=` : un sprite de bouton
propre à MineColonies (`windowhutallinventory.xml:9-10` par exemple), avec ses états
survolé / désactivé.

**Ce que le SDK offre.** `widget_button` dessine **uniquement** `widget/button` vanilla
(`src/host/draw.rs:155-163`), et `Button` n'a pas de champ de sprite
(`src/client/widgets.rs:82-100`). La primitive `gui_nine_slice` existe (`:78`), donc la
partie visible est faisable à la main ; ce qui est perdu est l'enregistrement pour le
narrateur et le focus manette que `widget_button` fait.

`ButtonSprites` (#4491) ajouterait `normal` / `highlighted` / `disabled` / `label_offset`
sur `Button` (`/workspace/RAPPORT_4491.md` §2). Pas dans ce build.

**Ce que le module fait.** `Op::NineSlice` + `Op::Text` centré, plus
`Gap::ButtonNarration { id, sprite }` : le bouton est dessiné, sa narration ne l'est pas.

## 3. Aucun appel ne prend une échelle de texte, et `PoseOp` n'est pas exporté

**Voulu.** `textscale` est utilisé **37 fois** dans les XML (0,5 à 1,5 ; 17 fois 0,8).
Java applique `ms.scale(textScale, textScale, 1)`
(`AbstractTextElement.java:313`).

**Ce que le SDK offre.** `text`, `gui_paint`, `widget_button` et `GuiDraw::Text` dessinent
à l'échelle de la police ; rien ne prend un facteur. La piste du lot est fermée aussi :
`GuiDraw::Push(Vec<PoseOp>)` existe (`src/client/hud_layout.rs:106`) mais **`PoseOp` n'est
pas réexporté** — `generated/mod_api_exports_root.rs:151` n'exporte que
`GuiDraw` et `HealthInputs`, donc un mod ne peut pas construire un `Push`.

**Ce que le module fait.** `Gap::ScaledText { id, scale }`, et la ligne est dessinée à
l'échelle 1. La position verticale, elle, tient compte du facteur (comme Java).

## 4. Le vocabulaire des composants est à moitié exporté

**Voulu.** Un label MineColonies est un `Text` : couleur, gras, texte traduit. Pour le
mesurer, Java utilise `Font.width(FormattedText)`.

**Ce que le SDK offre.** `Text`, `TextFormError` et `TextHover` sont exportés
(`generated/mod_api_exports_root.rs:265-266`) ; **`TextContent` et `TextStyle` ne le sont
pas**. Conséquences mesurables :

- on ne peut pas **lire** un `Text` : ses champs sont publics mais son contenu n'est pas
  nommable, donc `DrawContext::text_width` (qui prend un `&str`, `src/host/draw.rs:87`) ne
  peut pas être alimenté depuis un composant ;
- on ne peut pas **écrire** un `Text` stylé : `Text::with_style` prend un `TextStyle`
  (`src/text/mod.rs:237`) que le mod ne peut pas nommer, et `Text::new` prend un
  `TextContent` (`:199`) non exporté. Restent `Text::literal` (`:220`), `Text::translate`
  et `Text::append` (`:232`).

**Ce que le module fait.** `Line { text: Text, plain: Option<String> }` porte les deux
formes, et `Gap::Unmeasurable { id }` nomme le cas d'un composant centré ou aligné à
droite : la ligne est alors posée au bord gauche. La couleur d'un label passe par
`DrawContext::text(argb)`, pas par le style du composant.

## 5. `widget_button` dessine un seul paragraphe

**Voulu.** Un `Button` est un `AbstractTextElement` : son `text` est une **liste** de
composants dessinés l'un sous l'autre (`AbstractTextElement.java:95`, `:331-369`).

**Ce que le SDK offre.** `widget_button` centre **un** `Text`
(`src/host/draw.rs:149-163`).

**Ce que le module fait.** Il dessine le premier paragraphe et signale
`Gap::ButtonLines { id, dropped }`.

## 6. Pas de dégradé

**Voulu.** Une barre de progression MineColonies est un `Gradient` dimensionné au ratio,
posé sur l'`Image` du cadre
(`core/client/gui/WindowResearchTree.java:569-590`, `progressRatio` `:577`) ; 9 balises
`<gradient>` dans 6 fichiers XML, et le `Gradient` de BlockUI fond entre deux couleurs
(`controls/Gradient.java:13-18`).

**Ce que le SDK offre.** `fill` prend **une** couleur ARGB (`src/host/draw.rs:43`) et
`GuiDraw::Fill` est plat aussi (`src/client/hud_layout.rs:80-86`). Un dégradé se ferait en
N `fill` d'une ligne ; ce module ne le fait pas.

**Ce que le module fait.** Deux `fill` (piste + partie remplie) et
`Gap::Gradient { id }` quand le modèle demande un dégradé.

## 7. La barre de défilement n'est pas modélisée, et `Widgets` n'a pas la molette

**Voulu.** `ScrollingView` réserve 8 pixels à droite (`views/ScrollingView.java:12`, `:43`)
et dessine un pouce déplaçable.

**Ce que le SDK offre.** Le clip nécessaire existe (`push_clip` / `pop_clip`,
`src/host/draw.rs:122-130`) — la liste est donc dessinable, contrairement à ce que dit
`/workspace/ISSUES_WIDGETS.md`, écrit avant que le clip n'arrive. En revanche `Widgets`
(`src/client/widgets.rs:300-625`) ne connaît que `Button` et `TextField` : ni molette, ni
glissement de pouce.

**Ce que le module fait.** Il rétrécit la fenêtre de la liste de
`List::SCROLLBAR_WIDTH` et signale `Gap::Scrollbar { id }`. La molette est à brancher sur
`screen_mouse_scrolled_pre` (`src/hooks/screen_mouse_scrolled_pre.rs:6-31`), dont le
`scroll` alimente `List::scroll_by` / `List::page_height`.

## 8. Pas d'imbrication : une ligne de liste n'est pas une boîte

**Voulu.** La ligne de la plus grosse liste de MineColonies est un `<box>` avec un
`<itemicon>`, deux `<text>` et un `<button>`
(`windowhutallinventory.xml:11-20`).

**Ce que le SDK offre.** Tout ce qu'il faut pour la dessiner (`item`, `text`,
`gui_nine_slice`), mais ce module n'a **qu'un niveau de panes** : `Row` porte une ligne.

**Ce que le module fait.** Une ligne = un `Line`. Les icônes et boutons dans une ligne
demandent une hiérarchie `Widget` dans `Widget`, qui est le lot suivant — la structure est
prête (`Widget.body` est une énumération, `Pane` est déjà un type à part).

---

## Récapitulatif : chaque `Gap` et son coût

| `Gap` | Ce qui n'est pas dessiné | Occurrences mesurées |
| --- | --- | --- |
| `WrappedLabel` | tout le label | 11 `<text wrap="true">` |
| `CutLine` | rien (la ligne déborde), mais le résultat n'est pas celui de Java | non mesurable sans la police |
| `Unmeasurable` | l'alignement (posé à gauche) | tout label traduit centré |
| `ScaledText` | l'échelle | 37 `textscale` |
| `Gradient` | le fondu | 9 `<gradient>` / 6 fichiers, plus l'arbre de recherche |
| `ButtonNarration` | la narration et le focus manette | 226 boutons sur 271 |
| `ButtonLines` | les paragraphes 2..n d'un bouton | à mesurer |
| `Scrollbar` | le pouce | toute liste à `scrollbar` |

## Re-vérifier

```sh
# le contrat #4490 n'est pas dans ce SDK
rg -uu -n "text_lines|ButtonSprites" /home/zaccou/opencode-server/workspace/port-minecolonies/.horizon/sdk/api/crates/
# → aucune sortie attendue

# ce qui est réexporté du texte et du dessin
grep -n "pub use text\|pub use host :: draw\|hud_layout ::" <SDK>/generated/mod_api_exports_root.rs

# les mesures XML
python3 - <<'EOF'
import re, pathlib
d = pathlib.Path('.../assets/minecolonies/gui')
b = s = t = w = 0
for p in d.rglob('*.xml'):
    x = p.read_text(errors='replace')
    for m in re.finditer(r'<button\b[^>]*>', x, re.S):
        b += 1; s += 'source=' in m.group(0)
    for m in re.finditer(r'<text\b[^>]*>', x, re.S):
        t += 1; w += 'wrap="true"' in m.group(0)
print(b, s, t, w)   # 271 226 342 11
EOF
```
