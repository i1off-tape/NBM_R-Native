fn nbm_r_tick(game: &mut crate::game::Game<'_>) {
    if !game.native_on(crate::game::nbm_r::ID) {
        return;
    }

    // 1. Level chunk diffusion and decay
    crate::game::nbm_r::on_level_tick("minecraft:overworld", false);

    // 2. Player dosimetry and exposure update & 3. Sickness evaluation and damage
    if let Some(p) = game.native_player() {
        let chunk = crate::game::nbm_r::dosimetry::pos_to_chunk(p.x, p.z);
        let chunk_rad = crate::game::nbm_r::get_chunk_radiation(&p.dimension, chunk.0, chunk.1);

        let mut hazmat_pieces = 0;
        for stack in game.armor.iter().flatten() {
            if crate::game::nbm_r::content::is_hazmat_piece(&stack.item) {
                hazmat_pieces += 1;
            }
        }

        let items_iter = game
            .inventory
            .iter()
            .flatten()
            .chain(game.offhand.as_ref())
            .map(|s| (s.item.as_ref(), s.count));
        let items_vec: Vec<(&str, u32)> = items_iter.collect();
        let inv_emission = crate::game::nbm_r::total_item_radiation(items_vec.iter().copied());

        crate::game::nbm_r::record_player_exposure(
            0,
            chunk_rad,
            &items_vec,
            hazmat_pieces,
            0.05,
        );
        let _eval = crate::game::nbm_r::tick_player_sickness(0);
    }

    // 4. Geiger counter audio scheduling and click dispatch
    if game.native_window() {
        if let Some(p) = game.native_player() {
            let mut items = Vec::new();
            for slot in 0..9.min(game.inventory.len()) {
                if let Some(stack) = &game.inventory[slot] {
                    if stack.count > 0 {
                        items.push(stack.item.as_ref());
                    }
                }
            }
            if let Some(stack) = &game.offhand {
                if stack.count > 0 {
                    items.push(stack.item.as_ref());
                }
            }

            if crate::game::nbm_r::geiger_hud::has_geiger_counter(items) {
                let (cx, cz) = crate::game::nbm_r::dosimetry::pos_to_chunk(p.x, p.z);
                let chunk_rad = crate::game::nbm_r::get_chunk_radiation(&p.dimension, cx, cz);

                let inv_emission = crate::game::nbm_r::total_item_radiation(
                    game.inventory
                        .iter()
                        .flatten()
                        .chain(game.offhand.as_ref())
                        .map(|s| (s.item.as_ref(), s.count)),
                );
                let ambient_rad = chunk_rad + inv_emission;

                let master_vol = game.settings.master_volume;
                let player_vol = game.settings.volumes.get(7).copied().unwrap_or(1.0);
                let effective_vol =
                    crate::game::nbm_r::geiger_hud::effective_audio_volume(1.0, master_vol, player_vol);
                if effective_vol > 0.0 {
                    let should_click =
                        crate::game::nbm_r::lock_geiger_audio(|state| state.tick(ambient_rad, 0.05));

                    if should_click {
                        game.player_menu_sound("nbm_r:geiger.click");
                    }
                }
            }
        }
    }
}
