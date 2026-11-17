//! Which model each citizen asks for, and how its texture name is built.
//!
//! Java reference: `api/client/render/modeltype/` (`IModelType.java`,
//! `SimpleModelType.java`, `ISimpleModelType.java:24-96`, `ModModelTypes.java`),
//! `apiimp/initializer/ModModelTypeInitializer.java` (the 42 registrations),
//! `core/event/ClientRegistryHandler.java:130-155` (the layer ids), and the fifty
//! `getModel()` overrides under `core/colony/jobs/`.
//!
//! **The model is not the job.** `IJob.getModel()` is one method per job class
//! and eight jobs share a model, four name none at all; a *citizen* names a model
//! type ([`ModelType`]) and the renderer resolves male/female from that type
//! (`RenderBipedCitizen.java:88-92`). So the identity a texture is built from is
//! the **model type path**, not the job path: a forester is `forester`, its
//! model layer is `minecolonies:male_lumberjack`, and its job is `lumberjack` —
//! three different strings for one citizen.
//!
//! | Question | Answer | Where |
//! | --- | --- | --- |
//! | how many model types | **42** | [`MODEL_TYPES`] |
//! | how many own more than one texture | **6** | [`MULTI_TEXTURE`] |
//! | how many jobs | 50 | `crate::jobs::Job::ALL` |
//! | jobs naming a model of their own | 46 | [`JOB_MODELS`] |
//! | jobs falling back to `citizen` | 4 | [`FALLBACK_JOBS`] |
//!
//! **Three facts the brief does not have.**
//!
//! 1. **The texture index wraps, it is not clamped**: `(textureId % numTextures) + 1`
//!    (`ISimpleModelType.java:89`), so a citizen with texture id 4 on a
//!    three-texture model wears texture 2.
//! 2. **The halloween folder is decided once per session and never again.**
//!    `cachedHalloweenStyle` is a `static` filled on the first texture request
//!    (`:60-73`) — a client still running on November 3rd keeps the `nether`
//!    skins. [`StyleCache`] freezes the same way, and says so.
//! 3. **The ghost mode is dead code.** `EventHandler.java:773-777` computes the
//!    halloween window and then assigns `RenderBipedCitizen.isItGhostTime = false`,
//!    the value it already has; nothing in the tree assigns `true`. [`Ghost`]
//!    reproduces that instead of inventing a fade.
//!
//! **What this file does not do**: no file is opened, no texture is read. The
//! fallback that Java decides by asking the resource manager
//! (`ISimpleModelType.java:92-96`) is returned as a *pair* — the port cannot know
//! which one exists, so it hands both to the caller.

use crate::jobs::Job;

/// `ISimpleModelType.BASE_FOLDER` (`:26`): every citizen texture lives under it.
pub const BASE_FOLDER: &str = "textures/entity/citizen/";

/// `ISimpleModelType.DEFAULT_FOLDER` (`:29`): the folder used when the styled one
/// is missing (`:96`).
pub const DEFAULT_FOLDER: &str = "default";

/// `cachedHalloweenStyle`'s value in the window (`:64`).
pub const HALLOWEEN_STYLE: &str = "nether";

/// `AbstractEntityCitizen`'s `DATA_TEXTURE_SUFFIX` default (`:307`), the `_b` of
/// `settlermale1_b.png`.
pub const DEFAULT_SUFFIX: &str = "_b";

/// The namespace every model type id is declared in (`ModModelTypes.java:9-50`).
pub const NAMESPACE: &str = "minecolonies";

/// One of the model types `ModModelTypeInitializer.init` registers.
///
/// `path` is the registry path (`ModModelTypes.SETTLER_ID = minecolonies:settler`),
/// `textures` is `SimpleModelType`'s second constructor argument (**not** the
/// number of files in the tree), and the two layers are the `ModelLayerLocation`
/// ids the initialiser bakes — which for two rows do **not** follow `path`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: four fields, all read by the tests and the texture builder
pub struct ModelType {
    /// The registry path, without its namespace: `settler`, `courier`.
    pub path: &'static str,
    /// `numTextures` (`SimpleModelType.java:22`); at least one for every row.
    pub textures: u8,
    /// The male `ModelLayerLocation` id: `minecolonies:male_settler`, or vanilla's
    /// `minecraft:player#main` for `custom`.
    pub male_layer: &'static str,
    /// The female one, `None` only for `custom` — Java passes `null`
    /// (`ModModelTypeInitializer.java:26`) and the renderer would then fall over:
    /// see [`Refusal::NoFemaleModel`].
    pub female_layer: Option<&'static str>,
    /// `fichier.java:ligne` of the `SimpleModelType` call this row copies.
    pub source: &'static str,
}

impl ModelType {
    /// A row of the table, spelled in the order the initialiser writes it.
    pub const fn new(
        path: &'static str,
        textures: u8,
        male_layer: &'static str,
        female_layer: Option<&'static str>,
        source: &'static str,
    ) -> ModelType {
        ModelType {
            path,
            textures,
            male_layer,
            female_layer,
            source,
        }
    }

    /// The full model type id, `minecolonies:settler`.
    pub fn id(&self) -> String {
        format!("{NAMESPACE}:{}", self.path)
    }

    /// The layer the renderer bakes for this citizen (`RenderBipedCitizen.java:40`
    /// bakes the vanilla player layer as its *base*, then replaces the model with
    /// the type's, `:88-92`).
    pub fn layer(&self, female: bool) -> Option<&'static str> {
        if female {
            self.female_layer
        } else {
            Some(self.male_layer)
        }
    }

    /// The texture this citizen wears, as `ISimpleModelType.getTexture` builds it
    /// (`:60-96`).
    ///
    /// `min(style)` picks the folder, `suffix` is `DATA_TEXTURE_SUFFIX`. Both
    /// paths are returned: Java asks the resource manager whether the styled one
    /// exists and falls back to `default/` (`:92-96`), which a wasm mod cannot do.
    pub fn texture(
        &self,
        female: bool,
        texture_id: u32,
        suffix: &str,
        style: &str,
    ) -> Result<TexturePath, Refusal> {
        if self.textures == 0 {
            return Err(Refusal::NoTextures { model: self.path });
        }
        // `ISimpleModelType.java:89` — modulo, then +1: an id past the count wraps.
        let index = (texture_id % u32::from(self.textures)) + 1;
        let mut file = String::with_capacity(48);
        file.push_str(self.path);
        file.push_str(if female { "female" } else { "male" });
        file.push_str(&index.to_string());
        file.push_str(suffix);
        file.push_str(".png");

        let mut primary = String::with_capacity(BASE_FOLDER.len() + style.len() + file.len() + 1);
        primary.push_str(BASE_FOLDER);
        primary.push_str(style);
        primary.push('/');
        primary.push_str(&file);

        let mut fallback = String::with_capacity(BASE_FOLDER.len() + DEFAULT_FOLDER.len() + file.len() + 1);
        fallback.push_str(BASE_FOLDER);
        fallback.push_str(DEFAULT_FOLDER);
        fallback.push('/');
        fallback.push_str(&file);

        Ok(TexturePath { primary, fallback })
    }
}

/// The styled texture name and the one Java falls back to (`:92-96`).
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: two strings, both handed to the caller on purpose
pub struct TexturePath {
    /// `textures/entity/citizen/<style>/<model><sex><n><suffix>.png`.
    pub primary: String,
    /// `textures/entity/citizen/default/…`, the same file name.
    pub fallback: String,
}

/// What a texture request can refuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three closed reasons
pub enum Refusal {
    /// `numTextures` is 0: Java divides by it and throws (`:89`).
    NoTextures { model: &'static str },
    /// A female citizen on `custom`, whose female model is `null`
    /// (`ModModelTypeInitializer.java:26`).
    NoFemaleModel { model: &'static str },
    /// A model path no row declares.
    UnknownModel { path: &'static str },
}

impl Refusal {
    /// A stable name, namespaced.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::NoTextures { .. } => "render.refusal.no_textures",
            Refusal::NoFemaleModel { .. } => "render.refusal.no_female_model",
            Refusal::UnknownModel { .. } => "render.refusal.unknown_model",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// The 42 model types, in `ModModelTypeInitializer.init`'s registration order.
///
/// Six own three textures and the other thirty-six own one — the whole table is
/// spelled here rather than derived, because `path` and the layer name disagree
/// for `courier`/`male_deliveryman`, `forester`/`male_lumberjack` and
/// `knight`/`male_knight`'s duplicate `KNIGHT_ID`.
pub const MODEL_TYPES: &[ModelType] = &[
    ModelType::new("settler", 3, "minecolonies:male_settler", Some("minecolonies:female_settler"), "ModModelTypeInitializer.java:23"),
    ModelType::new("custom", 1, "minecraft:player#main", None, "ModModelTypeInitializer.java:26"),
    ModelType::new("citizen", 3, "minecolonies:male_citizen", Some("minecolonies:female_citizen"), "ModModelTypeInitializer.java:29"),
    ModelType::new("noble", 3, "minecolonies:male_citizennoble", Some("minecolonies:female_citizennoble"), "ModModelTypeInitializer.java:32"),
    ModelType::new("aristocrat", 3, "minecolonies:male_aristocrat", Some("minecolonies:female_aristocrat"), "ModModelTypeInitializer.java:35"),
    ModelType::new("builder", 1, "minecolonies:male_builder", Some("minecolonies:female_builder"), "ModModelTypeInitializer.java:38"),
    ModelType::new("courier", 1, "minecolonies:male_deliveryman", Some("minecolonies:female_deliveryman"), "ModModelTypeInitializer.java:41"),
    ModelType::new("miner", 1, "minecolonies:male_miner", Some("minecolonies:female_miner"), "ModModelTypeInitializer.java:44"),
    ModelType::new("forester", 1, "minecolonies:male_lumberjack", Some("minecolonies:female_lumberjack"), "ModModelTypeInitializer.java:47"),
    ModelType::new("farmer", 1, "minecolonies:male_farmer", Some("minecolonies:female_farmer"), "ModModelTypeInitializer.java:50"),
    ModelType::new("fisherman", 1, "minecolonies:male_fisherman", Some("minecolonies:female_fisherman"), "ModModelTypeInitializer.java:53"),
    ModelType::new("undertaker", 1, "minecolonies:male_undertaker", Some("minecolonies:female_undertaker"), "ModModelTypeInitializer.java:56"),
    ModelType::new("archer", 1, "minecolonies:male_archer", Some("minecolonies:female_archer"), "ModModelTypeInitializer.java:59"),
    ModelType::new("knight", 1, "minecolonies:male_knight", Some("minecolonies:female_knight"), "ModModelTypeInitializer.java:62"),
    ModelType::new("baker", 1, "minecolonies:male_baker", Some("minecolonies:female_baker"), "ModModelTypeInitializer.java:65"),
    ModelType::new("sheepfarmer", 1, "minecolonies:male_sheepfarmer", Some("minecolonies:female_sheepfarmer"), "ModModelTypeInitializer.java:68"),
    ModelType::new("cowfarmer", 1, "minecolonies:male_cowfarmer", Some("minecolonies:female_cowfarmer"), "ModModelTypeInitializer.java:71"),
    ModelType::new("pigfarmer", 1, "minecolonies:male_pigfarmer", Some("minecolonies:female_pigfarmer"), "ModModelTypeInitializer.java:74"),
    ModelType::new("chickenfarmer", 1, "minecolonies:male_chickenfarmer", Some("minecolonies:female_chickenfarmer"), "ModModelTypeInitializer.java:77"),
    ModelType::new("composter", 1, "minecolonies:male_composter", Some("minecolonies:female_composter"), "ModModelTypeInitializer.java:80"),
    ModelType::new("smelter", 1, "minecolonies:male_smelter", Some("minecolonies:female_smelter"), "ModModelTypeInitializer.java:83"),
    ModelType::new("cook", 1, "minecolonies:male_cook", Some("minecolonies:female_cook"), "ModModelTypeInitializer.java:86"),
    ModelType::new("student", 3, "minecolonies:male_student", Some("minecolonies:female_student"), "ModModelTypeInitializer.java:89"),
    ModelType::new("crafter", 1, "minecolonies:male_crafter", Some("minecolonies:female_crafter"), "ModModelTypeInitializer.java:92"),
    ModelType::new("blacksmith", 1, "minecolonies:male_blacksmith", Some("minecolonies:female_blacksmith"), "ModModelTypeInitializer.java:95"),
    ModelType::new("child", 3, "minecolonies:male_child", Some("minecolonies:female_child"), "ModModelTypeInitializer.java:98"),
    ModelType::new("healer", 1, "minecolonies:male_healer", Some("minecolonies:female_healer"), "ModModelTypeInitializer.java:101"),
    ModelType::new("teacher", 1, "minecolonies:male_teacher", Some("minecolonies:female_teacher"), "ModModelTypeInitializer.java:104"),
    ModelType::new("glassblower", 1, "minecolonies:male_glassblower", Some("minecolonies:female_glassblower"), "ModModelTypeInitializer.java:107"),
    ModelType::new("dyer", 1, "minecolonies:male_dyer", Some("minecolonies:female_dyer"), "ModModelTypeInitializer.java:110"),
    ModelType::new("mechanist", 1, "minecolonies:male_mechanist", Some("minecolonies:female_mechanist"), "ModModelTypeInitializer.java:113"),
    ModelType::new("fletcher", 1, "minecolonies:male_fletcher", Some("minecolonies:female_fletcher"), "ModModelTypeInitializer.java:116"),
    ModelType::new("concretemixer", 1, "minecolonies:male_concretemixer", Some("minecolonies:female_concretemixer"), "ModModelTypeInitializer.java:119"),
    ModelType::new("rabbitherder", 1, "minecolonies:male_rabbitherder", Some("minecolonies:female_rabbitherder"), "ModModelTypeInitializer.java:122"),
    ModelType::new("planter", 1, "minecolonies:male_planter", Some("minecolonies:female_planter"), "ModModelTypeInitializer.java:125"),
    ModelType::new("beekeeper", 1, "minecolonies:male_beekeeper", Some("minecolonies:female_beekeeper"), "ModModelTypeInitializer.java:128"),
    ModelType::new("netherworker", 1, "minecolonies:male_netherworker", Some("minecolonies:female_netherworker"), "ModModelTypeInitializer.java:131"),
    ModelType::new("druid", 1, "minecolonies:male_druid", Some("minecolonies:female_druid"), "ModModelTypeInitializer.java:134"),
    ModelType::new("enchanter", 1, "minecolonies:male_enchanter", Some("minecolonies:female_enchanter"), "ModModelTypeInitializer.java:137"),
    ModelType::new("florist", 1, "minecolonies:male_florist", Some("minecolonies:female_florist"), "ModModelTypeInitializer.java:140"),
    ModelType::new("carpenter", 1, "minecolonies:male_carpenter", Some("minecolonies:female_carpenter"), "ModModelTypeInitializer.java:143"),
    ModelType::new("alchemist", 1, "minecolonies:male_alchemist", Some("minecolonies:female_alchemist"), "ModModelTypeInitializer.java:146"),
];

/// The six model types whose `numTextures` is 3 (`ModModelTypeInitializer.java:23-98`).
pub const MULTI_TEXTURE: [&str; 6] = ["settler", "citizen", "noble", "aristocrat", "student", "child"];

/// The model Java falls back to: `AbstractJob.getModel` answers
/// `ModModelTypes.CITIZEN_ID` (`AbstractJob.java:121`).
pub const CITIZEN_PATH: &str = "citizen";

/// The index of [`CITIZEN_PATH`] in [`MODEL_TYPES`], checked by a test.
const CITIZEN_INDEX: usize = 2;

/// The `citizen` row, the default every job without a model of its own gets.
pub fn citizen_model() -> &'static ModelType {
    &MODEL_TYPES[CITIZEN_INDEX]
}

/// One `getModel()` override: a job path and the model path it answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three strings, all compared
pub struct JobModel {
    /// `Job::path()` of the job class.
    pub job: &'static str,
    /// The model type path the class returns.
    pub model: &'static str,
    /// `fichier.java:ligne` of the `getModel()` body.
    pub source: &'static str,
}

/// The 46 jobs that name a model of their own, one row per `getModel()`.
///
/// The other four (`huscarl`, `marksman`, `stonemason`, `placeholder`) override
/// nothing at all: `AbstractJobGuard`, `AbstractJobStructure` and `AbstractJob`
/// have no `getModel`, so they land on [`CITIZEN_PATH`] like every job would.
pub const JOB_MODELS: &[JobModel] = &[
    JobModel { job: "alchemist", model: "alchemist", source: "JobAlchemist.java:38" },
    JobModel { job: "archertraining", model: "archer", source: "JobArcherTraining.java:26" },
    JobModel { job: "baker", model: "baker", source: "JobBaker.java:36" },
    JobModel { job: "beekeeper", model: "beekeeper", source: "JobBeekeeper.java:79" },
    JobModel { job: "blacksmith", model: "blacksmith", source: "JobBlacksmith.java:31" },
    JobModel { job: "builder", model: "builder", source: "JobBuilder.java:28" },
    JobModel { job: "cavalry", model: "knight", source: "guard/JobCavalry.java:82" },
    JobModel { job: "chef", model: "cook", source: "JobChef.java:31" },
    JobModel { job: "chickenherder", model: "chickenfarmer", source: "JobChickenHerder.java:64" },
    JobModel { job: "combattraining", model: "knight", source: "JobCombatTraining.java:27" },
    JobModel { job: "composter", model: "composter", source: "JobComposter.java:33" },
    JobModel { job: "concretemixer", model: "concretemixer", source: "JobConcreteMixer.java:43" },
    JobModel { job: "cook", model: "cook", source: "JobCook.java:33" },
    JobModel { job: "cowboy", model: "cowfarmer", source: "JobCowboy.java:50" },
    JobModel { job: "crusher", model: "smelter", source: "JobCrusher.java:45" },
    JobModel { job: "deliveryman", model: "courier", source: "JobDeliveryman.java:105" },
    JobModel { job: "druid", model: "druid", source: "guard/JobDruid.java:56" },
    JobModel { job: "dyer", model: "dyer", source: "JobDyer.java:31" },
    JobModel { job: "enchanter", model: "enchanter", source: "JobEnchanter.java:53" },
    JobModel { job: "farmer", model: "farmer", source: "JobFarmer.java:28" },
    JobModel { job: "fisherman", model: "fisherman", source: "JobFisherman.java:63" },
    JobModel { job: "fletcher", model: "fletcher", source: "JobFletcher.java:43" },
    JobModel { job: "florist", model: "florist", source: "JobFlorist.java:25" },
    JobModel { job: "glassblower", model: "glassblower", source: "JobGlassblower.java:31" },
    JobModel { job: "healer", model: "healer", source: "JobHealer.java:46" },
    JobModel { job: "knight", model: "knight", source: "guard/JobKnight.java:74" },
    JobModel { job: "lumberjack", model: "forester", source: "JobLumberjack.java:66" },
    JobModel { job: "mechanic", model: "mechanist", source: "JobMechanic.java:43" },
    JobModel { job: "miner", model: "miner", source: "JobMiner.java:32" },
    JobModel { job: "netherworker", model: "netherworker", source: "JobNetherWorker.java:116" },
    JobModel { job: "planter", model: "planter", source: "JobPlanter.java:30" },
    JobModel { job: "pupil", model: "child", source: "JobPupil.java:28" },
    JobModel { job: "quarrier", model: "miner", source: "JobQuarrier.java:40" },
    JobModel { job: "rabbitherder", model: "rabbitherder", source: "JobRabbitHerder.java:50" },
    JobModel { job: "ranger", model: "archer", source: "guard/JobRanger.java:35" },
    JobModel { job: "researcher", model: "student", source: "JobResearch.java:52" },
    JobModel { job: "sawmill", model: "carpenter", source: "JobSawmill.java:43" },
    JobModel { job: "shepherd", model: "sheepfarmer", source: "JobShepherd.java:51" },
    JobModel { job: "sifter", model: "smelter", source: "JobSifter.java:36" },
    JobModel { job: "smelter", model: "smelter", source: "JobSmelter.java:33" },
    JobModel { job: "stablemaster", model: "cowfarmer", source: "JobStablemaster.java:52" },
    JobModel { job: "stonesmeltery", model: "crafter", source: "AbstractJobCrafter.java:99" },
    JobModel { job: "student", model: "student", source: "JobStudent.java:39" },
    JobModel { job: "swineherder", model: "pigfarmer", source: "JobSwineHerder.java:50" },
    JobModel { job: "teacher", model: "teacher", source: "JobTeacher.java:28" },
    JobModel { job: "undertaker", model: "undertaker", source: "JobUndertaker.java:28" },
];

/// The four jobs that override nothing, and so wear the plain citizen model.
pub const FALLBACK_JOBS: [&str; 4] = ["huscarl", "marksman", "stonemason", "placeholder"];

/// The row a path names, or `None`.
pub fn model_by_path(path: &str) -> Option<&'static ModelType> {
    MODEL_TYPES.iter().find(|row| row.path == path)
}

/// The `getModel()` row a job path names, or `None` when the class overrides nothing.
pub fn job_model_row(job_path: &str) -> Option<&'static JobModel> {
    JOB_MODELS.iter().find(|row| row.job == job_path)
}

/// The model type a job wears: its own `getModel()`, else `citizen`
/// (`AbstractJob.java:121`).
pub fn model_for(job: Job) -> Result<&'static ModelType, Refusal> {
    let path = match job_model_row(job.path()) {
        Some(row) => row.model,
        None => CITIZEN_PATH,
    };
    model_by_path(path).ok_or(Refusal::UnknownModel { path })
}

/// Whether a date falls in the halloween window `ISimpleModelType.java:60-68`
/// tests: **October 29th or later**, or **November 1st-2nd**.
pub fn halloween_window(month: u8, day: u8) -> bool {
    (month == 10 && day >= 29) || (month == 11 && day <= 2)
}

/// The style folder a citizen's texture comes from, once the session's answer is
/// known: `cachedHalloweenStyle` when it is not empty, the entity's own otherwise
/// (`ISimpleModelType.java:79-84`).
pub fn style_folder<'a>(entity_style: &'a str, cached: &'a str) -> &'a str {
    if cached.is_empty() {
        entity_style
    } else {
        cached
    }
}

/// `SimpleModelType.cachedHalloweenStyle`: a `static` that is written once and
/// read for the rest of the session (`:60-73`).
///
/// **It is not a per-call answer.** Java computes it on the first texture request
/// and never revisits it, so a client that joins on October 30th still dresses
/// its citizens in `nether` on November 10th. Reproduced, not fixed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: one field, and `None` is a state the caller can observe
pub struct StyleCache {
    frozen: Option<&'static str>,
}

impl StyleCache {
    /// A cache nothing has asked yet.
    pub const fn new() -> StyleCache {
        StyleCache { frozen: None }
    }

    /// The cached style, freezing it on the first call: `"nether"` inside
    /// [`halloween_window`], `""` outside.
    pub fn halloween(&mut self, month: u8, day: u8) -> &'static str {
        match self.frozen {
            Some(style) => style,
            None => {
                let style = if halloween_window(month, day) {
                    HALLOWEEN_STYLE
                } else {
                    ""
                };
                self.frozen = Some(style);
                style
            }
        }
    }

    /// Whether a date has already been asked — the observable half of "frozen".
    pub fn is_frozen(&self) -> bool {
        self.frozen.is_some()
    }
}

/// `RenderBipedCitizen.GHOST_ALPHA` (`:66`): the colour the shader is set to when
/// the ghost mode is on.
pub const GHOST_ALPHA: f32 = 0.3;

/// The ghost mode as the tree actually implements it.
///
/// `EventHandler.onWorldLoad` computes the window (October 31st, November 1st-2nd)
/// and then **assigns `false`** to the flag it was meant to arm
/// (`EventHandler.java:773-777`); the declaration is `false`
/// (`RenderBipedCitizen.java:31`) and no other file in the tree writes `true`.
/// So the 0.3 fade never happens, and [`Ghost::armed`] stays `false` forever.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: one boolean, and the tests read it
pub struct Ghost {
    /// `RenderBipedCitizen.isItGhostTime`.
    pub armed: bool,
}

impl Ghost {
    /// What a world load does with the flag on that date.
    pub fn after_world_load(_month: u8, _day: u8) -> Ghost {
        Ghost { armed: false }
    }

    /// The alpha the renderer draws a citizen with (`:62-79`): 0.3 when armed,
    /// opaque otherwise.
    pub fn alpha(&self) -> f32 {
        if self.armed { GHOST_ALPHA } else { 1.0 }
    }
}

/// April 1st, the one date `CitizenModel.isItApril1st` is set on
/// (`EventHandler.java:779-783`).
pub fn april_first(month: u8, day: u8) -> bool {
    month == 4 && day == 1
}

/// One named part of the humanoid model, in the order `CitizenModel.setupAnim`'s
/// april switch hides them (`CitizenModel.java:57-73`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the seven parts the switch names
pub enum Part {
    /// `leftArm` — `civilianID % 7 == 0`.
    LeftArm,
    /// `rightArm` — `1`.
    RightArm,
    /// `body` — `2`.
    Body,
    /// `head` — `3`.
    Head,
    /// `hat` — `4`.
    Hat,
    /// `leftLeg` — `5`.
    LeftLeg,
    /// `rightLeg` — `6`.
    RightLeg,
}

impl Part {
    /// Every part the switch can hide, in case order.
    pub const ALL: [Part; 7] = [
        Part::LeftArm,
        Part::RightArm,
        Part::Body,
        Part::Head,
        Part::Hat,
        Part::LeftLeg,
        Part::RightLeg,
    ];

    /// `ActorModelPart`'s name, as Blockbench spells it.
    pub fn name(self) -> &'static str {
        match self {
            Part::LeftArm => "left_arm",
            Part::RightArm => "right_arm",
            Part::Body => "body",
            Part::Head => "head",
            Part::Hat => "hat",
            Part::LeftLeg => "left_leg",
            Part::RightLeg => "right_leg",
        }
    }
}

/// The part the april switch hides for a citizen id, or `None` outside April 1st
/// (`CitizenModel.java:56-73`).
pub fn april_hidden_part(april: bool, civilian_id: u32) -> Option<Part> {
    if !april {
        return None;
    }
    let index = (civilian_id % Part::ALL.len() as u32) as usize;
    Some(Part::ALL[index])
}

#[cfg(test)]
mod tests {
    use super::*;
    // The job table is the other half of this file's join; imported here rather
    // than reached through the parent's private `use`.
    use crate::jobs::Job;

    #[test]
    fn forty_two_model_types_and_no_path_twice() {
        assert_eq!(MODEL_TYPES.len(), 42);
        let mut paths: Vec<&str> = MODEL_TYPES.iter().map(|row| row.path).collect();
        paths.sort_unstable();
        let mut unique = paths.clone();
        unique.dedup();
        assert_eq!(unique.len(), 42, "two rows share a path");
        // `KNIGHT_ID` and `KNIGHT_GUARD_ID` are the same string in Java
        // (`ModModelTypes.java:30`, `:48`) and the initialiser registers the
        // `knight` layer once — the table must not carry it twice either.
        assert_eq!(
            MODEL_TYPES.iter().filter(|row| row.path == "knight").count(),
            1
        );
    }

    #[test]
    fn six_rows_own_three_textures_and_the_rest_one() {
        let three: Vec<&str> = MODEL_TYPES
            .iter()
            .filter(|row| row.textures == 3)
            .map(|row| row.path)
            .collect();
        assert_eq!(three, Vec::from(MULTI_TEXTURE));
        assert_eq!(
            MODEL_TYPES.iter().filter(|row| row.textures == 1).count(),
            36
        );
        assert!(MODEL_TYPES.iter().all(|row| row.textures != 0));
    }

    #[test]
    fn every_row_carries_its_source_line() {
        for row in MODEL_TYPES {
            assert!(row.source.starts_with("ModModelTypeInitializer.java:"), "{}", row.path);
        }
    }

    #[test]
    fn the_citizen_row_is_the_one_the_index_names() {
        assert_eq!(citizen_model().path, CITIZEN_PATH);
        assert_eq!(
            MODEL_TYPES
                .iter()
                .filter(|row| row.path == CITIZEN_PATH)
                .count(),
            1
        );
        assert_eq!(MODEL_TYPES[CITIZEN_INDEX].path, CITIZEN_PATH);
    }

    #[test]
    fn forty_six_jobs_name_a_model_and_four_name_none() {
        assert_eq!(JOB_MODELS.len(), 46);
        assert_eq!(FALLBACK_JOBS.len(), 4);
        assert_eq!(JOB_MODELS.len() + FALLBACK_JOBS.len(), 50);
        let mut jobs: Vec<&str> = JOB_MODELS.iter().map(|row| row.job).collect();
        jobs.sort_unstable();
        let mut unique = jobs.clone();
        unique.dedup();
        assert_eq!(unique.len(), 46, "a job is listed twice");
    }

    #[test]
    fn every_row_names_a_model_that_exists() {
        // The test that stops an invented model path: a typo here is a citizen
        // drawn with nothing.
        for row in JOB_MODELS {
            assert!(
                model_by_path(row.model).is_some(),
                "{} names {}, which no row declares",
                row.job,
                row.model
            );
            assert!(row.source.ends_with(".java:") || row.source.contains(".java:"));
        }
    }

    #[test]
    fn every_one_of_the_fifty_jobs_resolves_and_the_four_match_the_table() {
        let mut fell_back = Vec::new();
        for job in Job::ALL {
            let model = model_for(*job).expect("every job has a model");
            if job_model_row(job.path()).is_none() {
                fell_back.push(job.path());
            } else {
                assert_ne!(model.path, "", "{} resolved to nothing", job.path());
            }
        }
        fell_back.sort_unstable();
        let mut expected = Vec::from(FALLBACK_JOBS);
        expected.sort_unstable();
        assert_eq!(fell_back, expected);
    }

    #[test]
    fn a_job_and_its_model_are_three_different_strings() {
        // The finding this file exists for: forester has job `lumberjack`, model
        // `forester`, layer `male_lumberjack`.
        let job = Job::from_path("lumberjack").expect("a known job");
        let model = model_for(job).expect("resolves");
        assert_eq!(job.path(), "lumberjack");
        assert_eq!(model.path, "forester");
        assert_eq!(model.layer(false), Some("minecolonies:male_lumberjack"));
        // And the courier, which keeps `courier` as a model but not as a job.
        let courier = model_for(Job::from_path("deliveryman").expect("a known job"));
        assert_eq!(courier.expect("resolves").path, "courier");
    }

    #[test]
    fn a_settler_texture_is_the_one_the_mercenary_is_drawn_with() {
        // `RenderMercenary.java:32` names `textures/entity/citizen/default/settlermale1_b.png`;
        // the builder must produce exactly that for id 0.
        let settler = model_by_path("settler").expect("a row");
        let path = settler
            .texture(false, 0, DEFAULT_SUFFIX, DEFAULT_FOLDER)
            .expect("three textures");
        assert_eq!(
            path.primary,
            "textures/entity/citizen/default/settlermale1_b.png"
        );
        assert_eq!(path.fallback, path.primary);
    }

    #[test]
    fn the_texture_index_wraps_instead_of_clamping() {
        let settler = model_by_path("settler").expect("a row");
        for (id, index) in [(0, 1), (1, 2), (2, 3), (3, 1), (4, 2), (99, 1)] {
            let path = settler
                .texture(false, id, DEFAULT_SUFFIX, DEFAULT_FOLDER)
                .expect("three textures");
            assert!(
                path.primary.contains(&format!("settlermale{index}_b.png")),
                "id {id} should be {index}: {}",
                path.primary
            );
        }
    }

    #[test]
    fn the_sex_marker_is_part_of_the_file_name() {
        let citizen = model_by_path("citizen").expect("a row");
        let female = citizen
            .texture(true, 4, DEFAULT_SUFFIX, DEFAULT_FOLDER)
            .expect("three textures");
        assert_eq!(
            female.primary,
            "textures/entity/citizen/default/citizenfemale2_b.png"
        );
        assert_eq!(citizen.layer(true), Some("minecolonies:female_citizen"));
        assert_eq!(citizen.layer(false), Some("minecolonies:male_citizen"));
    }

    #[test]
    fn the_suffix_is_not_always_b() {
        // `DATA_TEXTURE_SUFFIX` (`AbstractEntityCitizen.java:82`, default `:307`)
        // is synched data: a colony may set another one.
        let builder = model_by_path("builder").expect("a row");
        let path = builder
            .texture(false, 0, "_santa", DEFAULT_FOLDER)
            .expect("one texture");
        assert_eq!(
            path.primary,
            "textures/entity/citizen/default/buildermale1_santa.png"
        );
    }

    #[test]
    fn a_styled_folder_comes_with_the_default_one_beside_it() {
        let child = model_by_path("child").expect("a row");
        let path = child
            .texture(true, 1, DEFAULT_SUFFIX, HALLOWEEN_STYLE)
            .expect("three textures");
        assert_eq!(
            path.primary,
            "textures/entity/citizen/nether/childfemale2_b.png"
        );
        assert_eq!(
            path.fallback,
            "textures/entity/citizen/default/childfemale2_b.png"
        );
        assert_ne!(path.primary, path.fallback);
    }

    #[test]
    fn the_custom_type_has_no_female_model_and_a_vanilla_layer() {
        let custom = model_by_path("custom").expect("a row");
        assert_eq!(custom.layer(true), None);
        assert_eq!(custom.layer(false), Some("minecraft:player#main"));
        assert_eq!(custom.textures, 1);
        assert_eq!(custom.id(), "minecolonies:custom");
    }

    #[test]
    fn a_model_with_no_texture_refuses_by_name() {
        let broken = ModelType::new("broken", 0, "x", None, "test");
        let err = broken
            .texture(false, 0, DEFAULT_SUFFIX, DEFAULT_FOLDER)
            .expect_err("zero textures");
        assert_eq!(err, Refusal::NoTextures { model: "broken" });
        assert_eq!(err.name(), "render.refusal.no_textures");
        assert_eq!(err.to_string(), "render.refusal.no_textures");
    }

    #[test]
    fn an_unknown_path_refuses_by_name() {
        let err = model_by_path("nonesuch")
            .map(|row| row.path)
            .ok_or(Refusal::UnknownModel { path: "nonesuch" })
            .expect_err("no such row");
        assert_eq!(err.name(), "render.refusal.unknown_model");
    }

    #[test]
    fn the_halloween_window_is_october_29_to_november_2() {
        assert!(!halloween_window(10, 28));
        assert!(halloween_window(10, 29));
        assert!(halloween_window(10, 31));
        assert!(halloween_window(11, 1));
        assert!(halloween_window(11, 2));
        assert!(!halloween_window(11, 3));
        assert!(!halloween_window(9, 30));
        assert!(!halloween_window(12, 1));
    }

    #[test]
    fn the_style_is_frozen_by_the_first_request_not_by_the_date() {
        let mut cache = StyleCache::new();
        assert!(!cache.is_frozen());
        // Joins on November 5th: no halloween, and no costume later either.
        assert_eq!(cache.halloween(11, 5), "");
        assert!(cache.is_frozen());
        assert_eq!(cache.halloween(10, 31), "", "the first answer sticks");

        let mut early = StyleCache::new();
        assert_eq!(early.halloween(10, 30), HALLOWEEN_STYLE);
        assert_eq!(early.halloween(11, 20), HALLOWEEN_STYLE);
    }

    #[test]
    fn the_cached_style_overrides_the_entity_style() {
        assert_eq!(style_folder("desert", ""), "desert");
        assert_eq!(style_folder("desert", HALLOWEEN_STYLE), HALLOWEEN_STYLE);
    }

    #[test]
    fn the_ghost_mode_never_arms_because_java_assigns_false() {
        // `EventHandler.java:773-777`: the window is computed, then the flag is
        // set to the value it already has.
        for (month, day) in [(10, 31), (11, 1), (11, 2)] {
            let ghost = Ghost::after_world_load(month, day);
            assert!(!ghost.armed, "{month}/{day}");
            assert_eq!(ghost.alpha(), 1.0);
        }
        assert_eq!(Ghost::after_world_load(6, 6).alpha(), 1.0);
        // The constant exists — the renderer would use it — it is just unreachable.
        assert!((Ghost { armed: true }.alpha() - GHOST_ALPHA).abs() < f32::EPSILON);
    }

    #[test]
    fn april_first_hides_one_of_seven_parts_by_citizen_id() {
        assert_eq!(april_hidden_part(false, 3), None);
        assert_eq!(april_hidden_part(true, 0), Some(Part::LeftArm));
        assert_eq!(april_hidden_part(true, 1), Some(Part::RightArm));
        assert_eq!(april_hidden_part(true, 6), Some(Part::RightLeg));
        assert_eq!(april_hidden_part(true, 7), Some(Part::LeftArm));
        assert_eq!(april_hidden_part(true, 100), Some(Part::Body));
        assert!(april_first(4, 1));
        assert!(!april_first(4, 2));
    }

    #[test]
    fn the_seven_parts_are_named_once_each() {
        let mut names: Vec<&str> = Part::ALL.iter().map(|part| part.name()).collect();
        names.sort_unstable();
        let mut unique = names.clone();
        unique.dedup();
        assert_eq!(unique.len(), 7);
        assert_eq!(Part::ALL.len(), 7);
    }
}
