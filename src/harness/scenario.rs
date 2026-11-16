//! "Place the hut, click it, hire": the slice replayed the way the engine runs it.

use mod_api::__private::built;
use mod_api::{
    BlockClick, BlockEntityEvent, BlockEntityPart, BlockHit, BlockState, BlockStateRecord, Command,
    CommandArgument, CommandArgumentValue, CommandInvocation, CommandSource, Compound, DimensionKey,
    Flow, Hand, HostSide, InteractionResult, LevelDimension, LevelSide, LogicalSide, PermissionLevel,
    RegisterCommandsEvent, RightClickBlockEvent, RunModCommandEvent, TriState,
};

use crate::harness::{FakeHost, block_key};
use crate::harness::host::PLAYER_ENTITY;
use crate::{hire, hut, menu};

/// A world with one hut in it, and the player the steps run as.
pub struct Scenario {
    /// The host the port talks to, and the log of what it asked.
    pub host: FakeHost,
    /// Where the hut stands.
    pub hut: [i32; 3],
    uuid: u128,
}

impl Scenario {
    /// Puts `hut::BLOCK_ID` at `pos` facing `facing`, and gives its block entity the
    /// compound the Part answers.
    ///
    /// This is the engine's placement, not a test shortcut: `block_entity` fires
    /// inside the write that creates the entity (`LevelChunk.setBlockState`), and a
    /// Part that answers nothing leaves an empty compound
    /// (`crates/horizon-mod-vocab/src/hooks/block_entity.rs:5-11`). So a port that
    /// stopped writing its initial data is visible here as a hut with no data.
    pub fn place(pos: [i32; 3], facing: &str) -> Self {
        let mut host = FakeHost::new();
        host.level.put_block(pos, hut::BLOCK_ID, Some(facing));
        let mut event = BlockEntityEvent {
            block: hut::BLOCK_ID,
            state: state_of(hut::BLOCK_ID),
        };
        let data = hut::Hut
            .block_entity(LevelSide::Server(&host), &mut event)
            .unwrap_or_default();
        host.level.put_data(pos, data);
        Self {
            host,
            hut: pos,
            uuid: 0,
        }
    }

    /// The player whose click or command is running.
    pub fn with_player(mut self, uuid: u128) -> Self {
        self.host.join(uuid);
        self.uuid = uuid;
        self
    }

    /// `register_commands`: the usages the mod declared, as the server's tree gets
    /// them.
    pub fn register(&mut self) -> Vec<Command> {
        let mut commands = Vec::new();
        let mut event = RegisterCommandsEvent {
            commands: &mut commands,
        };
        hire::register_commands(&mut self.host, &mut event);
        commands
    }

    /// `right_click_block`: the click on the hut, on the server side, empty-handed.
    /// The event is built the way the game builds it — through
    /// `__private::built::BlockClick`, whose fields are the game's own
    /// (`crates/horizon-mod-vocab/src/built.rs:194-224`) — because the API's
    /// `BlockClick` is `#[non_exhaustive]` and has no constructor for a mod.
    pub fn click(&mut self) -> Flow {
        let built_click = built::BlockClick {
            side: LogicalSide::Server,
            dimension: LevelDimension::Overworld,
            pos: self.hut,
            hit: BlockHit::new(centre(self.hut), [0, 1, 0], false),
            hand: Hand::Main,
            stack: None,
            player: self.uuid,
            player_entity_id: PLAYER_ENTITY as i32,
        };
        let click = BlockClick::from(built_click);
        let mut use_block = TriState::Default;
        let mut use_item = TriState::Default;
        let mut cancellation_result = InteractionResult::default();
        let mut event = RightClickBlockEvent {
            click: &click,
            use_block: &mut use_block,
            use_item: &mut use_item,
            cancellation_result: &mut cancellation_result,
            cancelled: false,
        };
        menu::right_click_block(HostSide::Server(&mut self.host), &mut event)
    }

    /// `/mcproto hire <pos>`, run from the console: the path that needs no screen.
    pub fn hire(&mut self) -> Flow {
        let invocation = CommandInvocation::new(
            vec![hire::ROOT.to_owned(), "hire".to_owned()],
            vec![CommandArgument::new(
                hire::POS.to_owned(),
                CommandArgumentValue::BlockPos(self.hut),
            )],
            DimensionKey::OVERWORLD,
            centre(self.hut),
        );
        let source = CommandSource::Console {
            permission: PermissionLevel::Owners,
        };
        let mut event = RunModCommandEvent {
            source: &source,
            command: &invocation,
            cancelled: false,
        };
        hire::run(&mut self.host, &mut event)
    }

    /// The data the hut's block entity holds now.
    pub fn data(&self) -> Compound {
        self.host.level.data_at(self.hut).cloned().unwrap_or_default()
    }

    /// Its fields, read the way the port reads them.
    pub fn state(&self) -> hut::State {
        hut::state_of(&self.data())
    }
}

/// The block state a placement writes: the block's key, state ordinal 0.
fn state_of(block: &str) -> BlockState {
    BlockStateRecord::new(block_key(block), 0).into()
}

/// The centre of a block, which is where a click lands and what a command source
/// reads as its position.
fn centre(pos: [i32; 3]) -> [f64; 3] {
    [
        f64::from(pos[0]) + 0.5,
        f64::from(pos[1]),
        f64::from(pos[2]) + 0.5,
    ]
}
