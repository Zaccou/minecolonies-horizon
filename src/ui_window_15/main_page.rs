//! La page d'accueil : le mercenaire, les trois menus de style, et le verrou.
//!
//! Java reference : `core/client/gui/townhall/WindowMainPage.java` (409 lignes),
//! `gui/townhall/layoutactions.xml`, `Constants.java:34` pour la recharge du
//! mercenaire.
//!
//! **Le verrou n'est pas une permission, c'est un appel HTTPS.**
//! `checkFeatureUnlock` ouvre une connexion a
//! `https://auth.minecolonies.com/api/minecraft/<uuid>/features` dans un thread
//! et range la reponse dans un `static` (`WindowMainPage.java:312-349`). Le port
//! n'a ni client, ni reseau, ni `static` de session : la valeur arrive donc en
//! parametre de [`visuals`], et c'est le seul endroit de la page ou l'appelant
//! doit fournir un booleen qu'aucune donnee du monde ne contient (`MANQUANT.md`
//! §1).

use super::refusal::Refusal;

/// Quarante minutes en ticks : `TICKS_SECOND * SECONDS_A_MINUTE * 40`
/// (`Constants.java:34`), soit `20 * 60 * 40`.
pub const TICKS_FORTY_MIN: i64 = 48_000;

/// L'index du pack de texture que `resettexture` renvoie
/// (`WindowMainPage.java:249` : `TEXTURE_PACKS.get(0)`).
pub const PACK_RESET_INDEX: usize = 0;

/// Les quatre cles d'infobulle du verrou (`:285-303`).
pub const PATREON_KEY: &str = "com.minecolonies.core.townhall.patreon";
pub const TEXTURES_TIP_KEY: &str = "com.minecolonies.core.townhall.patreon.textures";
pub const NAMES_TIP_KEY: &str = "com.minecolonies.core.townhall.patreon.names";
pub const NEEDS_OWNER_KEY: &str = "com.minecolonies.core.townhall.patreon.needs_owner";

/// Les ticks qui restent avant que le mercenaire soit rappelable.
pub fn cooldown_left(use_time: i64, now: i64) -> i64 {
    (use_time + TICKS_FORTY_MIN - now).max(0)
}

/// Le bouton `mercenaries` peut-il servir (`WindowMainPage.java:373-377`) ?
///
/// Java ne teste la recharge que si `useTime != 0` : une colonie qui n'a jamais
/// appele de mercenaire n'a pas d'horodatage, et le bouton reste allume.
pub fn mercenary_ready(use_time: i64, now: i64) -> Result<(), Refusal> {
    if use_time == 0 {
        return Ok(());
    }
    if now - use_time < TICKS_FORTY_MIN {
        return Err(Refusal::MercenaryCooldown);
    }
    Ok(())
}

/// L'etat des cinq controles de style.
///
/// Java les allume et les eteint en deux branches (`:259-306`) : la
/// fonctionnalite debloquee **et** le proprietaire, ou tout le reste. Le
/// proprietaire est celui de la colonie (`getPermissions().getOwner()`), pas
/// l'operateur de la page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Visuals {
    /// Le bouton Patreon, montre quand le joueur n'a pas acces aux menus.
    pub patreon: bool,
    /// Le menu des textures est visible (il cede sa place a `reset`).
    pub textures_shown: bool,
    pub textures_enabled: bool,
    pub names_enabled: bool,
    /// Le bouton qui remet le pack par defaut.
    pub reset: bool,
    /// La premiere ligne de l'infobulle du menu des textures.
    pub textures_tip: Option<&'static str>,
    /// Celle du menu des noms — **deux textes differents**, pas un.
    pub names_tip: Option<&'static str>,
    /// La ligne que les deux infobulles ajoutent quand le joueur n'est pas le
    /// proprietaire (`:297-303`).
    pub needs_owner: bool,
}

/// L'etat des controles de style (`:259-306`).
pub fn visuals(unlocked: bool, owner: bool, texture_is_default: bool) -> Visuals {
    if unlocked && owner {
        return Visuals {
            patreon: false,
            textures_shown: true,
            textures_enabled: true,
            names_enabled: true,
            reset: false,
            textures_tip: None,
            names_tip: None,
            needs_owner: false,
        };
    }
    Visuals {
        patreon: true,
        // Java cache le menu quand un pack non standard est actif, pour laisser
        // la place au bouton qui y revient (`:274-282`).
        textures_shown: texture_is_default,
        textures_enabled: false,
        names_enabled: false,
        reset: !texture_is_default,
        textures_tip: Some(TEXTURES_TIP_KEY),
        names_tip: Some(NAMES_TIP_KEY),
        needs_owner: unlocked && !owner,
    }
}

/// Les trois menus de la page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three dropdowns, three handlers
pub enum Picker {
    /// La couleur de l'equipe.
    Color,
    /// Le pack de textures.
    Texture,
    /// Le pack de noms.
    NameStyle,
}

/// Un changement de menu envoie-t-il un message ?
///
/// **Deux regles, pas une.** Les menus de texture et de nom ne parlent que si
/// l'indice a bouge (`:204`, `:217`) ; celui de la couleur envoie a chaque
/// changement, sans comparer (`:228-231`) — c'est pourquoi choisir la couleur
/// deja active renvoie quand meme un `TeamColonyColorChangeMessage`.
pub fn picker_sends(picker: Picker, initial: usize, chosen: usize) -> bool {
    match picker {
        Picker::Color => true,
        Picker::Texture | Picker::NameStyle => chosen != initial,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mercenary_waits_forty_minutes_and_a_colony_that_never_called_one_waits_nothing() {
        assert_eq!(mercenary_ready(0, 1_000_000), Ok(()), "useTime 0 means never used");
        assert_eq!(mercenary_ready(100, 100), Err(Refusal::MercenaryCooldown));
        assert_eq!(mercenary_ready(100, 100 + TICKS_FORTY_MIN - 1), Err(Refusal::MercenaryCooldown));
        assert_eq!(mercenary_ready(100, 100 + TICKS_FORTY_MIN), Ok(()), "the bound is inclusive");
        assert_eq!(cooldown_left(100, 100), TICKS_FORTY_MIN);
        assert_eq!(cooldown_left(100, 100 + TICKS_FORTY_MIN), 0);
        assert_eq!(cooldown_left(100, 10 * TICKS_FORTY_MIN), 0, "never negative");
        assert_eq!(TICKS_FORTY_MIN, 20 * 60 * 40);
    }

    #[test]
    fn the_owner_with_the_unlock_gets_both_menus_and_loses_the_patreon_button() {
        let open = visuals(true, true, false);
        assert!(!open.patreon);
        assert!(open.textures_enabled);
        assert!(open.names_enabled);
        assert!(open.textures_shown);
        assert!(!open.reset, "the reset button is hidden when both menus work");
        assert_eq!(open.textures_tip, None);
        assert_eq!(open.names_tip, None);
        assert!(!open.needs_owner);
    }

    #[test]
    fn without_the_unlock_both_menus_are_dead_and_a_custom_texture_hands_its_place_to_reset() {
        // Third argument is `texture_is_default`: false = a custom pack is on.
        let locked = visuals(false, true, false);
        assert!(locked.patreon);
        assert!(!locked.textures_enabled);
        assert!(!locked.names_enabled);
        assert_eq!(locked.textures_tip, Some(TEXTURES_TIP_KEY));
        assert_eq!(locked.names_tip, Some(NAMES_TIP_KEY));
        assert_ne!(locked.textures_tip, locked.names_tip, "two panes, two tooltips");
        assert!(!locked.needs_owner, "the owner needs no extra line");
        // A custom pack is active: the menu goes away and the reset appears
        // (`WindowMainPage.java:274-278`, `if (!...equals("default"))`).
        assert!(!locked.textures_shown);
        assert!(locked.reset);
        // The default pack: the menu stays and there is nothing to reset
        // (`:279-282`).
        let default = visuals(false, true, true);
        assert!(default.textures_shown);
        assert!(!default.reset);
    }

    #[test]
    fn an_unlocked_player_who_is_not_the_owner_is_told_so() {
        let other = visuals(true, false, true);
        assert!(other.needs_owner, "WindowMainPage.java:297-303");
        assert!(other.patreon);
        assert!(!other.textures_enabled);
        assert_eq!(other.textures_tip, Some(TEXTURES_TIP_KEY));
        // The two menus of the page talk differently to the server.
        assert!(picker_sends(Picker::Color, 3, 3), "the colour always sends");
        assert!(!picker_sends(Picker::Texture, 3, 3));
        assert!(picker_sends(Picker::Texture, 3, 1));
        assert!(!picker_sends(Picker::NameStyle, 0, 0));
        assert_eq!(PACK_RESET_INDEX, 0);
        assert!(PATREON_KEY.starts_with("com.minecolonies.core.townhall."));
    }
}
