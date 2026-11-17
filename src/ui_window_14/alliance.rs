//! La page d'alliance du tribunal : trois listes, quatre boutons, cinq types
//! d'evenement, trois statuts — et **un seul message**.
//!
//! Java reference : `core/client/gui/townhall/WindowAlliancePage.java` (233 lignes, la
//! classe), son XML `gui/townhall/layoutalliance.xml` (56), et les types de
//! `api/colony/connections/` qu'elle lit : `DiplomacyStatus.java` (20),
//! `ConnectionEventType.java` (22), `ConnectionEvent.java` (44),
//! `ColonyConnection.java` (84), `IColonyConnectionManager.java` (112). Cote serveur :
//! `core/network/messages/server/colony/TriggerConnectionEventMessage.java` (77) et
//! `core/colony/managers/ColonyConnectionManager.java:722-804` (la machine d'etat).
//!
//! # Ce que cette page est, et ce qu'elle n'est pas
//!
//! `WindowAlliancePage` est une classe de fenetre (`:23`) mais son XML est un
//! **layout** : `layoutalliance.xml:2` monte `gui/townhall/windowtownhall.xml` et ne
//! dessine que la zone de droite. On y entre par l'onglet `alliances`
//! (`WindowConstants.java:37`, lie a `AbstractWindowTownHall.java:42`), et la page
//! eteint puis rallume sa propre vignette d'onglet (`AbstractWindowTownHall.java:44-47`).
//! Il n'y a donc pas de fenetre autonome a ouvrir : c'est un onglet du tribunal.
//!
//! # Le seul ecran de ce port dont un bouton change l'etat d'une autre colonie
//!
//! Les sept boutons des trois fenetres de `ui_window_13` agissent dans la fenetre ; les
//! quatre d'ici envoient `TriggerConnectionEventMessage` et le serveur ecrit dans
//! **les deux** colonies (`ColonyConnectionManager.java:758`, `:781`). C'est aussi
//! pourquoi tout refus de ces boutons est **silencieux** : la decision n'est pas dans
//! la fenetre, et le serveur abandonne sans un mot au joueur (`:50-57`, `:745-748`).
//!
//! # Quatre faits que le source impose
//!
//! 1. **Un evenement par colonie, pas un journal.** Les evenements vivent dans un
//!    `TreeMap<Integer, ConnectionEvent>` **cle par id de colonie d'origine**
//!    (`:63`, `:732`) : un second evenement de la meme colonie **remplace** le premier.
//!    Une demande d'alliance peut donc disparaitre sous une declaration de querelle, et
//!    le bouton `acceptally` avec elle.
//! 2. **La liste des evenements est inversee par id, pas par date.** La fenetre affiche
//!    `size - 1 - index` (`:217-218`) sur une `List` qui sort du `TreeMap` en ordre
//!    **croissant d'id** (`:789`). Le premier de la liste est donc la colonie de plus
//!    grand id, pas l'evenement le plus recent.
//! 3. **La page vide ne regarde pas les connexions indirectes.** `missingconnections`
//!    s'affiche quand les evenements **et** les connexions directes sont vides
//!    (`:69`) ; une colonie qui n'aurait que des indirectes verrait l'aide a la place de
//!    ses listes.
//! 4. **`DISCONNECTED` a une cle de langue et aucun producteur.** Les cinq types ont une
//!    ligne dans `manual_en_us.json:3028-3032`, mais les quatre boutons n'en produisent
//!    que quatre : chercher `DISCONNECTED` dans l'arbre Java ne rend que sa declaration
//!    (`ConnectionEventType.java:12`). Un evenement de ce type venu d'une sauvegarde
//!    s'affiche donc sans qu'aucun code ne l'ait cree.
//!
//! # Ce que le port reprend, et d'ou
//!
//! `crate::network::colony_settings::TriggerConnectionEvent` existe deja et porte
//! `event: Vec<u8>` parce que `ConnectionEvent` n'est pas porte
//! (`src/network/MANQUANT.md` §1). Ce fichier **fabrique ces octets**
//! ([`event_bytes`]) selon `ConnectionEvent.serializeByteBuf` (`:26-31`), et
//! [`Click::message`] rend le message du port, rempli. Rien n'est invente sur le fil : la
//! mise en forme est celle de `FriendlyByteBuf`.

use crate::base::Dimension;
use crate::network::colony_settings::TriggerConnectionEvent;
use crate::ui::action::Effect;
use crate::ui::field::{Field, Scope, Source, Value};

use super::Refusal;

/// La classe Java de la page.
/// `WindowAlliancePage.java:23`.
pub const JAVA: &str = "WindowAlliancePage";

/// Le layout, sous `assets/minecolonies/`.
/// `WindowAlliancePage.java:63`.
pub const RESOURCE: &str = "minecolonies:gui/townhall/layoutalliance.xml";

/// La fenetre dans laquelle le layout est monte (`layoutalliance.xml:2`).
pub const FRAME: &str = "minecolonies:gui/townhall/windowtownhall.xml";

/// L'onglet qui ouvre la page (`WindowConstants.java:37`,
/// `AbstractWindowTownHall.java:42`).
pub const ENTRY_TAB: &str = "alliances";

/// Le message que les quatre boutons envoient.
/// `WindowAlliancePage.java:96`, `:103`, `:110`, `:120`.
pub const MESSAGE: &str = "TriggerConnectionEventMessage";

/// La vue qui porte l'aide, quand il n'y a rien a lister
/// (`layoutalliance.xml:4`, basculee a `WindowAlliancePage.java:71`).
pub const MISSING_VIEW: &str = "missingconnections";

/// La vue qui porte les trois listes (`layoutalliance.xml:9`, basculee a `:77`).
pub const ACTIVE_VIEW: &str = "activeconnections";

/// Le premier texte de l'aide (`layoutalliance.xml:5`).
pub const HOWTO_KEY: &str = "com.minecolonies.core.gui.connectioneventlist.howto";
/// Le second (`layoutalliance.xml:6`).
pub const NO_CONNECTIONS_KEY: &str = "com.minecolonies.core.gui.connectioneventlist.noconnections";

/// La liste des evenements (`WindowAlliancePage.java:40`).
pub const EVENTS: &str = "connectioneventlist";
/// La liste des connexions directes (`:38`).
pub const DIRECT: &str = "directcolonylist";
/// La liste des connexions indirectes (`:39`).
pub const INDIRECT: &str = "indirectcolonylist";

/// Les trois libelles de liste, qui sont **trois panneaux de meme id `desc`**
/// (`layoutalliance.xml:10`, `:22`, `:38`) en plus du `desc` d'une ligne d'evenement
/// (`:14`) : quatre panneaux, un id, deux portees.
pub const EVENTS_LABEL_KEY: &str = "com.minecolonies.core.gui.connectioneventlist.desc";
/// `layoutalliance.xml:22`.
pub const DIRECT_LABEL_KEY: &str = "com.minecolonies.core.gui.colonylist.direct.desc";
/// `layoutalliance.xml:38`.
pub const INDIRECT_LABEL_KEY: &str = "com.minecolonies.core.gui.colonylist.indirect.desc";

/// Le libelle de la distance (`WindowAlliancePage.java:179`).
pub const DISTANCE_KEY: &str = "com.minecolonies.coremod.dist.blocks";

/// `writeUtf` borne une chaine a 32767 octets, et `readUtf` les relit avec ce maximum
/// (`ConnectionEvent.java:42`).
pub const MAX_NAME_BYTES: usize = 32767;

// ---------------------------------------------------------------------------
// Les deux enumerations de la diplomatie.
// ---------------------------------------------------------------------------

/// `DiplomacyStatus.java:8-10` — **trois** valeurs, dans cet ordre de declaration.
///
/// L'ordre est une donnee : c'est `ordinal()` qui part dans la sauvegarde
/// (`ColonyConnection.java:55`) et sur le fil (`:73`), pas le nom.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: trois valeurs dans le Java, et un ordinal de plus serait une migration
pub enum Status {
    /// Les deux colonies sont alliees.
    Allies,
    /// Etat par defaut, et celui de `getColonyDiplomacyStatus` pour un id inconnu
    /// (`ColonyConnectionManager.java:803`).
    Neutral,
    /// Querelle declaree.
    Hostile,
}

impl Status {
    /// Les trois, dans l'ordre de declaration.
    pub const ALL: [Status; 3] = [Status::Allies, Status::Neutral, Status::Hostile];

    /// `DiplomacyStatus.java:18` : `"com.minecolonies.core.colony.diplomacy.status." +
    /// name().toLowerCase()`.
    pub fn translation_key(self) -> &'static str {
        match self {
            Status::Allies => "com.minecolonies.core.colony.diplomacy.status.allies",
            Status::Neutral => "com.minecolonies.core.colony.diplomacy.status.neutral",
            Status::Hostile => "com.minecolonies.core.colony.diplomacy.status.hostile",
        }
    }

    /// La partie qui suit le prefixe, telle que `toLowerCase()` la produit.
    pub fn slug(self) -> &'static str {
        match self {
            Status::Allies => "allies",
            Status::Neutral => "neutral",
            Status::Hostile => "hostile",
        }
    }

    /// `ordinal()` — ce que la sauvegarde ecrit (`ColonyConnection.java:55`).
    pub fn ordinal(self) -> i32 {
        match self {
            Status::Allies => 0,
            Status::Neutral => 1,
            Status::Hostile => 2,
        }
    }

    /// L'ordinal lu dans une sauvegarde, **refuse** quand il n'existe pas.
    ///
    /// Java fait `DiplomacyStatus.values()[ordinal]` (`ColonyConnection.java:64`, `:81`)
    /// et leve sur un ordinal hors bornes : un `-1` ou un `3` dans un fichier ecrit a la
    /// main ferme l'ecran. Le port rend un refus nomme.
    pub fn from_ordinal(ordinal: i32) -> Result<Status, Refusal> {
        match ordinal {
            0 => Ok(Status::Allies),
            1 => Ok(Status::Neutral),
            2 => Ok(Status::Hostile),
            other => Err(Refusal::UnknownStatus { ordinal: other }),
        }
    }
}

/// `ConnectionEventType.java:8-12` — **cinq** valeurs, dans cet ordre.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: cinq valeurs, dont une que rien ne produit
pub enum EventKind {
    /// Une colonie demande l'alliance.
    AllyRequest,
    /// L'autre accepte.
    AllyConfirmed,
    /// Une querelle est declaree.
    FeudStarted,
    /// Retour a la neutralite.
    NeutralSet,
    /// Les liens sont rompus — **aucun code ne le produit**
    /// ([`EventKind::produced_by_page`]).
    Disconnected,
}

impl EventKind {
    /// Les cinq, dans l'ordre de declaration.
    pub const ALL: [EventKind; 5] = [
        EventKind::AllyRequest,
        EventKind::AllyConfirmed,
        EventKind::FeudStarted,
        EventKind::NeutralSet,
        EventKind::Disconnected,
    ];

    /// `ConnectionEventType.java:20` : `"com.minecolonies.core.gui.connectionevent." +
    /// name().toLowerCase()`.
    pub fn translation_key(self) -> &'static str {
        match self {
            EventKind::AllyRequest => "com.minecolonies.core.gui.connectionevent.ally_request",
            EventKind::AllyConfirmed => "com.minecolonies.core.gui.connectionevent.ally_confirmed",
            EventKind::FeudStarted => "com.minecolonies.core.gui.connectionevent.feud_started",
            EventKind::NeutralSet => "com.minecolonies.core.gui.connectionevent.neutral_set",
            EventKind::Disconnected => "com.minecolonies.core.gui.connectionevent.disconnected",
        }
    }

    /// La partie qui suit le prefixe.
    pub fn slug(self) -> &'static str {
        match self {
            EventKind::AllyRequest => "ally_request",
            EventKind::AllyConfirmed => "ally_confirmed",
            EventKind::FeudStarted => "feud_started",
            EventKind::NeutralSet => "neutral_set",
            EventKind::Disconnected => "disconnected",
        }
    }

    /// `ordinal()`, ce que la sauvegarde et le fil portent
    /// (`ConnectionEvent.java:22`, `:30`).
    pub fn ordinal(self) -> i32 {
        match self {
            EventKind::AllyRequest => 0,
            EventKind::AllyConfirmed => 1,
            EventKind::FeudStarted => 2,
            EventKind::NeutralSet => 3,
            EventKind::Disconnected => 4,
        }
    }

    /// L'ordinal lu dans une sauvegarde, refuse quand il n'existe pas
    /// (`ConnectionEvent.java:37`, `:42` — meme `values()[…]` que le statut).
    pub fn from_ordinal(ordinal: i32) -> Result<EventKind, Refusal> {
        match ordinal {
            0 => Ok(EventKind::AllyRequest),
            1 => Ok(EventKind::AllyConfirmed),
            2 => Ok(EventKind::FeudStarted),
            3 => Ok(EventKind::NeutralSet),
            4 => Ok(EventKind::Disconnected),
            other => Err(Refusal::UnknownEventKind { ordinal: other }),
        }
    }

    /// Le statut que cet evenement **etablit**, `None` quand il n'en etablit aucun.
    ///
    /// C'est le `switch` de `ColonyConnectionManager.java:750-756` : trois types
    /// changent le statut, `ALLY_REQUEST` et `DISCONNECTED` tombent dans le `default` et
    /// le laissent tel quel.
    pub fn sets(self) -> Option<Status> {
        match self {
            EventKind::AllyConfirmed => Some(Status::Allies),
            EventKind::FeudStarted => Some(Status::Hostile),
            EventKind::NeutralSet => Some(Status::Neutral),
            EventKind::AllyRequest | EventKind::Disconnected => None,
        }
    }

    /// Vrai quand l'un des quatre boutons de la page peut produire cet evenement.
    ///
    /// Les quatre boutons en envoient quatre (`WindowAlliancePage.java:93-122`) ;
    /// `DISCONNECTED` n'est construit nulle part.
    pub fn produced_by_page(self) -> bool {
        !matches!(self, EventKind::Disconnected)
    }
}

// ---------------------------------------------------------------------------
// Les quatre boutons.
// ---------------------------------------------------------------------------

/// Les quatre boutons de la page, avec leur id d'XML.
///
/// Les trois premiers vivent dans **deux** listes (`layoutalliance.xml:29-35`,
/// `:45-51`) : leur id n'est donc pas unique dans la fenetre, c'est leur portee qui
/// l'est — d'ou [`Ally::scopes`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: quatre `registerButton` (`WindowAlliancePage.java:83-86`)
pub enum Ally {
    /// `WindowAlliancePage.java:28`.
    RequestAlly,
    /// `WindowAlliancePage.java:29`.
    StartFeud,
    /// `WindowAlliancePage.java:30`.
    SetNeutral,
    /// `WindowAlliancePage.java:33` — le seul de la liste d'evenements.
    AcceptAlly,
}

/// Les quatre boutons, dans l'ordre des `registerButton` (`:83-86`).
pub const ALLIES: [Ally; 4] =
    [Ally::RequestAlly, Ally::StartFeud, Ally::SetNeutral, Ally::AcceptAlly];

/// Les portees des deux listes de colonies.
static TWO_LISTS: [Scope; 2] = [Scope::Row(DIRECT), Scope::Row(INDIRECT)];
/// La portee de la liste d'evenements.
static ONE_LIST: [Scope; 1] = [Scope::Row(EVENTS)];

impl Ally {
    /// L'id d'XML, celui que `registerButton` lie.
    pub fn id(self) -> &'static str {
        match self {
            Ally::RequestAlly => "requestally",
            Ally::StartFeud => "startfeud",
            Ally::SetNeutral => "setneutral",
            Ally::AcceptAlly => "acceptally",
        }
    }

    /// La cle du libelle que le XML dessine (`layoutalliance.xml:29`, `:31`, `:34`,
    /// `:16`).
    pub fn label_key(self) -> &'static str {
        match self {
            Ally::RequestAlly => "com.minecolonies.core.gui.colonylist.request_ally",
            Ally::StartFeud => "com.minecolonies.core.gui.colonylist.start_feud",
            Ally::SetNeutral => "com.minecolonies.core.gui.colonylist.set_neutral",
            Ally::AcceptAlly => "com.minecolonies.core.gui.connectioneventlist.accept_ally",
        }
    }

    /// Le type d'evenement que le clic envoie (`:93-122`).
    pub fn kind(self) -> EventKind {
        match self {
            Ally::RequestAlly => EventKind::AllyRequest,
            Ally::StartFeud => EventKind::FeudStarted,
            Ally::SetNeutral => EventKind::NeutralSet,
            Ally::AcceptAlly => EventKind::AllyConfirmed,
        }
    }

    /// Toutes les portees ou ce bouton apparaît : deux pour les trois premiers
    /// (`layoutalliance.xml:29`, `:45`), une pour `acceptally` (`:16`).
    pub fn scopes(self) -> &'static [Scope] {
        match self {
            Ally::RequestAlly | Ally::StartFeud | Ally::SetNeutral => &TWO_LISTS,
            Ally::AcceptAlly => &ONE_LIST,
        }
    }

    /// Vrai quand le bouton est dessine sur une ligne de colonie de ce statut.
    ///
    /// `None` quand la question ne s'applique pas : `acceptally` n'est pas dans une
    /// liste de colonies. `requestally` et `startfeud` n'apparaissent qu'en `NEUTRAL`,
    /// `setneutral` seulement en dehors (`:182-184`) — trois boutons, deux conditions, et
    /// le XML cache `setneutral` par defaut (`layoutalliance.xml:35`).
    ///
    /// **Les deux demandes partagent la meme condition** (`:182` et `:183` sont deux
    /// `<` a `== NEUTRAL`) : elles apparaissent et disparaissent ensemble. Une colonie
    /// neutre montre donc **deux** boutons, une alliee ou une hostile **un seul**
    /// (`setneutral`). Ce n'est pas un partage deux-contre-un.
    pub fn shown_on_connection(self, status: Status) -> Option<bool> {
        match self {
            Ally::RequestAlly | Ally::StartFeud => Some(status == Status::Neutral),
            Ally::SetNeutral => Some(status != Status::Neutral),
            Ally::AcceptAlly => None,
        }
    }

    /// Vrai quand le bouton est dessine sur une ligne d'evenement.
    ///
    /// `eventData.connectionEventType() == ALLY_REQUEST && diplomacyStatus != ALLIES`
    /// (`:223`) : il faut **les deux**, et le statut est celui de la colonie de
    /// l'evenement, lu avec `NEUTRAL` par defaut pour un id inconnu (`:793-804`).
    pub fn shown_on_event(self, kind: EventKind, status: Status) -> Option<bool> {
        match self {
            Ally::AcceptAlly => Some(kind == EventKind::AllyRequest && status != Status::Allies),
            Ally::RequestAlly | Ally::StartFeud | Ally::SetNeutral => None,
        }
    }

    /// Les refus d'un clic, dans l'ordre ou ils tombent.
    ///
    /// Un est dans la fenetre (`NotShown`), trois dans le serveur : `MANAGE_HUTS`
    /// (`TriggerConnectionEventMessage.java:50`), la colonie cible disparue (`:53-57`),
    /// l'origine qui n'est plus connectee (`ColonyConnectionManager.java:745-748`).
    /// `acceptally` porte le dernier en propre : c'est le seul dont la cible vient d'une
    /// ligne d'evenement, donc le seul qu'un evenement orphelin peut laisser sans effet.
    pub fn refusals(self) -> &'static [Refusal] {
        const COLONY_ROW: &[Refusal] =
            &[Refusal::NotShown, Refusal::NoPermission, Refusal::TargetColonyGone];
        const EVENT_ROW: &[Refusal] = &[
            Refusal::NotShown,
            Refusal::NoPermission,
            Refusal::TargetColonyGone,
            Refusal::OriginNotConnected,
        ];
        match self {
            Ally::RequestAlly | Ally::StartFeud | Ally::SetNeutral => COLONY_ROW,
            Ally::AcceptAlly => EVENT_ROW,
        }
    }

    /// L'effet du clic, au vocabulaire de `src/ui` : c'est tout ce que ce module
    /// partage avec `crate::ui::action::Action`.
    ///
    /// **Pourquoi pas un `Action`.** `Action.refusals` est
    /// `&'static [crate::ui::action::Refusal]`, une enumeration **fermee** des refus
    /// des ecrans de hutte : `NoPermission`, `TargetColonyGone` et
    /// `OriginNotConnected` n'y existent pas, et ce module n'a pas le droit de
    /// l'etendre. Un `Action` porte par ce bouton aurait donc zero refus et
    /// pretendrait que le clic ne peut pas echouer — c'est faux, le serveur peut
    /// l'abandonner (`TriggerConnectionEventMessage.java:50-57`). Les quatre elements
    /// d'un bouton sont donc portes par [`Ally`] : [`Ally::id`], [`Ally::label_key`],
    /// [`Ally::effect`], [`Ally::refusals`]. `ui_window_13` a tranche pareil, et pour
    /// la meme raison.
    pub fn effect(self) -> Effect {
        Effect::Sends(MESSAGE)
    }
}

// ---------------------------------------------------------------------------
// Les deux sortes de ligne.
// ---------------------------------------------------------------------------

/// Une colonie connectee, telle que `ColonyConnection` la porte.
///
/// Quatre champs publics (`ColonyConnection.java:16-19`) : un id, un nom mis en cache,
/// la position de sa porte et le statut. La classe est sauvegardee en NBT (`:49-57`)
/// avec le **statut en ordinal** — d'ou [`Status::from_ordinal`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Connection<'a> {
    pub id: i32,
    pub name: &'a str,
    /// `ColonyConnection.pos`, la porte de la colonie : c'est d'elle que la distance est
    /// mesuree (`WindowAlliancePage.java:179`).
    pub gate: [i32; 3],
    pub status: Status,
}

impl<'a> Connection<'a> {
    /// La ligne d'etat, qui n'est qu'une cle de traduction (`:180`).
    pub fn state_key(&self) -> &'static str {
        self.status.translation_key()
    }

    /// La distance a `center`, telle que la page l'ecrit (`:178-179`).
    pub fn distance(&self, center: [i32; 3]) -> i32 {
        distance(self.gate, center)
    }
}

/// Un evenement recu d'une autre colonie (`ConnectionEvent.java:15`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event<'a> {
    pub id: i32,
    pub name: &'a str,
    pub kind: EventKind,
}

impl<'a> Event<'a> {
    /// La ligne de description : la cle du type (`:221`).
    pub fn desc_key(&self) -> &'static str {
        self.kind.translation_key()
    }
}

/// La distance entiere que la page affiche :
/// `(int) BlockPosUtil.dist(colonyData.pos, colony.getCenter())` (`:179`),
/// `BlockPosUtil.dist` etant la racine de la somme des carres
/// (`BlockPosUtil.java:563-569`).
///
/// **Une divergence assumee.** Java calcule en `int` : au-dela de 46 340 blocs d'ecart
/// sur un axe, le carre deborde et la distance affichee devient fausse (le monde fait 60
/// millions de blocs de large, donc c'est atteignable). Le port calcule en `i64` et ne
/// deborde pas. Reproduire un debordement ne servirait aucun joueur, et la valeur juste
/// est celle que la formule veut dire.
pub fn distance(gate: [i32; 3], center: [i32; 3]) -> i32 {
    let dx = gate[0] as i64 - center[0] as i64;
    let dy = gate[1] as i64 - center[1] as i64;
    let dz = gate[2] as i64 - center[2] as i64;
    let sq = dx * dx + dy * dy + dz * dz;
    (sq as f64).sqrt() as i32
}

// ---------------------------------------------------------------------------
// L'etat de la page.
// ---------------------------------------------------------------------------

/// Laquelle des deux vues de la page est montree.
///
/// `missingconnections` quand les evenements **et** les directes sont vides (`:69`) ;
/// `activeconnections` autrement (`:77`). Les indirectes ne sont pas testees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: deux vues, l'une cache l'autre
pub enum Panes {
    /// L'aide : « voici comment construire une alliance » (`layoutalliance.xml:5-6`).
    Empty,
    /// Les trois listes.
    Active,
}

impl Panes {
    /// L'id de la vue montree (`layoutalliance.xml:4`, `:9`).
    pub fn view(self) -> &'static str {
        match self {
            Panes::Empty => MISSING_VIEW,
            Panes::Active => ACTIVE_VIEW,
        }
    }

    /// La vue cachee.
    pub fn hidden(self) -> &'static str {
        match self {
            Panes::Empty => ACTIVE_VIEW,
            Panes::Active => MISSING_VIEW,
        }
    }
}

/// La page telle que la fenetre la lit a la construction et a chaque `onUpdate`
/// (`:80-81`, `:142-146`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page<'a> {
    /// `buildingView.getColony().getID()` — l'origine de chaque evenement envoye
    /// (`:96`).
    pub colony_id: i32,
    /// `getColony().getName()` (`:96`).
    pub colony_name: &'a str,
    /// `getColony().getCenter()`, le point de mesure des distances (`:179`).
    pub center: [i32; 3],
    /// `getDirectlyConnectedColonies().values()`, en ordre **croissant d'id** — c'est un
    /// `TreeMap` (`:80`, `ColonyConnectionManager.java:53`).
    pub direct: Vec<Connection<'a>>,
    /// `getIndirectlyConnectedColonies().values()`, meme ordre (`:81`).
    pub indirect: Vec<Connection<'a>>,
    /// `getConnectionEvents()`, en ordre croissant d'id (`:789`) — la fenetre l'inverse a
    /// l'affichage (`:217-218`).
    pub events: Vec<Event<'a>>,
}

impl<'a> Page<'a> {
    /// Une page vide : trois listes, aucun contenu.
    pub fn new(colony_id: i32, colony_name: &'a str, center: [i32; 3]) -> Page<'a> {
        Page { colony_id, colony_name, center, direct: Vec::new(), indirect: Vec::new(), events: Vec::new() }
    }

    /// Laquelle des deux vues s'affiche (`:69-78`).
    pub fn panes(&self) -> Panes {
        if self.events.is_empty() && self.direct.is_empty() {
            Panes::Empty
        } else {
            Panes::Active
        }
    }

    /// Le statut de la colonie `id`, avec `NEUTRAL` pour un id que ni les directes ni les
    /// indirectes ne portent (`ColonyConnectionManager.java:793-804`).
    pub fn status_of(&self, id: i32) -> Status {
        self.find(id).map_or(Status::Neutral, |connection| connection.status)
    }

    /// La connexion de cet id, directe ou indirecte.
    pub fn find(&self, id: i32) -> Option<&Connection<'a>> {
        self.direct.iter().chain(self.indirect.iter()).find(|connection| connection.id == id)
    }

    /// La colonie d'une ligne de liste de colonies, ou le refus nomme.
    ///
    /// La portee est **obligatoire** : `requestally` porte le meme id dans les deux
    /// listes, donc un index seul ne dit pas de quelle liste il vient. Java le devine
    /// parce que `getListElementIndexByPane` rend `-1` pour la liste qui ne contient pas
    /// le panneau (`:126`, `:133`) ; le port prend la portee du panneau, ce qui rend la
    /// question decidable sans lire un `-1`.
    ///
    /// Un index negatif — ce que `getListElementIndexByPane` rend hors d'une liste —
    /// n'est donc **pas** un plantage : c'est [`Refusal::ColumnRowOutOfRange`]. Java, lui,
    /// fait `indirectConnectionData.get(-1)` (`:134`) et leve.
    pub fn connection(&self, scope: Scope, row: i32) -> Result<&Connection<'a>, Refusal> {
        let name = match scope {
            Scope::Window => return Err(Refusal::NotShown),
            Scope::Row(name) => name,
        };
        let list = if name == DIRECT {
            &self.direct
        } else if name == INDIRECT {
            &self.indirect
        } else {
            return Err(Refusal::NotShown);
        };
        if row < 0 || row as usize >= list.len() {
            return Err(Refusal::ColumnRowOutOfRange { row, list: name });
        }
        match list.get(row as usize) {
            Some(connection) => Ok(connection),
            None => Err(Refusal::ColumnRowOutOfRange { row, list: name }),
        }
    }

    /// L'evenement d'une ligne, avec l'inversion de Java : la ligne 0 est le **dernier**
    /// element de la liste (`:117`, `:217-218`).
    pub fn event(&self, row: i32) -> Result<&Event<'a>, Refusal> {
        let count = self.events.len();
        if row < 0 || row as usize >= count {
            return Err(Refusal::EventRowOutOfRange { row, count });
        }
        // `revIndex = size - 1 - row` (`:117`).
        let reversed = count - 1 - row as usize;
        match self.events.get(reversed) {
            Some(event) => Ok(event),
            None => Err(Refusal::EventRowOutOfRange { row, count }),
        }
    }

    /// Ce qu'un clic envoie, ou le refus qui dit pourquoi il ne dessine rien.
    ///
    /// Trois boutons prennent l'id de **leur** ligne ; `acceptally` prend l'id de
    /// l'evenement (`:121`), pas celui de la colonie. Rien n'est applique ici : la fenetre
    /// envoie et attend la resynchronisation (`:139-147` relit tout).
    pub fn send(&self, ally: Ally, scope: Scope, row: i32) -> Result<Click, Refusal> {
        if !ally.scopes().contains(&scope) {
            return Err(Refusal::NotShown);
        }
        let (target, kind) = match ally {
            Ally::RequestAlly | Ally::StartFeud | Ally::SetNeutral => {
                let connection = self.connection(scope, row)?;
                if ally.shown_on_connection(connection.status) != Some(true) {
                    return Err(Refusal::NotShown);
                }
                (connection.id, ally.kind())
            }
            Ally::AcceptAlly => {
                let event = self.event(row)?;
                if ally.shown_on_event(event.kind, self.status_of(event.id)) != Some(true) {
                    return Err(Refusal::NotShown);
                }
                (event.id, ally.kind())
            }
        };
        Ok(Click {
            target_colony_id: target,
            kind,
            event: event_bytes(self.colony_id, self.colony_name, kind)?,
        })
    }
}

/// Un clic qui a passe toutes les conditions : la colonie visee, le type, et les octets
/// de `ConnectionEvent.serializeByteBuf` (`ConnectionEvent.java:26-31`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Click {
    /// `targetColonyId` du message (`:97`, `:104`, `:111`, `:121`).
    pub target_colony_id: i32,
    pub kind: EventKind,
    /// `buf.writeInt(id)`, `buf.writeUtf(name)`, `buf.writeInt(ordinal)`.
    pub event: Vec<u8>,
}

impl Click {
    /// Le message que le port declare deja, rempli.
    ///
    /// `TriggerConnectionEvent` attend l'origine, la dimension, les octets et la cible
    /// (`src/network/colony_settings.rs:379-395`).
    pub fn message(&self, origin: i32, dimension: &Dimension) -> TriggerConnectionEvent {
        TriggerConnectionEvent {
            colony_id: origin,
            dimension: dimension.clone(),
            event: self.event.clone(),
            target_colony_id: self.target_colony_id,
        }
    }
}

/// Les octets d'un `ConnectionEvent`, dans l'ordre de `serializeByteBuf`
/// (`ConnectionEvent.java:26-31`).
///
/// `writeUtf` ecrit un VarInt de longueur puis les octets UTF-8 ; `writeInt` ecrit quatre
/// octets gros-boutistes. Le nom est refuse au-dela de [`MAX_NAME_BYTES`], la borne que
/// `readUtf(32767)` (`:42`) impose en face.
pub fn event_bytes(origin: i32, name: &str, kind: EventKind) -> Result<Vec<u8>, Refusal> {
    let bytes = name.as_bytes();
    if bytes.len() > MAX_NAME_BYTES {
        return Err(Refusal::NameTooLong { len: bytes.len() });
    }
    let mut out = Vec::with_capacity(8 + bytes.len() + 5);
    out.extend_from_slice(&origin.to_be_bytes());
    var_int(&mut out, bytes.len() as i32);
    out.extend_from_slice(bytes);
    out.extend_from_slice(&kind.ordinal().to_be_bytes());
    Ok(out)
}

/// `FriendlyByteBuf.writeVarInt` : sept bits par octet, le bit de poids fort disant qu'il
/// en reste un.
fn var_int(out: &mut Vec<u8>, value: i32) {
    let mut left = value as u32;
    loop {
        if left & !0x7F == 0 {
            out.push(left as u8);
            return;
        }
        out.push(((left & 0x7F) | 0x80) as u8);
        left >>= 7;
    }
}

/// Ce qu'un evenement recu change, cote serveur.
///
/// `ColonyConnectionManager.triggerConnectionEvent` dans l'ordre de son corps, et dans
/// cet ordre precis :
///
/// 1. **l'evenement est enregistre avant tout controle** (`:732`) — un evenement d'une
///    colonie qui n'est plus connectee apparait donc dans le journal ;
/// 2. l'origine est cherchee dans les directes puis les indirectes (`:735-743`) ; absente
///    des deux, la fonction **sort** (`:745-748`) : le statut ne bouge pas ;
/// 3. le statut est calcule par le `switch` (`:750-756`) et ecrit dans la carte concernee
///    (`:758`) ;
/// 4. l'autre colonie recoit le meme statut (`:778-781`).
///
/// `Ok(None)` est le cas d'`ALLY_REQUEST` : enregistre, et aucun statut ne change.
pub fn receive<'a>(
    direct: &mut Vec<Connection<'a>>,
    indirect: &mut Vec<Connection<'a>>,
    events: &mut Vec<Event<'a>>,
    origin: Event<'a>,
) -> Result<Option<Status>, Refusal> {
    // `connectionEvents.put(id, event)` : une cle par colonie, donc un remplacement
    // (`:63`, `:732`).
    match events.iter_mut().find(|event| event.id == origin.id) {
        Some(existing) => *existing = origin,
        None => {
            events.push(origin);
            events.sort_by(|left, right| left.id.cmp(&right.id));
        }
    }
    let in_direct = direct.iter().any(|connection| connection.id == origin.id);
    let holder = if in_direct {
        direct
    } else if indirect.iter().any(|connection| connection.id == origin.id) {
        indirect
    } else {
        return Err(Refusal::OriginNotConnected);
    };
    match holder.iter_mut().find(|connection| connection.id == origin.id) {
        Some(connection) => match origin.kind.sets() {
            Some(status) => {
                connection.status = status;
                Ok(Some(status))
            }
            None => Ok(None),
        },
        None => Err(Refusal::OriginNotConnected),
    }
}

/// Les onze panneaux que la classe ecrit, dans l'ordre du `updateElement` des deux listes
/// de colonies (`:177-180`) puis de la liste d'evenements (`:220-221`).
///
/// Un panneau de ligne de la liste directe et son homologue de la liste indirecte sont
/// **deux declarations**, pas une : meme id, deux portees.
pub const FIELDS: &[Field] = &[
    Field::new(DIRECT, Value::Row, Source::Colony),
    Field::in_row(DIRECT, "name", Value::Name, Source::Colony),
    Field::in_row(DIRECT, "distance", Value::Count, Source::Colony),
    Field::in_row(DIRECT, "state", Value::Text, Source::Colony),
    Field::new(INDIRECT, Value::Row, Source::Colony),
    Field::in_row(INDIRECT, "name", Value::Name, Source::Colony),
    Field::in_row(INDIRECT, "distance", Value::Count, Source::Colony),
    Field::in_row(INDIRECT, "state", Value::Text, Source::Colony),
    Field::new(EVENTS, Value::Row, Source::Colony),
    Field::in_row(EVENTS, "name", Value::Name, Source::Colony),
    // Le `desc` d'une ligne d'evenement (`layoutalliance.xml:14`) et les trois `desc` qui
    // titrent les listes (`:10`, `:22`, `:38`) partagent l'id : quatre panneaux, deux
    // portees, et la classe n'ecrit que celui des lignes (`:221`).
    Field::in_row(EVENTS, "desc", Value::Text, Source::Colony),
];

// Aucune fonction `actions()` ici : [`ALLIES`] **est** la liste des quatre boutons, et
// un bouton de cette page n'est pas un `crate::ui::action::Action` (voir
// [`Ally::effect`], et la raison : `Action.refusals` est une enumeration fermee).

#[cfg(test)]
mod tests {
    use super::*;

    fn page() -> Page<'static> {
        let mut page = Page::new(1, "Royaume", [0, 64, 0]);
        page.direct = vec![
            Connection { id: 3, name: "Côte", gate: [30, 64, 40], status: Status::Allies },
            Connection { id: 7, name: "Vallée", gate: [300, 64, 400], status: Status::Neutral },
        ];
        page.indirect =
            vec![Connection { id: 9, name: "Loin", gate: [0, 64, 100], status: Status::Hostile }];
        page.events = vec![
            Event { id: 3, name: "Côte", kind: EventKind::AllyConfirmed },
            Event { id: 7, name: "Vallée", kind: EventKind::AllyRequest },
        ];
        page
    }

    fn send(page: &Page<'_>, ally: Ally, scope: Scope, row: i32) -> Click {
        match page.send(ally, scope, row) {
            Ok(send) => send,
            Err(refusal) => panic!("{refusal}"),
        }
    }

    #[test]
    fn three_statuses_in_java_order_with_a_key_built_from_the_name() {
        assert_eq!(Status::ALL.len(), 3);
        for (ordinal, status) in Status::ALL.iter().enumerate() {
            assert_eq!(status.ordinal(), ordinal as i32, "DiplomacyStatus.java:8-10");
            assert_eq!(Status::from_ordinal(ordinal as i32), Ok(*status));
            let expected =
                format!("com.minecolonies.core.colony.diplomacy.status.{}", status.slug());
            assert_eq!(status.translation_key(), expected, "DiplomacyStatus.java:18");
        }
        // Un ordinal hors bornes est un refus nomme, pas une indexation.
        assert_eq!(Status::from_ordinal(3), Err(Refusal::UnknownStatus { ordinal: 3 }));
        assert_eq!(Status::from_ordinal(-1), Err(Refusal::UnknownStatus { ordinal: -1 }));
    }

    #[test]
    fn five_event_kinds_and_only_four_are_produced_by_the_page() {
        assert_eq!(EventKind::ALL.len(), 5);
        for (ordinal, kind) in EventKind::ALL.iter().enumerate() {
            assert_eq!(kind.ordinal(), ordinal as i32, "ConnectionEventType.java:8-12");
            assert_eq!(EventKind::from_ordinal(ordinal as i32), Ok(*kind));
            let expected = format!("com.minecolonies.core.gui.connectionevent.{}", kind.slug());
            assert_eq!(kind.translation_key(), expected, "ConnectionEventType.java:20");
        }
        let produced: Vec<&str> = EventKind::ALL
            .iter()
            .filter(|kind| kind.produced_by_page())
            .map(|kind| kind.slug())
            .collect();
        assert_eq!(produced, ["ally_request", "ally_confirmed", "feud_started", "neutral_set"]);
        // `DISCONNECTED` a une cle de langue (`manual_en_us.json:3032`) et aucun
        // producteur dans l'arbre.
        assert_eq!(
            EventKind::Disconnected.translation_key(),
            "com.minecolonies.core.gui.connectionevent.disconnected"
        );
        assert_eq!(EventKind::from_ordinal(5), Err(Refusal::UnknownEventKind { ordinal: 5 }));
        assert_eq!(EventKind::from_ordinal(-1), Err(Refusal::UnknownEventKind { ordinal: -1 }));
    }

    #[test]
    fn an_event_sets_three_statuses_and_leaves_two_alone() {
        assert_eq!(EventKind::AllyConfirmed.sets(), Some(Status::Allies));
        assert_eq!(EventKind::FeudStarted.sets(), Some(Status::Hostile));
        assert_eq!(EventKind::NeutralSet.sets(), Some(Status::Neutral));
        // Le `default` du switch (`ColonyConnectionManager.java:755`).
        assert_eq!(EventKind::AllyRequest.sets(), None);
        assert_eq!(EventKind::Disconnected.sets(), None);
        // Une demande d'alliance n'allie personne : c'est l'acceptation qui le fait.
        assert_ne!(EventKind::AllyRequest.sets(), EventKind::AllyConfirmed.sets());
    }

    #[test]
    fn the_two_wish_buttons_show_only_on_neutral_and_the_third_only_elsewhere() {
        for ally in [Ally::RequestAlly, Ally::StartFeud] {
            assert_eq!(ally.shown_on_connection(Status::Neutral), Some(true), ":182-183");
            assert_eq!(ally.shown_on_connection(Status::Allies), Some(false));
            assert_eq!(ally.shown_on_connection(Status::Hostile), Some(false));
            assert_eq!(ally.shown_on_event(EventKind::AllyRequest, Status::Neutral), None);
        }
        assert_eq!(Ally::SetNeutral.shown_on_connection(Status::Neutral), Some(false), ":184");
        assert_eq!(Ally::SetNeutral.shown_on_connection(Status::Allies), Some(true));
        assert_eq!(Ally::SetNeutral.shown_on_connection(Status::Hostile), Some(true));
        // Les deux boutons de demande partagent **une seule** condition
        // (`== NEUTRAL`, `:182-183`) : ils s'eteignent ensemble. `setneutral` est la
        // negation de cette condition (`!= NEUTRAL`, `:184`), pas celle des deux
        // autres — donc une colonie neutre affiche 2 boutons, et une colonie alliee ou
        // hostile 1 seul (`setneutral`). Trois boutons, deux etats de dessin.
        for status in Status::ALL {
            let shown = [Ally::RequestAlly, Ally::StartFeud, Ally::SetNeutral]
                .iter()
                .filter(|ally| ally.shown_on_connection(status) == Some(true))
                .count();
            let expected = if status == Status::Neutral { 2 } else { 1 };
            assert_eq!(shown, expected, "{status:?} : {expected} bouton(s) de colonie");
        }
        // Et les deux demandes ne sont jamais dessinees sur une alliee ou une hostile.
        for status in [Status::Allies, Status::Hostile] {
            assert_eq!(Ally::RequestAlly.shown_on_connection(status), Some(false));
            assert_eq!(Ally::StartFeud.shown_on_connection(status), Some(false));
            assert_eq!(Ally::SetNeutral.shown_on_connection(status), Some(true));
        }
    }

    #[test]
    fn accept_alliance_needs_a_request_and_a_status_that_is_not_already_allied() {
        assert_eq!(
            Ally::AcceptAlly.shown_on_event(EventKind::AllyRequest, Status::Neutral),
            Some(true),
            ":223"
        );
        assert_eq!(Ally::AcceptAlly.shown_on_event(EventKind::AllyRequest, Status::Hostile), Some(true));
        // Les deux moities de la condition, separement fausses.
        assert_eq!(Ally::AcceptAlly.shown_on_event(EventKind::AllyRequest, Status::Allies), Some(false));
        assert_eq!(Ally::AcceptAlly.shown_on_event(EventKind::FeudStarted, Status::Neutral), Some(false));
        assert_eq!(Ally::AcceptAlly.shown_on_connection(Status::Neutral), None);
        // Un evenement d'une colonie inconnue lit le statut par defaut, `NEUTRAL`
        // (`ColonyConnectionManager.java:803`) : le bouton est dessine.
        let page = Page::new(1, "Royaume", [0, 0, 0]);
        assert_eq!(page.status_of(42), Status::Neutral);
    }

    #[test]
    fn two_buttons_live_in_both_colony_lists_and_one_in_the_event_list() {
        assert_eq!(ALLIES.len(), 4);
        for ally in ALLIES {
            for other in ALLIES {
                if ally != other {
                    assert_ne!(ally.id(), other.id(), "deux boutons, un id");
                }
            }
            assert!(ally.label_key().starts_with("com.minecolonies.core.gui."));
            assert_eq!(ally.effect(), Effect::Sends(MESSAGE));
            assert!(!ally.refusals().is_empty(), "chaque clic a ses refus nommes");
        }
        // Les trois premiers sont dans deux portees, `acceptally` dans une seule.
        for ally in [Ally::RequestAlly, Ally::StartFeud, Ally::SetNeutral] {
            assert_eq!(ally.scopes(), &[Scope::Row(DIRECT), Scope::Row(INDIRECT)]);
        }
        assert_eq!(Ally::AcceptAlly.scopes(), &[Scope::Row(EVENTS)]);
        // `acceptally` porte un refus de plus : c'est le seul qu'un evenement orphelin
        // peut laisser sans effet.
        assert_eq!(Ally::RequestAlly.refusals().len(), 3);
        assert_eq!(Ally::AcceptAlly.refusals().len(), 4);
        assert!(Ally::AcceptAlly.refusals().contains(&Refusal::OriginNotConnected));
        assert!(!Ally::RequestAlly.refusals().contains(&Refusal::OriginNotConnected));
    }

    #[test]
    fn the_empty_page_test_ignores_indirect_connections() {
        let mut page = Page::new(1, "Royaume", [0, 0, 0]);
        assert_eq!(page.panes(), Panes::Empty, ":69");
        assert_eq!(page.panes().view(), MISSING_VIEW);
        assert_eq!(page.panes().hidden(), ACTIVE_VIEW);
        // Une indirecte seule ne suffit pas : Java ne teste que les directes.
        page.indirect =
            vec![Connection { id: 9, name: "Loin", gate: [0, 0, 0], status: Status::Neutral }];
        assert_eq!(page.panes(), Panes::Empty, "les indirectes ne sont pas testees");
        // Un evenement suffit, meme sans aucune connexion.
        page.events = vec![Event { id: 9, name: "Loin", kind: EventKind::AllyRequest }];
        assert_eq!(page.panes(), Panes::Active);
        assert_eq!(page.panes().view(), ACTIVE_VIEW);
        // Et une directe suffit, meme sans evenement.
        let mut direct = Page::new(1, "Royaume", [0, 0, 0]);
        direct.direct =
            vec![Connection { id: 3, name: "Côte", gate: [0, 0, 0], status: Status::Allies }];
        assert_eq!(direct.panes(), Panes::Active);
    }

    #[test]
    fn a_row_names_the_colony_its_distance_and_its_status() {
        let page = page();
        let cells: Vec<(Value, &'static str)> = FIELDS
            .iter()
            .filter(|field| field.scope == Scope::Row(DIRECT))
            .map(|field| (field.value, field.id))
            .collect();
        assert_eq!(
            cells,
            [(Value::Name, "name"), (Value::Count, "distance"), (Value::Text, "state")]
        );
        let first = page.direct[1];
        assert_eq!(first.name, "Vallée");
        assert_eq!(first.distance(page.center), 500, "3-4-5 a l'echelle 100");
        assert_eq!(first.state_key(), "com.minecolonies.core.colony.diplomacy.status.neutral");
        assert_eq!(page.direct[0].state_key(), Status::Allies.translation_key());
        // Les onze panneaux, et aucune valeur hors des quatre que cette page emploie.
        assert_eq!(FIELDS.len(), 11);
        let mut used: Vec<Value> = FIELDS.iter().map(|field| field.value).collect();
        used.sort_unstable();
        used.dedup();
        // L'ordre est celui de `Value::ALL`, pas un ordre choisi ici.
        assert_eq!(used, [Value::Name, Value::Count, Value::Row, Value::Text]);
        // Les deux listes de colonies portent les memes trois panneaux, en deux portees.
        assert_eq!(FIELDS.iter().filter(|field| field.scope == Scope::Row(INDIRECT)).count(), 3);
        assert_eq!(FIELDS.iter().filter(|field| field.scope == Scope::Row(EVENTS)).count(), 2);
    }

    #[test]
    fn the_distance_is_truncated_like_java_and_does_not_overflow_like_it() {
        assert_eq!(distance([0, 0, 0], [0, 0, 0]), 0);
        assert_eq!(distance([3, 4, 0], [0, 0, 0]), 5);
        // Tronque, pas arrondi : sqrt(5) = 2.236 -> 2, comme `(int)` sur le `double`.
        assert_eq!(distance([1, 2, 0], [0, 0, 0]), 2);
        assert_eq!(distance([-3, -4, 0], [0, 0, 0]), 5, "la valeur absolue, sans signe");
        // Au-dela de 46 340 blocs, Java deborde en `int` et affiche faux ; le port calcule
        // en `i64` et reste juste (voir la doc de `distance`).
        assert_eq!(distance([40_000, 0, 0], [0, 0, 0]), 40_000);
        assert_eq!(distance([0, 0, 0], [0, 64, 0]), 64);
        // La meme mesure par la ligne, qui est ce que la fenetre ecrit (`:179`).
        let connection = Connection { id: 1, name: "A", gate: [0, 0, 100], status: Status::Neutral };
        assert_eq!(connection.distance([0, 0, 0]), 100);
    }

    #[test]
    fn the_event_list_is_reversed_by_colony_id_not_by_recency() {
        let page = page();
        // La liste portee est en ordre croissant d'id (`:789`), l'affichage l'inverse
        // (`:217-218`) : la ligne 0 est la colonie de plus grand id.
        assert_eq!(page.events[0].id, 3, "la liste elle-meme est croissante");
        assert_eq!(page.event(0).map(|event| event.id), Ok(7));
        assert_eq!(page.event(1).map(|event| event.id), Ok(3));
        assert_eq!(page.event(2), Err(Refusal::EventRowOutOfRange { row: 2, count: 2 }));
        assert_eq!(page.event(-1), Err(Refusal::EventRowOutOfRange { row: -1, count: 2 }));
        // La description d'un evenement est la cle de son type (`:221`).
        assert_eq!(
            page.event(0).map(|event| event.desc_key()),
            Ok("com.minecolonies.core.gui.connectionevent.ally_request")
        );
        // Un evenement ajoute par `receive` se place par id, pas par date d'arrivee.
        let mut events = page.events.clone();
        let newcomer = Event { id: 5, name: "Milieu", kind: EventKind::NeutralSet };
        assert_eq!(
            receive(&mut page.direct.clone(), &mut Vec::new(), &mut events, newcomer),
            Err(Refusal::OriginNotConnected)
        );
        let ids: Vec<i32> = events.iter().map(|event| event.id).collect();
        assert_eq!(ids, [3, 5, 7], "un TreeMap, donc croissant");
    }

    #[test]
    fn a_colony_row_is_read_from_the_list_its_scope_names() {
        let page = page();
        assert_eq!(page.connection(Scope::Row(DIRECT), 0).map(|c| c.id), Ok(3));
        assert_eq!(page.connection(Scope::Row(DIRECT), 1).map(|c| c.id), Ok(7));
        // La meme ligne sur la liste indirecte est une autre colonie, et c'est le fait :
        // `requestally` porte le meme id dans les deux listes.
        assert_eq!(page.connection(Scope::Row(INDIRECT), 0).map(|c| c.id), Ok(9));
        assert_eq!(
            page.connection(Scope::Row(INDIRECT), 1),
            Err(Refusal::ColumnRowOutOfRange { row: 1, list: INDIRECT })
        );
        // Un index negatif est ce que rend `getListElementIndexByPane` hors d'une liste ;
        // Java lit alors `get(-1)` (`:134`) et leve.
        assert_eq!(
            page.connection(Scope::Row(DIRECT), -1),
            Err(Refusal::ColumnRowOutOfRange { row: -1, list: DIRECT })
        );
        // La liste d'evenements n'est pas une liste de colonies.
        assert_eq!(page.connection(Scope::Row(EVENTS), 0), Err(Refusal::NotShown));
        assert_eq!(page.connection(Scope::Window, 0), Err(Refusal::NotShown));
        assert_eq!(page.find(99), None);
        assert_eq!(page.status_of(99), Status::Neutral);
        assert_eq!(page.find(9).map(|c| c.name), Some("Loin"));
    }

    #[test]
    fn a_click_targets_the_row_s_colony_except_accept_which_targets_the_event() {
        let page = page();
        let request = send(&page, Ally::RequestAlly, Scope::Row(DIRECT), 1);
        assert_eq!(request.target_colony_id, 7, ":111, l'id de la ligne");
        assert_eq!(request.kind, EventKind::AllyRequest);
        assert_eq!(
            send(&page, Ally::StartFeud, Scope::Row(DIRECT), 1).kind,
            EventKind::FeudStarted
        );
        // Le meme bouton, l'autre liste, une autre cible.
        assert_eq!(
            send(&page, Ally::SetNeutral, Scope::Row(INDIRECT), 0).target_colony_id,
            9,
            "la ligne indirecte est une autre colonie"
        );
        // `acceptally` : la ligne 0 de la liste inversee est l'evenement de la colonie 7.
        let accept = send(&page, Ally::AcceptAlly, Scope::Row(EVENTS), 0);
        assert_eq!(accept.target_colony_id, 7, ":121, l'id de l'evenement");
        assert_eq!(accept.kind, EventKind::AllyConfirmed);
        // Ce que la visibilite interdit ne part pas : la ligne 0 des directes est une
        // alliee, donc ses deux boutons de demande sont caches (`:182-183`).
        assert_eq!(page.send(Ally::RequestAlly, Scope::Row(DIRECT), 0), Err(Refusal::NotShown));
        assert_eq!(page.send(Ally::StartFeud, Scope::Row(DIRECT), 0), Err(Refusal::NotShown));
        // Une colonie hostile, elle, a bien son « set neutral » : la ligne 0 des indirectes
        // est hostile, donc `setneutral` y est dessine — et c'est `requestally` qui y est
        // cache (`:182-184`).
        assert_eq!(page.send(Ally::RequestAlly, Scope::Row(INDIRECT), 0), Err(Refusal::NotShown));
        assert_eq!(page.send(Ally::AcceptAlly, Scope::Row(EVENTS), 1), Err(Refusal::NotShown));
        // Un bouton de colonie sur la liste d'evenements, ou l'inverse, ne dessine rien.
        assert_eq!(page.send(Ally::RequestAlly, Scope::Row(EVENTS), 0), Err(Refusal::NotShown));
        assert_eq!(page.send(Ally::AcceptAlly, Scope::Row(DIRECT), 0), Err(Refusal::NotShown));
        assert_eq!(
            page.send(Ally::RequestAlly, Scope::Row(DIRECT), 9),
            Err(Refusal::ColumnRowOutOfRange { row: 9, list: DIRECT })
        );
    }

    #[test]
    fn a_click_sends_the_event_the_network_module_already_carries() {
        let page = page();
        let request = send(&page, Ally::RequestAlly, Scope::Row(DIRECT), 1);
        // `serializeByteBuf` : writeInt(id), writeUtf(name), writeInt(ordinal)
        // (`ConnectionEvent.java:26-31`). L'id est l'origine, pas la cible (`:110`).
        assert_eq!(&request.event[..4], &1i32.to_be_bytes());
        assert_eq!(request.event[4], 7, "la longueur du nom");
        assert_eq!(&request.event[5..12], b"Royaume");
        assert_eq!(&request.event[12..], &0i32.to_be_bytes(), "ALLY_REQUEST = 0");
        assert_eq!(request.event.len(), 16);
        // Le message du port, rempli : c'est lui que `src/network` transporte.
        let dimension = Dimension::new("minecraft:overworld");
        let message = request.message(page.colony_id, &dimension);
        assert_eq!(message.colony_id, 1);
        assert_eq!(message.target_colony_id, 7);
        assert_eq!(message.event, request.event);
        assert_eq!(message.dimension, dimension);
        // Un nom qui depasse la borne de `readUtf` est refuse, pas tronque.
        assert_eq!(
            event_bytes(1, &"a".repeat(MAX_NAME_BYTES + 1), EventKind::NeutralSet),
            Err(Refusal::NameTooLong { len: MAX_NAME_BYTES + 1 })
        );
        // Le VarInt de longueur tient sur deux octets des 128 caracteres.
        let long = match event_bytes(1, &"a".repeat(200), EventKind::Disconnected) {
            Ok(bytes) => bytes,
            Err(refusal) => panic!("{refusal}"),
        };
        assert_eq!(long[4], 0b1100_1000, "200 = 0x48, plus le bit de continuation");
        assert_eq!(long[5], 0b0000_0001);
        assert_eq!(long.len(), 4 + 2 + 200 + 4);
        assert_eq!(&long[long.len() - 4..], &4i32.to_be_bytes(), "DISCONNECTED = 4");
    }

    #[test]
    fn an_event_from_a_colony_we_are_not_connected_to_is_stored_and_changes_nothing() {
        let mut direct =
            vec![Connection { id: 7, name: "Vallée", gate: [0, 0, 0], status: Status::Neutral }];
        let mut indirect: Vec<Connection> = Vec::new();
        let mut events: Vec<Event> = Vec::new();
        // Une demande d'alliance change le journal et pas le statut : `ALLY_REQUEST` tombe
        // dans le `default` (`ColonyConnectionManager.java:755`).
        let origin = Event { id: 7, name: "Vallée", kind: EventKind::AllyRequest };
        assert_eq!(receive(&mut direct, &mut indirect, &mut events, origin), Ok(None));
        assert_eq!(events.len(), 1);
        assert_eq!(direct[0].status, Status::Neutral, "une demande n'allie personne");
        // L'acceptation, elle, allie.
        let accept = Event { id: 7, name: "Vallée", kind: EventKind::AllyConfirmed };
        assert_eq!(
            receive(&mut direct, &mut indirect, &mut events, accept),
            Ok(Some(Status::Allies))
        );
        assert_eq!(direct[0].status, Status::Allies);
        // Un second evenement de la MEME colonie remplace le premier : une cle par
        // colonie (`:63`, `:732`), pas un journal.
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, EventKind::AllyConfirmed);
        // D'une colonie qui n'est ni directe ni indirecte : l'evenement est enregistre
        // AVANT le controle (`:732`), puis la fonction sort (`:745-748`).
        let stranger = Event { id: 42, name: "Étrangère", kind: EventKind::FeudStarted };
        assert_eq!(
            receive(&mut direct, &mut indirect, &mut events, stranger),
            Err(Refusal::OriginNotConnected)
        );
        assert_eq!(events.len(), 2, "la ligne apparait quand meme dans le journal");
        assert!(events.iter().any(|event| event.id == 42));
    }

    #[test]
    fn an_indirect_colony_gets_its_status_in_the_indirect_list() {
        let mut direct: Vec<Connection> = Vec::new();
        let mut indirect =
            vec![Connection { id: 9, name: "Loin", gate: [0, 0, 0], status: Status::Neutral }];
        let mut events = Vec::new();
        let feud = Event { id: 9, name: "Loin", kind: EventKind::FeudStarted };
        assert_eq!(
            receive(&mut direct, &mut indirect, &mut events, feud),
            Ok(Some(Status::Hostile))
        );
        assert_eq!(indirect[0].status, Status::Hostile, ":740-743, la carte qui la porte");
        assert!(direct.is_empty(), "une indirecte ne devient pas directe");
        // Un troisieme evenement d'une colonie non connectee : enregistre, trie, sans
        // effet sur les deux listes.
        let third = Event { id: 2, name: "Bas", kind: EventKind::NeutralSet };
        assert_eq!(
            receive(&mut direct, &mut indirect, &mut events, third),
            Err(Refusal::OriginNotConnected)
        );
        let ids: Vec<i32> = events.iter().map(|event| event.id).collect();
        assert_eq!(ids, [2, 9], "un TreeMap, donc croissant (`:63`)");
    }

    #[test]
    fn the_page_names_its_class_its_layout_its_frame_and_its_tab() {
        assert_eq!(JAVA, "WindowAlliancePage");
        assert_eq!(RESOURCE, "minecolonies:gui/townhall/layoutalliance.xml");
        assert_ne!(RESOURCE, FRAME, "le layout monte la fenetre du tribunal, il n'en est pas une");
        assert_eq!(ENTRY_TAB, "alliances", "WindowConstants.java:37");
        assert_eq!(MESSAGE, "TriggerConnectionEventMessage");
        // Quatre panneaux `desc`, un id, deux portees (`layoutalliance.xml:10`, `:14`,
        // `:22`, `:38`) — et trois libelles differents.
        assert_ne!(EVENTS_LABEL_KEY, DIRECT_LABEL_KEY);
        assert_ne!(DIRECT_LABEL_KEY, INDIRECT_LABEL_KEY);
        assert!(DISTANCE_KEY.starts_with("com.minecolonies.coremod."));
        assert!(HOWTO_KEY.starts_with("com.minecolonies.core."));
        assert!(NO_CONNECTIONS_KEY.starts_with("com.minecolonies.core."));
        for ally in ALLIES {
            assert_eq!(ally.effect(), Effect::Sends(MESSAGE));
            for refusal in ally.refusals() {
                assert_eq!(refusal.key(), None, "aucun de ces refus ne parle au joueur");
            }
        }
    }
}
