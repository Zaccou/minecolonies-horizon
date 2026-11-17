//! La liste des recettes d'un artisan : neuf emplacements d'entrée, une sortie qui clignote,
//! quatre boutons par ligne et un cinquième sous la liste.
//!
//! Java reference : `core/client/gui/modules/building/WindowListRecipes.java` (la fenêtre,
//! `:79-292`) et `core/colony/buildings/moduleviews/CraftingModuleView.java` (la donnée et
//! les quatre mutations, `:131-352`), sur `gui/layouthuts/layoutlistrecipes.xml`
//! (`:7-36`).
//!
//! **Ce que cette fenêtre demande** : les recettes enseignées au module, celles qui sont
//! désactivées, le nombre d'actives et son plafond, les types de recette que le module
//! accepte, et — pour chaque recette — ses entrées avec leurs comptes, sa sortie, son outil
//! et son bloc intermédiaire. **Ce qu'elle rend** : neuf icônes, une icône de sortie animée,
//! une ligne de texte, quatre boutons de ligne et un bouton de fenêtre.
//!
//! # Vingt-huit clés de module, une seule fenêtre
//!
//! `CraftingModuleView::getWindow` (`:185-190`) est le `getWindow()` de **28** modules —
//! `farmer_work`, `baker_craft`, `baker_smelt`, `smelter_orebreak`, … ([`MODULE_KEYS`],
//! comptées dans `core/colony/buildings/`). Il n'existe **aucune** clé `crafting` : le
//! module est nommé `<hut>_<ce qu'il fait>`, et `src/ui::MODULE_SCREENS` n'en porte aucune
//! des 28. [`MODULE`] est donc l'une des 28 — `Source::Module` ne prend qu'une clé — et
//! c'est `MANQUANT.md` §1 qui dit la variante qui manque.
//!
//! # Trois comportements que la donnée doit garder
//!
//! 1. **La sortie tourne.** `onUpdate` incrémente `lifeCount` à chaque image sauf si Shift
//!    est tenu, et l'icône affiche `outputs[(lifeCount / 30) % outputs.size()]`
//!    (`:180`, `:282-292`, `LIFE_COUNT_DIVIDER = 30` à `WindowConstants.java:922`). Une
//!    recette à sorties alternatives **clignote donc entre elles**, et c'est le seul
//!    endroit où le temps entre dans ces quatre écrans.
//! 2. **Shift ne « décale » pas d'une ligne : il envoie en tête ou en queue.**
//!    `switchOrder(i, j, fullMove)` (`CraftingModuleView.java:220-240`) fait `add(0,
//!    remove(i))` quand `i > j` et `add(remove(i))` sinon : vers le haut la recette passe
//!    **première**, vers le bas **dernière**. C'est le serveur qui élargit ensuite au bloc
//!    de recettes identiques (`ChangeRecipePriorityMessage.java:93-103`, et le commentaire
//!    de `src/network/building_modules.rs:672-674`).
//! 3. **Le panneau `intermediate` n'est rendu visible que pour un outil.** Le bloc
//!    intermédiaire écrit son nom dans le panneau mais le `setVisible(true)` est
//!    **commenté** (`:218`) : une recette à fournaise affiche donc un texte que personne ne
//!    voit. [`Marker`] garde les deux cas séparés au lieu de corriger Java.

use crate::crafting::CrafterRecipe;
use crate::inventory::ItemName;
use crate::jobs::ToolKind;
use crate::ui::action::Effect;
use crate::ui::field::{Field, Source, Value};

use super::refusal::{Kind, Refusal};
use super::Button;

/// Les 28 clés de module dont la vue est `CraftingModuleView`
/// (`core/colony/buildings/**`, un `ModuleProducer` `…CraftingModuleView::new` chacun).
///
/// Elles ne se déduisent pas : le suffixe change — `_work`, `_craft`, `_smelt`,
/// `_smelting`, `_brew`, `_orebreak` — et `forester_craft` porte le nom de l'arbre, pas du
/// métier. Trois bâtiments en ont deux (`baker_craft` + `baker_smelt`), un en a trois
/// (`smelter_smelting`, `smelter_orebreak`).
pub const MODULE_KEYS: &[&str] = &[
    "farmer_work",
    "plantation_craft",
    "alchemist_craft",
    "alchemist_brew",
    "baker_craft",
    "baker_smelt",
    "blacksmith_craft",
    "concretemixer_craft",
    "crusher_craft",
    "dyer_craft",
    "dyer_smelt",
    "fletcher_craft",
    "glassblower_craft",
    "glassblower_smelting",
    "mechanic_craft",
    "sawmill_craft",
    "sifter_craft",
    "smelter_smelting",
    "smelter_orebreak",
    "stonemason_craft",
    "stonesmelter_smelting",
    "builder_craft",
    "chef_craft",
    "chef_smelt",
    "forester_craft",
    "miner_craft",
    "enchanter_craft",
    "netherworker_craft",
];

/// La clé que les panneaux citent : la première des 28, et un test le vérifie. Ce n'est pas
/// « la » clé du module — il n'y en a pas —, c'est la seule que `Source::Module` puisse
/// porter.
pub const MODULE: &str = "farmer_work";

/// La liste (`WindowListRecipes.java:39`).
pub const LIST: &str = "recipes";
/// Le compteur d'actives (`:44`, et le XML l'écrit : `layoutlistrecipes.xml:7`).
pub const STATUS: &str = "recipestatus";
/// L'icône de sortie (`:49`).
pub const OUTPUT: &str = "output";
/// L'étiquette de l'outil ou du bloc intermédiaire (`:208` et le XML `:29`).
pub const INTERMEDIATE: &str = "intermediate";
/// Le voile gris des recettes désactivées (`:223`, `:229`). Il ne porte **aucune** valeur :
/// c'est le drapeau [`Row::disabled`] dessiné, et c'est pourquoi ce n'est pas un [`Field`].
pub const GRADIENT: &str = "gradient";
/// Le bouton qui ouvre la grille d'artisanat (`:64`).
pub const CRAFTING: &str = "crafting";
/// Le bouton de retrait, un par ligne (`WindowConstants.java:584`).
pub const REMOVE: &str = "remove";
/// Le bouton d'activation, un par ligne (`WindowConstants.java:497`).
pub const TOGGLE: &str = "toggle";
/// Le bouton « monter », un par ligne (`WindowConstants.java:487` — son id est `up`).
pub const UP: &str = "up";
/// Le bouton « descendre », un par ligne (`WindowConstants.java:492` — son id est `down`).
pub const DOWN: &str = "down";

/// Le nombre d'emplacements d'entrée : `res1` à `res9`
/// (`WindowListRecipes.java:237-267`, `layoutlistrecipes.xml:13-23`).
pub const SLOT_COUNT: usize = 9;

/// L'id du panneau du `slot`-ième emplacement (base zéro), `res1` pour le premier
/// (`String.format(RESOURCE, i + 1)`, `:241`).
///
/// C'est une **fonction** et non neuf constantes parce que Java construit la même chaîne
/// dans ses trois boucles ; les neuf panneaux restent neuf [`Field`] distincts.
pub fn slot_pane(slot: usize) -> String {
    format!("res{}", slot + 1)
}

/// Le diviseur de l'animation de sortie (`WindowConstants.java:922`).
pub const LIFE_DIVIDER: i64 = 30;

/// Le titre (`layoutlistrecipes.xml:4-5`).
pub const TITLE_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.listrecipes";
/// Le compteur, deux arguments (`TranslationConstants.java:396`, « %s of %s » à
/// `manual_en_us.json:1013`).
pub const STATUS_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.recipestatus";
/// Le libellé de `crafting` (`manual_en_us.json:1010`).
pub const CRAFTING_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.crafting";
/// Le libellé de `remove` (`manual_en_us.json:1007`).
pub const REMOVE_KEY: &str = "com.minecolonies.coremod.gui.recipe.remove";
/// Le libellé de `toggle` quand la recette est désactivée (`:224`).
pub const ENABLE_KEY: &str = "com.minecolonies.coremod.gui.recipe.enable";
/// Le libellé de `toggle` quand elle est active (`:230`).
pub const DISABLE_KEY: &str = "com.minecolonies.coremod.gui.recipe.disable";
/// Le plafond de recettes, « Max number of recipes reached for hut level »
/// (`manual_en_us.json:1018`).
///
/// Cette fenêtre ne l'affiche **pas** : elle cache le bouton `toggle` à la place
/// (`:225`). La clé est gardée pour que le test du refus muet puisse le prouver.
pub const FULL_KEY: &str = "com.minecolonies.coremod.gui.recipe.full";

/// Les treize panneaux de la fenêtre : le compteur, la liste, la sortie, les neuf entrées et
/// l'étiquette d'outil.
const F_STATUS: Field = Field::new(STATUS, Value::Text, Source::Module(MODULE));
const F_LIST: Field = Field::new(LIST, Value::Row, Source::Module(MODULE));
const F_OUTPUT: Field = Field::in_row(LIST, OUTPUT, Value::Icon, Source::Module(MODULE));
const F_RES_1: Field = Field::in_row(LIST, "res1", Value::Icon, Source::Module(MODULE));
const F_RES_2: Field = Field::in_row(LIST, "res2", Value::Icon, Source::Module(MODULE));
const F_RES_3: Field = Field::in_row(LIST, "res3", Value::Icon, Source::Module(MODULE));
const F_RES_4: Field = Field::in_row(LIST, "res4", Value::Icon, Source::Module(MODULE));
const F_RES_5: Field = Field::in_row(LIST, "res5", Value::Icon, Source::Module(MODULE));
const F_RES_6: Field = Field::in_row(LIST, "res6", Value::Icon, Source::Module(MODULE));
const F_RES_7: Field = Field::in_row(LIST, "res7", Value::Icon, Source::Module(MODULE));
const F_RES_8: Field = Field::in_row(LIST, "res8", Value::Icon, Source::Module(MODULE));
const F_RES_9: Field = Field::in_row(LIST, "res9", Value::Icon, Source::Module(MODULE));
const F_INTERMEDIATE: Field =
    Field::in_row(LIST, INTERMEDIATE, Value::Text, Source::Module(MODULE));

/// Les panneaux de la fenêtre, dans l'ordre du XML.
pub const FIELDS: &[Field] = &[
    F_STATUS,
    F_LIST,
    F_OUTPUT,
    F_RES_1,
    F_RES_2,
    F_RES_3,
    F_RES_4,
    F_RES_5,
    F_RES_6,
    F_RES_7,
    F_RES_8,
    F_RES_9,
    F_INTERMEDIATE,
];

/// Les cinq boutons : un de fenêtre et quatre de ligne (`:88-92`, `layoutlistrecipes.xml:25-36`).
const A_CRAFTING: Button = Button::on_window(
    CRAFTING,
    CRAFTING_KEY,
    // `openCraftingGUI` ouvre le menu de l'entité de la hutte
    // (`CraftingModuleView.java:252-257`) : c'est le conteneur que ce port déclare déjà
    // (`src/hut.rs`, `hut::container`), pas un écran de ce module.
    Effect::Container,
    &[Kind::RecipeAlterationForbidden],
);
const A_UP: Button = Button::on_row(
    UP,
    "↥",
    Effect::Sends("ChangeRecipePriorityMessage"),
    &[Kind::EdgeOfList],
);
const A_DOWN: Button = Button::on_row(
    DOWN,
    "↧",
    Effect::Sends("ChangeRecipePriorityMessage"),
    &[Kind::EdgeOfList],
);
const A_REMOVE: Button = Button::on_row(
    REMOVE,
    REMOVE_KEY,
    Effect::Sends("AddRemoveRecipeMessage"),
    // Java's order: the alteration right first (:185), then the builtin (:188).
    &[Kind::RecipeAlterationForbidden, Kind::BuiltinRecipeNeedsControl],
);
const A_TOGGLE: Button = Button::on_row(
    TOGGLE,
    DISABLE_KEY,
    Effect::Sends("ToggleRecipeMessage"),
    &[Kind::RecipeLimitReached],
);

/// Les boutons de la fenêtre, dans l'ordre du XML : `up` et `down` avant `remove` et
/// `toggle` sur la ligne, `crafting` sous la liste.
pub const BUTTONS: &[Button] = &[A_UP, A_DOWN, A_REMOVE, A_TOGGLE, A_CRAFTING];

/// Le libellé d'un bouton `toggle`, selon l'état de la ligne (`:224`, `:230`).
pub fn toggle_label(disabled: bool) -> &'static str {
    if disabled {
        ENABLE_KEY
    } else {
        DISABLE_KEY
    }
}

/// Une recette telle que la ligne la dessine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// `getRecipeType().getOutputDisplayStacks()` (`:179`) : une ou plusieurs sorties, et
    /// c'est cette liste que l'animation parcourt.
    pub outputs: Vec<(ItemName, u32)>,
    /// `getInput()`, dans l'ordre (`:235-267`). Neuf au plus sont dessinées.
    pub inputs: Vec<(ItemName, u32)>,
    /// `getRequiredTool()` ; [`ToolKind::None`] quand la recette n'en demande pas
    /// (`:210-214`).
    pub tool: ToolKind,
    /// `getIntermediate()`, `None` pour `Blocks.AIR` (`:215-219`).
    pub intermediate: Option<ItemName>,
    /// `getRecipeSource() != null` : une recette intégrée, qu'on ne retire qu'avec Ctrl
    /// (`:188`).
    pub builtin: bool,
    /// Le drapeau que `toggle` bascule (`:221-232`, `CraftingModuleView.toggle`).
    pub disabled: bool,
}

impl Row {
    /// Une recette lue dans le format `crafterrecipes/` du port
    /// ([`CrafterRecipe`]), ce qui est la seule des deux moitiés que le port modélise.
    ///
    /// La sortie principale vient de `result`/`count` et les **alternatives** suivent :
    /// ce sont elles que `getOutputDisplayStacks` fait clignoter pour un type
    /// `multi-output`, et `additional-output` n'y est pas — c'est un bonus qui sort *avec*
    /// le résultat, pas une variante. `None` pour une recette sans résultat (une table de
    /// butin, `CustomRecipe.java:278-285`).
    pub fn from_crafter(recipe: &CrafterRecipe, builtin: bool) -> Option<Row> {
        let primary = recipe.result.as_ref()?;
        let mut outputs = vec![(primary.name().clone(), recipe.count)];
        for alternate in &recipe.alternate_output {
            outputs.push((alternate.item.name().clone(), alternate.count));
        }
        Some(Row {
            outputs,
            inputs: recipe
                .inputs
                .iter()
                .map(|input| (input.item.name().clone(), input.count))
                .collect(),
            tool: recipe.tool,
            intermediate: block_name(&recipe.intermediate.to_string()),
            builtin,
            disabled: false,
        })
    }

    /// Quelle entrée va dans quel emplacement, `None` pour un emplacement vide.
    ///
    /// Java a **trois** branches (`:235-267`) et elles ne sont pas trois cas d'un même
    /// dessin : moins de quatre entrées remplit 1, 2, 3 ; **quatre** entrées se posent en
    /// croix sur 1, 2, 4, 5 (`:249-260`, la forme d'une grille 2×2 centrée) ; cinq et plus
    /// remplissent 1 à 9 dans l'ordre. Une recette de quatre objets occupe donc quatre
    /// emplacements **disjoints** et laisse 3 et 6 à 9 vides.
    pub fn slots(&self) -> [Option<usize>; SLOT_COUNT] {
        slots_of(self.inputs.len())
    }

    /// L'icône de sortie à cette image de l'animation (`:180`) : `None` pour une recette
    /// sans sortie — Java lèverait `ArithmeticException` sur `% 0`.
    pub fn output_at(&self, life: i64) -> Option<(ItemName, u32)> {
        if self.outputs.is_empty() {
            return None;
        }
        let index = (life.max(0) / LIFE_DIVIDER) as usize % self.outputs.len();
        self.outputs.get(index).cloned()
    }

    /// Ce que le panneau `intermediate` porte, et s'il est visible (`:208-219`).
    pub fn marker(&self) -> Marker {
        if self.tool != ToolKind::None {
            Marker::Tool(self.tool)
        } else if let Some(block) = &self.intermediate {
            Marker::Block(block.clone())
        } else {
            Marker::Hidden
        }
    }

    /// La ligne `remove` est-elle grisée ? (`:188-196`)
    pub fn remove_needs_control(&self, control_down: bool) -> bool {
        self.builtin && !control_down
    }
}

/// Ce que le panneau `intermediate` contient — et la raison pour laquelle c'est un type.
///
/// `setVisible(false)` est appelé avant les deux branches (`:209`) ; la branche **outil**
/// rappelle `setVisible(true)` (`:214`) et la branche **bloc** ne le fait pas — la ligne est
/// commentée (`:218`). Un bloc intermédiaire écrit donc son nom dans un panneau que
/// personne ne dessine.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: les trois branches de Java, y compris celle qui ne s'affiche pas
pub enum Marker {
    /// Aucun outil et aucun bloc : panneau vide et caché.
    Hidden,
    /// Un outil : son nom, panneau **visible** (`:212-213`).
    Tool(ToolKind),
    /// Un bloc intermédiaire : son nom, panneau **caché** (`:217-218`).
    Block(ItemName),
}

impl Marker {
    /// Vrai quand Java rappelle `setVisible(true)`.
    pub fn is_visible(&self) -> bool {
        matches!(self, Marker::Tool(_))
    }

    /// Le texte écrit dans le panneau, quand il y en a un.
    pub fn text(&self) -> Option<String> {
        match self {
            Marker::Hidden => None,
            // `getDisplayName()` d'un `IEquipmentType` : un nom traduit, pas le chemin de
            // registre. Le port n'a que le chemin (`ToolKind::name`).
            Marker::Tool(tool) => Some(tool.name().to_owned()),
            Marker::Block(block) => Some(block.to_string()),
        }
    }
}

/// L'emplacement de chaque entrée, pour `inputs` entrées (`:235-267`).
pub fn slots_of(inputs: usize) -> [Option<usize>; SLOT_COUNT] {
    let mut slots = [None; SLOT_COUNT];
    if inputs == 4 {
        // `:249-260` : 0→1, 1→2, 2→4, 3→5.
        slots[0] = Some(0);
        slots[1] = Some(1);
        slots[3] = Some(2);
        slots[4] = Some(3);
    } else {
        for (slot, entry) in slots.iter_mut().enumerate() {
            if slot < inputs.min(SLOT_COUNT) {
                *entry = Some(slot);
            }
        }
    }
    slots
}

/// Le nom d'un bloc, `None` pour `minecraft:air` — le test `getIntermediate() !=
/// Blocks.AIR` (`:215`).
pub fn block_name(id: &str) -> Option<ItemName> {
    let name = ItemName::new(id)?;
    if name.as_str() == "minecraft:air" {
        None
    } else {
        Some(name)
    }
}

/// L'image suivante de l'animation : `lifeCount++` sauf si Shift est tenu
/// (`:282-289`).
///
/// Java incrémente un `int` par image d'écran ; un client qui laisse la fenêtre ouverte
/// des jours le fait déborder. Le port compte en `i64`, donc le débordement n'arrive pas
/// avant un million d'années d'images — la règle est la même, le compteur est plus large.
pub fn next_life(life: i64, shift_down: bool) -> i64 {
    if shift_down {
        life
    } else {
        life.wrapping_add(1)
    }
}

/// La direction d'un clic sur `up`/`down`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les deux boutons de la fenêtre
pub enum Direction {
    /// Le bouton `up` : vers l'index 0 (`forwardClicked`, `:124-131`).
    Up,
    /// Le bouton `down` : vers la fin (`backwardClicked`, `:111-118`).
    Down,
}

/// Le compteur « actives / plafond » (`:290`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status {
    /// La clé, deux arguments (`RECIPE_STATUS`).
    pub key: &'static str,
    pub active: i32,
    pub max: i32,
}

impl Status {
    /// Le rendu de `en_us` : « %s of %s » (`manual_en_us.json:1013`).
    ///
    /// Le format vit dans la langue, pas dans Java : c'est une clé à deux arguments, et
    /// ceci n'est qu'une des traductions possibles.
    pub fn render(self) -> String {
        format!("{} of {}", self.active, self.max)
    }
}

/// L'état du module : les recettes, les deux compteurs et le droit d'enseigner.
///
/// Les quatre mutations sont celles de `CraftingModuleView` : `toggle` (`:334-352`),
/// `removeRecipe` (`:217-225`), `switchOrder` (`:220-240`) et les compteurs (`:242-250`).
/// Aucun message n'est envoyé d'ici : `src/network/building_modules.rs` porte les trois
/// classes (`ToggleRecipe`, `AddRemoveRecipe`, `ChangeRecipePriority`), et c'est à
/// l'appelant de les remplir.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Basket {
    rows: Vec<Row>,
    active: i32,
    max: i32,
    alteration: bool,
}

impl Basket {
    /// `alteration` est `isRecipeAlterationAllowed()` : `!recipeTypeSet.isEmpty()`
    /// (`:145-148`), donc « ce module accepte des types de recette », pas « le joueur a le
    /// droit ».
    pub fn new(rows: Vec<Row>, active: i32, max: i32, alteration: bool) -> Basket {
        Basket { rows, active, max, alteration }
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn active(&self) -> i32 {
        self.active
    }

    pub fn max(&self) -> i32 {
        self.max
    }

    /// Le droit d'enseigner, tel que la fenêtre le lit pour masquer `crafting` (`:85`).
    pub fn alteration_allowed(&self) -> bool {
        self.alteration
    }

    /// Le compteur du haut de la fenêtre (`:290`).
    pub fn status(&self) -> Status {
        Status { key: STATUS_KEY, active: self.active, max: self.max }
    }

    /// Le compteur est-il dessiné ? `recipeStatus.setVisible(module.isRecipeAlterationAllowed())`
    /// (`:86`) : même condition que le bouton `crafting`, lue deux fois par le constructeur.
    pub fn status_visible(&self) -> bool {
        self.alteration
    }

    pub fn row(&self, index: usize) -> Result<&Row, Refusal> {
        self.rows
            .get(index)
            .ok_or(Refusal::NoSuchRow { row: index, rows: self.rows.len() })
    }

    /// `moduleView.toggle(row)` (`:100-105`) : bascule le drapeau, et n'ajuste le compte
    /// **que** pour une recette qui n'est pas intégrée (`CraftingModuleView.java:334-352`).
    ///
    /// Rend l'état `disabled` après le clic, qui est aussi ce que le libellé du bouton
    /// doit lire. Java ne borne pas `activeRecipes` : un module dont le compte a dérivé
    /// peut donc passer dessous zéro, et le port le laisse faire plutôt que d'inventer une
    /// correction que le serveur ne fait pas.
    pub fn toggle(&mut self, index: usize) -> Result<bool, Refusal> {
        let rows = self.rows.len();
        let row = self
            .rows
            .get_mut(index)
            .ok_or(Refusal::NoSuchRow { row: index, rows })?;
        row.disabled = !row.disabled;
        if !row.builtin {
            if row.disabled {
                self.active -= 1;
            } else {
                self.active += 1;
            }
        }
        Ok(row.disabled)
    }

    /// Le libellé que le bouton `toggle` porte pour cette ligne (`:224`, `:230`).
    pub fn toggle_label(&self, index: usize) -> Result<&'static str, Refusal> {
        Ok(toggle_label(self.row(index)?.disabled))
    }

    /// Le bouton `toggle` est-il dessiné ? (`:221-232`)
    ///
    /// Toujours, pour une recette active ; pour une recette désactivée, seulement si elle
    /// est intégrée ou s'il reste de la place sous le plafond.
    pub fn toggle_visible(&self, index: usize) -> Result<bool, Refusal> {
        let row = self.row(index)?;
        if !row.disabled {
            return Ok(true);
        }
        Ok(row.builtin || self.active < self.max)
    }

    /// Le bouton `remove` est-il actif ? (`:182-206`)
    ///
    /// `Ok` quand il l'est, [`Refusal::RecipeAlterationForbidden`] quand il est éteint
    /// (`off()`, `:204`), [`Refusal::BuiltinRecipeNeedsControl`] quand il est grisé pour
    /// une recette intégrée sans Ctrl (`:190-195`). L'ordre est celui de Java : le droit
    /// d'enseigner d'abord.
    pub fn remove_visible(&self, index: usize, control_down: bool) -> Result<(), Refusal> {
        let row = self.row(index)?;
        if !self.alteration {
            return Err(Refusal::RecipeAlterationForbidden);
        }
        if row.remove_needs_control(control_down) {
            return Err(Refusal::BuiltinRecipeNeedsControl);
        }
        Ok(())
    }

    /// `moduleView.removeRecipe(row)` (`:137-143`) : retire la ligne et rend ce que
    /// `AddRemoveRecipeMessage` doit transporter (`:140-142`).
    ///
    /// Le gestionnaire de Java ne teste **que** Ctrl (`:188`), le droit d'enseigner étant
    /// déjà lu par le rendu qui éteint le bouton (`:204`). Le port refuse les deux, comme
    /// il le fait pour la réparation de la mine : un clic peut arriver sur un bouton que le
    /// rendu précédent n'avait pas encore éteint.
    pub fn remove(&mut self, index: usize, control_down: bool) -> Result<Row, Refusal> {
        self.remove_visible(index, control_down)?;
        let doomed = self.rows.remove(index);
        // `CraftingModuleView.java:217-225` : une recette intégrée ne compte pas, une
        // recette désactivée non plus.
        if !doomed.builtin && !doomed.disabled {
            self.active -= 1;
        }
        Ok(doomed)
    }

    /// `moduleView.switchOrder(row, target, shift)` (`:111-131`, `CraftingModuleView.java:220-240`).
    ///
    /// Rend le nouvel index de la ligne. `full_move` (Shift) envoie la recette **en tête**
    /// (vers le haut) ou **en queue** (vers le bas) ; sans Shift, elle échange avec sa
    /// voisine. Aux deux extrémités, rien ne bouge et c'est
    /// [`Refusal::EdgeOfList`] — **le message part quand même** (`:116`, `:129`).
    pub fn move_row(
        &mut self,
        index: usize,
        direction: Direction,
        full_move: bool,
    ) -> Result<usize, Refusal> {
        let len = self.rows.len();
        if index >= len {
            return Err(Refusal::NoSuchRow { row: index, rows: len });
        }
        let edge = Refusal::EdgeOfList { row: index, len };
        let target = match direction {
            Direction::Up if index == 0 => return Err(edge),
            Direction::Down if index + 1 >= len => return Err(edge),
            Direction::Up => index - 1,
            Direction::Down => index + 1,
        };
        let moved = self.rows.remove(index);
        let to = if full_move {
            match direction {
                Direction::Up => 0,
                Direction::Down => len - 1,
            }
        } else {
            target
        };
        self.rows.insert(to, moved);
        Ok(to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str) -> ItemName {
        ItemName::new(name).expect("a registry name")
    }

    fn row(inputs: usize, outputs: usize) -> Row {
        Row {
            outputs: (0..outputs).map(|index| (item("minecraft:stone"), index as u32)).collect(),
            inputs: (0..inputs).map(|_| (item("minecraft:cobblestone"), 1)).collect(),
            tool: ToolKind::None,
            intermediate: None,
            builtin: false,
            disabled: false,
        }
    }

    #[test]
    fn twenty_eight_module_keys_open_one_window_and_the_field_names_one_of_them() {
        assert_eq!(MODULE_KEYS.len(), 28);
        assert_eq!(MODULE_KEYS[0], MODULE);
        for (index, key) in MODULE_KEYS.iter().enumerate() {
            assert!(!key.is_empty());
            for other in &MODULE_KEYS[index + 1..] {
                assert_ne!(key, other, "two modules, one key");
            }
        }
        // None of the 28 is in src/ui's table: the recipes tab has no line there.
        for key in MODULE_KEYS {
            assert!(
                crate::ui::module_screen(key).is_none(),
                "{key} is already described by src/ui"
            );
        }
    }

    #[test]
    fn the_thirteen_panes_are_the_ones_the_xml_declares() {
        assert_eq!(FIELDS.len(), 13);
        let slots: Vec<&Field> = FIELDS
            .iter()
            .filter(|field| field.scope == crate::ui::field::Scope::Row(LIST))
            .collect();
        // One output, nine entries, one label.
        assert_eq!(slots.len(), 11);
        for (slot, field) in FIELDS
            .iter()
            .filter(|field| field.id.starts_with("res"))
            .enumerate()
        {
            assert_eq!(field.id, slot_pane(slot));
            assert_eq!(field.value, Value::Icon);
        }
        assert_eq!(FIELDS[0].id, STATUS);
        assert_eq!(FIELDS[0].value, Value::Text, "the counter is a label, not a number");
        assert_eq!(FIELDS[1].value, Value::Row);
        // The gradient draws a flag, not a value: it is not a pane of this table.
        assert!(FIELDS.iter().all(|field| field.id != GRADIENT));
    }

    #[test]
    fn four_inputs_are_laid_out_in_a_cross_and_the_rest_in_a_row() {
        assert_eq!(slots_of(0), [None; SLOT_COUNT]);
        assert_eq!(slots_of(1)[0], Some(0));
        assert_eq!(slots_of(2)[1], Some(1));
        assert_eq!(slots_of(2)[2], None);
        // The 2x2 case: 1, 2, 4, 5 — WindowListRecipes.java:249-260.
        let cross = slots_of(4);
        assert_eq!(cross, [Some(0), Some(1), None, Some(2), Some(3), None, None, None, None]);
        // Five or more fills 1..9 in order, and nine is the ceiling.
        assert_eq!(slots_of(5)[4], Some(4));
        assert_eq!(slots_of(12)[8], Some(8));
        assert_eq!(slots_of(12).iter().filter(|slot| slot.is_none()).count(), 0);
        assert_eq!(row(4, 1).slots(), cross);
    }

    #[test]
    fn the_output_cycles_every_thirty_frames_and_stops_on_shift() {
        let two = row(1, 2);
        assert_eq!(two.output_at(0).map(|output| output.1), Some(0));
        assert_eq!(two.output_at(29).map(|output| output.1), Some(0));
        assert_eq!(two.output_at(30).map(|output| output.1), Some(1));
        assert_eq!(two.output_at(59).map(|output| output.1), Some(1));
        assert_eq!(two.output_at(60).map(|output| output.1), Some(0), "it wraps");
        assert_eq!(two.output_at(-5).map(|output| output.1), Some(0), "never negative");
        // A recipe with no output is None instead of Java's ArithmeticException.
        assert_eq!(row(1, 0).output_at(0), None);
        assert_eq!(next_life(7, false), 8);
        assert_eq!(next_life(7, true), 7, "onUpdate does not count while Shift is held");
        assert_eq!(LIFE_DIVIDER, 30);
    }

    #[test]
    fn a_tool_is_shown_and_an_intermediate_block_is_written_but_hidden() {
        let mut recipe = row(1, 1);
        assert_eq!(recipe.marker(), Marker::Hidden);
        assert_eq!(recipe.marker().text(), None);
        recipe.intermediate = block_name("minecraft:furnace");
        assert_eq!(recipe.marker(), Marker::Block(item("minecraft:furnace")));
        assert!(!recipe.marker().is_visible(), "setVisible(true) is commented out");
        assert_eq!(recipe.marker().text().as_deref(), Some("minecraft:furnace"));
        recipe.tool = ToolKind::Pickaxe;
        assert!(recipe.marker().is_visible(), "a tool wins over the block");
        assert_eq!(recipe.marker().text().as_deref(), Some("pickaxe"));
        // Air is not an intermediate block at all (:215).
        assert_eq!(block_name("minecraft:air"), None);
        assert_eq!(block_name("minecraft:furnace"), Some(item("minecraft:furnace")));
    }

    #[test]
    fn a_crafter_recipe_fills_a_row_and_a_recipe_without_result_does_not() {
        let json = mod_api::serde_json::from_str(
            r#"{"type":"recipe","crafter":"baker_craft","inputs":[{"item":"minecraft:wheat","count":3}],
                "result":"minecraft:bread","intermediate":"minecraft:furnace","tool":"pickaxe"}"#,
        )
        .expect("json");
        let parsed = CrafterRecipe::parse(&json).expect("a valid crafter recipe");
        let row = Row::from_crafter(&parsed, false).expect("a result");
        assert_eq!(row.inputs, vec![(item("minecraft:wheat"), 3)]);
        assert_eq!(row.outputs, vec![(item("minecraft:bread"), 1)]);
        assert_eq!(row.intermediate, Some(item("minecraft:furnace")));
        assert_eq!(row.tool, ToolKind::Pickaxe);
        assert!(!row.builtin);
        assert!(!row.disabled);
    }

    #[test]
    fn a_builtin_recipe_needs_control_and_a_taught_one_does_not() {
        let mut recipe = row(1, 1);
        recipe.builtin = true;
        let basket = Basket::new(vec![recipe], 1, 5, true);
        assert_eq!(
            basket.remove_visible(0, false),
            Err(Refusal::BuiltinRecipeNeedsControl)
        );
        assert_eq!(basket.remove_visible(0, true), Ok(()));
        // And with no crafting type taught, the button is off before anything else.
        let forbidden = Basket::new(vec![row(1, 1)], 1, 5, false);
        assert_eq!(
            forbidden.remove_visible(0, true),
            Err(Refusal::RecipeAlterationForbidden),
            "Java checks the alteration right first (:185)"
        );
        // The row is read before either right: Java fetches it at :177 and would throw.
        assert_eq!(
            forbidden.remove_visible(9, true),
            Err(Refusal::NoSuchRow { row: 9, rows: 1 })
        );
    }

    #[test]
    fn removing_a_taught_recipe_lowers_the_count_and_a_builtin_does_not() {
        let mut basket = Basket::new(vec![row(1, 1), row(1, 1)], 2, 5, true);
        let removed = basket.remove(0, false).expect("a taught recipe goes");
        assert!(!removed.builtin);
        assert_eq!(basket.active(), 1);
        assert_eq!(basket.len(), 1);
        // A disabled recipe does not count, so removing it changes nothing.
        let mut disabled = row(1, 1);
        disabled.disabled = true;
        let mut basket = Basket::new(vec![disabled], 4, 5, true);
        basket.remove(0, false).expect("a disabled recipe");
        assert_eq!(basket.active(), 4);
        // A builtin never counted either.
        let mut builtin = row(1, 1);
        builtin.builtin = true;
        let mut basket = Basket::new(vec![builtin], 2, 5, true);
        basket.remove(0, true).expect("control removes it");
        assert_eq!(basket.active(), 2);
    }

    #[test]
    fn toggling_moves_the_count_for_taught_recipes_only_and_hides_the_button_at_the_ceiling() {
        let mut basket = Basket::new(vec![row(1, 1), row(1, 1)], 2, 2, true);
        assert!(basket.toggle_visible(0).expect("row"));
        assert_eq!(basket.toggle(0), Ok(true), "the recipe is now disabled");
        assert_eq!(basket.active(), 1);
        assert_eq!(basket.toggle_label(0), Ok(ENABLE_KEY));
        assert!(basket.toggle_visible(0).expect("row"), "there is room again");
        assert_eq!(basket.toggle(0), Ok(false));
        assert_eq!(basket.active(), 2);
        assert_eq!(basket.toggle_label(0), Ok(DISABLE_KEY));
        assert_eq!(basket.toggle(1), Ok(true));
        // One active left, ceiling two: still room, so the button is drawn.
        assert!(basket.toggle_visible(1).expect("row"));
        assert_eq!(basket.toggle(0), Ok(true));
        assert_eq!(basket.active(), 0);
        assert!(basket.toggle_visible(0).expect("row"));
        assert_eq!(basket.status(), Status { key: STATUS_KEY, active: 0, max: 2 });
        assert_eq!(basket.status().render(), "0 of 2");
        assert!(basket.status_visible());
        // With no crafting type taught, the counter is hidden like the crafting button.
        assert!(!Basket::new(Vec::new(), 0, 0, false).status_visible());
        assert_eq!(
            basket.toggle(9),
            Err(Refusal::NoSuchRow { row: 9, rows: 2 })
        );
    }

    #[test]
    fn a_builtin_recipe_keeps_its_toggle_button_at_the_ceiling() {
        let mut builtin = row(1, 1);
        builtin.builtin = true;
        builtin.disabled = true;
        let basket = Basket::new(vec![builtin], 5, 5, true);
        assert!(
            basket.toggle_visible(0).expect("row"),
            ":225 — a builtin is always toggleable"
        );
        let mut full = row(1, 1);
        full.disabled = true;
        let blocked = Basket::new(vec![full], 5, 5, true);
        assert!(!blocked.toggle_visible(0).expect("row"));
        // The key exists and is never drawn by this window.
        assert_eq!(FULL_KEY, "com.minecolonies.coremod.gui.recipe.full");
        assert_ne!(FULL_KEY, super::super::refusal::BUILTIN_KEY);
    }

    #[test]
    fn shift_sends_the_recipe_to_the_front_or_the_back_and_a_plain_click_swaps() {
        let rows = || vec![row(1, 1), row(2, 1), row(3, 1)];
        let mut basket = Basket::new(rows(), 3, 5, true);
        assert_eq!(basket.move_row(2, Direction::Up, false), Ok(1));
        assert_eq!(basket.len(), 3);
        assert_eq!(basket.rows()[1].inputs.len(), 3, "the row really moved");
        // Shift up sends it first, not one step up.
        let mut basket = Basket::new(rows(), 3, 5, true);
        assert_eq!(basket.move_row(2, Direction::Up, true), Ok(0));
        assert_eq!(basket.rows()[0].inputs.len(), 3);
        // Shift down sends it last.
        let mut basket = Basket::new(rows(), 3, 5, true);
        assert_eq!(basket.move_row(0, Direction::Down, true), Ok(2));
        assert_eq!(basket.rows()[2].inputs.len(), 1);
        // The two ends refuse and leave the order alone.
        let mut basket = Basket::new(rows(), 3, 5, true);
        assert_eq!(
            basket.move_row(0, Direction::Up, false),
            Err(Refusal::EdgeOfList { row: 0, len: 3 })
        );
        assert_eq!(
            basket.move_row(2, Direction::Down, true),
            Err(Refusal::EdgeOfList { row: 2, len: 3 })
        );
        assert_eq!(basket.rows()[0].inputs.len(), 1);
        assert_eq!(
            basket.move_row(7, Direction::Up, false),
            Err(Refusal::NoSuchRow { row: 7, rows: 3 })
        );
    }
}
