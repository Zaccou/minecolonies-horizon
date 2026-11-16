# MineColonies for Horizon

An unofficial Rust port of [MineColonies](https://github.com/ldtteam/minecolonies) to
[Horizon](https://github.com/horizon-rust/horizon)'s mod API.

Not made or endorsed by the MineColonies team (LDT Team). All blocks, textures, recipes and the original
design are theirs, under GPL-3.0; this port keeps that licence and credits them in `NOTICE`.

## Where it stands

This is a **work in progress**, published early so the porting work is visible and reusable. Measured on
2026-11-16:

| | |
|---|---|
| Rust | ~24 200 lines |
| Tests | **406 passing** |
| Modules | 14 |
| Mod API items used | 574, **none added to the API** |

What compiles and is covered by tests today:

- **`base`** — the interfaces MineColonies implements, its constants with their Java values, and the position
  geometry (block, chunk, section, colony bounds).
- **`colony`** — `Colony`, the 27-bit rank permission table, work orders.
- **`building`** — the 50 building types, per-level costs, requirements.
- **`requests`** — requests, resolver, priority queue.
- **`jobs`** — the 49 jobs, employment, tools, the experience curve.
- **`citizens`** — states, transitions, priorities, needs.
- **`pathfinding`** — A*, typed nodes, walkable-path validation.
- **`commands`** — `/colony` and its nine subcommands.
- **`inventory`**, **`harness`** (a fake host for tests), **`wiring`** (`/colony` dispatched).
- **`advancements`** — the 62 colony advancements as pure predicates.
- **`founding`** — the first whole journey: a laid town hall becomes a colony, every refusal named, the
  writes ordered so a test proves none happens before a refusal.

Content: the MineColonies blocks that are plain content, their recipes, loot tables and tags.

What is **not** there yet, and why:

| Missing | Blocked by |
|---|---|
| Citizen AI (60 773 Java lines) | a per-entity tick and off-thread computation |
| Citizen rendering (14 202) | live entity types |
| The 128 network messages (~15 000) | packet emission |
| Item and block behaviour needing materials | the materials declaration format |

These are being moved to Horizon's **native route** (`[[native.mixin]]`, `[[native.loop]]`), which is how the
other ports — Create, Applied Energistics 2, Baritone — reach the game's own types.

## Building

This is a **native mod**: the package carries the crate, and the launcher compiles it against the exact game
build it runs with.

```bash
horizon-launcher native-build path/to/minecolonies-horizon --trust-native-code
```

`--trust-native-code` says you trust the mod's code to run with your rights: a native mod is not sandboxed.

The manifest pins the game build in `[native] horizon`; change that commit and rebuild to move to a newer
build.

## Licence

GPL-3.0-or-later, inherited from MineColonies. See `LICENSE` and `NOTICE`.
