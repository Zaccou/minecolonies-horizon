//! `WindowField` : l'écran de l'épouvantail — six valeurs, cinq boutons, et une largeur
//! calculée par un modulo qui peut diviser par zéro.
//!
//! Java reference : `core/client/gui/containers/WindowField.java` (374 lignes),
//! `assets/minecolonies/gui/windowfield.xml` (28 lignes), `core/tileentities/
//! TileEntityScarecrow.java:47`, `:104-107`, `:127-130`, `core/colony/buildingextensions/
//! FarmField.java:40-41`, `:58`, `api/constants/TranslationConstants.java:424`, `:514`,
//! `:518-526`, `:738` et `translation/GuiTranslationConstants.java:37-39`.
//!
//! # C'est la seule des quatre fenêtres avec des panneaux nommés
//!
//! Et elle n'est pas un écran de hutte : `BlockScarecrow.use` l'ouvre **côté client
//! seulement** (`BlockScarecrow.java:103-113`) sur un `TileEntityScarecrow`, jamais par un
//! bouton de `src/ui`. `crate::blocks::click` la nomme déjà (`blocks::Window::Field`).
//!
//! # Ce qu'elle affiche n'est ni un niveau, ni une progression, ni un stock
//!
//! Six valeurs, aucune des quatre que le brief attend d'un écran de bâtiment : le biome du
//! bloc (`:124-129`), le climat — les tags de biome cultivables que ce biome satisfait
//! (`:131-143`) —, l'exploitant ou l'absence d'exploitant (`:262-291`), l'icône de la
//! graine (`:296-302`), une icône constante (`windowfield.xml:19`) et **quatre largeurs**
//! (`:307-313`). Les largeurs sont dessinées **sur les boutons** : dans BlockUI un
//! `ButtonImage` *est* un panneau, donc les mêmes quatre ids figurent deux fois dans ce
//! fichier — une fois comme ce qu'ils montrent ([`FIELDS`]), une fois comme ce qu'ils font
//! ([`BUTTONS`]). Ce n'est pas un doublon, c'est le même objet.
//!
//! # La règle de la largeur, et l'exception qu'elle cache
//!
//! ```text
//! sum      = les quatre largeurs additionnées          (:184)
//! leftOver = MAX_RANGE - sum                            (:185)
//! width    = (current % min(current + leftOver, 20)) + 1 (:189)
//! ```
//!
//! Les quatre largeurs valent **5 par défaut** (`TileEntityScarecrow.java:47`,
//! `FarmField.java:41`) : leur somme vaut donc **déjà** `MAX_RANGE` = 20 (`FarmField.java:40`)
//! et `leftOver` vaut zéro à l'ouverture. Le premier clic donne `5 % 5 + 1 = 1` : le côté
//! **rétrécit** avant de grandir, et le cycle 1→5 se répète ensuite parce que la somme
//! retombe sur 20 pile quand le côté atteint 5. Et quand un côté vaut `0` alors que les
//! quatre totalisent 20, le diviseur vaut `0` : Java lève `ArithmeticException` et le client
//! tombe. Inatteignable par les boutons (la formule ajoute toujours 1), atteignable par un
//! composé bricolé — le port refuse par [`Refusal::FieldSizeDivideByZero`].
//!
//! # Deux appelants, deux avis sur un champ absent
//!
//! `updateFarmField` peut laisser `farmField` à `null` (`:224-244`). `setSeed` teste et ne
//! fait rien (`:213`) ; `selectSeed` **ne teste pas** et déréférence `farmField.getSeed()`
//! (`:157`) — une `NullPointerException` sur un clic. Le port refuse les deux par
//! [`Refusal::NoFieldForBlock`], et c'est la seule façon dont les deux moitiés du même
//! écran peuvent s'accorder.

use super::pane::Pane;
use super::refusal::Refusal;
use super::{Button, Effect, Naming};
use crate::network::building_modules::Face;
use crate::ui::field::Value;

/// La classe Java. `WindowField` hérite d'`AbstractWindowSkeleton`, pas d'un écran de hutte
/// (`:59`).
pub const JAVA: &str = "WindowField";
/// Le XML BlockUI (`:115`).
pub const RESOURCE: &str = "minecolonies:gui/windowfield.xml";
/// Le titre, écrit par le XML lui-même : `label="$(block.minecolonies.blockhutfield)"`
/// (`windowfield.xml:8`). Aucune classe ne l'écrit.
pub const HEADER_KEY: &str = "block.minecolonies.blockhutfield";
/// Le préfixe du biome (`WindowField.java:128`).
pub const BIOME_KEY: &str = "com.minecolonies.core.biome";
/// Le préfixe d'un tag de climat (`TranslationConstants.java:424`), suffixé par le chemin
/// du tag (`:140`).
pub const CROP_CLIMATE: &str = "com.minecolonies.core.tag.crop.biome";
/// Les quatre tags de biome cultivable, **dans l'ordre de la liste de Java**
/// (`ModTags.java:86-91`, `List.of(cold, temperate, humid, dry)` ;
/// les ids sont ceux de `TagConstants.java:50-53`).
///
/// L'ordre compte : la fenêtre parcourt cette liste (`:132-134`) et écrit les tags dans cet
/// ordre, pas dans celui du biome. C'est donc `froid, tempéré, humide, sec`, toujours.
pub const CROP_BIOME_TAGS: [&str; 4] = [
    "minecolonies:coldbiomes",
    "minecolonies:temperatebiomes",
    "minecolonies:humidbiomes",
    "minecolonies:drybiomes",
];
/// « Aucun fermier assigné » (`GuiTranslationConstants.java:39`, écrit à `:264`).
pub const FARMER_NONE_KEY: &str = "com.minecolonies.coremod.gui.field.noassignedfarmer";
/// « Fermier assigné » (`GuiTranslationConstants.java:37`), avec le nom en argument
/// (`:290`).
pub const FARMER_ASSIGNED_KEY: &str = "com.minecolonies.coremod.gui.field.assignedfarmer";
/// Le libellé du bouton de graine (`windowfield.xml:16`, `:156`).
pub const SELECT_SEED_KEY: &str = "com.minecolonies.coremod.gui.field.selectseed";
/// Le préfixe du nom absolu d'un côté (`TranslationConstants.java:738`), suffixé par
/// `Direction.getSerializedName()` (`:327`).
pub const DIRECTION_ABSOLUTE: &str = "com.minecolonies.coremod.gui.field.";
/// Les quatre clés du nom **relatif** à l'orientation du joueur
/// (`TranslationConstants.java:518-526`), dans l'ordre où [`relative_key`] les rend.
pub const DIRECTION_NEAREST_KEY: &str = "com.minecolonies.coremod.gui.field.near";
pub const DIRECTION_RIGHT_KEY: &str = "com.minecolonies.coremod.gui.field.to_right";
pub const DIRECTION_OPPOSITE_KEY: &str = "com.minecolonies.coremod.gui.field.opposite";
pub const DIRECTION_LEFT_KEY: &str = "com.minecolonies.coremod.gui.field.to_left";
/// Le préfixe des quatre ids de bouton directionnel (`:64`).
pub const DIRECTION_PREFIX: &str = "dir-resize-";
/// Le message qu'envoie une flèche (`:193`).
pub const RESIZE_MESSAGE: &str = "FarmFieldPlotResizeMessage";
/// Le message qu'envoie le choix de graine, une fois la fenêtre de sélection refermée
/// (`:215`).
pub const SEED_MESSAGE: &str = "FarmFieldUpdateSeedMessage";
/// La largeur d'une case : `FarmField.MAX_RANGE` (`FarmField.java:40`), déjà nommée par le
/// port dans [`crate::network::building_modules::MAX_FIELD_RANGE`] — c'est la même valeur,
/// et elle n'est écrite qu'une fois.
pub const MAX_RANGE: i32 = crate::network::building_modules::MAX_FIELD_RANGE;
/// La largeur d'ouverture : `FarmField.DEFAULT_RANGE` (`FarmField.java:41`), posée quatre
/// fois par `TileEntityScarecrow.java:47` et `FarmField.java:58`.
pub const DEFAULT_RANGE: i32 = 5;
/// Le côté d'une image de bouton (`:94`), qui est aussi le pas de la feuille de sprites
/// (`:324`).
pub const BUTTON_SIZE: i32 = 24;
/// Le bouton de choix de graine (`:74`).
pub const BUTTON_SELECT_SEED: &str = "select-seed";
/// La clé du module de ferme, quand un appelant en a besoin.
pub const MODULE_KEY: &str = "farmer";

/// Le panneau de l'exploitant (`:84`).
pub const PANE_FARMER: &str = "current-farmer";
/// Le panneau du biome (`:79`).
pub const PANE_BIOME: &str = "biome";
/// Le panneau du climat (`:82`).
pub const PANE_CLIMATE: &str = "climate";
/// L'icône de la graine (`:80`).
pub const PANE_SEED: &str = "current-seed";
/// L'icône constante du centre (`:70`), posée par le XML (`windowfield.xml:19`).
pub const PANE_CENTER: &str = "dir-center";

/// Les neuf panneaux de la fenêtre, dans l'ordre du XML (`windowfield.xml:10-27`) : cinq
/// valeurs, puis les quatre nombres des flèches.
///
/// Les quatre derniers sont les boutons directionnels : leur valeur est la largeur du côté
/// (`:312`), lue sur l'entité du bloc. Ils figurent aussi dans [`BUTTONS`] — le même id,
/// deux questions.
pub const FIELDS: &[Pane] = &[
    // `:264` / `:290` : la clé du libellé dépend de l'état, donc elle n'est pas ici. Voir
    // [`FarmerText`].
    Pane::new(PANE_FARMER, Value::Name, super::Source::CITIZEN),
    Pane::labelled(PANE_BIOME, BIOME_KEY, Value::Text, super::Source::Biome),
    Pane::labelled(PANE_CLIMATE, CROP_CLIMATE, Value::Text, super::Source::Tag),
    Pane::new(PANE_SEED, Value::Icon, super::Source::Extension),
    Pane::new(PANE_CENTER, Value::Icon, super::Source::Window),
    Pane::new("dir-resize-north", Value::Count, super::Source::BlockEntity),
    Pane::new("dir-resize-south", Value::Count, super::Source::BlockEntity),
    Pane::new("dir-resize-west", Value::Count, super::Source::BlockEntity),
    Pane::new("dir-resize-east", Value::Count, super::Source::BlockEntity),
];

/// Les cinq boutons, dans l'ordre du XML : la graine (`windowfield.xml:14`), puis les
/// quatre flèches (`:20`, `:22`, `:24`, `:26`).
pub const BUTTONS: &[Button] = &[
    Button {
        id: BUTTON_SELECT_SEED,
        label: SELECT_SEED_KEY,
        // `:154-161` : `new WindowSelectRes(this, …, (stack, qty) -> setSeed(stack)).open()`.
        effect: Effect::OpensJava("WindowSelectRes"),
        refusals: &[Refusal::NoColony, Refusal::NoFieldForBlock],
        naming: Naming::Xml,
    },
    Button {
        id: "dir-resize-north",
        label: "",
        effect: Effect::Sends(RESIZE_MESSAGE),
        refusals: &[Refusal::ButtonDisabled, Refusal::FieldSizeDivideByZero { face: Face::North }],
        naming: Naming::Xml,
    },
    Button {
        id: "dir-resize-south",
        label: "",
        effect: Effect::Sends(RESIZE_MESSAGE),
        refusals: &[Refusal::ButtonDisabled, Refusal::FieldSizeDivideByZero { face: Face::South }],
        naming: Naming::Xml,
    },
    Button {
        id: "dir-resize-west",
        label: "",
        effect: Effect::Sends(RESIZE_MESSAGE),
        refusals: &[Refusal::ButtonDisabled, Refusal::FieldSizeDivideByZero { face: Face::West }],
        naming: Naming::Xml,
    },
    Button {
        id: "dir-resize-east",
        label: "",
        effect: Effect::Sends(RESIZE_MESSAGE),
        refusals: &[Refusal::ButtonDisabled, Refusal::FieldSizeDivideByZero { face: Face::East }],
        naming: Naming::Xml,
    },
];

/// L'id du bouton d'un côté, tel que le XML le déclare.
pub fn direction_button(face: Face) -> &'static str {
    match face {
        Face::North => "dir-resize-north",
        Face::South => "dir-resize-south",
        Face::West => "dir-resize-west",
        Face::East => "dir-resize-east",
    }
}

/// Le côté que cet id de bouton nomme, `None` pour un id qui n'en est pas un
/// (`:176-177` : Java retire le préfixe et cherche le nom dans les quatre directions).
pub fn face_of_button(id: &str) -> Option<Face> {
    Face::parse(id.strip_prefix(DIRECTION_PREFIX)?)
}

/// Les quatre largeurs, indexées par `Direction.get2DDataValue` — l'ordre que
/// `TileEntityScarecrow` utilise (`:106`, `:129`) et que [`Face::index`] nomme.
pub type Sizes = [i32; 4];

/// Les largeurs d'ouverture : quatre fois [`DEFAULT_RANGE`] (`TileEntityScarecrow.java:47`).
pub const DEFAULT_SIZES: Sizes = [DEFAULT_RANGE; 4];

/// La largeur d'un côté.
pub fn width(sizes: Sizes, face: Face) -> i32 {
    sizes[face.index() as usize]
}

/// La somme des quatre (`:184`).
pub fn sum(sizes: Sizes) -> i32 {
    sizes.iter().sum()
}

/// Ce qu'il reste à répartir : `MAX_RANGE - sum` (`:185`). **Négatif** si le composé est
/// bricolé, zéro dès l'ouverture.
pub fn left_over(sizes: Sizes) -> i32 {
    MAX_RANGE - sum(sizes)
}

/// La largeur que le clic écrit, ou le refus quand le diviseur n'est pas strictement
/// positif (`:189`).
///
/// Java ne lève `ArithmeticException` qu'à **exactement** zéro ; un diviseur négatif, venu
/// d'un composé dont une case serait négative, donnerait une largeur négative que
/// `setFieldSize` écrirait telle quelle. Le port refuse les deux.
pub fn next_width(sizes: Sizes, face: Face) -> Result<i32, Refusal> {
    let current = width(sizes, face);
    let modulus = (current + left_over(sizes)).min(MAX_RANGE);
    if modulus <= 0 {
        return Err(Refusal::FieldSizeDivideByZero { face });
    }
    Ok((current % modulus) + 1)
}

/// `tileEntityScarecrow.setFieldSize(direction, radius)` (`:190`) →
/// `Math.min(radius, MAX_RANGE)` (`TileEntityScarecrow.java:104-107`). Rend la valeur
/// réellement écrite.
pub fn set_width(sizes: &mut Sizes, face: Face, value: i32) -> i32 {
    let clamped = value.min(MAX_RANGE);
    sizes[face.index() as usize] = clamped;
    clamped
}

/// Ce que le panneau de l'exploitant affiche, et quelle clé il porte (`:264`, `:290`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les deux branches de Java, et pas une troisième
pub enum FarmerText {
    /// `FIELD_GUI_NO_ASSIGNED_FARMER` : personne, ou personne de connu.
    Nobody,
    /// `FIELD_GUI_ASSIGNED_FARMER` avec le nom du citoyen en argument.
    Assigned(&'static str),
}

impl FarmerText {
    /// La clé du libellé.
    pub fn key(self) -> &'static str {
        match self {
            FarmerText::Nobody => FARMER_NONE_KEY,
            FarmerText::Assigned(_) => FARMER_ASSIGNED_KEY,
        }
    }

    /// Le nom, quand il y en a un.
    pub fn name(self) -> Option<&'static str> {
        match self {
            FarmerText::Nobody => None,
            FarmerText::Assigned(name) => Some(name),
        }
    }
}

/// Le texte du biome : le préfixe fixe, puis le nom traduit **ou** le chemin brut.
///
/// Java : `Component.translatable("com.minecolonies.core.biome").append(I18n.exists(key) ?
/// Component.translatable(key) : Component.literal(biomeID.getPath()))` (`:128-129`). Le
/// port n'a pas de registre de langue (`MANQUANT.md` §3), donc la traduction est un
/// argument : `translated` vaut `Some(nom)` quand la clé existe, `None` sinon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BiomeText {
    /// La clé du préfixe, toujours [`BIOME_KEY`].
    pub prefix: &'static str,
    /// Ce que Java accroche derrière.
    pub suffix: String,
}

/// La clé de langue d'un biome : `biome.<namespace>.<path>` (`:126`).
pub fn biome_lang_key(biome: &str) -> String {
    format!("biome.{}", biome.replace(':', "."))
}

/// Le chemin d'un biome, ce que Java imprime quand la clé de langue n'existe pas (`:129`,
/// `biomeID.getPath()`).
pub fn biome_path(biome: &str) -> &str {
    match biome.split_once(':') {
        Some((_namespace, path)) => path,
        None => biome,
    }
}

/// Les deux morceaux du texte du biome (`:128-129`).
pub fn biome_text(biome: &str, translated: Option<&str>) -> BiomeText {
    BiomeText {
        prefix: BIOME_KEY,
        suffix: match translated {
            Some(name) => name.to_owned(),
            None => biome_path(biome).to_owned(),
        },
    }
}

/// Les clés des tags de climat que ce biome satisfait, dans l'ordre de `cropBiomeTags`
/// (`:132-142`) : `CROP_CLIMATE + "." + chemin du tag`.
pub fn climate_keys(tags: &[&str]) -> Vec<String> {
    tags.iter()
        .map(|tag| format!("{CROP_CLIMATE}.{}", biome_path(tag)))
        .collect()
}

/// Le texte du climat : les clés jointes par une **virgule sans espace**, exactement le
/// `Component.literal(",")` que Java ajoute entre deux entrées (`:136-139`).
///
/// Liste vide : texte vide, et Java n'écrit alors rien du tout.
pub fn climate_text(tags: &[&str]) -> String {
    climate_keys(tags).join(",")
}

/// La clé du nom **relatif** d'un côté, vue depuis l'orientation du joueur
/// (`:354-366`).
///
/// Java calcule `facing.getOpposite().get2DDataValue() - direction.get2DDataValue()` puis
/// aiguille sur `1|-3`, `2|-2`, `3|-1`, défaut. `facing` est la direction horizontale la
/// plus proche du regard (`:356-357`, `Direction.orderedByNearest`), que l'appelant
/// fournit.
///
/// **La conséquence surprend, et c'est la ligne 359 :** la référence est l'**opposé** du
/// regard. Le côté que le joueur a dans le dos est donc celui que le tooltip appelle « le
/// plus proche » (`…field.near`), et celui qu'il regarde s'appelle « opposé »
/// (`…field.opposite`). Le port suit le source plutôt que l'intuition : un appelant qui
/// veut « devant moi » doit lire [`DIRECTION_OPPOSITE_KEY`].
pub fn relative_key(facing: Face, target: Face) -> &'static str {
    // `getOpposite()` sur un axe horizontal, c'est `+2` modulo 4 dans l'ordre 2D.
    let delta = ((facing.index() + 2) % 4 - target.index()).rem_euclid(4);
    match delta {
        0 => DIRECTION_NEAREST_KEY,
        1 => DIRECTION_RIGHT_KEY,
        2 => DIRECTION_OPPOSITE_KEY,
        _ => DIRECTION_LEFT_KEY,
    }
}

/// L'état de l'écran à un instant : ce qu'`updateAll` relit (`:196-203`).
///
/// **L'état initial** est [`Snapshot::opened`] : quatre largeurs de 5, aucune graine, aucun
/// exploitant, pas de colonie, pas de champ. Les transitions sont [`Snapshot::click`] et
/// [`Snapshot::pick`], les deux seules choses qu'un joueur puisse faire ici.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// Les quatre largeurs (`TileEntityScarecrow.getFieldSize()`, `:184`).
    pub sizes: Sizes,
    /// `button.isEnabled()` de chacune des quatre flèches (`:171`, `:315`). **Cette classe
    /// ne les ferme jamais** : rien en `WindowField.java` n'appelle `setEnabled(false)`, et
    /// la valeur d'ouverture est donc `true` quatre fois.
    pub enabled: [bool; 4],
    /// La graine choisie (`farmField.getSeed()`, `:300`), `None` quand le champ n'en a pas.
    pub seed: Option<&'static str>,
    /// L'exploitant, quand il y en a un (`:278-290`).
    pub farmer: Option<&'static str>,
    /// `getCurrentColony() != null` (`:339-346`) : commande quatre `setVisible`
    /// (`:253-256`).
    pub colony: bool,
    /// `farmField != null` (`:226-234`).
    pub has_field: bool,
    /// Le biome du bloc (`:124-125`).
    pub biome: &'static str,
    /// Les tags de climat que ce biome satisfait (`:132-142`).
    pub climates: &'static [&'static str],
}

impl Snapshot {
    /// L'état d'une fenêtre fraîchement ouverte sur un épouvantail d'une colonie.
    pub const fn opened(biome: &'static str, climates: &'static [&'static str]) -> Snapshot {
        Snapshot {
            sizes: DEFAULT_SIZES,
            enabled: [true; 4],
            seed: None,
            farmer: None,
            colony: true,
            has_field: true,
            biome,
            climates,
        }
    }

    /// L'état d'une fenêtre ouverte sans colonie : six panneaux restent, quatre se cachent.
    pub fn without_colony(biome: &'static str) -> Snapshot {
        Snapshot {
            sizes: DEFAULT_SIZES,
            enabled: [true; 4],
            seed: None,
            farmer: None,
            colony: false,
            has_field: false,
            biome,
            climates: &[],
        }
    }

    /// La largeur d'un côté.
    pub fn width(&self, face: Face) -> i32 {
        width(self.sizes, face)
    }

    /// Ce que le panneau de l'exploitant affiche (`:264`, `:290`).
    pub fn farmer_text(&self) -> FarmerText {
        match self.farmer {
            Some(name) => FarmerText::Assigned(name),
            None => FarmerText::Nobody,
        }
    }

    /// Si un panneau est visible : `updateElementStates` cache quatre panneaux quand il n'y
    /// a pas de colonie (`:253-256`) et **laisse les quatre flèches en place**.
    pub fn visible(&self, id: &str) -> bool {
        let hides = id == PANE_FARMER
            || id == BUTTON_SELECT_SEED
            || id == PANE_SEED
            || id == PANE_CENTER;
        if hides {
            self.colony
        } else {
            true
        }
    }

    /// Le clic sur un bouton (`:169-194` et `:151-162`), et l'état qui en sort.
    ///
    /// Rend l'étape à envoyer et le nouvel état : un clic sur une flèche écrit la largeur
    /// **localement** (`:190`) avant d'envoyer `FarmFieldPlotResizeMessage` (`:193`), donc
    /// l'écran mentirait s'il attendait la réponse du serveur.
    pub fn click(self, id: &str) -> Result<(Step, Snapshot), Refusal> {
        if id == BUTTON_SELECT_SEED {
            self.require_field()?;
            return Ok((Step::PickSeed, self));
        }
        // `:176-177` : Java retire le préfixe, puis cherche le nom dans les quatre
        // directions et sort si la recherche est vide (`:179-182`). Deux échecs distincts :
        // l'id n'est pas un bouton directionnel, ou il en a le préfixe sans en être un.
        let Some(name) = id.strip_prefix(DIRECTION_PREFIX) else {
            return Err(Refusal::UnknownButton);
        };
        let Some(face) = Face::parse(name) else {
            return Err(Refusal::UnknownDirection);
        };
        if !self.enabled[face.index() as usize] {
            return Err(Refusal::ButtonDisabled);
        }
        let value = next_width(self.sizes, face)?;
        let mut next = self;
        set_width(&mut next.sizes, face, value);
        Ok((Step::Resize { face, value }, next))
    }

    /// La graine choisie dans la fenêtre de sélection (`:161`, `(stack, qty) -> setSeed`).
    pub fn pick(self, seed: &'static str) -> Result<(Step, Snapshot), Refusal> {
        self.require_field()?;
        let mut next = self;
        next.seed = Some(seed);
        Ok((Step::Seed(seed), next))
    }

    /// `setSeed` teste `colonyView != null && farmField != null` (`:213`) et ne fait rien
    /// d'autre ; `selectSeed` ne teste pas et plante (`:157`). Le port refuse dans les deux
    /// cas, ce qui est la seule façon de faire dire la même chose aux deux moitiés.
    fn require_field(self) -> Result<(), Refusal> {
        if !self.colony {
            return Err(Refusal::NoColony);
        }
        if !self.has_field {
            return Err(Refusal::NoFieldForBlock);
        }
        Ok(())
    }
}

/// Ce qu'un clic de cet écran envoie ou ouvre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les trois issues d'un clic de cette fenêtre
pub enum Step {
    /// Écrire la largeur, puis `FarmFieldPlotResizeMessage(newRadius, direction, pos)`
    /// (`:193`).
    Resize {
        /// Le côté touché.
        face: Face,
        /// La largeur écrite, et envoyée.
        value: i32,
    },
    /// Ouvrir `WindowSelectRes` (`:154-161`). Aucun message : le choix revient par
    /// [`Snapshot::pick`].
    PickSeed,
    /// `FarmFieldUpdateSeedMessage(colonyView, stack, farmField.getPosition())`
    /// (`:215`).
    Seed(&'static str),
}

impl Step {
    /// Le message que cette étape envoie, `None` pour celle qui ouvre une fenêtre.
    pub fn message(self) -> Option<&'static str> {
        match self {
            Step::Resize { .. } => Some(RESIZE_MESSAGE),
            Step::Seed(_) => Some(SEED_MESSAGE),
            Step::PickSeed => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAINS: &str = "minecraft:plains";

    fn opened() -> Snapshot {
        Snapshot::opened(PLAINS, &["minecraft:is_overworld"])
    }

    #[test]
    fn nine_panes_five_buttons_and_no_level_progress_or_stock() {
        assert_eq!(FIELDS.len(), 9, "cinq valeurs et quatre nombres, windowfield.xml:10-27");
        let values: Vec<Value> = FIELDS.iter().map(|pane| pane.value).collect();
        assert_eq!(
            values,
            [
                Value::Name,
                Value::Text,
                Value::Text,
                Value::Icon,
                Value::Icon,
                Value::Count,
                Value::Count,
                Value::Count,
                Value::Count,
            ]
        );
        // Le fait du fichier : ni niveau, ni progression, ni stock, ni priorité.
        for absent in [Value::Level, Value::Progress, Value::Stock, Value::Priority] {
            assert!(
                !FIELDS.iter().any(|pane| pane.value == absent),
                "{} n'est pas sur cet écran",
                absent.name()
            );
        }
        assert_eq!(BUTTONS.len(), 5);
        for button in BUTTONS {
            assert_eq!(button.naming, Naming::Xml, "{} vient du XML", button.id);
        }
        // Le bouton de graine est le seul avec un libellé.
        assert_eq!(BUTTONS[0].label, SELECT_SEED_KEY);
        for button in &BUTTONS[1..] {
            assert_eq!(button.label, "", "une flèche n'a pas de texte, elle a un nombre");
        }
        assert_eq!(BUTTONS[0].effect, Effect::OpensJava("WindowSelectRes"));
    }

    #[test]
    fn the_four_widths_are_panes_and_buttons_at_once() {
        let panes: Vec<&str> = FIELDS
            .iter()
            .filter(|pane| pane.value == Value::Count)
            .map(|pane| pane.id)
            .collect();
        let arrows: Vec<&str> = BUTTONS[1..].iter().map(|button| button.id).collect();
        // Les mêmes quatre ids, dans l'ordre du XML (nord, sud, ouest, est).
        assert_eq!(panes, arrows);
        assert_eq!(arrows, ["dir-resize-north", "dir-resize-south", "dir-resize-west", "dir-resize-east"]);
        // Et chaque id se relit dans son côté.
        for face in Face::ALL {
            let id = direction_button(face);
            assert_eq!(face_of_button(id), Some(face));
            assert!(panes.contains(&id), "{id} n'est pas un panneau");
        }
        assert_eq!(face_of_button("dir-center"), None);
        assert_eq!(face_of_button("select-seed"), None);
    }

    #[test]
    fn the_default_field_is_already_full_so_the_first_click_shrinks_it() {
        // TileEntityScarecrow.java:47 : quatre fois 5, et MAX_RANGE vaut 20.
        assert_eq!(DEFAULT_SIZES, [5, 5, 5, 5]);
        assert_eq!(sum(DEFAULT_SIZES), MAX_RANGE);
        assert_eq!(left_over(DEFAULT_SIZES), 0, "il ne reste rien à répartir");
        // 5 % min(5 + 0, 20) + 1 = 1 : le côté rétrécit.
        assert_eq!(next_width(DEFAULT_SIZES, Face::North), Ok(1));
        assert_eq!(MAX_RANGE, 20);
        assert_eq!(MAX_RANGE, crate::ai_gather::farmer::MAX_RANGE, "une seule valeur de 20");
    }

    #[test]
    fn a_full_field_cycles_one_to_five_on_the_same_side() {
        let mut sizes = DEFAULT_SIZES;
        let mut seen = Vec::new();
        for _ in 0..5 {
            let value = match next_width(sizes, Face::North) {
                Ok(value) => value,
                Err(refusal) => panic!("le cycle a été refusé : {refusal}"),
            };
            set_width(&mut sizes, Face::North, value);
            seen.push(value);
        }
        assert_eq!(seen, [1, 2, 3, 4, 5], "le côté repart de 1 puis remonte");
        // Et le sixième clic le ramène à 1 : la somme retombe sur 20.
        assert_eq!(sum(sizes), MAX_RANGE);
        assert_eq!(next_width(sizes, Face::North), Ok(1));
    }

    #[test]
    fn a_side_at_zero_on_a_full_field_is_refused_instead_of_dividing_by_zero() {
        // Un composé bricolé : nord à 0, les trois autres à 20 au total.
        let broken: Sizes = [0, 20, 0, 0];
        assert_eq!(sum(broken), MAX_RANGE);
        assert_eq!(left_over(broken), 0);
        assert_eq!(
            next_width(broken, Face::North),
            Err(Refusal::FieldSizeDivideByZero { face: Face::North })
        );
        // Les autres côtés, eux, se calculent : 20 % 20 + 1 = 1.
        assert_eq!(next_width(broken, Face::West), Ok(1));
        assert_eq!(Refusal::FieldSizeDivideByZero { face: Face::North }.key(), None);
    }

    #[test]
    fn a_written_width_never_passes_the_maximum_and_the_index_is_the_two_d_value() {
        // Face::index est l'ordre de Direction.get2DDataValue : sud 0, ouest 1, nord 2, est 3.
        assert_eq!(Face::North.index(), 2);
        assert_eq!(Face::South.index(), 0);
        assert_eq!(Face::West.index(), 1);
        assert_eq!(Face::East.index(), 3);
        let sizes = [10, 20, 30, 40];
        assert_eq!(width(sizes, Face::South), 10);
        assert_eq!(width(sizes, Face::West), 20);
        assert_eq!(width(sizes, Face::North), 30);
        assert_eq!(width(sizes, Face::East), 40);
        // Et l'écriture borne à 20 (`TileEntityScarecrow.java:106`).
        let mut written = [0, 0, 0, 0];
        assert_eq!(set_width(&mut written, Face::East, 99), MAX_RANGE);
        assert_eq!(width(written, Face::East), MAX_RANGE);
        assert_eq!(set_width(&mut written, Face::East, 3), 3);
        assert_eq!(width(written, Face::East), 3);
        // Les trois autres cases n'ont pas bougé.
        assert_eq!(written, [0, 0, 0, 3]);
    }

    #[test]
    fn the_farmer_pane_has_two_labels_and_only_one_pane() {
        let pane = FIELDS
            .iter()
            .find(|pane| pane.id == PANE_FARMER)
            .copied()
            .unwrap_or_else(|| panic!("pas de panneau d'exploitant"));
        assert_eq!(pane.label, None, "la clé dépend de l'état, elle n'est pas sur le panneau");
        assert_eq!(pane.value, Value::Name);
        assert_eq!(pane.source, super::super::Source::CITIZEN);
        assert_eq!(opened().farmer_text(), FarmerText::Nobody);
        assert_eq!(opened().farmer_text().key(), FARMER_NONE_KEY);
        assert_eq!(opened().farmer_text().name(), None);
        let mut worked = opened();
        worked.farmer = Some("Alfred");
        assert_eq!(worked.farmer_text(), FarmerText::Assigned("Alfred"));
        assert_eq!(worked.farmer_text().key(), FARMER_ASSIGNED_KEY);
        assert_ne!(FARMER_NONE_KEY, FARMER_ASSIGNED_KEY);
    }

    #[test]
    fn the_biome_line_keeps_the_prefix_and_falls_back_to_the_path() {
        assert_eq!(biome_lang_key(PLAINS), "biome.minecraft.plains");
        assert_eq!(biome_path(PLAINS), "plains");
        assert_eq!(biome_path("plains"), "plains", "un id sans namespace reste entier");
        let known = biome_text(PLAINS, Some("Plaines"));
        assert_eq!(known.prefix, BIOME_KEY);
        assert_eq!(known.suffix, "Plaines");
        // I18n.exists faux : Java imprime le chemin, pas la clé.
        let unknown = biome_text(PLAINS, None);
        assert_eq!(unknown.suffix, "plains");
        assert_ne!(unknown.suffix, biome_lang_key(PLAINS));
        assert!(BIOME_KEY.starts_with("com.minecolonies."));
    }

    #[test]
    fn the_climate_is_a_comma_without_a_space_between_keys() {
        assert_eq!(climate_text(&[]), "", "aucun tag : Java n'écrit rien");
        assert_eq!(
            climate_text(&["minecraft:is_overworld"]),
            "com.minecolonies.core.tag.crop.biome.is_overworld"
        );
        assert_eq!(
            climate_text(&["minecraft:is_overworld", "minecraft:is_hot"]),
            "com.minecolonies.core.tag.crop.biome.is_overworld,com.minecolonies.core.tag.crop.biome.is_hot"
        );
        // Le séparateur est une virgule nue : `Component.literal(",")` (`:138`).
        assert!(!climate_text(&["minecraft:a", "minecraft:b"]).contains(", "));
        assert_eq!(climate_keys(&["minecraft:is_hot"]).len(), 1);
        // L'ordre est celui de la liste de Java, pas celui du biome.
        assert_eq!(
            climate_text(&CROP_BIOME_TAGS),
            "com.minecolonies.core.tag.crop.biome.coldbiomes,\
             com.minecolonies.core.tag.crop.biome.temperatebiomes,\
             com.minecolonies.core.tag.crop.biome.humidbiomes,\
             com.minecolonies.core.tag.crop.biome.drybiomes"
        );
        assert_eq!(CROP_BIOME_TAGS[0], "minecolonies:coldbiomes");
        assert_eq!(biome_path(CROP_BIOME_TAGS[0]), "coldbiomes");
    }

    #[test]
    fn the_relative_name_reads_the_players_facing() {
        // Java prend `facing` = la direction que le joueur **regarde**
        // (`WindowField.java:356-357`, `Direction.orderedByNearest` rend la plus proche en
        // premier), puis compare à `facing.getOpposite()` (`:359`) : la référence est donc
        // la direction **derrière** le joueur, et le côté « le plus proche » est celui qu'il
        // a dans le dos. Ce n'est pas une lecture intuitive, c'est la ligne 359.
        //
        // Joueur qui regarde le nord : `facing.getOpposite()` = sud = 2DDataValue 0, donc
        // delta = 0 - cible.
        assert_eq!(
            relative_key(Face::North, Face::South),
            DIRECTION_NEAREST_KEY,
            "0 - 0 = 0 → `default` (:364)"
        );
        assert_eq!(
            relative_key(Face::North, Face::East),
            DIRECTION_RIGHT_KEY,
            "0 - 3 = -3 → `case 1, -3` (:361)"
        );
        assert_eq!(
            relative_key(Face::North, Face::North),
            DIRECTION_OPPOSITE_KEY,
            "0 - 2 = -2 → `case 2, -2` (:362) : le côté devant soi est « opposé »"
        );
        assert_eq!(
            relative_key(Face::North, Face::West),
            DIRECTION_LEFT_KEY,
            "0 - 1 = -1 → `case 3, -1` (:363)"
        );
        // Les quatre clés sont distinctes, et aucune n'est le préfixe absolu.
        let keys = [
            relative_key(Face::South, Face::South),
            relative_key(Face::South, Face::West),
            relative_key(Face::South, Face::North),
            relative_key(Face::South, Face::East),
        ];
        for (index, key) in keys.iter().enumerate() {
            assert!(key.starts_with(DIRECTION_ABSOLUTE));
            for other in &keys[index + 1..] {
                assert_ne!(key, other);
            }
        }
        // Joueur qui regarde le sud : `facing.getOpposite()` = nord = 2, delta = 2 - cible.
        assert_eq!(
            keys,
            [DIRECTION_OPPOSITE_KEY, DIRECTION_RIGHT_KEY, DIRECTION_NEAREST_KEY, DIRECTION_LEFT_KEY]
        );
        // Le nom absolu, lui, est le préfixe plus le nom du côté (`:327`).
        assert_eq!(format!("{}{}", DIRECTION_ABSOLUTE, Face::East.name()), "com.minecolonies.coremod.gui.field.east");
    }

    #[test]
    fn a_click_writes_the_width_before_it_sends_it() {
        let state = opened();
        let (step, next) = match state.click("dir-resize-north") {
            Ok(pair) => pair,
            Err(refusal) => panic!("le clic a été refusé : {refusal}"),
        };
        assert_eq!(step, Step::Resize { face: Face::North, value: 1 });
        assert_eq!(next.width(Face::North), 1, "l'écran écrit avant d'envoyer (`:190`)");
        assert_eq!(next.width(Face::South), DEFAULT_RANGE, "un seul côté bouge");
        // Et l'état d'origine n'a pas changé : `click` rend le nouveau.
        assert_eq!(state.width(Face::North), DEFAULT_RANGE);
        // Le second clic sur le même bouton monte à 2.
        let (again, twice) = match next.click("dir-resize-north") {
            Ok(pair) => pair,
            Err(refusal) => panic!("le second clic a été refusé : {refusal}"),
        };
        assert_eq!(again, Step::Resize { face: Face::North, value: 2 });
        assert_eq!(twice.width(Face::North), 2);
    }

    #[test]
    fn a_click_on_a_closed_button_or_an_unknown_id_does_nothing() {
        let mut closed = opened();
        closed.enabled[Face::East.index() as usize] = false;
        assert_eq!(
            closed.click("dir-resize-east"),
            Err(Refusal::ButtonDisabled),
            "WindowField.java:171-174"
        );
        // Les trois autres restent ouvertes.
        assert!(closed.click("dir-resize-west").is_ok());
        assert_eq!(closed.click("select-seed"), Ok((Step::PickSeed, closed)));
        assert_eq!(closed.click("dir-center"), Err(Refusal::UnknownButton));
        assert_eq!(closed.click("done"), Err(Refusal::UnknownButton));
        // Le préfixe sans direction valide est l'autre branche de Java (`:179-182`).
        assert_eq!(closed.click("dir-resize-up"), Err(Refusal::UnknownDirection));
        assert_eq!(Refusal::UnknownDirection.key(), None);
    }

    #[test]
    fn without_a_colony_four_panes_are_hidden_and_the_arrows_are_not() {
        let alone = Snapshot::without_colony(PLAINS);
        assert!(!alone.colony);
        for hidden in [PANE_FARMER, BUTTON_SELECT_SEED, PANE_SEED, PANE_CENTER] {
            assert!(!alone.visible(hidden), "{hidden} devrait être caché, :253-256");
        }
        // Les quatre flèches restent : `updateElementStates` n'y touche pas.
        for face in Face::ALL {
            assert!(alone.visible(direction_button(face)));
        }
        assert!(alone.visible(PANE_BIOME));
        assert!(alone.visible(PANE_CLIMATE));
        // Et le clic sur la graine refuse, là où Java planterait (`:157`).
        assert_eq!(alone.click(BUTTON_SELECT_SEED), Err(Refusal::NoColony));
        assert_eq!(Refusal::NoColony.key(), None);
    }

    #[test]
    fn a_missing_field_refuses_the_seed_instead_of_dereferencing_it() {
        let mut no_field = opened();
        no_field.has_field = false;
        assert_eq!(no_field.click(BUTTON_SELECT_SEED), Err(Refusal::NoFieldForBlock));
        assert_eq!(no_field.pick("minecraft:wheat_seeds"), Err(Refusal::NoFieldForBlock));
        // Avec un champ, le choix passe et change la graine.
        let (step, next) = match opened().pick("minecraft:wheat_seeds") {
            Ok(pair) => pair,
            Err(refusal) => panic!("le choix de graine a été refusé : {refusal}"),
        };
        assert_eq!(step, Step::Seed("minecraft:wheat_seeds"));
        assert_eq!(next.seed, Some("minecraft:wheat_seeds"));
        assert_eq!(opened().seed, None);
    }
}
