//! Pourquoi une lecture ou un clic des quatre fenêtres est refusé — nommé.
//!
//! Java dit non de trois manières, et les trois sont des refus : une exception qui tombe
//! (`builders.get(index)`, `WindowBuildBuilding.java:187`), un bouton **caché**
//! (`:143-150`), et une phrase au joueur (`WindowBuildDecoration.java:164`).
//! [`Refusal::key`] ne répond `Some` que dans le troisième cas : un appelant ne doit donc
//! jamais imprimer [`Refusal::name`] là où le joueur attend une phrase.
//!
//! [`Refusal::Port`] reprend un refus de `src/ui` **tel quel** — les deux tables ne
//! peuvent pas diverger sur ce que « bâtiment non construit » veut dire, exactement comme
//! `ui::action::Refusal::HutRefuses` reprend ceux de `src/jobs`.

use std::fmt;

/// Un refus de ces quatre fenêtres.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Le bâtiment n'est pas un `AbstractBuildingBuilderView`
    /// (`WindowBuildBuilding.java:251`). Dans cet arbre un seul type en hérite,
    /// `BuildingBuilder.View` (`BuildingBuilder.java:213`).
    NotABuilderHut,
    /// Le bâtiment est un atelier de builder mais n'a pas de worker
    /// (`getWorkerName().isEmpty()`, `:251`).
    NoWorker,
    /// Le type du bâtiment est la mine (`:252`, `WindowBuildDecoration.java:173`).
    MinerHut,
    /// Aucune colonie à la position demandée (`WindowBuildDecoration.java:162-167`) :
    /// la fenêtre se ferme et le joueur reçoit `OUT_OF_COLONY`.
    OutOfColony,
    /// Le blueprint du niveau visé n'existe pas (`WindowBuildBuilding.java:323-329`) :
    /// `build` et `repair` sont cachés, `pickup` est montré.
    BlueprintMissing,
    /// `canBeUpgraded()` est faux (`:147-150`) : le bouton de build est caché.
    CannotUpgrade,
    /// Un index de liste hors bornes : Java lève `IndexOutOfBoundsException`
    /// (`:187`, `:205`, `:335`) là où le port rend un nom.
    IndexOutOfRange,
    /// Un fournisseur de liste vide : Java rend `""` pour tout index (`:402-408`).
    EmptyProvider,
    /// Une pile vide ou un compte de zéro n'entre pas dans la liste des ressources
    /// (`:365-368`) ; Java revient sans rien dire, donc pas de clé.
    EmptyStack,
    /// Le jeton demandé n'est pas dans la liste (`WindowClipBoard.java:149-152`, `:143-147`).
    NoSuchRequest,
    /// La requête n'est pas à la racine : `cancel` n'est offert qu'aux racines
    /// (`RequestTreeWindowModule.java:265-273`).
    NotRootRequest,
    /// Ce module ne remplit jamais une requête : `isFulfillable` rend `false` et
    /// `ClipboardRequestTreeWindowModule` ne le redéfinit pas (`:275-278`).
    FulfillUnsupported,
    /// La lecture des requêtes a levé : le gestionnaire est remis à zéro et la liste
    /// rendue vide (`WindowClipBoard.java:176-181`).
    RequestQueryFailed,
    /// Un id ou une classe que cette table ne décrit pas : `WindowInfo`,
    /// `WindowsBarracksSpies`, les treize écrans de `src/ui` et le reste des fenêtres de
    /// `core/client/gui/`.
    UnknownWindow,
    /// Un refus que `src/ui::action::Refusal` nomme déjà, repris tel quel.
    Port(crate::ui::action::Refusal),
}

impl Refusal {
    /// Un nom stable, préfixé par la fenêtre à laquelle il appartient.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::NotABuilderHut => "build.refusal.not_a_builder_hut",
            Refusal::NoWorker => "build.refusal.no_worker",
            Refusal::MinerHut => "build.refusal.miner",
            Refusal::OutOfColony => "build.refusal.out_of_colony",
            Refusal::BlueprintMissing => "build.refusal.no_blueprint",
            Refusal::CannotUpgrade => "build.refusal.cannot_upgrade",
            Refusal::IndexOutOfRange => "build.refusal.index",
            Refusal::EmptyProvider => "build.refusal.empty_provider",
            Refusal::EmptyStack => "build.refusal.empty_stack",
            Refusal::NoSuchRequest => "clipboard.refusal.no_such_request",
            Refusal::NotRootRequest => "clipboard.refusal.not_root",
            Refusal::FulfillUnsupported => "clipboard.refusal.no_fulfill",
            Refusal::RequestQueryFailed => "clipboard.refusal.query_failed",
            Refusal::UnknownWindow => "window.refusal.unknown",
            Refusal::Port(inner) => inner.name(),
        }
    }

    /// La clé de traduction que le joueur reçoit, quand il en reçoit une.
    ///
    /// Un seul de ces refus parle : `OUT_OF_COLONY` (`TranslationConstants.java:354`),
    /// que `WindowBuildDecoration.java:164` envoie au joueur. Les autres cachent un
    /// bouton ou lèvent une exception, et n'ont donc aucune clé.
    pub fn key(self) -> Option<&'static str> {
        match self {
            Refusal::OutOfColony => Some(super::build::OUT_OF_COLONY_KEY),
            Refusal::Port(inner) => inner.key(),
            _ => None,
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}
