// Le slot de la boucle publiée `block_entities.tick`, pour les block entities de mod.
//
// Ce fichier est injecté comme module du crate du jeu : il voit ce que voit
// `horizon_client::game::block_entity_ticks`, sans aucune ligne `use`. Il peut appeler le crate du
// port par son nom Rust (`ported_minecolonies`).

/// Compte les ticks que la boucle du jeu donne aux block entities de mod, et transmet au port.
#[derive(Default)]
pub struct HutTick {
    seen: u64,
}

impl BlockEntityTickSlot for HutTick {
    fn before(&mut self, tick: &mut BlockEntityTick<'_, '_>) -> LoopFlow {
        // `Mod` est le genre que le jeu donne à tout block entity d'un mod : c'est le nôtre.
        if tick.kind != TickedKind::Mod {
            return LoopFlow::Continue;
        }
        self.seen += 1;
        // La preuve de la chaîne : le tick traverse le jeu, le slot, et arrive dans le crate du port.
        ported_minecolonies::hut::note_mod_tick(self.seen);
        LoopFlow::Continue
    }
}
