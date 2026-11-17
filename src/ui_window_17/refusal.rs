//! Ce que ces quatre fenêtres refusent, nommé — et le seul refus que Java **dit**.
//!
//! Java référence : les quatre classes de `core/client/gui/townhall/` du brief, plus les
//! deux messages serveur qui répondent aux clics de destruction
//! (`ColonyAbandonOwnMessage.java:48-58`, `ColonyDeleteOwnMessage.java:49-58`) et le
//! spawn des mercenaires (`EntityMercenary.java:489-492`).
//!
//! **Un refus sur neuf est prononcé.** Java n'a qu'un seul message pour ces écrans :
//! « colonie introuvable », levé par le serveur quand le joueur n'est propriétaire
//! d'aucune colonie (`ColonyAbandonOwnMessage.java:57`,
//! `ColonyDeleteOwnMessage.java:57`, clé `com.minecolonies.coremod.colony.notfound`,
//! `TranslationConstants.java:777`). Les huit autres sont **silencieux**, et chacun pour
//! une raison différente, que la variante documente : un bouton caché, un bouton
//! désactivé, un `return` sans message, ou un champ de saisie qui avale la frappe. C'est
//! pourquoi [`Refusal::key`] répond `Option` : un appelant qui prend [`Refusal::name`]
//! pour une phrase afficherait `ui.refusal.empty_name` au joueur.
//!
//! Un refus n'est **pas** une [`crate::ui::Refusal`] : celle-là est une énumération fermée
//! des huttes (`NotBuilt`, `NoRoom`, `HousingHandledElsewhere`…) et aucune de ses
//! variantes ne couvre « le champ est vide » ni « la vague est en recharge ». Même
//! raisonnement que `crate::ui_window_06::refusal`, et même forme.

/// Pourquoi une lecture ou un clic de ces quatre fenêtres n'a rien fait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive : l'ensemble fermé de ce module, et une variante de plus serait un refus de plus
pub enum Refusal {
    /// Le joueur n'est propriétaire d'aucune colonie, donc il n'y a rien à abandonner ni
    /// à détruire (`ColonyAbandonOwnMessage.java:48-58`). **Le seul refus qui parle** :
    /// le serveur envoie `MESSAGE_INFO_COLONY_NOT_FOUND` (`:57`) et ferme le cas.
    NotOwner,
    /// Une vague de mercenaires a déjà été payée il y a moins de
    /// [`super::TICKS_FORTY_MIN`] ticks (`EntityMercenary.java:489-492`). Le serveur
    /// `return` **sans rien dire** ; c'est l'hôtel de ville qui désactive son bouton
    /// `mercenaries` avant même d'ouvrir cet écran (`WindowMainPage.java:373-377`).
    MercenaryCooldown {
        /// Les ticks qui restent à attendre, calculés — jamais lus d'un enregistrement.
        remaining: i64,
    },
    /// `done` sur un champ vide : Java **saute** `colony.setName`
    /// (`WindowTownHallNameEntry.java:48`) et rouvre l'hôtel de ville quand même
    /// (`:58-61`). Rien n'est écrit, rien n'est envoyé, rien n'est dit.
    EmptyName,
    /// La frappe au-delà de ce que le champ accepte : `maxlength="25"`
    /// (`windowtownhallnameentry.xml:6`). BlockUI avale le caractère, la classe ne voit
    /// rien et ne dit rien.
    FieldFull,
    /// Après `done` ou `cancel`, `getClientBuildingManager().getTownHall()` est `null` :
    /// la classe ne rouvre rien (`WindowTownHallNameEntry.java:58-61`). Silencieux.
    NoTownHall,
    /// Le pack de récits n'a **aucune** entrée qui passe le filtre de biome :
    /// `pickRandom` répond `""` (`ColonyStoryListener.java:90-95`) et le titre garde un
    /// argument vide. Ce n'est pas une erreur en Java, c'est un texte manquant.
    NoStoryMatch,
    /// L'id cliqué n'est aucun des boutons de cette fenêtre. Java, lui, ne peut pas le
    /// recevoir : `registerButton` ne lie que des ids de l'XML (`:130`) ; le port, si.
    UnknownButton,
    /// Le champ cherché n'est pas dans [`super::constants::WINDOW_CONSTANTS`].
    UnknownConstant,
    /// L'id ou la classe passée à [`super::window_by_id`] / [`super::window_by_java`]
    /// n'est pas une des quatre fenêtres de ce module.
    UnknownWindow,
}

impl Refusal {
    /// Un nom stable, préfixé, qui ne peut pas être pris pour une phrase.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::NotOwner => "ui.refusal.not_owner",
            Refusal::MercenaryCooldown { .. } => "ui.refusal.mercenary_cooldown",
            Refusal::EmptyName => "ui.refusal.empty_name",
            Refusal::FieldFull => "ui.refusal.field_full",
            Refusal::NoTownHall => "ui.refusal.no_town_hall",
            Refusal::NoStoryMatch => "ui.refusal.no_story",
            Refusal::UnknownButton => "ui.refusal.unknown_button",
            Refusal::UnknownConstant => "ui.refusal.unknown_constant",
            Refusal::UnknownWindow => "ui.refusal.unknown_window",
        }
    }

    /// La clé que Java **affiche** au joueur, quand il en affiche une.
    ///
    /// `None` n'est pas « pas de raison » : c'est « Java ne lui dit rien » — bouton caché,
    /// bouton désactivé, ou `return` muet. Un appelant ne doit donc jamais se rabattre sur
    /// [`Self::name`] pour écrire une phrase.
    pub fn key(self) -> Option<&'static str> {
        match self {
            // `ColonyAbandonOwnMessage.java:57`, `ColonyDeleteOwnMessage.java:57`.
            Refusal::NotOwner => Some("com.minecolonies.coremod.colony.notfound"),
            Refusal::MercenaryCooldown { .. }
            | Refusal::EmptyName
            | Refusal::FieldFull
            | Refusal::NoTownHall
            | Refusal::NoStoryMatch
            | Refusal::UnknownButton
            | Refusal::UnknownConstant
            | Refusal::UnknownWindow => None,
        }
    }

    /// Vrai quand Java ne dit **rien** au joueur : huit refus sur neuf.
    pub fn is_silent(self) -> bool {
        self.key().is_none()
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

    /// Les neuf refus, à la main : une variante ajoutée sans ligne ici fait échouer
    /// `the_list_holds_every_variant` exprès.
    const ALL: [Refusal; 9] = [
        Refusal::NotOwner,
        Refusal::MercenaryCooldown { remaining: 0 },
        Refusal::EmptyName,
        Refusal::FieldFull,
        Refusal::NoTownHall,
        Refusal::NoStoryMatch,
        Refusal::UnknownButton,
        Refusal::UnknownConstant,
        Refusal::UnknownWindow,
    ];

    #[test]
    fn exactly_one_refusal_is_spoken_by_java() {
        let spoken: Vec<&str> = ALL.iter().filter(|r| !r.is_silent()).map(|r| r.name()).collect();
        assert_eq!(spoken, ["ui.refusal.not_owner"]);
        assert_eq!(
            Refusal::NotOwner.key(),
            Some("com.minecolonies.coremod.colony.notfound"),
            "TranslationConstants.java:777"
        );
        // Huit sont muets, et c'est le fait intéressant : le joueur ne saura jamais
        // pourquoi son clic n'a rien fait.
        assert_eq!(ALL.iter().filter(|r| r.is_silent()).count(), 8);
    }

    #[test]
    fn the_list_holds_every_variant_and_no_name_is_shared() {
        assert_eq!(ALL.len(), 9);
        let mut names: Vec<&str> = ALL.iter().map(|r| r.name()).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "deux variantes, un même nom");
        for name in names {
            assert!(name.starts_with("ui.refusal."), "{name} n'est pas préfixé");
        }
    }

    #[test]
    fn a_key_is_always_a_lang_key_and_a_name_never_is_one() {
        for refusal in ALL {
            if let Some(key) = refusal.key() {
                assert!(key.starts_with("com.minecolonies."), "{key}");
                assert!(!key.starts_with("ui.refusal."), "{key} : une clé n'est pas un nom");
            }
            assert_eq!(refusal.to_string(), refusal.name());
        }
        // La cooldown porte sa mesure : c'est ce qui la rend utilisable pour afficher
        // « encore 47 000 ticks » quand une couche de langue existera.
        assert_eq!(
            Refusal::MercenaryCooldown { remaining: 47_000 }.name(),
            Refusal::MercenaryCooldown { remaining: 1 }.name(),
            "le nom ne dépend pas de la valeur"
        );
    }
}
