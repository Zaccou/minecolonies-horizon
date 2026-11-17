//! L'hotel de ville : les quatre pages du brief, ce qu'elles demandent et ce
//! qu'elles rendent.
//!
//! Java reference : `core/client/gui/townhall/WindowMainPage.java` (409 lignes),
//! `WindowInfoPage.java` (311), `WindowCitizenPage.java` (297),
//! `WindowPermissionsPage.java` (751) — les quatre sources du brief — plus
//! `AbstractWindowTownHall.java` (61, la barre laterale commune),
//! `AbstractWindowSkeleton.java:75-93` et `:215-249` (le tourne-page), et
//! `core/colony/permissions/Permissions.java:315-323` (la regle d'edition).
//!
//! **Meme forme que `src/ui`, pas une deuxieme.** Un panneau est
//! [`crate::ui::field::Field`] ([`Pane::to_ui`]), une valeur est
//! [`crate::ui::Value`], une portee est [`crate::ui::field::Scope`], un effet
//! est [`crate::ui::action::Effect`]. Ce qui est propre a l'hotel de ville :
//! [`Pane::declared`] (le XML et la ligne qui **creent** l'id), [`Reader`] (quel
//! accesseur de la colonie, la ou `UiSource::Colony` ne dit que « la colonie »),
//! [`Button::scope`] (quatre listes de la meme fenetre nomment leur ligne
//! `name`) et [`Refusal`], qui n'est pas celui des huttes.
//!
//! | Fichier | Question |
//! | --- | --- |
//! | `pane` | un panneau : ce qu'il porte, d'ou vient sa valeur, la ligne qui le declare |
//! | `page` | les quatre pages, les sept onglets, la barre laterale |
//! | `citizen` | le filtre, la selection, la joie agregee, les trois etiquettes courtes |
//! | `info` | l'intervalle, les evenements, les deux fleches d'un ordre de travail |
//! | `permissions` | le rang edite, la matrice, les cases libres, le tourne-page |
//! | `main_page` | la recharge du mercenaire, les trois menus de style, le verrou |
//! | `refusal` | les vingt-deux refus, dont trois sont dits au joueur |
//!
//! # Les comptes, mesures
//!
//! | Quoi | Combien | Ou |
//! | --- | --- | --- |
//! | fichiers dans `gui/townhall/` | **14**, dont 1 base abstraite | `ls` |
//! | fenetres de cet hotel de ville | 13 | les 14 moins `AbstractWindowTownHall` |
//! | onglets de la barre laterale | **7** | [`TABS`] |
//! | pages decrites ici | **4** | [`PAGES`] |
//! | onglets qui sortent de ce module | 3 | [`Refusal::PageOutsideThisSlice`] |
//! | fenetres atteintes autrement | 6 | renommage, mercenaire, gestion, reactivation, abandon, echec de creation |
//! | vues (dont 3 enfants d'un `switch`) | **7** | [`Page::views`] |
//! | panneaux | **49** | 6 + 9 + 13 + 21 |
//! | boutons | **28** | dont 7 se replient dans `src/ui` |
//! | lecteurs | 11, dont **9** repliables sur `ui::Source` | [`Reader`] |
//! | refus | **22**, dont 3 dits | [`Refusal::key`] |
//! | panneaux de progression | **0** | aucun `%` sur ces quatre pages |
//! | panneaux de stock | **0** | aucune ligne « disponible / requis » |
//!
//! # Quatre premisses que la source contredit
//!
//! 1. **« les champs affiches (nom, niveau, progression, stock) » : deux sur
//!    quatre n'existent pas ici.** Le nom est celui de la colonie
//!    (`WindowMainPage.java:371`) ; le niveau n'a **pas de panneau**, Java
//!    l'ajoute au texte du bouton « actions » (`:93-94`) ; il n'y a **aucune**
//!    progression et **aucun** stock sur ces quatre pages — la seule valeur qui
//!    compte quelque chose est `pageNum`, soit `n/3`, un [`crate::ui::Value::Step`].
//!    Les deux autres sont sur la hutte du builder (`src/ui`, onglet reserves).
//! 2. **« les actions (embaucher, renvoyer, prioriser) » : aucun des trois n'est
//!    a cet endroit.** Embaucher et renvoyer un ouvrier sont sur la fenetre de
//!    la hutte (`src/ui::action`, `hire` et `recall`). Ici : `recallone` renvoie
//!    **un** citoyen chez lui (`WindowCitizenPage.java:160-167`) ; `plus` et
//!    `minus` priorisent **un ordre de travail**, pas un citoyen
//!    (`WindowInfoPage.java:201-210`) ; et « ajouter un joueur » est une
//!    permission, pas une embauche (`WindowPermissionsPage.java:721-726`).
//! 3. **« l'etat initial et les transitions » : l'etat initial n'est pas
//!    dessine.** `setPage` n'est jamais appele a la construction, donc le
//!    compteur de page reste vide (`label=""`, `layoutpermissions.xml:113`)
//!    jusqu'au premier clic, et le `switch` **borne** au lieu de boucler
//!    (`endless` n'est pas pose, `SwitchView.java:179-180`). Le rang edite part
//!    a l'officier avec `removeRank` eteint (`:117-118`). L'onglet ouvert cache
//!    son icone et montre son grand ruban (`AbstractWindowTownHall.java:44-47`).
//! 4. **Deux defauts de Java sont gardes et nommes, pas corriges.** La liste des
//!    evenements de permissions inverse la liste **vivante** dans
//!    `updateElement` (`:440`) et le clic indexe la liste non inversee avec
//!    l'indice inverse (`:203-207`) : le mauvais joueur est ajoute. Et
//!    `onUpdate` cache l'icone de l'entite qui dort (`:281-289`) alors que
//!    `fillCitizenInfo` la montre (`:147-152`) — voir
//!    [`citizen::entity_icon_visible`].
//!
//! # Ce qui n'est pas ici
//!
//! Le **dessin** (ni position, ni taille, ni sprite : `TRANCHE.md` §3 M2),
//! l'**envoi** (aucun message ne part), les **valeurs** elles-memes (un panneau
//! dit d'ou sa valeur vient, jamais ce qu'elle est : `crate::colony::Citizen`
//! n'a qu'un id et un nom), et les **trois pages hors module**. Les manques
//! mesures sont dans `MANQUANT.md`.

pub mod citizen;
pub mod info;
pub mod main_page;
pub mod page;
pub mod pane;
pub mod permissions;
pub mod refusal;

pub use citizen::{
    entity_icon_visible, face, filtered, row_value, short_happiness, short_health,
    short_saturation, title_number, Face, Selection, Vue, HAPPINESS_CEILING,
    SATURATION_CEILING,
};
pub use info::{
    arrows_hidden, days_of, drop_order, kept, keeps, lower, one_line, raise, row_of, sorted,
    Evenement, EventKind, DEFAULT_KEY, INTERVALS, STATS_DEFAULT_KEY,
};
pub use main_page::{
    cooldown_left, mercenary_ready, picker_sends, visuals, Picker, Visuals, PACK_RESET_INDEX,
    TICKS_FORTY_MIN,
};
pub use page::{
    open_tab, page_by_id, page_by_java, sidebar, tab, Act, Bookmark, Button, Local, Page, Tab,
    Target, Title, View, PAGES, TABS, ADD_PLAYER_VIEW, FREE_BLOCK_VIEW, MANAGE_RANKS_VIEW,
};
pub use pane::{Pane, Reader};
pub use permissions::{
    can_alter, free_at, looks_like_block, may_add_player, may_add_rank, may_remove_player, on_off,
    on_off_key, parse_free, parse_position, rank_name, type_index, type_key, FreeEntry, FreeKind,
    Pages, Ranks, INITIAL_RANK, TYPE_KEYS, VIEWS,
};
pub use refusal::Refusal;
