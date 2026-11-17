//! Pourquoi une lecture ou un clic ne rend rien, nomme — et qui le dit au joueur.
//!
//! **Trois des quatre especes de refus de ces fenetres ne parlent pas au joueur**, et
//! c'est le fait qui commande tout ce fichier :
//!
//! - deux sont des **plantages** de Java, qu'aucune garde n'entoure : un module de
//!   reglages qui ne porte pas une des cinq cles leve des la construction de la fenetre
//!   (`WindowSettings.java:29-38`), et un nom de metier vide casse le `substring(0, 1)`
//!   d'une ligne (`WindowStatsPage.java:204`) ;
//! - quatre sont des **silences** : `SettingsModuleView.trigger` sort sans un mot quand
//!   le reglage n'est pas actif (`:120-121`), la liste des habitants absente fait
//!   `return` avant de la remplir (`WindowStatsPage.java:177-180`), et le ramassage de
//!   l'hotel de ville sort sur trois conditions (`PickUpTownhallMessage.java:66`, `:71`,
//!   `:76`) ;
//! - les huit autres sont des **phrases**, levees par le serveur ou par le message qui a
//!   ouvert la fenetre. [`Refusal::key`] les rend, et c'est la seule espece qui a le
//!   droit d'en avoir une : un appelant ne doit jamais imprimer [`Refusal::name`] comme
//!   une phrase, c'est un id de ce port.
//!
//! La quatrieme espece est la faute de **notre** table : un id qu'aucune ligne de
//! [`super::WINDOWS`] ne porte.

/// Pourquoi une lecture ou un clic ne rend rien.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les dix-sept refus de ces quatre fenetres et des deux messages qui les ouvrent
pub enum Refusal {
    /// Un id de fenetre qu'aucune ligne de [`super::WINDOWS`] ne porte.
    UnknownWindow,
    /// Un id de bouton que cette fenetre n'enregistre pas — dont celui de la liste
    /// deroulante (`layoutstats.xml:19`, que personne n'enregistre) et les deux
    /// `trigger` d'une autre ligne que celle demandee.
    UnknownButton,
    /// Un id d'onglet que [`super::TABS`] ne porte pas.
    UnknownTab,
    /// Le module de reglages de la colonie ne porte pas une des cinq cles : Java ne
    /// verifie pas le `getSetting` de `@Nullable` et leve
    /// (`WindowSettings.java:29-38`, `SettingsModuleView.java:90-95`).
    SettingsModuleMissing,
    /// Un nom de metier vide fait `substring(0, 1)` sur rien
    /// (`WindowStatsPage.java:204`) : Java leve, la ou `Component.translatable` d'une
    /// cle absente rend la cle et n'est jamais vide.
    EmptyJobName,
    /// Le reglage existe mais n'est pas actif : le clic sort sans rien faire
    /// (`SettingsModuleView.java:120-121`). **Inatteignable pour les cinq reglages de
    /// l'hotel de ville**, qui sont cinq `BoolSetting` et dont `isActive` rend toujours
    /// vrai (`ISetting.java:164-167`) ; atteignable pour un reglage compose.
    SettingInactive,
    /// La liste des habitants n'est pas dans la fenetre : la population est abandonnee
    /// et les statistiques restent (`WindowStatsPage.java:177-180`).
    NoPopulationList,
    /// Une colonie possede deja le bloc : le ramassage sort sans un mot
    /// (`PickUpTownhallMessage.java:71-75`).
    PickupColonyHere,
    /// Le bloc n'est pas un hotel de ville, ou le joueur est hors de portee
    /// (`PickUpTownhallMessage.java:76-79`).
    PickupWrongBlock,
    /// Une colonie existe deja a cette position : le message le dit au chat et n'ouvre
    /// aucune fenetre (`GetColonyInfoMessage.java:80-84`).
    BlockAlreadyAColony,
    /// Trop pres du spawn du monde (`GetColonyInfoMessage.java:96-99`,
    /// `CreateColonyMessage.java:147-152`).
    TooCloseToSpawn,
    /// Trop loin du spawn du monde (`GetColonyInfoMessage.java:100-103`,
    /// `CreateColonyMessage.java:153-158`).
    TooFarFromSpawn,
    /// Trop pres d'une autre colonie, avant meme d'ouvrir le pacte
    /// (`GetColonyInfoMessage.java:120-123`).
    TooCloseToColony,
    /// Trop pres d'une autre colonie, au moment ou le serveur cree
    /// (`CreateColonyMessage.java:164-168`).
    CreateDeniedTooClose,
    /// Le joueur possede deja une colonie : le serveur refuse de fonder
    /// (`CreateColonyMessage.java:188-198`).
    FoundingFailed,
    /// Le bloc de l'hotel de ville n'a pas d'entite de colonie
    /// (`CreateColonyMessage.java:120-126`).
    NoTownHallEntity,
    /// L'inventaire du joueur est plein : l'hotel de ville reste pose
    /// (`PickUpTownhallMessage.java:82-89`).
    PickupInventoryFull,
}

/// L'espece d'un refus : ce que Java fait vraiment a cet endroit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: quatre especes, et la troisieme est un plantage
pub enum Kind {
    /// Notre table nomme un id qui n'existe pas : la faute est ici.
    Table,
    /// Java verifie et sort sans rien dire.
    Silence,
    /// Java ne verifie pas et leve.
    Crash,
    /// Java le dit au joueur, avec une cle de traduction.
    Said,
}

impl Refusal {
    /// Les dix-sept refus, dans l'ordre ou ce fichier les declare.
    pub const ALL: [Refusal; 17] = [
        Refusal::UnknownWindow,
        Refusal::UnknownButton,
        Refusal::UnknownTab,
        Refusal::SettingsModuleMissing,
        Refusal::EmptyJobName,
        Refusal::SettingInactive,
        Refusal::NoPopulationList,
        Refusal::PickupColonyHere,
        Refusal::PickupWrongBlock,
        Refusal::BlockAlreadyAColony,
        Refusal::TooCloseToSpawn,
        Refusal::TooFarFromSpawn,
        Refusal::TooCloseToColony,
        Refusal::CreateDeniedTooClose,
        Refusal::FoundingFailed,
        Refusal::NoTownHallEntity,
        Refusal::PickupInventoryFull,
    ];

    /// Un nom stable, prefixe comme un id de registre pour ne jamais passer pour une
    /// phrase du jeu.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::UnknownWindow => "ui16.refusal.unknown_window",
            Refusal::UnknownButton => "ui16.refusal.unknown_button",
            Refusal::UnknownTab => "ui16.refusal.unknown_tab",
            Refusal::SettingsModuleMissing => "ui16.refusal.settings_module_missing",
            Refusal::EmptyJobName => "ui16.refusal.empty_job_name",
            Refusal::SettingInactive => "ui16.refusal.setting_inactive",
            Refusal::NoPopulationList => "ui16.refusal.no_population_list",
            Refusal::PickupColonyHere => "ui16.refusal.pickup_colony_here",
            Refusal::PickupWrongBlock => "ui16.refusal.pickup_wrong_block",
            Refusal::BlockAlreadyAColony => "ui16.refusal.block_already_a_colony",
            Refusal::TooCloseToSpawn => "ui16.refusal.too_close_to_spawn",
            Refusal::TooFarFromSpawn => "ui16.refusal.too_far_from_spawn",
            Refusal::TooCloseToColony => "ui16.refusal.too_close_to_colony",
            Refusal::CreateDeniedTooClose => "ui16.refusal.create_denied_too_close",
            Refusal::FoundingFailed => "ui16.refusal.founding_failed",
            Refusal::NoTownHallEntity => "ui16.refusal.no_town_hall_entity",
            Refusal::PickupInventoryFull => "ui16.refusal.pickup_inventory_full",
        }
    }

    /// La cle que Java donne au joueur, quand il en donne une.
    ///
    /// `None` n'est pas « pas de raison » : c'est « Java n'ouvre pas la bouche », ou
    /// « Java leve ». Un appelant qui n'a pas de cle ici n'a rien a afficher.
    pub fn key(self) -> Option<&'static str> {
        match self {
            // GetColonyInfoMessage.java:82
            Refusal::BlockAlreadyAColony => Some("com.minecolonies.coremod.gui.nobuilding"),
            // GetColonyInfoMessage.java:98, CreateColonyMessage.java:150
            Refusal::TooCloseToSpawn => Some("com.minecolonies.core.founding.tooclosetospawn"),
            // GetColonyInfoMessage.java:102, CreateColonyMessage.java:155
            Refusal::TooFarFromSpawn => Some("com.minecolonies.core.founding.toofarfromspawn"),
            // GetColonyInfoMessage.java:123
            Refusal::TooCloseToColony => Some("com.minecolonies.core.founding.tooclosetocolony"),
            // CreateColonyMessage.java:166, TranslationConstants.java:566
            Refusal::CreateDeniedTooClose => {
                Some("com.minecolonies.coremod.gui.colony.denied.tooclose")
            }
            // CreateColonyMessage.java:195, TranslationConstants.java:666
            Refusal::FoundingFailed => Some("com.minecolonies.coremod.gui.colony.create.failed"),
            // CreateColonyMessage.java:122, TranslationConstants.java:660
            Refusal::NoTownHallEntity => {
                Some("com.minecolonies.coremod.gui.colony.create.notileentity")
            }
            // PickUpTownhallMessage.java:88, TranslationConstants.java:574
            Refusal::PickupInventoryFull => Some("com.minecolonies.coremod.playerinvfull"),
            Refusal::UnknownWindow
            | Refusal::UnknownButton
            | Refusal::UnknownTab
            | Refusal::SettingsModuleMissing
            | Refusal::EmptyJobName
            | Refusal::SettingInactive
            | Refusal::NoPopulationList
            | Refusal::PickupColonyHere
            | Refusal::PickupWrongBlock => None,
        }
    }

    /// L'espece du refus.
    pub fn kind(self) -> Kind {
        match self {
            Refusal::UnknownWindow | Refusal::UnknownButton | Refusal::UnknownTab => Kind::Table,
            Refusal::SettingsModuleMissing | Refusal::EmptyJobName => Kind::Crash,
            Refusal::SettingInactive
            | Refusal::NoPopulationList
            | Refusal::PickupColonyHere
            | Refusal::PickupWrongBlock => Kind::Silence,
            Refusal::BlockAlreadyAColony
            | Refusal::TooCloseToSpawn
            | Refusal::TooFarFromSpawn
            | Refusal::TooCloseToColony
            | Refusal::CreateDeniedTooClose
            | Refusal::FoundingFailed
            | Refusal::NoTownHallEntity
            | Refusal::PickupInventoryFull => Kind::Said,
        }
    }

    /// Vrai quand Java ne survivrait pas a cet etat.
    pub fn is_crash(self) -> bool {
        self.kind() == Kind::Crash
    }

    /// Vrai quand le joueur lit quelque chose. Exactement les huit refus qui ont une
    /// cle, ce qu'un test tient.
    pub fn is_said(self) -> bool {
        self.kind() == Kind::Said
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seventeen_refusals_in_four_kinds() {
        let count = |kind: Kind| Refusal::ALL.iter().filter(|refusal| refusal.kind() == kind).count();
        assert_eq!(
            (count(Kind::Table), count(Kind::Crash), count(Kind::Silence), count(Kind::Said)),
            (3, 2, 4, 8),
            "3 fautes de table, 2 plantages, 4 silences, 8 phrases"
        );
        assert_eq!(Refusal::ALL.len(), 17);
        for refusal in Refusal::ALL {
            assert_eq!(refusal.is_crash(), refusal.kind() == Kind::Crash);
            assert_eq!(refusal.is_said(), refusal.kind() == Kind::Said);
        }
        // Les quatre silences, nommes un par un : deux sont dans le code d'une fenetre,
        // deux dans le message de ramassage.
        let silences: Vec<&str> = Refusal::ALL
            .iter()
            .filter(|refusal| refusal.kind() == Kind::Silence)
            .map(|refusal| refusal.name())
            .collect();
        assert_eq!(
            silences,
            [
                "ui16.refusal.setting_inactive",
                "ui16.refusal.no_population_list",
                "ui16.refusal.pickup_colony_here",
                "ui16.refusal.pickup_wrong_block",
            ]
        );
    }

    #[test]
    fn exactly_the_eight_said_refusals_have_a_player_facing_key() {
        for refusal in Refusal::ALL {
            assert_eq!(
                refusal.key().is_some(),
                refusal.is_said(),
                "{} n'est pas d'accord avec son espece",
                refusal.name()
            );
            if let Some(key) = refusal.key() {
                assert!(
                    key.starts_with("com.minecolonies."),
                    "{key} n'est pas une cle de traduction"
                );
            }
        }
        // Les trois refus de ramassage et de fondation sont ceux du serveur : le
        // dernier mot n'est pas a la fenetre.
        assert_eq!(
            Refusal::PickupInventoryFull.key(),
            Some("com.minecolonies.coremod.playerinvfull")
        );
        assert_eq!(
            Refusal::CreateDeniedTooClose.key(),
            Some("com.minecolonies.coremod.gui.colony.denied.tooclose")
        );
        assert_eq!(Refusal::PickupWrongBlock.key(), None);
        assert_eq!(Refusal::EmptyJobName.key(), None);
    }

    #[test]
    fn every_name_is_namespaced_unique_and_prints_itself() {
        for (index, refusal) in Refusal::ALL.iter().enumerate() {
            assert!(
                refusal.name().starts_with("ui16.refusal."),
                "{} n'est pas prefixe",
                refusal.name()
            );
            for other in &Refusal::ALL[index + 1..] {
                assert_ne!(refusal.name(), other.name(), "deux refus, un nom");
            }
        }
        assert_eq!(Refusal::EmptyJobName.to_string(), "ui16.refusal.empty_job_name");
        let as_error: &dyn std::error::Error = &Refusal::SettingsModuleMissing;
        assert_eq!(as_error.to_string(), "ui16.refusal.settings_module_missing");
    }
}
