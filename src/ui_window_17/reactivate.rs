//! `WindowTownHallColonyReactivate` : la colonie abandonnée raconte son histoire, et
//! `create` passe la main à l'écran de fondation.
//!
//! Java référence : `core/client/gui/townhall/WindowTownHallColonyReactivate.java` (61
//! lignes), `gui/townhall/windowcolonyreactivate.xml` (17), `ColonyStoryListener.java`
//! (le pack de récits, `:88-97` et `:241-262`), et le message qui ouvre l'écran
//! (`OpenReactivateColonyMessage.java:29-41`).
//!
//! **Ce que la fenêtre demande** : trois choses, dans cet ordre et du même `Random`
//! (`:45-48`) — la graine est `pos.asLong()`, donc **la même hutte raconte la même
//! histoire à chaque ouverture**, puis un pré-nom tiré de `abandonedColonyNames`, puis un
//! récit tiré de `abandonedColonyStories`, chacun filtré par le biome de l'hôtel de ville.
//! Les deux tirages partagent le même générateur : changer l'ordre change l'histoire.
//!
//! **Ce qu'elle rend** : trois textes composés (`:50-52`) et deux boutons. `cancel` ferme
//! (`:42`). `create` **ne crée rien** : il ouvre `WindowTownHallColonyManage` par-dessus,
//! sans fermer cet écran, avec cinq arguments dont le pré-nom (`:60`) — c'est
//! [`Handoff`], et le booléen `true` y est le drapeau de réactivation.
//!
//! **Le pré-nom n'est pas un nom.** `pickRandom` répond `""` quand aucune entrée du pack ne
//! passe le filtre de biome (`ColonyStoryListener.java:90-95`) ; le titre affiche alors la
//! clé avec un argument vide, et l'écran suivant remplace le champ par
//! `"%s's Colony"` (`WindowTownHallColonyManage.java:54`). [`Resolution`] garde les deux
//! états, parce que « pas de récit » et « récit vide » ne se distinguent qu'ici.
//!
//! **Ce que la fenêtre ne dit pas** : ni le nom de la colonie la plus proche, ni sa
//! distance. Les deux champs existent (`:31-32`), sont lus du message
//! (`OpenReactivateColonyMessage.java:29`) et ne sont **écrits nulle part** : ils
//! repartent tels quels dans [`Handoff`]. C'est l'écran de la fondation qui les affiche
//! (`WindowTownHallColonyManage.java:56-63`).

use crate::ui::field::Value;

use super::{Button, Header, Leaves, Opener, Pane, Refusal, Window, COLONY_REACTIVATE_ID};

/// La classe (`WindowTownHallColonyReactivate.java:22`).
pub const JAVA: &str = "WindowTownHallColonyReactivate";
/// Le XML (`:36`).
pub const RESOURCE: &str = "minecolonies:gui/townhall/windowcolonyreactivate.xml";
/// Le titre (`windowcolonyreactivate.xml:7`, écrit `:50`).
pub const TITLE: &str = "title";
/// Le récit (`:9`, écrit `:51`).
pub const TEXT1: &str = "text1";
/// La question (`:11`, écrit `:52`).
pub const TEXT2: &str = "text2";
/// La croix (`:5`, liée `:42`) — une image, donc pas de libellé.
pub const CANCEL: &str = "cancel";
/// Le bouton qui continue (`:13-16`, lié `:43`).
pub const CREATE: &str = "create";

/// Le titre, avec le pré-nom en argument (`:50`).
pub const TITLE_KEY: &str = "com.minecolonies.core.gui.colony.reactivate.title";
/// La question, avec le pré-nom (`:52`).
pub const QUESTION_KEY: &str = "com.minecolonies.core.gui.colony.reactivate.question";
/// Le libellé du bouton `create` (`windowcolonyreactivate.xml:16`).
pub const CREATE_KEY: &str = "com.minecolonies.core.gui.reactivate.button";
/// Le message qui ouvre cet écran (`OpenReactivateColonyMessage.java:40`).
pub const OPENER_CLASS: &str = "OpenReactivateColonyMessage";

const XML: &str = "minecolonies/gui/townhall/windowcolonyreactivate.xml";
const JAVA_FILE: &str = "WindowTownHallColonyReactivate.java";

/// Une entrée du pack de récits : un filtre de biome et une **clé de traduction**.
///
/// `StoryText` (`ColonyStoryListener.java:241`) porte un `BiomeFilter` et un `content` ;
/// `content` est passé à `Component.translatable` (`:51`), donc c'est une clé, pas une
/// phrase — une entrée dont la clé n'existe pas s'affiche littéralement, comme en Java.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoryText {
    pub filter: BiomeFilter,
    pub key: String,
}

impl StoryText {
    /// Une entrée sans filtre : elle passe partout (`BiomeFilter.ALL`,
    /// `ColonyStoryListener.java:194`).
    pub fn anywhere(key: &str) -> StoryText {
        StoryText { filter: BiomeFilter::All, key: key.to_owned() }
    }

    /// Une entrée restreinte à un biome ou à un tag.
    pub fn only(filter: BiomeFilter, key: &str) -> StoryText {
        StoryText { filter, key: key.to_owned() }
    }
}

/// Le filtre d'une entrée : tous les biomes, un id, ou un tag.
///
/// `BiomeFilter` (`ColonyStoryListener.java:219-233`) : `"#ns:tag"` devient un tag,
/// `"ns:id"` un id. Le **ou** de deux filtres compte (`:201-204`), et [`BiomeFilter::All`]
/// l'emporte sur tout — c'est ce qui rend une entrée sans `biomes` universelle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BiomeFilter {
    /// Aucun `biomes` dans le JSON (`:154`).
    All,
    /// Un id de biome (`:227-232`).
    Id(String),
    /// Un tag de biome (`:221-226`).
    Tag(String),
}

impl BiomeFilter {
    /// Lit `"#ns:tag"` ou `"ns:id"` (`:219-233`).
    ///
    /// Java lève `IllegalStateException` sur une valeur vide et `NullPointerException`
    /// sur un id inconnu, parce qu'il résout le registre du serveur (`:229-231`) — un
    /// pack chargé au client n'a pas de serveur. Le port n'a pas de registre : il garde
    /// la chaîne et ne résout rien, donc [`Biome::tags`] est ce qui répond.
    pub fn parse(value: &str) -> Result<BiomeFilter, Refusal> {
        if value.is_empty() {
            return Err(Refusal::NoStoryMatch);
        }
        match value.strip_prefix('#') {
            Some(tag) if !tag.is_empty() => Ok(BiomeFilter::Tag(tag.to_owned())),
            Some(_) => Err(Refusal::NoStoryMatch),
            None => Ok(BiomeFilter::Id(value.to_owned())),
        }
    }

    /// Vrai quand `biome` passe ce filtre (`:248-251`).
    pub fn matches(&self, biome: &Biome<'_>) -> bool {
        match self {
            BiomeFilter::All => true,
            BiomeFilter::Id(id) => biome.id == id.as_str(),
            BiomeFilter::Tag(tag) => biome.tags.iter().any(|held| *held == tag.as_str()),
        }
    }
}

/// Le biome que le niveau répond pour la position de l'hôtel de ville
/// (`:46`, `mc.level.getBiome(pos)`).
///
/// Java reçoit un `Holder<Biome>` et le filtre fait `b.is(tag)` ou `b.equals(holder)`
/// (`:201-232`), puis le titre prend `biome.unwrapKey().get()` (`:51`) — un `get()` sur
/// un `Optional` : **un biome sans clé, donc un biome de runtime, fait planter la
/// fenêtre**. Le port ne peut pas représenter ce cas (l'id est obligatoire), ce qui vaut
/// mieux qu'un refus qui ne se déclencherait jamais.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Biome<'a> {
    /// `ResourceLocation` du biome, `ns:path`.
    pub id: &'a str,
    /// Les tags de ce biome, sans `#`.
    pub tags: &'a [&'a str],
}

impl<'a> Biome<'a> {
    /// Un biome sans tag.
    pub fn new(id: &'a str) -> Biome<'a> {
        Biome { id, tags: &[] }
    }

    /// La clé de langue que le titre passe en troisième argument
    /// (`:51`, `location().toLanguageKey("biome")`).
    pub fn language_key(&self) -> String {
        match self.id.split_once(':') {
            Some((namespace, path)) => format!("biome.{namespace}.{path}"),
            // `ResourceLocation` sans `:` est déjà `minecraft:` côté Java : une entrée
            // sans namespace s'affiche donc comme une entrée du namespace par défaut.
            None => format!("biome.minecraft.{}", self.id),
        }
    }
}

/// Les entrées du pack qui passent le filtre de biome (`StoryText.allMatches`,
/// `ColonyStoryListener.java:259-262`).
pub fn all_matches<'a>(stories: &'a [StoryText], biome: &Biome<'_>) -> Vec<&'a str> {
    stories
        .iter()
        .filter(|story| story.filter.matches(biome))
        .map(|story| story.key.as_str())
        .collect()
}

/// Le tirage d'un texte au sort : rien quand la liste est vide, comme Java
/// (`ColonyStoryListener.java:91-96`), jamais un indice hors bornes.
pub fn pick<'a>(matches: &[&'a str], rng: &mut Random48) -> Option<&'a str> {
    let bound = u32::try_from(matches.len()).ok()?;
    let index = rng.next_int(bound)?;
    matches.get(index as usize).copied()
}

/// La position d'un bloc, empaquetée comme `BlockPos.asLong()` — la graine du tirage
/// (`WindowTownHallColonyReactivate.java:45`).
///
/// C'est une méthode **vanilla** (26.3), pas une ligne de MineColonies : la disposition
/// est `x` sur 26 bits en haut, `z` sur 26 au milieu, `y` sur 12 en bas, et le résultat
/// est un `long` signé. Les coordonnées sont signées, donc le décalage aussi.
pub fn as_long(pos: [i32; 3]) -> i64 {
    let x = (pos[0] & 0x03FF_FFFF) as u64;
    let z = (pos[2] & 0x03FF_FFFF) as u64;
    let y = (pos[1] & 0xFFF) as u64;
    ((x << 38) | (z << 12) | y) as i64
}

/// `java.util.Random` : le générateur **exact** que Java installe avec `new Random(seed)`.
///
/// Trente-cinq ans de compatibilité : un LCG de 48 bits, multiplicateur `0x5DEECE66D`,
/// incrément `0xB`, et la graine mélangée à l'instanciation. Le reconstituer est la seule
/// façon que « la même hutte raconte la même histoire » soit vrai **entre Java et ce
/// port** ; un autre générateur donnerait une histoire stable, mais pas la bonne.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Random48 {
    state: u64,
}

/// Les 48 bits utiles.
const MASK: u64 = (1 << 48) - 1;
/// Le multiplicateur de `java.util.Random`.
const MULTIPLIER: u64 = 0x0005_DEEC_E66D;
/// L'incrément de `java.util.Random`.
const ADDEND: u64 = 0xB;

impl Random48 {
    /// `new Random(seed)` : la graine est mélangée avant usage.
    pub fn new(seed: i64) -> Random48 {
        Random48 { state: ((seed as u64) ^ MULTIPLIER) & MASK }
    }

    /// `next(bits)` : la source de tous les autres tirages.
    fn next(&mut self, bits: u32) -> u32 {
        self.state = self.state.wrapping_mul(MULTIPLIER).wrapping_add(ADDEND) & MASK;
        (self.state >> (48 - bits)) as u32
    }

    /// `nextInt(bound)` : la branche des puissances de deux, puis le rejet de Java.
    ///
    /// `None` sur `bound == 0`, là où Java lève `IllegalArgumentException` : un appelant
    /// qui n'a pas vérifié sa liste vide obtient un refus, pas un plantage.
    pub fn next_int(&mut self, bound: u32) -> Option<u32> {
        if bound == 0 {
            return None;
        }
        if bound & bound.wrapping_sub(1) == 0 {
            return Some(((u64::from(bound) * u64::from(self.next(31))) >> 31) as u32);
        }
        loop {
            let bits = self.next(31);
            let value = bits % bound;
            // `bits - value + (bound - 1) < 2^31` : le rejet qui rend la distribution exacte.
            if u64::from(bits) - u64::from(value) + u64::from(bound) - 1 < (1 << 31) {
                return Some(value);
            }
        }
    }
}

/// Ce que le pack a répondu pour cette hutte : deux tirages, ou deux absences.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolution {
    /// Le pré-nom tiré de `abandonedColonyNames` ; `None` quand aucune entrée ne passe.
    pub pre_name: Option<String>,
    /// Le récit tiré de `abandonedColonyStories` ; `None` de la même façon.
    pub story: Option<String>,
    /// La clé de langue du biome, troisième argument du récit (`:51`).
    pub biome_key: String,
}

impl Resolution {
    /// Le pré-nom, ou le refus qui dit pourquoi il manque.
    pub fn pre_name(&self) -> Result<&str, Refusal> {
        self.pre_name.as_deref().ok_or(Refusal::NoStoryMatch)
    }

    /// Le récit, ou le refus.
    pub fn story(&self) -> Result<&str, Refusal> {
        self.story.as_deref().ok_or(Refusal::NoStoryMatch)
    }

    /// Ce que le panneau affiche **réellement** : Java passe `""` à
    /// `Component.translatable`, donc l'argument est vide et la clé s'affiche seule.
    pub fn shown_pre_name(&self) -> &str {
        self.pre_name.as_deref().unwrap_or("")
    }

    /// Le récit tel qu'il s'affiche, même vide.
    pub fn shown_story(&self) -> &str {
        self.story.as_deref().unwrap_or("")
    }
}

/// Les deux tirages de l'ouverture, **dans l'ordre de Java** et sur **un seul**
/// générateur (`:45-48`).
///
/// Inverser les deux appels change le résultat : le pré-nom consomme le premier tirage.
pub fn resolve(
    names: &[StoryText],
    stories: &[StoryText],
    biome: &Biome<'_>,
    pos: [i32; 3],
) -> Resolution {
    let mut rng = Random48::new(as_long(pos));
    let pre_name = pick(&all_matches(names, biome), &mut rng).map(str::to_owned);
    let story = pick(&all_matches(stories, biome), &mut rng).map(str::to_owned);
    Resolution { pre_name, story, biome_key: biome.language_key() }
}

/// Les cinq arguments que `create` passe à `WindowTownHallColonyManage`
/// (`WindowTownHallColonyReactivate.java:60`).
///
/// **Le cinquième n'est pas « réactiver »**, malgré son nom : l'écran de fondation le met
/// dans le champ `claim` de `CreateColonyMessage` (`WindowTownHallColonyManage.java:80`),
/// où le serveur lit « la hutte a un pack de structure, réutilise-le » et envoie une chaîne
/// de pack vide (`:129-131`). La réactivation, elle, est décidée serveur par le tag
/// `DEACTIVATED` de la hutte (`:134-142`). Les deux chemins se rejoignent donc : `claim`
/// vrai **et** tag posé.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handoff {
    /// La position de l'hôtel de ville (`:37`).
    pub pos: [i32; 3],
    /// Le nom de la colonie la plus proche, **jamais affiché ici** (`:38`).
    pub closest_name: String,
    /// Sa distance, jamais affichée non plus (`:39`).
    pub closest_distance: i32,
    /// Le pré-nom tiré au sort, `""` quand le pack n'a rien donné (`:47`).
    pub pre_name: String,
    /// Le drapeau que la fenêtre passe à `true` (`:60`).
    pub reactivate: bool,
}

impl Handoff {
    /// Le champ `claim` du message de fondation qu'il deviendra (`:80`).
    pub fn claim(&self) -> bool {
        self.reactivate
    }

    /// Le nom de pack de structure que l'écran suivant enverra : `Some("")` en
    /// réactivation (`WindowTownHallColonyManage.java:82`), `None` sinon — le pack
    /// sélectionné par le joueur est l'état du client, que ce port n'a pas.
    pub fn pack_name(&self) -> Option<&'static str> {
        if self.reactivate {
            Some("")
        } else {
            None
        }
    }

    /// Le nom qui pré-remplira le champ de l'écran suivant : le pré-nom, ou
    /// `"%s's Colony"` quand il est vide (`WindowTownHallColonyManage.java:54`).
    pub fn next_field(&self, player_name: &str) -> String {
        if self.pre_name.is_empty() {
            format!("{player_name}'s Colony")
        } else {
            self.pre_name.clone()
        }
    }
}

/// Ce qu'un clic laisse à l'appelant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Out {
    /// `create` : l'écran de fondation s'ouvre **par-dessus**, rien ne se ferme (`:60`).
    Manage(Handoff),
    /// `cancel` : la fenêtre se ferme (`:42`, `AbstractWindowSkeleton.close`).
    Close,
}

/// Le clic d'un bouton, par id.
pub fn press(id: &str, state: &Handoff) -> Result<Out, Refusal> {
    match id {
        CREATE => Ok(Out::Manage(state.clone())),
        CANCEL => Ok(Out::Close),
        _ => Err(Refusal::UnknownButton),
    }
}

/// Le titre, tel que la classe l'écrit (`:50`) : la clé et son argument.
pub fn title(state: &Resolution) -> (&'static str, String) {
    (TITLE_KEY, state.shown_pre_name().to_owned())
}

/// La question, avec le pré-nom (`:52`).
pub fn question(state: &Resolution) -> (&'static str, String) {
    (QUESTION_KEY, state.shown_pre_name().to_owned())
}

/// Les trois panneaux, dans l'ordre où le constructeur les écrit (`:50-52`).
const PANES: &[Pane] = &[
    Pane::written(TITLE, Value::Text, super::Source::Story, (XML, 7), (JAVA_FILE, 50)),
    Pane::written(TEXT1, Value::Text, super::Source::Story, (XML, 9), (JAVA_FILE, 51)),
    Pane::written(TEXT2, Value::Text, super::Source::Story, (XML, 11), (JAVA_FILE, 52)),
];

/// Les deux boutons : fermer, et passer à l'écran de fondation.
const BUTTONS: &[Button] = &[
    Button::local(CANCEL, "", Leaves::Closes),
    Button::local(CREATE, CREATE_KEY, Leaves::Stays),
];

/// L'écran.
pub static WINDOW: Window = Window {
    id: COLONY_REACTIVATE_ID,
    java: JAVA,
    resource: RESOURCE,
    header: Header::None,
    opener: Opener::Java(OPENER_CLASS),
    panes: PANES,
    buttons: BUTTONS,
    skeleton: true,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn handoff(pre_name: &str) -> Handoff {
        Handoff {
            pos: [10, 64, -20],
            closest_name: "Voisine".to_owned(),
            closest_distance: 250,
            pre_name: pre_name.to_owned(),
            reactivate: true,
        }
    }

    #[test]
    fn the_pack_is_drawn_from_one_seeded_generator_name_first() {
        // Quatre noms, trois récits : le second tirage a une autre borne, ce qui rend
        // l'ordre des deux appels visible.
        let names: Vec<StoryText> =
            ["a", "b", "c", "d"].iter().copied().map(StoryText::anywhere).collect();
        let stories: Vec<StoryText> =
            ["s0", "s1", "s2"].iter().copied().map(StoryText::anywhere).collect();
        let biome = Biome::new("minecraft:plains");
        // La graine de (0, 0, 0) est 0 ; `new Random(0).nextInt(4)` vaut 2 et le suivant
        // `nextInt(3)` vaut 1 — les deux valeurs du LCG de Java, pas d'un autre.
        let here = resolve(&names, &stories, &biome, [0, 0, 0]);
        assert_eq!(here.pre_name.as_deref(), Some("c"));
        assert_eq!(here.story.as_deref(), Some("s1"));
        // La même hutte, deux fois : la même histoire.
        assert_eq!(here, resolve(&names, &stories, &biome, [0, 0, 0]));
        // Et deux huttes différentes ne sont pas forcées de différer — mais la graine
        // change, donc le premier tirage aussi, sur cet exemple.
        let there = resolve(&names, &stories, &biome, [100, 64, -37]);
        assert_ne!(there.pre_name, here.pre_name);
    }

    #[test]
    fn the_seed_is_the_packed_block_position() {
        // `BlockPos.asLong()` : x sur 26 bits en haut, z sur 26 au milieu, y sur 12 en bas.
        assert_eq!(as_long([0, 0, 0]), 0);
        assert_eq!(as_long([1, 0, 0]), 1 << 38);
        assert_eq!(as_long([0, 0, 1]), 1 << 12);
        assert_eq!(as_long([0, 1, 0]), 1);
        // Les coordonnées sont signées : le paquet aussi.
        assert_eq!(as_long([0, -1, 0]), 0xFFF, "y sur 12 bits");
        assert_eq!(as_long([0, 0, -1]), 0x3FF_FFFF_i64 << 12, "z sur 26 bits, décalé de 12");
        assert_eq!(as_long([-1, 0, 0]), (-1i64) << 38);
        // Et la graine 0 est bien celle d'un hôtel de ville à l'origine.
        assert_eq!(Random48::new(as_long([0, 0, 0])), Random48::new(0));
    }

    #[test]
    fn the_generator_is_java_util_random_and_not_a_lookalike() {
        // Les cinq premiers `nextInt(100)` de `new Random(42)`.
        let mut rng = Random48::new(42);
        let drawn: Vec<u32> = (0..5).filter_map(|_| rng.next_int(100)).collect();
        assert_eq!(drawn, [30, 63, 48, 84, 70]);
        // `nextInt(1)` vaut toujours 0, par la branche des puissances de deux.
        let mut one = Random48::new(7);
        assert_eq!(one.next_int(1), Some(0));
        // Une liste vide est un refus, pas un indice hors bornes : Java, lui, lève.
        assert_eq!(Random48::new(7).next_int(0), None);
    }

    #[test]
    fn a_filter_that_matches_nothing_leaves_an_empty_argument_and_a_named_refusal() {
        let names = vec![StoryText::only(BiomeFilter::Id("minecraft:desert".to_owned()), "story.desert")];
        let stories = vec![StoryText::only(BiomeFilter::Tag("minecraft:is_forest".to_owned()), "story.forest")];
        let plains = Biome::new("minecraft:plains");
        let empty = resolve(&names, &stories, &plains, [0, 64, 0]);
        assert_eq!(empty.pre_name, None);
        assert_eq!(empty.story, None);
        assert_eq!(empty.pre_name(), Err(Refusal::NoStoryMatch));
        assert_eq!(empty.shown_story(), "", "Java passe \"\" à Component.translatable");
        // Le récit des trois panneaux garde un argument vide : la clé s'affiche seule.
        assert_eq!(title(&empty).1, "");
        assert_eq!(question(&empty).1, "");
        // Et le pré-nom vide de l'écran suivant devient un nom par défaut.
        assert_eq!(handoff("").next_field("Simon"), "Simon's Colony");
        assert_eq!(handoff("Halte").next_field("Simon"), "Halte");
    }

    #[test]
    fn a_filter_reads_a_tag_an_id_and_all_wins_the_or() {
        assert_eq!(BiomeFilter::parse("#minecraft:is_forest"), Ok(BiomeFilter::Tag("minecraft:is_forest".to_owned())));
        assert_eq!(BiomeFilter::parse("minecraft:desert"), Ok(BiomeFilter::Id("minecraft:desert".to_owned())));
        assert_eq!(BiomeFilter::parse(""), Err(Refusal::NoStoryMatch));
        assert_eq!(BiomeFilter::parse("#"), Err(Refusal::NoStoryMatch));
        let tags: &[&str] = &["minecraft:is_forest", "minecraft:is_overworld"];
        let forest = Biome { id: "minecraft:forest", tags };
        assert!(BiomeFilter::All.matches(&forest), "une entrée sans `biomes` passe partout");
        assert!(BiomeFilter::Id("minecraft:forest".to_owned()).matches(&forest));
        assert!(!BiomeFilter::Id("minecraft:desert".to_owned()).matches(&forest));
        assert!(BiomeFilter::Tag("minecraft:is_forest".to_owned()).matches(&forest));
        assert!(!BiomeFilter::Tag("minecraft:is_jungle".to_owned()).matches(&forest));
        // La clé de langue du biome est le troisième argument du récit (:51).
        assert_eq!(forest.language_key(), "biome.minecraft.forest");
        assert_eq!(Biome::new("forest").language_key(), "biome.minecraft.forest");
        // Et les deux listes filtrées gardent l'ordre du pack.
        let pack = vec![
            StoryText::anywhere("a"),
            StoryText::only(BiomeFilter::Id("minecraft:desert".to_owned()), "b"),
            StoryText::anywhere("c"),
        ];
        assert_eq!(all_matches(&pack, &forest), ["a", "c"]);
    }

    #[test]
    fn create_hands_the_prenane_over_and_cancel_closes() {
        let state = handoff("Halte");
        assert_eq!(press(CREATE, &state), Ok(Out::Manage(state.clone())));
        assert_eq!(press(CANCEL, &state), Ok(Out::Close));
        assert_eq!(press("back", &state), Err(Refusal::UnknownButton), "aucun autre bouton n'est lié");
        // Le drapeau repart à `true` et devient `claim` d'un message de fondation.
        match press(CREATE, &state) {
            Ok(Out::Manage(next)) => {
                assert!(next.reactivate);
                assert!(next.claim());
                assert_eq!(next.pack_name(), Some(""), "le pack de la hutte est réutilisé");
                assert_eq!(next.closest_name, "Voisine", "jamais affiché ici, transmis quand même");
                assert_eq!(next.closest_distance, 250);
            }
            other => panic!("{other:?}"),
        }
        // `create` ouvre un écran par-dessus sans fermer : c'est `Leaves::Stays`.
        let create = match WINDOW.button(CREATE) {
            Some(button) => button,
            None => panic!("le bouton create a disparu"),
        };
        assert_eq!(create.leaves, Leaves::Stays);
        assert!(create.is_local(), "create n'envoie aucun message : il ouvre une fenêtre");
        // Le pré-nom est bien celui du tirage, pas le nom de la colonie voisine.
        assert_eq!(title(&Resolution { pre_name: Some("Halte".to_owned()), story: None, biome_key: String::new() }).1, "Halte");
    }
}
