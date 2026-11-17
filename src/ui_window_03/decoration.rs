//! `WindowDecorationController` : le contrôleur d'une décoration — un nom, deux boutons,
//! et quatre conditions pour qu'un bouton apparaisse.
//!
//! Java reference: `core/client/gui/WindowDecorationController.java` (179 lignes),
//! `assets/minecolonies/gui/windowdecorationcontroller.xml:1-17`,
//! `api/util/Utils.java:199-219` (`getBlueprintLevel`), et l'ouvert par
//! `BlockDecorationController.java:167`.
//!
//! **Ce n'est pas une hutte.** La fenêtre ne lit aucun `IBuildingView` : elle prend le
//! `TileEntityDecorationController` posé dans le monde (`:53`) et n'en lit que quatre
//! choses — le chemin du blueprint, le nom du pack, la rotation et le miroir (`:58`,
//! `:140-156`). Le port n'a pas cette entité, donc [`State`] porte le chemin et les deux
//! **verdicts** que la lecture du fichier produit ; la lecture elle-même est
//! `crate::ui_window_02::browser` (même question, autre fenêtre) et `MANQUANT.md` §2 dit
//! pourquoi elle n'est pas refaite ici.
//!
//! # Le niveau n'est pas un champ, il est dans le texte
//!
//! Un lecteur qui cherche « le niveau affiché » dans cette fenêtre ne le trouvera pas :
//! `LABEL_NAME` reçoit le **chemin**, découpé en lignes (`:58-59`), et le niveau n'y
//! survit que comme dernier caractère du nom de fichier — `builder1.blueprint` s'affiche
//! `builder1`. C'est aussi de là que `build` tire le niveau suivant (`:139-141`), avec
//! `getBlueprintLevel` qui lit **un caractère** et non un nombre : `builder10.blueprint`
//! a le niveau `0` (`Utils.java:201-209`) et son « niveau suivant » est
//! `builder11.blueprint`. Le port reproduit la limite au lieu de la corriger en silence :
//! un blueprint à deux chiffres qui change de numéro serait un autre comportement que
//! celui du jeu.
//!
//! # `Build` et `Repair` : le clic ne suit pas le libellé
//!
//! Avec un ordre de travail en cours, `build` s'appelle « annuler » (`:76-80`) — mais son
//! handler ne teste rien : il rouvre `WindowBuildDecoration` sur le **niveau suivant**
//! (`:137-157`). Le libellé change, le clic non. Et comme seul `REPAIR` est distingué
//! (`:77`), une déconstruction (`WorkOrderType.REMOVE`) affiche « Cancel Build ».

use crate::ui::field::{Field, Source, Value};

use super::refusal::Refusal;

/// La classe Java.
pub const JAVA: &str = "WindowDecorationController";
/// Le XML, chargé à `:52`.
pub const RESOURCE: &str = "minecolonies:gui/windowdecorationcontroller.xml";

/// Le panneau du nom (`WindowConstants.java:1090`, écrit à `:58-59`).
pub const NAME_LABEL: &str = "name";
/// Le bouton de construction (`WindowConstants.java:555`).
pub const BUILD_BUTTON: &str = "build";
/// Le bouton de réparation (`WindowConstants.java:556`).
pub const REPAIR_BUTTON: &str = "repair";
/// La croix, qui n'a même pas de libellé dans l'XML (`windowdecorationcontroller.xml:8-9`).
pub const CANCEL_BUTTON: &str = "cancel";

/// Le libellé de `build` quand rien n'est en cours (`TranslationConstants.java:474`).
pub const UPGRADE_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.upgrade";
/// Le libellé de `build` sur un ordre de construction ou de démolition
/// (`TranslationConstants.java:462`, posé à `:76`).
pub const CANCEL_BUILD_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.cancelbuild";
/// Le libellé de `build` sur un ordre de réparation (`TranslationConstants.java:466`, `:79`).
pub const CANCEL_REPAIR_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.cancelrepair";
/// Le libellé de `repair`, que la classe **n'écrit jamais** : il vient de l'XML
/// (`windowdecorationcontroller.xml:13`).
pub const REPAIR_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.repair";

/// La fenêtre qu'un des deux boutons ouvre (`:144`, `:165`).
pub const OPENS: &str = "WindowBuildDecoration";
/// Le message que cette fenêtre remet à la suivante, qui l'enverra (`:149`, `:170`).
pub const REQUEST_MESSAGE: &str = "DecorationBuildRequestMessage";

/// Le seul panneau que la classe remplit.
///
/// `Source::Hut` est le compound d'une **entité de bloc** ; ici c'est celui de
/// `TileEntityDecorationController`, pas d'une hutte — le port n'a qu'un type pour cette
/// idée (`crate::hut::State`), et le panneau ne lit que deux clés de l'entité.
pub const FIELDS: &[Field] = &[Field::new(NAME_LABEL, Value::Text, Source::Hut)];

/// L'ordre de travail posé sur cette position, s'il y en a un (`:69`).
///
/// Le type Java en a **quatre** (`WorkOrderType.java:7-12`) et la fenêtre n'en distingue
/// qu'un : `REPAIR` (`:77`). Les trois autres prennent le libellé « annuler la
/// construction », y compris la démolition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// Aucun ordre à cette position.
    None,
    /// `WorkOrderType.BUILD`.
    Build,
    /// `WorkOrderType.UPGRADE`.
    Upgrade,
    /// `WorkOrderType.REPAIR` — le seul que `:77` sépare.
    Repair,
    /// `WorkOrderType.REMOVE`, la déconstruction.
    Remove,
}

impl Order {
    /// Le libellé que `build` porte sur cet ordre (`:76-80`).
    pub const fn label(self) -> &'static str {
        match self {
            Order::Repair => CANCEL_REPAIR_KEY,
            // Java écrit ce libellé dès qu'il y a un ordre et que ce n'est pas REPAIR.
            Order::Build | Order::Upgrade | Order::Remove => CANCEL_BUILD_KEY,
            // Aucun ordre : ce n'est pas ce libellé qui s'affiche (`:84`).
            Order::None => UPGRADE_KEY,
        }
    }

    /// Vrai quand la fenêtre se contente d'annoncer un ordre en cours.
    pub const fn is_pending(self) -> bool {
        !matches!(self, Order::None)
    }
}

/// Ce que la lecture d'un blueprint a répondu du **bloc primaire** (`:93-97`, `:109-113`).
///
/// Trois états et non deux : `StructurePacks` peut rendre `null` sans lever (`:91`, `:107`)
/// et le `catch` peut avaler l'exception avant tout callback (`:118-121`). Dans les deux
/// cas Java n'appelle pas `show()`, donc le bouton reste caché — mais la cause n'est pas
/// la même, et un port qui confondrait « pas encore lu » et « pas le bon bloc » mentirait
/// sur ce qu'il sait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Primary {
    /// Pas encore lu, ou lecture refusée : le callback n'a rien dit.
    Unread,
    /// Le bloc primaire **est** `blockDecorationPlaceholder` : le bouton s'affiche.
    Placeholder,
    /// Le bloc primaire est autre chose : le bouton reste caché.
    Other,
}

/// Ce que la fenêtre montre de ses deux boutons : `None` = caché, `Some(clé)` = visible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Buttons {
    pub build: Option<&'static str>,
    pub repair: Option<&'static str>,
}

/// Ce qu'un clic fait : refermer, puis ouvrir quoi, sur quel chemin, avec quel ordre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Click {
    /// Vrai pour les trois boutons : `cancel` ferme (`:131`) et les deux autres ferment
    /// **avant** d'ouvrir la fenêtre suivante (`:143`, `:164`).
    pub closes: bool,
    /// `Some(`[`OPENS`]`)` pour `build` et `repair`, `None` pour `cancel`.
    pub opens: Option<&'static str>,
    /// Le chemin passé à la fenêtre suivante (`:141`, `:167`).
    pub path: String,
    /// L'ordre que le message suivant portera (`:149`, `:170`).
    pub order: Order,
}

/// Ce que la fenêtre sait du contrôleur, à un instant donné.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    /// `controller.getBlueprintPath()`, `.blueprint` compris (`:58`).
    pub path: String,
    /// L'ordre de travail trouvé à cette position (`:69-71`).
    pub order: Order,
    /// `view != null` (`:67`) : hors colonie, la fenêtre ne montre **rien**.
    pub colony: bool,
    /// Le verdict du blueprint **courant** (`:88-99`).
    pub current: Primary,
    /// Le verdict du blueprint du **niveau suivant** (`:101-116`).
    pub next: Primary,
}

impl State {
    /// Le niveau du chemin, ou `None` pour le `-1` de Java.
    ///
    /// C'est `Utils.getBlueprintLevel` (`Utils.java:199-219`), déjà porté par
    /// `crate::ui_window_02::browser::path_segment` pour le navigateur : la règle est la
    /// même, donc elle est appelée et non recopiée.
    pub fn level(&self) -> Option<i32> {
        crate::ui_window_02::browser::path_segment(&self.path).1
    }

    /// Le texte du panneau `name` (`:58-59`).
    ///
    /// Java retire l'extension, ramène les deux séparateurs à `/`, puis coupe : une ligne
    /// par dossier. L'ordre des trois `replace` compte (`\\` avant `/`).
    pub fn name_text(&self) -> String {
        self.path.replace(".blueprint", "").replace('\\', "/").replace('/', "\n")
    }

    /// L'état des deux boutons, tel que le constructeur le laisse (`:63-123`).
    ///
    /// Les deux sont cachés au départ (`:64-65`) et **rien** ne les montre hors colonie :
    /// le `if (view != null)` englobe les deux `show()` (`:67-123`).
    pub fn buttons(&self) -> Buttons {
        if !self.colony {
            return Buttons { build: None, repair: None };
        }
        if self.order.is_pending() {
            // Ordre en cours : `build` montre « annuler » et `repair` reste caché
            // (`:72-81`) — même si le clic, lui, ne teste pas l'ordre (`:137`).
            return Buttons { build: Some(self.order.label()), repair: None };
        }
        // Sans ordre : le libellé est écrit à `:84`, puis les deux `show()` dépendent de
        // la lecture du blueprint. `repair` regarde le courant, `build` le suivant.
        let repair = match self.current {
            Primary::Placeholder => Some(REPAIR_KEY),
            Primary::Unread | Primary::Other => None,
        };
        let build = match (self.level(), self.next) {
            (Some(_), Primary::Placeholder) => Some(UPGRADE_KEY),
            // `level == -1` : `:101` ne demande même pas le blueprint suivant.
            _ => None,
        };
        Buttons { build, repair }
    }

    /// Un clic, ou le refus qui explique pourquoi le bouton n'était pas là.
    pub fn press(&self, id: &str) -> Result<Click, Refusal> {
        if id == CANCEL_BUTTON {
            return Ok(Click {
                closes: true,
                opens: None,
                path: self.path.clone(),
                order: Order::None,
            });
        }
        let buttons = self.buttons();
        if id == BUILD_BUTTON {
            // `:137-157` : le niveau suivant est calculé **sans** regarder l'ordre, et sans
            // garde sur `-1` (contrairement à `:101`) — `upgrade_path` rend alors le chemin
            // inchangé, donc un build sur un blueprint non numéroté vise le même fichier.
            let level = self.level();
            if buttons.build.is_none() {
                return Err(self.build_refusal());
            }
            return Ok(Click {
                closes: true,
                opens: Some(OPENS),
                path: upgrade_path(&self.path, level),
                order: Order::Build,
            });
        }
        if id == REPAIR_BUTTON {
            if buttons.repair.is_none() {
                return Err(self.repair_refusal());
            }
            return Ok(Click {
                closes: true,
                opens: Some(OPENS),
                // `:167` : la réparation garde le chemin courant.
                path: self.path.clone(),
                order: Order::Repair,
            });
        }
        Err(Refusal::UnknownButton)
    }

    /// Pourquoi `build` est caché — dans l'ordre où Java décide.
    fn build_refusal(&self) -> Refusal {
        if !self.colony {
            return Refusal::NoColony;
        }
        if self.level().is_none() {
            return Refusal::NoLevel;
        }
        match self.next {
            Primary::Unread => Refusal::BlueprintUnreadable,
            Primary::Other | Primary::Placeholder => Refusal::NotPlaceholder,
        }
    }

    /// Pourquoi `repair` est caché. Un ordre en cours suffit, et c'est le cas courant.
    fn repair_refusal(&self) -> Refusal {
        if !self.colony {
            return Refusal::NoColony;
        }
        if self.order.is_pending() {
            return Refusal::ButtonOff;
        }
        match self.current {
            Primary::Unread => Refusal::BlueprintUnreadable,
            Primary::Other | Primary::Placeholder => Refusal::NotPlaceholder,
        }
    }
}

/// Le chemin du niveau suivant, par **remplacement de texte** (`:141`).
///
/// Java écrit `path.replace(level + ".blueprint", (level + 1) + ".blueprint")`. Trois
/// conséquences que le port garde : le niveau est un caractère (`Utils.java:201`), donc
/// `builder10.blueprint` de niveau `0` donne `builder11.blueprint` ; un niveau inconnu
/// (`-1`) ne correspond à rien et rend le chemin **inchangé** ; et `level == 9` donne
/// `10.blueprint`, ce qui reste juste.
pub fn upgrade_path(path: &str, level: Option<i32>) -> String {
    let level = match level {
        Some(level) => level,
        None => return path.to_owned(),
    };
    path.replace(&format!("{level}.blueprint"), &format!("{}.blueprint", level + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controller(path: &str) -> State {
        State {
            path: path.to_owned(),
            order: Order::None,
            colony: true,
            current: Primary::Placeholder,
            next: Primary::Placeholder,
        }
    }

    #[test]
    fn the_name_prints_one_line_per_path_segment() {
        // :58-59 : extension retirée, deux séparateurs, puis coupure.
        assert_eq!(
            controller("castle/gate/builder1.blueprint").name_text(),
            "castle\ngate\nbuilder1"
        );
        assert_eq!(controller("b1.blueprint").name_text(), "b1");
        // Un chemin Windows passe par la même moulinette.
        assert_eq!(controller("castle\\b1.blueprint").name_text(), "castle\nb1");
        // Le niveau n'est pas un panneau : il n'existe que dans ce texte.
        assert!(controller("castle/builder10.blueprint").name_text().ends_with("builder10"));
    }

    #[test]
    fn the_level_is_the_last_character_and_not_the_number() {
        // Utils.java:201-209, via le portage déjà fait pour le navigateur.
        assert_eq!(controller("castle/builder1.blueprint").level(), Some(1));
        assert_eq!(controller("castle/builder10.blueprint").level(), Some(0));
        assert_eq!(controller("castle/castle.blueprint").level(), None);
    }

    #[test]
    fn the_next_path_is_a_text_replacement_and_keeps_the_one_digit_quirk() {
        assert_eq!(
            upgrade_path("castle/builder1.blueprint", Some(1)),
            "castle/builder2.blueprint"
        );
        assert_eq!(
            upgrade_path("castle/builder10.blueprint", Some(0)),
            "castle/builder11.blueprint"
        );
        // Le seul cas où le remplacement ne mord pas : pas de niveau du tout.
        assert_eq!(upgrade_path("castle/castle.blueprint", None), "castle/castle.blueprint");
        // Un `-1` non gardé, comme `:141` qui ne teste pas `:101`.
        assert_eq!(upgrade_path("castle/castle.blueprint", Some(-1)), "castle/castle.blueprint");
    }

    #[test]
    fn a_pending_order_shows_build_with_a_cancel_label_and_hides_repair() {
        for order in [Order::Build, Order::Upgrade, Order::Repair, Order::Remove] {
            let mut state = controller("castle/builder1.blueprint");
            state.order = order;
            let buttons = state.buttons();
            assert_eq!(buttons.build, Some(order.label()), "{order:?}");
            // :74-81 : seul `build` est montré dans cette branche.
            assert_eq!(buttons.repair, None, "{order:?}");
        }
        // Seul REPAIR a un libellé différent — la démolition affiche « annuler la construction ».
        assert_eq!(Order::Repair.label(), CANCEL_REPAIR_KEY);
        assert_eq!(Order::Remove.label(), CANCEL_BUILD_KEY);
        assert_eq!(Order::None.label(), UPGRADE_KEY);
    }

    #[test]
    fn without_an_order_each_button_follows_its_own_blueprint() {
        // :96 — `repair` suit le blueprint courant ; :112 — `build` suit le suivant.
        let mut state = controller("castle/builder1.blueprint");
        assert_eq!(state.buttons(), Buttons { build: Some(UPGRADE_KEY), repair: Some(REPAIR_KEY) });

        state.current = Primary::Other;
        assert_eq!(state.buttons(), Buttons { build: Some(UPGRADE_KEY), repair: None });

        state.next = Primary::Other;
        assert_eq!(state.buttons(), Buttons { build: None, repair: None });

        // Pas de niveau : `build` n'est jamais montré, même sur un placeholder (`:101`).
        let nameless = controller("castle/castle.blueprint");
        assert_eq!(nameless.level(), None);
        assert_eq!(nameless.buttons().build, None);
        assert_eq!(nameless.buttons().repair, Some(REPAIR_KEY));
    }

    #[test]
    fn no_colony_or_an_unread_blueprint_leaves_the_two_buttons_hidden() {
        let mut state = controller("castle/builder1.blueprint");
        state.colony = false;
        assert_eq!(state.buttons(), Buttons { build: None, repair: None });
        assert_eq!(state.press(BUILD_BUTTON), Err(Refusal::NoColony));
        assert_eq!(state.press(REPAIR_BUTTON), Err(Refusal::NoColony));

        // :118-121 : le pack lève, le catch avale, aucun callback n'appelle show().
        let mut unread = controller("castle/builder1.blueprint");
        unread.current = Primary::Unread;
        unread.next = Primary::Unread;
        assert_eq!(unread.buttons(), Buttons { build: None, repair: None });
        assert_eq!(unread.press(BUILD_BUTTON), Err(Refusal::BlueprintUnreadable));
        assert_eq!(unread.press(REPAIR_BUTTON), Err(Refusal::BlueprintUnreadable));
    }

    #[test]
    fn the_two_buttons_open_the_build_window_and_never_test_the_order() {
        // `cancel` ferme sans rien ouvrir (:129-132).
        let state = controller("castle/builder1.blueprint");
        assert_eq!(
            state.press(CANCEL_BUTTON),
            Ok(Click {
                closes: true,
                opens: None,
                path: "castle/builder1.blueprint".to_owned(),
                order: Order::None,
            })
        );

        // `build` sur un ordre en cours : libellé « annuler », clic « construire »
        // (`:74-80` puis `:137-157`).
        let mut pending = controller("castle/builder1.blueprint");
        pending.order = Order::Remove;
        assert_eq!(pending.buttons().build, Some(CANCEL_BUILD_KEY));
        assert_eq!(
            pending.press(BUILD_BUTTON),
            Ok(Click {
                closes: true,
                opens: Some(OPENS),
                path: "castle/builder2.blueprint".to_owned(),
                order: Order::Build,
            })
        );

        // `repair` garde le chemin courant (:167) et porte REPAIR (:170).
        assert_eq!(
            state.press(REPAIR_BUTTON),
            Ok(Click {
                closes: true,
                opens: Some(OPENS),
                path: "castle/builder1.blueprint".to_owned(),
                order: Order::Repair,
            })
        );

        // Un id que la fenêtre n'enregistre pas.
        assert_eq!(state.press("spies"), Err(Refusal::UnknownButton));
    }
}
