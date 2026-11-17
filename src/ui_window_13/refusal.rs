//! Pourquoi une lecture ou un clic ne rend rien, nomme.
//!
//! **Aucun de ces refus n'a de cle de traduction, et c'est le fait.** Ces fenetres ne
//! disent jamais non au joueur : elles sortent sans un mot (`WindowSelectRequest.java:134`)
//! ou elles **levent** — `questItems.get(-1)` (`WindowQuestLog.java:161`), `split[1]`
//! sur un id qui ne se decoupe pas (`WindowStatsModule.java:167`, `:171`), un modele
//! de quete absent (`WindowQuestLogAvailableQuestModule.java:37`), un intervalle
//! inconnu (`WindowStatsModule.java:110`). [`Refusal::key`] repond donc `None`
//! partout, et [`Refusal::kind`] separe les trois especes : trois ids casses dans
//! *notre* table, un silence de Java, quatre plantages. Une exception n'est pas un
//! refus ; le port la nomme au lieu de la laisser passer, comme `crate::requests`
//! refuse une transition que Java laisse faire.

/// Pourquoi une lecture ou un clic ne rend rien.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les huit refus de ces trois fenetres et d'une page
pub enum Refusal {
    /// Un id de fenetre qu'aucune ligne de [`super::WINDOWS`] ne porte.
    UnknownWindow,
    /// Une page que [`super::quest_log::Page::ALL`] ne porte pas.
    UnknownPage,
    /// Un id de bouton que personne n'enregistre — dont `questLocator` sur la page
    /// « terminees », ou `windowquestlog.xml:46-50` ne dessine pas le panneau.
    UnknownButton,
    /// Ligne hors de la liste fraiche : `WindowSelectRequest.java:134` verifie
    /// `row >= 0 && row < requests.size()` et **sort sans rien rendre**.
    RowOutOfRange,
    /// Ligne inexistante : le journal ne verifie **pas** l'indice
    /// (`WindowQuestLog.java:161`), donc `-1` leve.
    NoRow,
    /// Modele de quete absent du registre client, dont Java lit le nom sans verifier
    /// (`WindowQuestLogAvailableQuestModule.java:35-37`).
    UnknownTemplate,
    /// Id de statistique qui ne se decoupe pas : `"a;"` et `"a;b'"` donnent un
    /// tableau trop court, `"a';b"` un morceau vide (`WindowStatsModule.java:165-177`).
    BrokenStatId,
    /// Cle d'intervalle inconnue : `INTERVAL.get` rend `null` et le `int` qui le
    /// deroule leve (`WindowStatsModule.java:110`, `:157`).
    UnknownInterval,
}

/// L'espece d'un refus : ce que Java fait vraiment a cet endroit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: trois especes, et la troisieme est un plantage
pub enum Kind {
    /// Notre table nomme un id qui n'existe pas : la faute est ici.
    Table,
    /// Java verifie et sort sans rien rendre.
    Silence,
    /// Java ne verifie pas et leve.
    Crash,
}

impl Refusal {
    /// Les huit refus, dans l'ordre ou ce fichier les declare.
    pub const ALL: [Refusal; 8] = [
        Refusal::UnknownWindow,
        Refusal::UnknownPage,
        Refusal::UnknownButton,
        Refusal::RowOutOfRange,
        Refusal::NoRow,
        Refusal::UnknownTemplate,
        Refusal::BrokenStatId,
        Refusal::UnknownInterval,
    ];

    /// Un nom stable, prefixe comme un id de registre pour ne jamais passer pour une
    /// phrase du jeu.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::UnknownWindow => "ui13.refusal.unknown_window",
            Refusal::UnknownPage => "ui13.refusal.unknown_page",
            Refusal::UnknownButton => "ui13.refusal.unknown_button",
            Refusal::RowOutOfRange => "ui13.refusal.row_out_of_range",
            Refusal::NoRow => "ui13.refusal.no_row",
            Refusal::UnknownTemplate => "ui13.refusal.unknown_template",
            Refusal::BrokenStatId => "ui13.refusal.broken_stat_id",
            Refusal::UnknownInterval => "ui13.refusal.unknown_interval",
        }
    }

    /// La cle que Java donne au joueur : **aucune**, pour les huit. Ces fenetres
    /// n'appellent jamais `MessageUtils` : qui ne voit rien ne voit rien parce que
    /// Java ne dit rien.
    pub fn key(self) -> Option<&'static str> {
        None
    }

    /// L'espece du refus.
    pub fn kind(self) -> Kind {
        match self {
            Refusal::UnknownWindow | Refusal::UnknownPage | Refusal::UnknownButton => Kind::Table,
            Refusal::RowOutOfRange => Kind::Silence,
            Refusal::NoRow
            | Refusal::UnknownTemplate
            | Refusal::BrokenStatId
            | Refusal::UnknownInterval => Kind::Crash,
        }
    }

    /// Vrai quand Java ne survivrait pas a cet etat.
    pub fn is_crash(self) -> bool {
        self.kind() == Kind::Crash
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
    fn eight_refusals_in_three_kinds_and_only_one_is_a_silence() {
        let count = |kind: Kind| Refusal::ALL.iter().filter(|refusal| refusal.kind() == kind).count();
        assert_eq!(
            (count(Kind::Table), count(Kind::Silence), count(Kind::Crash)),
            (3, 1, 4)
        );
        // Le seul silence est celui de la fenetre des requetes, qui verifie avant.
        let silences: Vec<&str> = Refusal::ALL
            .iter()
            .filter(|refusal| refusal.kind() == Kind::Silence)
            .map(|refusal| refusal.name())
            .collect();
        assert_eq!(silences, ["ui13.refusal.row_out_of_range"]);
        for refusal in Refusal::ALL {
            assert_eq!(refusal.is_crash(), refusal.kind() == Kind::Crash);
        }
    }

    #[test]
    fn no_refusal_has_a_player_facing_key_and_every_name_is_namespaced() {
        for refusal in Refusal::ALL {
            assert_eq!(refusal.key(), None, "{} parle au joueur", refusal.name());
            assert!(
                refusal.name().starts_with("ui13.refusal."),
                "{} n'est pas prefixe",
                refusal.name()
            );
            assert_eq!(
                Refusal::ALL.iter().filter(|other| other.name() == refusal.name()).count(),
                1,
                "deux refus, un nom"
            );
        }
        assert_eq!(Refusal::NoRow.to_string(), "ui13.refusal.no_row");
        let as_error: &dyn std::error::Error = &Refusal::BrokenStatId;
        assert_eq!(as_error.to_string(), "ui13.refusal.broken_stat_id");
    }
}
