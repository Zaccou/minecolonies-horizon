//! Pourquoi un clic de l'hotel de ville ne fait rien, et si le joueur est prevenu.
//!
//! Le refus de `src/ui` ([`crate::ui::action::Refusal`]) est celui des huttes :
//! une hutte pas construite, une place prise, un citoyen en route. Aucun de ces
//! six refus n'arrive ici, et les trois refus de l'hotel de ville qui portent un
//! texte n'ont pas de variante la-bas : c'est pourquoi ce module a le sien,
//! comme le journal de quetes a le sien.
//!
//! **Deux especes, et la difference est celle de Java.** Un refus *dit* est un
//! `lang` key que Java affiche en infobulle ou en message
//! (`WindowPermissionsPage.java:326`, `:330`) ; un refus *muet* est un bouton
//! que Java eteint et n'explique pas d'une phrase
//! (`WindowMainPage.java:376`, `WindowPermissionsPage.java:663`). [`Refusal::key`]
//! repond `None` pour les muets, pour qu'un appelant ne puisse pas imprimer un
//! nom technique la ou le joueur attend une phrase.

/// Un clic refuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own closed set
pub enum Refusal {
    /// L'onglet existe dans la barre laterale et sa page n'est pas dans ce
    /// module : `WindowStatsPage`, `WindowSettings`, `WindowAlliancePage`
    /// (`AbstractWindowTownHall.java:40-42`).
    PageOutsideThisSlice { window: &'static str },
    /// La fenetre demandee n'est aucune des sept de la barre laterale.
    UnknownWindowId,
    /// Aucun citoyen selectionne : `recallOneClicked` sort sans rien envoyer
    /// (`WindowCitizenPage.java:162-165`).
    NoCitizenSelected,
    /// Une ligne hors de la liste filtree (`:116`).
    CitizenNotInList { index: usize },
    /// La premiere ligne n'a rien au-dessus : Java n'envoie pas
    /// (`WindowInfoPage.java:201`).
    FirstRowHasNoAbove,
    /// La derniere ligne n'a rien en dessous. Java **lit** `get(i + 1)` sous la
    /// garde `i <= size` (`:206-210`) : le bouton est cache sur la derniere
    /// ligne (`:270-274`), sinon c'est un `IndexOutOfBoundsException`.
    LastRowHasNoBelow,
    /// Le `hiddenId` d'une ligne ne nomme aucun ordre vivant
    /// (`:193`, `:226`).
    UnknownWorkOrder { id: i32 },
    /// La premiere page du `switch` : `prevPage` est eteint et `AbstractWindowSkeleton.java:237-240`
    /// ne sort pas de la page 1 (`endlessScrolling` est faux,
    /// `SwitchView.java:179-180` borne l'index).
    FirstPage,
    /// La derniere page du `switch` : `nextPage` est eteint
    /// (`AbstractWindowSkeleton.java:241-244`).
    LastPage,
    /// Ajouter un joueur demande `EDIT_PERMISSIONS` : Java eteint le champ et
    /// le bouton, et met l'infobulle sur le champ
    /// (`WindowPermissionsPage.java:314-326`).
    MayNotAddPlayer,
    /// Ajouter un rang demande la meme permission, avec **une autre** infobulle
    /// (`:329-331`).
    MayNotAddRank,
    /// Le proprietaire n'est ni retire ni retrograde : le bouton est eteint et
    /// le menu deroulant du rang aussi (`:661-666`), et un retrait est ignore
    /// (`:739-742`).
    OwnerRankImmutable,
    /// L'onglet « gerer les permissions » n'a pas de rang selectionne : Java
    /// part toujours de l'officier (`:117`), donc cet etat n'arrive qu'apres une
    /// suppression de rang.
    NoRankSelected,
    /// Un nom de rang vide est refuse, avec le son d'erreur
    /// (`WindowPermissionsPage.java:250`, `:237`).
    RankNameEmpty,
    /// Un nom de rang deja pris est refuse (`:254-259`).
    RankNameTaken,
    /// Les cinq rangs de Java sont crees avec `isInitial = true`
    /// (`Permissions.java:171`), et `removeRank` n'est allume que pour un rang
    /// qui ne l'est pas (`WindowPermissionsPage.java:416`). Le port n'a que ces
    /// cinq, donc le bouton ne sert jamais.
    StockRankInitial,
    /// Le port n'a pas de rang personnalise : [`crate::colony::permissions::Rank`]
    /// est une enum fermee de cinq, alors que Java en cree a la volee
    /// (`Permissions.java:1054-1076`). Ajouter ou retirer un rang n'a rien ou
    /// s'ecrire.
    CustomRanksUnsupported,
    /// Les bits d'un rang sont **calcules** par [`crate::colony::permissions::Rank::mask`],
    /// la ou Java les stocke et les modifie (`Permissions.java:173-219`,
    /// `:569`). Le bouton `trigger` et le menu « type de rang » ecrivent un
    /// `long` que le port ne garde pas.
    RankBitsDerived,
    /// Le texte de la case libre n'est ni un identifiant de bloc ni trois
    /// coordonnees (`WindowPermissionsPage.java:540-561`).
    UnknownBlockOrPosition,
    /// La case libre demandee n'existe plus dans les deux listes
    /// (`:456-476`).
    UnknownFreeEntry { row: usize },
    /// Le mercenaire n'est pas rappelable : moins de quarante minutes depuis le
    /// dernier appel (`WindowMainPage.java:373-377`, `TICKS_FOURTY_MIN`).
    /// Le temps restant se lit par [`crate::ui_window_15::main_page::cooldown_left`] :
    /// un refus de table ne porte pas de valeur, sans quoi la meme regle aurait
    /// deux ecritures selon l'endroit qui la lit.
    MercenaryCooldown,
    /// Les choix de texture et de nom sont verrouilles : la fonctionnalite
    /// Patreon n'est pas debloquee, ou le joueur n'est pas le proprietaire
    /// (`WindowMainPage.java:259-306`).
    VisualsLocked,
}

impl Refusal {
    /// Un nom stable, espace de noms inclus, pour qu'il ne puisse pas passer
    /// pour une phrase.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::PageOutsideThisSlice { .. } => "hall.refusal.page_not_here",
            Refusal::UnknownWindowId => "hall.refusal.unknown_window",
            Refusal::NoCitizenSelected => "hall.refusal.no_citizen",
            Refusal::CitizenNotInList { .. } => "hall.refusal.not_in_list",
            Refusal::FirstRowHasNoAbove => "hall.refusal.first_row",
            Refusal::LastRowHasNoBelow => "hall.refusal.last_row",
            Refusal::UnknownWorkOrder { .. } => "hall.refusal.unknown_order",
            Refusal::FirstPage => "hall.refusal.first_page",
            Refusal::LastPage => "hall.refusal.last_page",
            Refusal::MayNotAddPlayer => "hall.refusal.may_not_add_player",
            Refusal::MayNotAddRank => "hall.refusal.may_not_add_rank",
            Refusal::OwnerRankImmutable => "hall.refusal.owner",
            Refusal::NoRankSelected => "hall.refusal.no_rank",
            Refusal::RankNameEmpty => "hall.refusal.rank_name_empty",
            Refusal::RankNameTaken => "hall.refusal.rank_name_taken",
            Refusal::StockRankInitial => "hall.refusal.stock_rank",
            Refusal::CustomRanksUnsupported => "hall.refusal.custom_ranks",
            Refusal::RankBitsDerived => "hall.refusal.rank_bits",
            Refusal::UnknownBlockOrPosition => "hall.refusal.not_a_block",
            Refusal::UnknownFreeEntry { .. } => "hall.refusal.free_entry",
            Refusal::MercenaryCooldown => "hall.refusal.mercenary",
            Refusal::VisualsLocked => "hall.refusal.visuals",
        }
    }

    /// La cle de Java, quand Java dit quelque chose au joueur.
    ///
    /// Quatre refus sur dix-neuf : les quinze autres sont des boutons eteints.
    pub fn key(self) -> Option<&'static str> {
        match self {
            Refusal::MayNotAddPlayer => {
                Some("com.minecolonies.coremod.gui.townhall.player_permission_error")
            }
            Refusal::MayNotAddRank => {
                Some("com.minecolonies.core.gui.townhall.rank_permission_error")
            }
            Refusal::VisualsLocked => Some("com.minecolonies.core.townhall.patreon"),
            // The tooltip of the field, not of the button: the same sentence
            // the click is missing.
            Refusal::CustomRanksUnsupported => None,
            Refusal::PageOutsideThisSlice { .. }
            | Refusal::UnknownWindowId
            | Refusal::NoCitizenSelected
            | Refusal::CitizenNotInList { .. }
            | Refusal::FirstRowHasNoAbove
            | Refusal::LastRowHasNoBelow
            | Refusal::UnknownWorkOrder { .. }
            | Refusal::FirstPage
            | Refusal::LastPage
            | Refusal::OwnerRankImmutable
            | Refusal::NoRankSelected
            | Refusal::RankNameEmpty
            | Refusal::RankNameTaken
            | Refusal::StockRankInitial
            | Refusal::RankBitsDerived
            | Refusal::UnknownBlockOrPosition
            | Refusal::UnknownFreeEntry { .. }
            | Refusal::MercenaryCooldown => None,
        }
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

    fn every() -> Vec<Refusal> {
        vec![
            Refusal::PageOutsideThisSlice { window: "WindowStatsPage" },
            Refusal::UnknownWindowId,
            Refusal::NoCitizenSelected,
            Refusal::CitizenNotInList { index: 3 },
            Refusal::FirstRowHasNoAbove,
            Refusal::LastRowHasNoBelow,
            Refusal::UnknownWorkOrder { id: 7 },
            Refusal::FirstPage,
            Refusal::LastPage,
            Refusal::MayNotAddPlayer,
            Refusal::MayNotAddRank,
            Refusal::OwnerRankImmutable,
            Refusal::NoRankSelected,
            Refusal::RankNameEmpty,
            Refusal::RankNameTaken,
            Refusal::StockRankInitial,
            Refusal::CustomRanksUnsupported,
            Refusal::RankBitsDerived,
            Refusal::UnknownBlockOrPosition,
            Refusal::UnknownFreeEntry { row: 2 },
            Refusal::MercenaryCooldown,
            Refusal::VisualsLocked,
        ]
    }

    #[test]
    fn every_refusal_has_its_own_namespaced_name() {
        let all = every();
        assert_eq!(all.len(), 22);
        for (index, refusal) in all.iter().enumerate() {
            assert!(refusal.name().starts_with("hall.refusal."), "{refusal}");
            for other in &all[index + 1..] {
                assert_ne!(refusal.name(), other.name(), "two refusals share a name");
            }
        }
    }

    #[test]
    fn only_three_refusals_are_told_to_the_player() {
        let keyed = every().into_iter().filter(|refusal| refusal.key().is_some()).count();
        assert_eq!(keyed, 3, "the rest are disabled buttons");
        assert_eq!(
            Refusal::MayNotAddPlayer.key(),
            Some("com.minecolonies.coremod.gui.townhall.player_permission_error")
        );
        // Two different sentences for the two fields of the same page.
        assert_ne!(Refusal::MayNotAddPlayer.key(), Refusal::MayNotAddRank.key());
        assert_eq!(Refusal::OwnerRankImmutable.key(), None);
    }
}
