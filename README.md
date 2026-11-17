# MineColonies for Horizon

An unofficial Rust port of [MineColonies](https://github.com/ldtteam/minecolonies) to
[Horizon](https://github.com/horizon-rust/horizon). Not made or endorsed by the MineColonies team
(LDT Team): every block, texture, recipe, advancement and blueprint is theirs, under GPL-3.0, and this
port keeps that licence. See `NOTICE`.

## Where it stands

A **work in progress, ported but never executed**. Measured on 2026-11-17:

| | |
|---|---|
| Rust | ~305 000 lines, **95 modules** |
| Tests | **5 492 passing** |
| Content | 86 blocks · 112 items · 154 recipes · 146 loot tables · 372 models · 284 textures |
| Blueprints | 9 515, as the Java mod ships them |

The Java source is 313 960 lines, so the code volume is comparable — but volume is not the measure
that matters here, and this README does not pretend otherwise.

### What is ported

| Domain | Modules |
|---|---|
| Colony | `colony`, `founding`, `manager`, `requests`, `events`, `advancements`, `persistence` |
| Buildings | `building` (50 types, per-level costs, behaviour: place, upgrade, break) |
| People | `citizens`, `citizens::happiness`, `entity`, `jobs` (49 jobs), `jobs::kinds` |
| Work | `ai` (the decision), `ai_gather`, `ai_build`, `ai_work_01…12` (the 36 job goals) |
| World | `blocks` (86), `items` (112), `structures`, `blueprint`, `placement`, `worldgen`, `tile_entities` |
| Interface | `ui`, `ui_window_01…17`, `widgets`, `client_gui`, `client_model`, `render` |
| Plumbing | `network` (127 messages), `channel`, `crafting`, `recipes_content`, `compat`, `util` |

Every rule cites its Java source as `file.java:line`, in a comment or in a `source` field.

### What is *not* done — read this before the table above

**Nothing here has ever run in the game.** The port compiles and its tests pass; no one has seen a
colony appear. Three things stand between this and a playable mod:

1. **The wiring.** The rules are a library: `src/lib.rs` still registers one hut block and two
   commands, from the first vertical slice. Connecting the modules to the game's hooks is in progress
   (`wire_founding`, `wire_click`, `wire_tick`, `wire_persistence`, `wire_commands`).
2. **A per-entity tick.** Horizon's entity tick hook is global, so the 36 AI goals — the largest single
   piece of this port — can decide but are never called each tick.
3. **The native build.** `horizon-launcher native-build` stops on
   `The mod tools for nightly-2026-11-13 aren't ready yet`: the artefact is missing on their CI, not in
   this repository. Until it exists, the module cannot be built for the game at all.

And one caveat on quality: **fidelity audits are a work in progress.** Eight have been run over ~300
Java methods, finding and fixing real divergences (7 in `compatibility`, 5 in `game_events`, 3 in
`generation`) — but most of the 305 000 lines have not been compared line by line with the Java yet.

## Native mod, not WebAssembly

The package carries the **crate**; the launcher compiles it against the exact game build it runs with.
That is how the other ports in the registry work — Create, Applied Energistics 2, Baritone — and it is
why no SDK download or ABI hash has to match.

```toml
[native]
game = "26.3"
horizon = "db7638b3…"
mixins = "e59da909…"
```

Horizon's **native route** (`[[native.mixin]]`, `[[native.loop]]`, `[[native.bridge]]`) is what makes a
port of this size possible: it reaches the game's own types where the sandboxed API stops.

## Building

```bash
cargo test                     # the only judge of "it compiles"
horizon-launcher install latest
horizon-launcher prepare --version <tag>
horizon-launcher rust install
horizon-launcher native-build . --server-output out --trust-native-code
```

The last command currently fails on the missing mod tools described above.

## Licence

GPL-3.0-or-later, inherited from MineColonies. See `LICENSE` and `NOTICE`.
