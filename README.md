# MineColonies for Horizon

An unofficial Rust port of [MineColonies](https://github.com/ldtteam/minecolonies) to
[Horizon](https://github.com/horizon-rust/horizon)'s mod API.

Not made or endorsed by the MineColonies team (LDT Team). All blocks, items, textures, recipes, loot
tables, advancements, structure blueprints and the original design are theirs, under GPL-3.0. This port
keeps that licence and credits them in `NOTICE`.

## Where it stands

A **work in progress**, published early so the porting work is visible and reusable. Measured on
2026-11-16:

| | |
|---|---|
| Rust | **44 734 lines** |
| Tests | **718 passing** |
| Modules | 20 |
| Content | 154 recipes · 146 loot tables · 98 tags · 9 515 building blueprints |

Every rule the port implements cites its Java source as `file.java:line`, in a comment or in a
`source` field. Nothing here was written from memory.

### What is ported

| Module | What it carries |
|---|---|
| `base` | the interfaces MineColonies implements, its constants with their Java values, and the position geometry (block, chunk, section, colony bounds) |
| `colony` | `Colony`, the 27-bit rank permission table, work orders |
| `building` | the 50 building types, per-level costs, requirements, modules |
| `requests` | requests, resolver, priority queue |
| `jobs` | the 49 jobs, employment, tools, the experience curve |
| `jobs::kinds` | **what each of the 49 jobs does**: accepted items with their minimum tool level, products and their ticks, conditions, experience grants (127 accepted items, 46 products, 94 named constants) |
| `citizens` | states, transitions, priorities, needs |
| `ai` | **the decision, not the world**: the state machine and its four passes, the selector fed by `WorldFacts`, priorities, give-up conditions — testable without a game running |
| `network` | the **127 messages** of the Java mod, one type each, with compound round trips |
| `pathfinding` | A*, typed nodes, walkable-path validation |
| `commands` | `/colony` and its nine subcommands |
| `advancements` | the 62 colony advancements as pure predicates |
| `founding` | the first whole journey: a laid town hall becomes a colony, every refusal named, the writes ordered so a test proves none happens before a refusal |
| `structures` | resolving a building and a level to the template to place |
| `inventory`, `harness`, `wiring`, `hut`, `hire`, `menu`, `citizen` | inventory and stacks, a fake host for tests, `/colony` dispatch, the hut block entity |

Content: the recipes, loot tables and tags copied verbatim from the generated source, and the 9 515
blueprints the mod ships.

### What is not there yet

| Missing | Why |
|---|---|
| Placing a whole colony in a real world | the templates convert (9 515/9 515, verified cell by cell) but weigh 1.9 GB; the port ships the 57 MB of blueprints and converts on demand |
| The other 86 blocks and their behaviour | they descend from `AbstractColonyBlock`; the native route now reaches what they need |
| Screen and entity rendering | the Java models do not transpose |
| Registering the network channel | the messages are ported, the channel is not |

## Native mod, not WebAssembly

The package carries the **crate**; the launcher compiles it against the exact game build it runs
with. That is how every ported mod in the registry works — Create, Applied Energistics 2, Baritone —
and it is why no SDK or ABI hash has to match.

The manifest pins one build:

```toml
[native]
game = "26.3"
horizon = "db7638b3…"
mixins = "e59da909…"
```

Horizon's **native route** is what makes a port of this size possible: `[[native.mixin]]` to change a
game function, `[[native.loop]]` to run code inside a published hot loop such as the block entity
tick, `[[native.bridge]]` to reach private fields. The port already declares one loop slot
(`src/loops/hut_tick.rs`, `block_entities.tick`).

## Building

```bash
horizon-launcher install latest
horizon-launcher prepare --version <tag>
horizon-launcher rust install
horizon-launcher native-build path/to/minecolonies-horizon --server-output out --trust-native-code
```

`--server-output` builds a standalone dedicated-server engine: MineColonies is a server mod, and this
is the mode that proves a native mod can run one. `--trust-native-code` says you read the code and
trust it with your rights — a native mod is not sandboxed.

## Licence

GPL-3.0-or-later, inherited from MineColonies. See `LICENSE` and `NOTICE`.
