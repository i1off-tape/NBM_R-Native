fn nbm_r_audio_tick(game: &mut crate::game::Game<'_>) {
    if !game.native_on(crate::game::nbm_r::ID) {
        return;
    }
    if !game.native_window() {
        return;
    }
    let Some(p) = game.native_player() else {
        return;
    };

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

    if !crate::game::nbm_r::geiger_hud::has_geiger_counter(items) {
        return;
    }

    let (cx, cz) = crate::game::nbm_r::dosimetry::pos_to_chunk(p.x, p.z);
    let chunk_rad = crate::game::nbm_r::get_chunk_radiation(&p.dimension, cx, cz);

    let mut inv_emission = 0.0;
    for stack in game.inventory.iter().flatten() {
        if stack.count > 0 {
            inv_emission +=
                crate::game::nbm_r::content::item_radiation_emission(&stack.item) * stack.count as f32;
        }
    }
    if let Some(stack) = &game.offhand {
        if stack.count > 0 {
            inv_emission +=
                crate::game::nbm_r::content::item_radiation_emission(&stack.item) * stack.count as f32;
        }
    }

    let ambient_rad = chunk_rad + inv_emission;

    let master_vol = game.settings.master_volume;
    let player_vol = game.settings.volumes.get(7).copied().unwrap_or(1.0);
    let effective_vol =
        crate::game::nbm_r::geiger_hud::effective_audio_volume(1.0, master_vol, player_vol);
    if effective_vol <= 0.0 {
        return;
    }

    let should_click =
        crate::game::nbm_r::lock_geiger_audio(|state| state.tick(ambient_rad, 0.05));

    if should_click {
        let at = [p.x, p.y, p.z];
        game.native_play_local_sound("nbm_r:geiger.click", at, effective_vol, 1.0, 0);
    }
}
