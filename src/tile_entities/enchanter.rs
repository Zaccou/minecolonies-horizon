//! L'enchanter : le livre qui s'ouvre quand un joueur approche. Côté client.
//!
//! Référence Java : `core/tileentities/TileEntityEnchanter.java` (133 lignes).
//! La classe étend `TileEntityColonyBuilding` (`:17`) : le côté serveur, c'est
//! la hutte de [`super::building`], et `tick()` lui délègue d'abord
//! (`super.tick()`, `:52`). Ce fichier ne porte que l'animation, qui ne tourne
//! que sur le client (`:55-58` : `if (!level.isClientSide) return;`).
//!
//! **Deux calculs de Java ne font rien, et c'est reproduit.** L'expression
//! `(this.bookRotation + Math.PI % (2 * Math.PI)) - Math.PI`
//! (`:86`) se lit `(x + π) - π` : `%` passe **avant** `+`, donc `π % 2π = π`.
//! Idem pour `circleBasedRot` (`:102`). Le port garde la fonction
//! [`java_normalise`] pour que le fait soit testé au lieu d'être commenté, puis
//! écrit la ligne qui reste : `bookRotation += (tRot - bookRotation) × 0,4`.
//!
//! **Rien n'est sauvé.** Java ne sauve aucun de ces flottants : une animation
//! repart de zéro à chaque chargement du monde, et le port ne sauve rien non
//! plus.

/// L'identifiant d'enregistrement (`TileEntityInitializer.java:40`).
pub const TYPE_ID: &str = "minecolonies:enchanter";

/// La distance à laquelle Java cherche un joueur, en blocs (`:62`).
pub const PLAYER_RANGE: f64 = 3.0;
/// L'incrément d'ouverture par tick (`:68`, `:83`).
pub const SPREAD_STEP: f32 = 0.1;
/// La rotation du livre au repos (`:82`).
pub const IDLE_SPIN: f32 = 0.02;
/// La vitesse à laquelle le livre rattrape son angle (`:104`).
pub const ROTATION_EASING: f32 = 0.4;
/// La probabilité sur 40 qu'une page se tourne même livre ouvert (`:69`).
pub const FLIP_CHANCE: i32 = 40;
/// Le pas maximal d'une page par tick (`:109`).
pub const PAGE_STEP: f32 = 0.2;
/// La vitesse du ressort de page (`:110`).
pub const PAGE_EASING: f32 = 0.9;
/// Le nombre d'itérations après lequel le port renonce à attendre que le tirage
/// change (`:73-77`). Java n'a pas de garde parce qu'un `Random` réel finit
/// toujours par changer ; une fermeture de test peut ne pas le faire, et une
/// boucle infinie dans une block entity est pire qu'un angle figé.
pub const FLIP_ATTEMPTS: u32 = 32;

/// La normalisation d'angle de Java, **telle qu'elle est écrite** (`:86`, `:102`) :
/// `(value + π % 2π) - π`. Comme `π % 2π == π`, c'est l'identité. Conservée
/// nommée pour que ce soit un test et non une note.
pub fn java_normalise(value: f32) -> f32 {
    // Java calcule en `double` (`Math.PI` en est un) puis affecte à un `float`
    // (`:86`) : le port fait la même chose, sinon les arrondis diffèrent.
    let pi = std::f64::consts::PI;
    ((f64::from(value) + pi % (2.0 * pi)) - pi) as f32
}

/// L'état de l'animation. Les dix champs de Java (`:19-28`), dans son ordre.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
// exhaustif : les champs publics de la classe Java
pub struct Book {
    pub tick_count: u32,
    pub page_flip: f32,
    pub page_flip_prev: f32,
    pub flip_t: f32,
    pub flip_a: f32,
    pub book_spread: f32,
    pub book_spread_prev: f32,
    pub book_rotation: f32,
    pub book_rotation_prev: f32,
    pub t_rot: f32,
}

impl Book {
    pub fn new() -> Book {
        Book::default()
    }

    /// Un tick d'animation (`:59-112`).
    ///
    /// `player_angle` est `atan2(dz, dx)` du joueur le plus proche du centre du
    /// bloc (`:63-67`), `None` quand personne n'est à moins de trois blocs. Le
    /// calcul de la distance et la recherche du joueur restent à l'appelant : la
    /// règle ici est l'animation, pas le monde.
    ///
    /// `rng` rend un entier de `0..bound`, comme `Random.nextInt`.
    pub fn tick(&mut self, player_angle: Option<f32>, rng: &mut dyn FnMut(i32) -> i32) {
        self.book_spread_prev = self.book_spread;
        self.book_rotation_prev = self.book_rotation;

        match player_angle {
            Some(angle) => {
                self.t_rot = angle;
                self.book_spread += SPREAD_STEP;
                if self.book_spread < 0.5 || rng(FLIP_CHANCE) == 0 {
                    let before = self.flip_t;
                    let mut attempts = 0;
                    loop {
                        self.flip_t += (rng(4) - rng(4)) as f32;
                        attempts += 1;
                        if self.flip_t != before || attempts >= FLIP_ATTEMPTS {
                            break;
                        }
                    }
                }
            }
            None => {
                self.t_rot += IDLE_SPIN;
                self.book_spread -= SPREAD_STEP;
            }
        }

        // `:86` : la normalisation de Java ne fait rien (voir `java_normalise`).
        self.book_rotation = java_normalise(self.book_rotation);
        while self.book_rotation < -std::f32::consts::PI {
            self.book_rotation += 2.0 * std::f32::consts::PI;
        }
        while self.t_rot >= std::f32::consts::PI {
            self.t_rot -= 2.0 * std::f32::consts::PI;
        }
        while self.t_rot < -std::f32::consts::PI {
            self.t_rot += 2.0 * std::f32::consts::PI;
        }

        // `:102` : `circleBasedRot` est `t_rot - book_rotation`, pour la même
        // raison de précédence.
        let circle_based = self.t_rot - self.book_rotation;
        self.book_rotation += circle_based * ROTATION_EASING;
        self.book_spread = self.book_spread.clamp(0.0, 1.0);
        self.tick_count = self.tick_count.saturating_add(1);

        self.page_flip_prev = self.page_flip;
        let acceleration = ((self.flip_t - self.page_flip) * 0.4).clamp(-PAGE_STEP, PAGE_STEP);
        self.flip_a += (acceleration - self.flip_a) * PAGE_EASING;
        self.page_flip += self.flip_a;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un tirage qui ne change jamais : la garde de [`FLIP_ATTEMPTS`] est ce qui
    /// empêche la boucle de tourner sans fin.
    fn stuck() -> impl FnMut(i32) -> i32 {
        |_| 0
    }

    #[test]
    fn un_livre_neuf_est_ferme_et_immobile() {
        let book = Book::new();
        assert_eq!(book.book_spread, 0.0);
        assert_eq!(book.book_rotation, 0.0);
        assert_eq!(book.tick_count, 0);
        assert_eq!(book, Book::default());
    }

    #[test]
    fn sans_joueur_le_livre_tourne_lentement_et_se_referme() {
        let mut book = Book::new();
        book.book_spread = 0.3;
        let mut rng = stuck();
        book.tick(None, &mut rng);
        assert_eq!(book.t_rot, 0.02);
        assert!((book.book_spread - 0.2).abs() < 1e-6);
        assert_eq!(book.book_spread_prev, 0.3, "l'ancienne valeur est gardée");
        assert_eq!(book.tick_count, 1);
        // Fermé jusqu'au bout : la pince de Java empêche le négatif.
        for _ in 0..20 {
            book.tick(None, &mut rng);
        }
        assert_eq!(book.book_spread, 0.0);
    }

    #[test]
    fn un_joueur_ouvre_le_livre_et_le_livre_suit_son_angle() {
        let mut book = Book::new();
        let mut rng = stuck();
        book.tick(Some(1.0), &mut rng);
        assert_eq!(book.t_rot, 1.0);
        assert!((book.book_spread - 0.1).abs() < 1e-6);
        // `bookRotation += (tRot - bookRotation) * 0,4` : 40 % du chemin.
        assert!((book.book_rotation - 0.4).abs() < 1e-6);
        book.tick(Some(1.0), &mut rng);
        assert!((book.book_rotation - 0.64).abs() < 1e-5, "0,4 + 0,6 × 0,4");
        // L'ouverture est plafonnée à 1.
        for _ in 0..20 {
            book.tick(Some(1.0), &mut rng);
        }
        assert_eq!(book.book_spread, 1.0);
    }

    #[test]
    fn la_normalisation_d_angle_de_java_est_l_identite() {
        // `Math.PI % (2 * Math.PI)` vaut π : Java ajoute puis retranche, et le
        // résultat est la valeur d'entrée — y compris hors de [-π, π). L'écart
        // qui reste est celui de l'arrondi flottant, pas d'une règle.
        for value in [0.0f32, 0.3, -0.3, 3.1, -3.1, 7.5, -7.5] {
            assert!(
                (java_normalise(value) - value).abs() < 1e-6,
                "valeur {value} : {}",
                java_normalise(value)
            );
        }
    }

    #[test]
    fn l_angle_de_repos_se_ramene_dans_le_tour_complet() {
        let mut book = Book::new();
        book.t_rot = std::f32::consts::PI - 0.01;
        let mut rng = stuck();
        book.tick(None, &mut rng);
        // 0,02 au-dessus de π : ramené de 2π, donc juste sous -π + 0,01.
        assert!((book.t_rot - (0.01 - std::f32::consts::PI)).abs() < 1e-4);
        assert!(book.t_rot < 0.0);
    }

    #[test]
    fn une_page_se_tourne_quand_le_livre_est_ferme_et_la_garde_borne_le_tirage() {
        let mut book = Book::new();
        // Livre fermé : la condition `bookSpread < 0,5` est vraie, donc le tirage
        // est appelé — un tirage figé ne bloque pas la boucle.
        let mut rng = stuck();
        book.tick(Some(0.0), &mut rng);
        assert_eq!(book.flip_t, 0.0, "un tirage nul ne fait pas bouger la page");
        assert_eq!(book.tick_count, 1);
    }

    #[test]
    fn le_ressort_de_page_avance_par_pas_de_vingt_pour_cent_au_plus() {
        let mut book = Book::new();
        book.flip_t = 10.0;
        book.book_spread = 1.0;
        // Le livre est ouvert et la chance ne se déclenche pas : `flip_t` bouge
        // seulement si `rng(40) == 0`.
        let mut rng = |bound: i32| if bound == 40 { 1 } else { 0 };
        book.tick(Some(0.0), &mut rng);
        assert_eq!(book.flip_t, 10.0);
        // (10 - 0) × 0,4 = 4, pincé à 0,2 ; flipA part de 0 → 0,2 × 0,9 = 0,18.
        assert!((book.flip_a - 0.18).abs() < 1e-6);
        assert!((book.page_flip - 0.18).abs() < 1e-6);
        assert_eq!(book.page_flip_prev, 0.0);
    }
}
