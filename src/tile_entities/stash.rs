//! La cache : un bloc qui réclame un ramassage dès qu'on y dépose quelque chose.
//!
//! Référence Java : `core/tileentities/TileEntityStash.java` (96 lignes).
//! La classe étend `TileEntityColonyBuilding` (`:34`) : elle hérite du
//! rattachement, du schéma et de l'accès, portés une fois dans
//! [`super::building`]. Ce fichier ne porte que sa différence, l'inventaire qui
//! **prévient**.
//!
//! **Ce que fait un dépôt.** `NotifyingRackInventory.onContentsChanged`
//! (`:57-75`) ne fait rien de moins que quatre tests en cascade : le monde est
//! un serveur, la position est dans une colonie (`:61`), le contenu n'est pas
//! vide, et le bâtiment existe ; puis `createPickupRequest(STACKSIZE, true)`
//! (`:68`), qui **crée au plus une requête par bâtiment** (le commentaire de
//! Java le dit à `:67`). Si une requête est créée, un message part aux joueurs à
//! six blocs (`:69-73`).
//!
//! **Le port sépare la décision de l'effet.** Créer une requête demande le
//! gestionnaire de requêtes de la colonie et l'envoi d'un message : le port
//! nomme donc ce qui est dû, et l'appelant le fait. Un test couvre les quatre
//! refus sans jeu.

/// L'identifiant d'enregistrement (`TileEntityInitializer.java:42`).
pub const TYPE_ID: &str = "minecolonies:stash";

/// La taille demandée par la requête de ramassage : `Constants.STACKSIZE`
/// (`:68`, `Constants.java:23`).
pub const PICKUP_SIZE: u32 = 64;
/// Le rayon du message, en blocs (`sendToClose(..., 6, ...)`, `:69`).
pub const NOTIFY_RADIUS: u32 = 6;
/// La clé du message qui prévient les joueurs proches (`:69`,
/// `api/util/constant/TranslationConstants.java:43`).
pub const MESSAGE_FORCE_PICKUP: &str = "entity.deliveryman.forcepickup";

/// Pourquoi un dépôt n'a rien déclenché.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les quatre tests de `onContentsChanged`, dans l'ordre de Java
pub enum Ignored {
    /// Un retrait, pas un dépôt : Java ne teste que le contenu **non** vide
    /// (`:68`).
    Empty,
    /// La position n'est dans aucune colonie (`:61`).
    OutsideColony,
    /// Aucun bâtiment à cette position : le bloc n'est rattaché à rien
    /// (`:63-65`).
    NoBuilding,
    /// Une requête de ramassage est déjà ouverte pour ce bâtiment
    /// (`createPickupRequest` rend faux, `:68`).
    AlreadyAsked,
}

impl Ignored {
    pub fn name(self) -> &'static str {
        match self {
            Ignored::Empty => "tile_entities.stash.empty",
            Ignored::OutsideColony => "tile_entities.stash.outside_colony",
            Ignored::NoBuilding => "tile_entities.stash.no_building",
            Ignored::AlreadyAsked => "tile_entities.stash.already_asked",
        }
    }
}

/// Ce qu'un dépôt déclenche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : une requête et un message, ou rien
pub struct Pickup {
    /// La taille demandée (`:68`).
    pub size: u32,
    /// Le rayon du message (`:69`).
    pub radius: u32,
}

/// La règle de la cache, sans jeu : `in_colony` est
/// `isCoordinateInAnyColony` (`:61`), `has_building` est le bâtiment trouvé à la
/// position (`:65`), et `request_created` est ce que
/// `building.createPickupRequest` répond (`:68`).
pub fn on_contents_changed(
    non_empty: bool,
    in_colony: bool,
    has_building: bool,
    request_created: bool,
) -> Result<Pickup, Ignored> {
    if !non_empty {
        return Err(Ignored::Empty);
    }
    if !in_colony {
        return Err(Ignored::OutsideColony);
    }
    if !has_building {
        return Err(Ignored::NoBuilding);
    }
    if !request_created {
        return Err(Ignored::AlreadyAsked);
    }
    Ok(Pickup {
        size: PICKUP_SIZE,
        radius: NOTIFY_RADIUS,
    })
}

/// La cache, vue comme une hutte de bâtiment qui prévient.
///
/// Elle n'a **aucun champ sauvé à elle** : Java ne sauve que ceux de
/// `TileEntityColonyBuilding` et l'inventaire du rack. Le type existe pour que
/// la table des 14 ait une entrée nommée, et pour porter [`Stash::on_change`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustif : aucun champ, comme la classe Java
pub struct Stash;

impl Stash {
    pub fn new() -> Stash {
        Stash
    }

    /// Le dépôt, avec les quatre entrées que la règle demande.
    pub fn on_change(
        &self,
        non_empty: bool,
        in_colony: bool,
        has_building: bool,
        request_created: bool,
    ) -> Result<Pickup, Ignored> {
        on_contents_changed(non_empty, in_colony, has_building, request_created)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_depot_dans_une_colonie_avec_un_batiment_reclame_un_ramassage() {
        let stash = Stash::new();
        assert_eq!(
            stash.on_change(true, true, true, true),
            Ok(Pickup {
                size: 64,
                radius: 6
            })
        );
        assert_eq!(PICKUP_SIZE, 64, "Constants.STACKSIZE");
    }

    #[test]
    fn les_quatre_refus_sont_nommes_et_dans_l_ordre_de_java() {
        // Vider la cache n'est pas un dépôt : c'est le premier test de Java, et il
        // passe avant l'appartenance à la colonie.
        assert_eq!(
            on_contents_changed(false, false, false, false),
            Err(Ignored::Empty)
        );
        assert_eq!(
            on_contents_changed(true, false, false, false),
            Err(Ignored::OutsideColony)
        );
        assert_eq!(
            on_contents_changed(true, true, false, false),
            Err(Ignored::NoBuilding)
        );
        assert_eq!(
            on_contents_changed(true, true, true, false),
            Err(Ignored::AlreadyAsked)
        );
        let mut names = std::collections::BTreeSet::new();
        for ignored in [
            Ignored::Empty,
            Ignored::OutsideColony,
            Ignored::NoBuilding,
            Ignored::AlreadyAsked,
        ] {
            assert!(names.insert(ignored.name()));
        }
        assert_eq!(names.len(), 4, "quatre refus, quatre noms");
    }

    #[test]
    fn une_seule_requete_par_batiment_puis_le_silence() {
        // `createPickupRequest` rend vrai la première fois, faux ensuite : la
        // cache ne redemande pas tant que la requête est ouverte.
        assert!(on_contents_changed(true, true, true, true).is_ok());
        assert_eq!(
            on_contents_changed(true, true, true, false),
            Err(Ignored::AlreadyAsked)
        );
    }

    #[test]
    fn le_message_de_ramassage_a_sa_propre_cle() {
        // La valeur Java, recopiée de `api/util/constant/TranslationConstants.java:43` :
        // `COM_MINECOLONIES_COREMOD_ENTITY_DELIVERYMAN_FORCEPICKUP` vaut
        // `entity.deliveryman.forcepickup`, **sans** le préfixe
        // `com.minecolonies.coremod.` que portent les clés d'entrepôt
        // (`:57-61`). La constante s'appelle « coremod » mais sa valeur ne l'est
        // pas : c'est la valeur qui compte, c'est elle que le fichier `lang`
        // doit porter.
        assert_eq!(MESSAGE_FORCE_PICKUP, "entity.deliveryman.forcepickup");
        assert_ne!(
            MESSAGE_FORCE_PICKUP,
            "com.minecolonies.coremod.entity.deliveryman.forcepickup"
        );
        // Le message d'échec est une clé distincte (`:45`), que ce module
        // n'émet pas : il ne sert pas à la cache.
        assert_ne!(MESSAGE_FORCE_PICKUP, "entity.deliveryman.forcepickupfailed");
    }
}
