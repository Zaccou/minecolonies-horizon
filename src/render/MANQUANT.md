# Ce qui manque au module `render`, et ce que je voulais faire

Chaque entrée dit **ce qui n'est pas possible**, **pourquoi** (avec la ligne), et
**quel code aurait été écrit**. Rien de tout cela n'est du code fictif dans les
`.rs` : ce fichier est la liste des trous.

Les références SDK sont relatives à `.horizon/sdk/api/crates/horizon-mod-vocab/src/`.

---

## 1. Une texture d'entité n'est pas un sprite d'atlas

**Ce que Java fait.** Une peau de citoyen est un PNG autonome, lu par
`RenderType.entityCutoutNoCull` (`api/client/render/modeltype/CitizenModel.java:30`)
et liée à la ressource entière : `model.renderToBuffer(..., RenderType, ...)`
adresse la texture en 64×32, une seule image pour tout le modèle.

**Ce que l'API offre.** Trois lectures de sprites, toutes sur un **atlas** :
`texture_sprites(&[&str])` (`host/client_render.rs:84`, atlas de blocs),
`gui_sprite` (`:91`, atlas GUI), `block_sprites` (`:78`). Le dessin
(`BlockEntityDrawContext::quad`/`cube`, `host/draw.rs:242-260`) prend un nom de
sprite et une rectangle UV ; l'enregistrement d'images à l'exécution
(`hooks/register_runtime_sprites.rs:8-27`) n'accepte que des PNG ≤ 256×256 / 256 Kio
poussés dans les atlas **bloc** ou **GUI**, sous `mymod:block/…` ou `mymod:gui/…`.

**Ce que j'aurais écrit.** `models::ModelType::texture` rend déjà les deux chemins
(`primary` stylé, `fallback` par défaut) ; la suite serait un
`register_runtime_sprites` poussant le PNG sous `minecolonies:block/citizen/<nom>`
puis un `cube` avec UV explicites par face. Une peau 64×32 tient dans le budget
(2 048 px sur 1 000 000).

**Ce que ça coûte, et qui n'est pas rattrapable.** Le `RenderType` disparaît :
`entityCutoutNoCull` découpe les pixels transparents et n'applique pas de cull ;
un sprite d'atlas n'a pas ce choix. Les peaux de MineColonies sont opaques sur
toute la surface du modèle, donc la perte est nulle *pour ce mod* — c'est un fait
mesuré sur ses 42 types, pas une hypothèse. Le second coût est plus gênant : plus
d'adressage « texture entière », donc chaque face doit nommer son rectangle UV.

---

## 2. `getNameTagOffsetY()` appartient au moteur

**Ce que Java fait.** L'icône de statut est posée à
`getNameTagOffsetY() + 0.3` (`RenderBipedCitizen.java:115`). La méthode n'est
**redéfinie nulle part** dans `minecolonies-src` (un seul site d'appel, aucune
déclaration) : c'est la valeur de vanilla, qui dépend de la hauteur de l'entité.

**Ce que l'API offre.** `LivingRenderState` n'a pas ce champ
(`client/entity_renderers.rs:38-71`) ; la hauteur, elle, est connue du moteur.

**Ce que j'ai écrit.** `above::icon_anchor_y(name_tag_offset_y)` prend la valeur en
argument. **Ce que je n'ai pas écrit** : un calcul de hauteur, qui aurait inventé un
nombre que Java ne calcule pas ici.

---

## 3. Deux champs de `setupMainModelFrom` n'existent pas dans l'état de rendu

`RenderBipedCitizen.setupMainModelFrom` (`:85-99`) pose cinq choses sur le modèle.
Trois ont un champ dans `LivingRenderState` (`is_baby`, `scale`, `body_rot`) ; deux
non (`states::MODEL_INPUTS` le dit ligne par ligne) :

- **`model.riding = citizen.getVehicle() != null`** (`:95`) : aucun champ
  « à cheval ». Le cavalier (`minecolonies:cavalry_horse` + `JobCavalry`) est
  précisément le cas d'usage du mod.
- **`model.attackTime = citizen.attackAnim`** (`:97`) : aucun champ de coup. Le
  balancement d'attaque n'est donc pas posable.

**Ce que j'aurais écrit** : `LivingRenderState { riding: bool, attack_time: f32 }`
côté SDK, et deux lignes dans `states`. C'est une demande d'API, pas un contournement.

---

## 4. Tout le rendu GPU : c'est la moitié des 388 Ko

Les appels ci-dessous n'ont **aucun** équivalent et ne doivent pas en avoir un dans
ce module : Horizon dessine, un mod décrit. Chacun est listé avec ce qu'il servait.

| Appel Java | Servait à | Fichier |
| --- | --- | --- |
| `VertexConsumer.vertex(...).uv(...).endVertex()` | les 4 sommets de l'icône de statut | `RenderBipedCitizen.java:126-129` |
| `RenderType.create(...)` + `DepthTestStateShard(GL_ALWAYS)` | l'icône traverse les blocs | `worldevent/RenderTypes.java:79-95` |
| `VertexBuffer.upload/drawWithShader` | le cadre de colonie est construit une fois par chunk et rejoué | `worldevent/ColonyBorderRenderer.java:130-160` |
| `BannerRenderer.renderPatterns(...)` | les motifs de bannière de la colonie | `TileEntityColonyFlagRenderer.java:111` |
| `ModelBakery.BANNER_BASE` (`Material`) | le matériau du drap et du mât | `:100-104` |
| `ModelBlockRenderer.tesselateBlock(...)` + `enableCaching` | la déco dessine le bloc d'à côté | `TileEntityDecoControllerRenderer.java:48-78` |
| `ItemRenderer.getFoilBufferDirect(...)` | l'effet « enchanté » sur la lance | `projectile/RendererSpear.java:53` |
| `RenderSystem.setShaderColor(1,1,1,0.3)` | le fantôme d'Halloween (mort, cf. `models::Ghost`) | `RenderBipedCitizen.java:66-76` |
| `Font.drawInBatch(...)` | tous les textes de panneau et de tombe | `TileEntityColonySignRenderer.java:210` |
| `Minecraft.getInstance().getItemRenderer().renderStatic(...)` | le placeholder du drapeau en créatif | `TileEntityColonyFlagRenderer.java:88-95` |

**Ce que j'ai écrit à la place** : les *décisions* (couleur, largeur, profondeur,
échelle, offsets, contenu du texte) sont des données testables, et le dessin se
raccorde par `BlockEntityDrawContext` (`translate`, `rotate_about`, `scale`,
`quad`, `cube`, `block`, `model_part`, `light`) — qui existe, lui.

**Une nuance de précision, pas une règle.** `BlockEntityDrawContext::translate`
prend des `f32` (`host/draw.rs:185`) là où Java translate par des `double` (un
`Vec3`, `TileEntityDecoControllerRenderer.java:61-75`) : l'écart est d'au plus
6 × 10⁻⁸ bloc et ne déplace aucun pixel, mais il est réel — pour une borne de
forme à 0,9, `max - 1` vaut `-0.09999999999999998` en Java et `-0.100000024` ici,
et aucun des deux n'est `-0.1`. C'est pourquoi le test de `deco_offset` prend des
bornes exactement représentables (0.25, 0.75) plutôt qu'un epsilon.

---

## 5. La couture est dans `lib.rs`, et je n'y écris pas

Les trois hooks dont ce module a besoin **existent dans ce build** :

| Hook | Fichier | Ce que le module en ferait |
| --- | --- | --- |
| `register_entity_renderers` | `hooks/register_entity_renderers.rs:1-90` | `entities::RENDERERS` → un renderer par type, lisant `EntityRenderState` |
| `render_name_tag` | `hooks/render_name_tag.rs:5-11` | `above::above(...)` → l'icône de statut |
| `block_entity_submit` + `block_entity_render_bounding_box` | `hooks/block_entity_submit.rs`, `:1-20` | `blocks::BLOCK_RENDERERS` → les sept rendus de bloc |

Aucun n'est abonnable depuis `src/render` : `impl Hooks for Mod` est dans `lib.rs`,
qui appartient à l'orchestrateur. **Ce que j'aurais écrit** :
`fn register_entity_renderers(&self, _c: &RegistrationContext, event: &mut RegisterEntityRenderersEvent<'_>)`
qui boucle sur `RENDERERS`, et
`fn render_name_tag(&self, host: ClientHost<'_>, event: &mut RenderNameTagEvent<'_>) -> Flow`
qui appelle `above::above`.

---

## 6. Ce que le port refuse d'inventer

- **`IClientItemExtensions.of(itemstack).getArmPose(...)`**
  (`RenderUtils.java:72-76`) : un point d'extension NeoForge par item. Sans lui,
  `states::arm_pose` s'arrête aux sept animations vanilla et répond `Item` pour le
  reste — c'est exactement ce que fait Java quand aucune extension n'est
  enregistrée, donc rien n'est perdu *tant que* aucun mod d'armure n'est installé.
- **La peau personnalisée par UUID** : `CitizenArmorLayer` remplit un `GameProfile`
  par appel réseau au service de session (`:80-108`) pour dessiner une tête de
  joueur. Aucune API de profil dans ce build ; `Texture::Citizen` ne porte donc que
  les 42 types.
- **Le panoramique de bannière** : les motifs viennent d'un `List<Pair<Holder<BannerPattern>, DyeColor>>`
  lu dans le NBT (`TileEntityColonyFlagRenderer.java:52`). Le port n'a pas de type
  pour un motif ; `blocks` ne modélise donc que le balancement, l'échelle et le mât.
- **La largeur d'un texte traduit pendant une pose** : la sign découpe un nom à
  90 px (`TileEntityColonySignRenderer.java:157`). `text_split` existe
  (`host/client_text.rs:40-48`) et `text_width` aussi (`:28-38`), mais les deux
  prennent un `Text`, que ce crate ne sait pas construire depuis une clé + un
  argument. `blocks::sign_rows` rend donc les lignes **et** la décision de
  découpe (90 px, 2 lignes), et l'appelant qui a un handle de texte découpe.
- **Le tri par priorité des interactions** : `getStatusIcon` lit
  `sortedInteractions.get(0)` (`CitizenDataView.java:550`). L'échelle de priorité
  appartient à `crate::citizens::priority` ; `above::above` reçoit l'icône du haut
  et le booléen « au moins CHITCHAT », pas une liste.

---

## 7. Deux prémisses de la commande sont fausses (et le code le dit)

1. **« marche » n'est pas un état du mod.** Aucun `RENDER_META_*` ne concerne la
   marche : le balancement des jambes est `walkAnimationPos`/`walkAnimationSpeed`,
   rempli par le moteur pour toute entité vivante
   (`client/entity_renderers.rs:51-56`). Ce que le mod possède est le **travail**,
   le **sommeil** et le **portage** (17 marques, `states::MARKS`).
2. **Il n'y a pas de « barre » au-dessus d'une entité.** Rien dans
   `core/client/render/` ne dessine de barre de vie, de barre de progression ni de
   boss bar au-dessus d'un citoyen. Au-dessus d'une tête il y a exactement deux
   choses : le nametag vanilla (moteur) et l'icône 10×10 du mod (`above`).

En prime, **le mode fantôme d'Halloween est du code mort** :
`EventHandler.java:773-777` calcule la fenêtre puis affecte `false` au drapeau, et
rien dans l'arbre ne l'affecte à `true`. `models::Ghost` le reproduit au lieu
d'inventer un fondu.

---

## 8. Les 62 fichiers, un par un

| Sort | Fichiers | Où |
| --- | ---: | --- |
| portés en règles | 41 | `models`, `states`, `above`, `entities`, `blocks`, `overlay` |
| portés en partie (les nombres oui, le dessin non) | 16 | idem + §4 |
| non portés, et pourquoi | 5 | `package-info.java` (aucun code) |

Les quatre fichiers qui ne sont **pas** portés alors qu'ils contiennent du code :

- **`CitizenArmorLayer.java`** (10,6 Ko) : l'armure d'un citoyen vient de
  `ICitizenDataView.getDisplayArmor` et se dessine avec les modèles d'armure vanilla
  (`ModelLayers.PLAYER_INNER_ARMOR`/`OUTER`). Les deux couches sont dans
  `entities::Layer::Armor` ; le choix par emplacement, la teinture de cuir
  (`DyeableLeatherItem.getColor`, `:150-158`) et les `ArmorTrim` sont du rendu
  d'item, pas une règle du mod — ils appartiennent au jeu.
- **`RenderFishHook.java`** (6,8 Ko) : la ligne entre la canne et le bouchon est du
  `lineStrip` de 16 segments (`:139-148`), et le calcul des points lit
  `citizen.xo/yo/zo` (les positions de la frame précédente), que
  `EntityRenderState` ne porte pas. Rien de tout cela n'est une règle.
- **`SpearItemTileEntityRenderer.java`** (2,1 Ko) : rendu d'item en main
  (`BlockEntityWithoutLevelRenderer`). Horizon a
  `hooks/register_item_models.rs` ; c'est le domaine de `crate::items`, pas d'ici.
- **`TileEntityColonyFlagRenderer.java`** : porté pour le balancement, l'échelle, le
  mât et le mode créatif ; pas porté pour les motifs (§6).

---

## 9. Ce que le module ne fait pas du tout, par choix

Pas d'hôte, pas d'horloge, pas de fichier : `mod.rs` a un test
(`nothing_in_this_module_calls_a_host_or_opens_a_file`) qui refuse `mod_api::`,
`std::fs` et `std::net` dans ces sept fichiers. C'est ce qui permet de tester les
59 cas sans jeu, sans wasm et sans ressources préparées — et c'est aussi pourquoi
tout ce fichier existe : ce qui ne peut pas être testé sans GPU est ici, nommé,
avec ce qu'il aurait fallu.
