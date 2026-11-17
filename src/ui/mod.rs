//! The hut screens: **the data each screen asks for, and the actions it renders**.
//!
//! Java reference: `core/client/gui/` — the `WindowHut*` files, the four hut windows that
//! are not named that way, the two `Abstract*` bases, and the `getWindow()` chain that
//! decides which one a building opens (`api/colony/buildings/views/IBuildingView.java:157`,
//! `AbstractBuildingView.java:390-399`, `api/colony/buildings/modules/IBuildingModuleView.java:47`).
//!
//! **Not the drawing.** No `pos`, no `size`, no sprite, no slot layout: those are the
//! XML's and `TRANCHE.md` §3 M2 already says this slice has none. What is here is what a
//! window class *asks a view for* and what its buttons *send back* — the two things a
//! future client slice can be built on, and the two things a test can hold still.
//!
//! | File | Question |
//! | --- | --- |
//! | [`field`] | what one pane holds, and where its value comes from |
//! | [`action`] | what one button does, and every way it is refused |
//! | [`screen`] | the thirteen screens: title, fields, buttons, gate |
//! | [`table`] | which screen a hut opens, and which screen a métier reaches |
//!
//! # The counts, because four of the obvious ones are wrong
//!
//! | What | How many | Where |
//! | --- | --- | --- |
//! | `WindowHut*.java` files | **9** | `find core/client/gui -name 'WindowHut*.java'` |
//! | screens this module describes | 13 | [`screen::SCREENS`] |
//! | of those, a `WindowHut*` file | 9 | [`screen::HUT_WINDOWS`] |
//! | building kinds | 54 | `Kind::ALL` (`jobs` note) |
//! | kinds that open a hut screen | 52 | [`table::main_screen`] |
//! | kinds that share the worker screen | **43** | [`screen::HUT_WORKER_ID`] |
//! | kinds that share the minimum screen | 5 | [`screen::HUT_MIN_ID`] |
//! | module keys that name a tab | 5 → 3 screens | [`table::MODULE_SCREENS`] |
//! | métiers that reach a screen | 49 | [`table::screen_of`] |
//! | **main screens those 49 reach** | **3** | worker, minimum, and the builder's own |
//! | métiers that reach two screens | 2 — quarrier, courier | [`table::screens_of`] |
//!
//! # Three premises the source contradicts
//!
//! 1. **"un `WindowHut*` par métier" is false, and the count says why: nine files for
//!    forty-nine métiers.** They are generic windows. `WindowHutWorkerModulePlaceholder`
//!    is opened by every hut that carries a `WorkerBuildingModule`
//!    (`AbstractBuildingView.java:392-399`), and it reads the *module* for the job, the
//!    two skills and the head count. **The métier lives in the module, not in the
//!    window**: the forty-nine métiers reach three main screens between them, and the four
//!    windows that are not `WindowHut*` are the barracks, the builder's reserves tab, the
//!    hire window and the home assignment window.
//!
//! 2. **`AbstractWindowBuilding` does not exist.** The base classes are
//!    `AbstractBuildingWindow` (`AbstractBuildingWindow.java:25`, which builds the
//!    sidebar) and `AbstractBuildingMainWindow` (`:25`, which owns the title and the five
//!    inherited buttons). Searching the tree for `AbstractWindowBuilding` answers nothing;
//!    the name in the brief is not a class.
//!
//! 3. **"progression" and "stock" are not on a hut screen.** Of the thirteen screens,
//!    **one** shows a percentage and a stock line — the builder's reserves tab
//!    (`WindowBuilderResModule.java:105`, `:209`). The worker hut shows a name, a level, a
//!    worker name and a delivery priority, and its stock is behind the `inventory` button,
//!    in the game's own container or in the separate all-inventory window. A caller that
//!    expects a progress bar on every hut is looking for a pane Java never draws; the test
//!    `the_reserves_tab_is_the_only_screen_that_shows_progress_and_stock` holds the count.
//!
//! A fourth, smaller one: **the rename field and the rename rule disagree in Java.** The
//! XML takes twenty-five characters (`windowhutnameentry.xml:6`) and the class cuts at
//! fifteen with a warning (`WindowHutNameEntry.java:28`, `:59-63`). Both numbers are kept
//! ([`action::MAX_NAME_LENGTH`], [`action::NAME_FIELD_MAX_LENGTH`]) because only the second
//! one is a rule.
//!
//! # What is *not* here, and what each gap costs
//!
//! - **the drawing.** The SDK has a mod screen and two widgets
//!   (`crates/horizon-mod-vocab/src/client/widgets.rs:1-30`: a `Widgets` group with
//!   `add_button` and `add_text_field`), and **no list**, while seven of these thirteen
//!   screens are lists (workers, residents, spawn points, resources, mine levels, tasks,
//!   all-inventory). `src/ui/MANQUANT.md` says exactly which widget is missing and what it
//!   would have been used for.
//! - **which hut carries which module.** A tab appears when the *building* declares the
//!   module (`AbstractBuildingWindow.java:67-83` walks `getAllModuleViews()`), so the
//!   membership is `ModBuildingsInitializer`'s business, not a screen's. This table maps a
//!   module key to a screen; the hut × module rows are the employment table's for the
//!   worker module and the buildings slice's for the rest.
//! - **the values themselves.** A screen here says *where* a value comes from
//!   ([`field::Source`]), never what it is: the port has no client building view, and the
//!   compound a hut saves (`crate::hut::State`) holds a level and a head count, not a
//!   worker list.
//! - **the network.** Sixteen of these actions carry the **name** of a Java message
//!   ([`action::Effect::Sends`]) and nothing is sent: there is no client, no packet and no
//!   server-side handler for `HireFireMessage` in this port yet.
//! - **the other windows.** The town hall and the postbox have a window and no hut screen
//!   ([`action::Refusal::NotAHutScreen`]); `WindowInfo`, `WindowBuildBuilding`,
//!   `WindowHutGuide`'s spy list and the build tool are named by class
//!   ([`action::Effect::OpensJava`]) and not described.

pub mod action;
pub mod field;
pub mod screen;
pub mod table;

pub use action::{
    Action, BuildLabel, Effect, HiringMode, Refusal, build_label, hands_on_housing, may_assign,
    may_unassign, next_hiring_mode, next_sort, rename, MAX_NAME_LENGTH, NAME_FIELD_MAX_LENGTH,
    SORT_STAGES,
};
pub use field::{
    Field, PriorityText, Scope, Source, Value, alter_priority, percent, priority_text, stock_text,
    OCCUPANCY_KEY, PRIORITY_MAX, PRIORITY_NEVER_KEY, PRIORITY_PREFIX_KEY, PROGRESS_KEY, STEP_KEY,
};
pub use screen::{Gate, Screen, Title, HUT_WINDOWS, SCREENS};
pub use table::{
    has_worker_module, main_screen, module_screen, screen_by_id, screen_of, screens_of, worker_hut,
    MODULE_SCREENS,
};
