//! Une cellule de blueprint : qui la pose, ce qu'il écrit, et ce qu'il refuse.
//!
//! Référence Java : `com.minecolonies.core.placementhandlers` — 19 classes
//! `IPlacementHandler`, l'initialiseur qui les enregistre, une classe qui n'est **jamais**
//! enregistrée (`SolidPlaceholderPlacementHandler`), et les deux handlers de `main/`.
//! La sélection, elle, n'appartient pas à MineColonies : c'est
//! `com.ldtteam.structurize.placement.handlers.placement.PlacementHandlers`, la liste dans
//! laquelle l'initialiseur s'insère.
//!
//! # La question, en une phrase
//!
//! Pour une cellule — un état de bloc et l'entité de bloc qui le suit — [`Table::java`]
//! répond **quel handler la possède**, [`decide`] répond **ce que ce handler en fait**, et
//! [`Verdict`] nomme la réponse : les cellules qu'il écrit (avec la donnée qui suit
//! chacune), celle qu'il vide, la raison pour laquelle il n'écrit rien, ou le refus.
//!
//! # L'ordre, qui est toute la règle
//!
//! Java n'a ni priorité ni score : `PlacementHandlers.getHandler`
//! (`PlacementHandlers.java:162-184`) parcourt **une** liste et prend le premier handler
//! dont `canHandle` répond `true` ; le handler général accepte tout et ferme la liste.
//! L'ordre de la liste *est* la règle de sélection, et il se construit en deux temps qu'il
//! est facile d'inverser :
//!
//! 1. le bloc `static` de structurize ajoute 22 lignes dans son ordre à lui (`:55-79`) ;
//! 2. `PlacementHandlerInitializer.initHandlers()` (`PlacementHandlerInitializer.java:20-41`)
//!    appelle `PlacementHandlers.add(handler)` pour 18 de ses 19 classes, et
//!    `add(handler, override)` pour la classe générale. Or `add(handler)` **insère à
//!    l'index 3** (`:141-148`) : ces 18 lignes se retrouvent dans l'ordre **inverse** de
//!    leur enregistrement, juste après l'air et les deux placeholders de substitution, et
//!    avant tout ce que structurize possède.
//!
//! [`Table::java`] n'écrit donc **pas** l'ordre résultat : elle rejoue les deux étapes
//! ([`table::insert_at_three`], [`table::replace`]) et l'ordre en sort. Un ordre tapé à la
//! main serait juste le jour où on le tape et faux à la première ligne déplacée.
//!
//! # Ce que l'ordre mesuré fait vraiment (quatre surprises, pas des suppositions)
//!
//! - **`BarracksTowerHandler` ne s'exécute jamais.** La tour de caserne est une
//!   `AbstractBlockHut` (`api/blocks/ModBlocks.java:41`) et `HutPlacementHandler` la
//!   réclame en position 5, alors que `BarracksTowerHandler` est en 15
//!   (`BarracksTowerHandler.java:33`). La preuve que Java le sait est dans l'autre handler :
//!   `HutPlacementHandler.getRequiredItems` traite explicitement le cas
//!   `blockHutBarracksTower` (`HutPlacementHandler.java:125-128`).
//! - **Les deux lignes DO de structurize sont mortes aussi** : `DoBlockPlacementHandler`
//!   de MineColonies (position 17) réclame tout `IMateriallyTexturedBlock` sauf le rack, et
//!   arrive avant les leurs (36 et 37).
//! - **`SolidPlaceholderPlacementHandler` n'est jamais enregistré** :
//!   `initHandlers()` liste 19 classes et n'inclut pas celle-là
//!   (`PlacementHandlerInitializer.java:20-41`) ; c'est le placeholder de substitution de
//!   *structurize* qui occupe l'index 1.
//! - **`JigsawPlacementHandler` écrit de l'air quand il ne comprend pas son
//!   `final_state`** (`JigsawPlacementHandler.java:70-91` : `finalState` part à l'air, un
//!   `catch` se contente d'un `warn`, puis `setBlockState(finalState)`). Le port refuse
//!   ([`Refusal::BadFinalState`]) au lieu d'effacer la cellule.
//!
//! # Où le handler écrit, et où l'écriture est refusée
//!
//! Le port coupe en deux, à l'endroit où Java n'a qu'un appel :
//!
//! | question | qui répond |
//! | --- | --- |
//! | quel handler, quel état, quelle donnée suit | ce module ([`decide`]) |
//! | la cellule est-elle libre, chargée, protégée ? | [`crate::colony_world`] |
//! | la donnée d'un conteneur survit-elle ? | [`Cell::change`] → `Change::Keep` |
//!
//! [`Cell::change`] ne devine rien : elle traduit un [`Cell`] en
//! [`Change`](crate::colony_world::intent::Change) d'après
//! [`Occupant`](crate::colony_world::guard::Occupant), et c'est
//! [`Intent::plan`](crate::colony_world::intent::Intent::plan) qui refuse — `Occupied` sur
//! un bloc, `Empty` sur un vide, `WouldDropContents` sur un conteneur. Un écrasement
//! silencieux n'est donc pas représentable : la seule façon d'écrire passe par un `Intent`,
//! et un `Intent` n'existe que si les six questions ont répondu oui.
//!
//! # Ce que ce module ne porte pas, et pourquoi
//!
//! - **`getRequiredItems`** (un tiers de chaque handler Java) : c'est le coût d'une
//!   structure, et `crate::building::cost` le tient déjà. Les listes Java ne servent qu'à
//!   l'alimenter.
//! - **les 22 lignes de structurize** : elles sont nommées pour que l'ordre soit complet et
//!   répondent [`Verdict::Delegated`]. Les porter est un autre travail ; `MANQUANT.md` dit
//!   ce que chacune coûte.
//! - **l'état dérivé du voisinage** : `GeneralBlockPlacementHandler.java:52-68` et
//!   `DoBlockPlacementHandler.java:64-83` réclament un `getStateForPlacement` sur les murs,
//!   barrières, barreaux et piliers pour que la clôture collée à une clôture se connecte.
//!   `edit_blocks` écrit un état complet et ne porte aucun drapeau
//!   (`colony_world::engine::UPDATE_FLAG`), donc le port écrit l'état du blueprint tel quel
//!   et le dit dans `MANQUANT.md` §1.

use crate::blueprint::{BlockEntity, BlockState};
use crate::colony_world::guard::Occupant;
use crate::colony_world::intent::Change;
use crate::structures::Placing;

pub mod handlers;
pub mod kind;
pub mod survival;
pub mod table;

pub use handlers::{decide, parse_state};
pub use survival::{Admission, Gate, Supply};
pub use table::{Handler, Owner, Row, Rule, Table};

/// La question de Java que les handlers lisent le plus : `fancyPlacement()`.
///
/// Ce n'est pas un détail de confort, c'est la différence entre **copier un blueprint** et
/// **le construire**. L'argument `complete` de `getExpectedBlockState` vaut
/// `!fancyPlacement()` (`InfestedBlocksPlacementHandler.java:41`,
/// `DoBlockPlacementHandler.java:129`) : en [`Mode::Sketch`] on colle ce que le fichier
/// contient, en [`Mode::Fancy`] on pose ce qu'un constructeur peut réellement dépenser.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustif : les deux valeurs de fancyPlacement(), rien d'autre
pub enum Mode {
    /// `fancyPlacement() == false` — le scan, l'aperçu : la cellule reçoit l'état du
    /// blueprint, placeholder compris.
    Sketch,
    /// `fancyPlacement() == true` — l'outil de construction : les matériaux peuvent être
    /// substitués (l'herbe devient de la terre, l'infesté devient de la pierre, un bloc DO
    /// prend son item générique).
    #[default]
    Fancy,
}

impl Mode {
    /// L'argument `complete` des handlers Java, qui est `!fancyPlacement()`.
    pub fn complete(self) -> bool {
        self == Mode::Sketch
    }
}

/// Ce que le monde tient à la cellule visée.
///
/// Les trois réponses que les handlers lisent, et pas une de plus : Java lit
/// `world.getBlockState(pos)` (l'état), `world.getBlockEntity(pos)` (la donnée) et
/// `canBeReplaced()` (l'occupant). Un appelant qui n'a rien lu laisse les deux premiers à
/// `None` — et les handlers répondent alors comme Java répondrait devant de l'air.
#[derive(Clone, Copy, Debug)]
pub struct Here<'a> {
    /// La cellule est-elle libre, pleine, chargée, ou un conteneur
    /// (`crate::colony_world::guard::Occupant`).
    pub occupant: Occupant,
    /// L'état en place, quand l'appelant a pu le lire (`level_block_state`).
    pub state: Option<&'a BlockState>,
    /// L'entité de bloc en place, quand il y en a une et qu'elle a été lue.
    pub data: Option<&'a BlockEntity>,
}

impl<'a> Here<'a> {
    /// Rien n'a été lu : la cellule est traitée comme remplaçable et vide.
    pub fn open() -> Here<'a> {
        Here {
            occupant: Occupant::Replaceable,
            state: None,
            data: None,
        }
    }
}

/// Une cellule à poser, telle que le handler la décide.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    /// La cellule du monde.
    pub at: [i32; 3],
    /// L'état à écrire.
    pub state: BlockState,
    /// La donnée du blueprint qui suit ce bloc, ou [`Follow::None`].
    pub follow: Follow,
}

impl Cell {
    /// La modification à demander au moteur pour cette cellule, d'après ce qui s'y trouve.
    ///
    /// | occupant | `follow` | `Change` | pourquoi |
    /// | --- | --- | --- | --- |
    /// | remplaçable | — | `Place` | `BlockState.canBeReplaced()` (`colony_world::guard::Occupant`) |
    /// | plein | — | `Replace` | le bloc en place est perdu, et `plan` le sait |
    /// | conteneur | `Data`, même bloc | `Keep` | Java réécrit le bloc pour reposer sa donnée (`RackPlacementHandler.java:42-46`) : rien à écrire |
    /// | conteneur | autre | `Replace` | `plan` refuse alors `WouldDropContents` |
    fn change(&self, here: &Here<'_>) -> Change {
        let block = self.state.to_string();
        match here.occupant {
            Occupant::Replaceable | Occupant::NotLoaded => Change::Place {
                at: self.at,
                block,
            },
            Occupant::Solid => Change::Replace {
                at: self.at,
                block,
            },
            Occupant::Container
                if self.follow == Follow::Data
                    && here.state.is_some_and(|world| world.name == self.state.name) =>
            {
                Change::Keep { at: self.at }
            }
            Occupant::Container => Change::Replace {
                at: self.at,
                block,
            },
        }
    }
}

/// La donnée de bloc qui suit un bloc posé.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : la donnée suit, ou elle ne suit pas
pub enum Follow {
    /// Le blueprint ne porte aucune entité de bloc pour cette cellule.
    None,
    /// La donnée du blueprint s'écrit dans l'entité de bloc de la cellule
    /// (`PlacementHandlers.java:1344-1350`, `handleTileEntityPlacement`), tournée et
    /// mirroitée par la pose.
    Data,
}

impl Follow {
    /// `Data` dès que le blueprint porte une entité de bloc pour la cellule.
    pub fn of(data: Option<&BlockEntity>) -> Follow {
        if data.is_some() {
            Follow::Data
        } else {
            Follow::None
        }
    }
}

/// Ce qu'un handler répond pour une cellule — `ActionProcessingResult` et ce qu'il cache.
///
/// Java n'a que trois valeurs (`IPlacementHandler.ActionProcessingResult` : `SUCCESS`,
/// `PASS`, `DENY`) et s'en sert pour six situations différentes : écrire, écrire deux
/// cellules, vider, ne rien faire exprès, laisser passer, refuser. Le port les sépare,
/// parce que « `SUCCESS` sans écriture » est justement le cas qu'un relecteur ne voit pas.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustif : les six issues qu'un handler Java peut avoir
pub enum Verdict {
    /// Le handler pose : une cellule, ou les deux d'un bloc double (basse d'abord).
    Writes(Vec<Cell>),
    /// La cellule finit vide (`AirPlacementHandler`, et le waypoint de
    /// `WayPointBlockPlacementHandler.java:40`).
    Clears,
    /// Le handler a réclamé le bloc et n'a rien posé, pour la raison nommée.
    Spares(Why),
    /// Java `PASS` : la cellule tient déjà ce que le blueprint demande, ou ce n'est pas
    /// l'affaire de ce handler.
    Passes,
    /// Java `DENY`, ou un refus que le port ajoute là où Java se taît.
    Refuses(Refusal),
    /// La ligne appartient à structurize : ce module ne la décide pas.
    Delegated(Handler),
}

/// Pourquoi un handler réclame une cellule sans y écrire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les cinq raisons mesurées dans le Java
pub enum Why {
    /// Posé seulement en [`Mode::Sketch`] : un bloc de structure, une tombe nommée
    /// (`BuilderIgnorePlacementHandler.java:44-62`, `NamedGravePlacementHandler.java:36-42`).
    SketchOnly,
    /// Le `final_state` d'un jigsaw est `structure_void` (`JigsawPlacementHandler.java:86-89`).
    VoidState,
    /// Un jigsaw sans `final_state` : Java ne pose rien (`:66`).
    NoFinalState,
    /// La moitié haute d'un bloc double, dont la moitié basse a déjà écrit les deux
    /// (`DoDoorBlockPlacementHandler.java:50`, `FieldPlacementHandler.java:42`).
    AlreadyPaired,
    /// Le monde tient déjà la réponse attendue et Java la garde
    /// (`GravePlacementHandler.java:43-46` : une tombe debout n'est jamais réécrite).
    Kept,
}

impl Why {
    /// Le nom de la raison, préfixé, pour une ligne de journal.
    pub fn name(self) -> &'static str {
        match self {
            Why::SketchOnly => "placement.why.sketch_only",
            Why::VoidState => "placement.why.void_state",
            Why::NoFinalState => "placement.why.no_final_state",
            Why::AlreadyPaired => "placement.why.already_paired",
            Why::Kept => "placement.why.kept",
        }
    }
}

/// Pourquoi ce module n'a pas de réponse.
///
/// **Les refus d'écriture ne sont pas ici.** « La place est occupée », « le sol manque »,
/// « la donnée d'un conteneur serait perdue » sont les refus de
/// [`crate::colony_world::refusal`], posés par `Intent::plan` après les six questions. Un
/// second jeu de noms pour les mêmes situations rendrait deux lignes de journal
/// incomparables.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustif : les quatre refus propres au placement
pub enum Refusal {
    /// Aucune ligne de la table n'a accepté l'état. `Table::java` finit par une ligne qui
    /// accepte tout, donc ce refus ne peut venir que d'une table construite sans elle ;
    /// Java, lui, journalise et retombe sur le handler général
    /// (`PlacementHandlers.java:182-183`).
    NoHandler { state: String },
    /// Une hutte sans blueprint : `getBluePrint() == null`
    /// (`HutPlacementHandler.java:53-56`, `DENY`).
    NoBlueprint { at: [i32; 3] },
    /// Le `final_state` d'un jigsaw ne se lit pas comme un état de bloc. Java écrit de
    /// l'air dans ce cas (`JigsawPlacementHandler.java:70-91`) ; le port refuse.
    BadFinalState { state: String },
    /// Une propriété que le handler lit et que l'état ne porte pas — Java lèverait une
    /// exception au milieu du collage (`DoDoorBlockPlacementHandler.java:50`).
    MissingProperty {
        state: String,
        property: &'static str,
    },
}

impl Refusal {
    /// Le nom du refus, préfixé pour qu'on ne le confonde pas avec une phrase.
    pub fn name(&self) -> &'static str {
        match self {
            Refusal::NoHandler { .. } => "placement.refusal.no_handler",
            Refusal::NoBlueprint { .. } => "placement.refusal.no_blueprint",
            Refusal::BadFinalState { .. } => "placement.refusal.bad_final_state",
            Refusal::MissingProperty { .. } => "placement.refusal.missing_property",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// Tout ce qu'un handler Java peut lire, en données.
///
/// Il n'y a **aucun appel au monde** ici : les six choses que les 19 handlers demandent à
/// leur contexte sont toutes des valeurs — la cellule, l'état, la donnée qui suit, le mode,
/// ce qui est en place, la pose de la structure et l'existence d'une colonie.
#[derive(Clone, Copy, Debug)]
pub struct Request<'a> {
    /// La cellule du monde.
    pub at: [i32; 3],
    /// L'état que le blueprint demande.
    pub state: &'a BlockState,
    /// L'entité de bloc que le blueprint stocke pour cette cellule, s'il en stocke une
    /// ([`crate::blueprint::Blueprint::block_entity_at`]).
    pub data: Option<&'a BlockEntity>,
    /// `fancyPlacement()`.
    pub mode: Mode,
    /// Ce qui est en place.
    pub here: Here<'a>,
    /// La pose de la structure entière, quand le placement vient d'un blueprint. `None`
    /// est le `getBluePrint() == null` de `HutPlacementHandler.java:53`.
    pub placing: Option<&'a Placing>,
    /// Une colonie touche-t-elle la cellule ? `IColonyManager.getClosestColony` en répond un
    /// (`WayPointBlockPlacementHandler.java:41`) ; [`crate::manager`] tient la vraie
    /// réponse, ce champ n'en est que le résultat.
    pub in_colony: bool,
}

impl<'a> Request<'a> {
    /// Une requête nue : le mode de l'outil de construction, rien de lu, aucune colonie.
    pub fn new(at: [i32; 3], state: &'a BlockState) -> Request<'a> {
        Request {
            at,
            state,
            data: None,
            mode: Mode::Fancy,
            here: Here::open(),
            placing: None,
            in_colony: false,
        }
    }

    /// Le même état, copié comme un scan le fait.
    pub fn sketch(mut self) -> Self {
        self.mode = Mode::Sketch;
        self
    }

    /// La donnée qui suit le bloc.
    pub fn with_data(mut self, data: &'a BlockEntity) -> Self {
        self.data = Some(data);
        self
    }

    /// Ce qui est en place.
    pub fn with_here(mut self, here: Here<'a>) -> Self {
        self.here = here;
        self
    }

    /// L'état en place, et sa nature.
    pub fn with_world(mut self, state: &'a BlockState, occupant: Occupant) -> Self {
        self.here = Here {
            occupant,
            state: Some(state),
            data: None,
        };
        self
    }

    /// Une colonie touche la cellule.
    pub fn in_colony(mut self) -> Self {
        self.in_colony = true;
        self
    }

    /// La pose de la structure.
    pub fn with_placing(mut self, placing: &'a Placing) -> Self {
        self.placing = Some(placing);
        self
    }
}

#[cfg(test)]
pub(crate) mod fakes {
    //! Les états et les entités de bloc dont les tests de ce module ont besoin.

    use std::collections::BTreeMap;

    use mod_api::{Compound, Tag};

    use crate::blueprint::{BlockEntity, BlockState, Pos};

    /// Un état sans propriété.
    pub fn state(id: &str) -> BlockState {
        BlockState {
            name: id.to_owned(),
            properties: BTreeMap::new(),
        }
    }

    /// Un état avec ses propriétés.
    pub fn state_of(id: &str, properties: &[(&str, &str)]) -> BlockState {
        BlockState {
            name: id.to_owned(),
            properties: properties
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        }
    }

    /// Une entité de bloc portant un `final_state`, comme un jigsaw en sauve.
    pub fn jigsaw(final_state: Option<&str>) -> BlockEntity {
        let mut data = Compound::new();
        if let Some(text) = final_state {
            data.insert("final_state".into(), Tag::String(text.into()));
        }
        BlockEntity {
            pos: Pos::new(0, 0, 0),
            id: "minecraft:jigsaw".to_owned(),
            data,
        }
    }

    /// Une entité de bloc nue.
    pub fn entity(id: &str) -> BlockEntity {
        BlockEntity {
            pos: Pos::new(0, 0, 0),
            id: id.to_owned(),
            data: Compound::new(),
        }
    }
}
