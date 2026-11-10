fn nbm_r_player_tick(game: &mut crate::game::Game<'_>) {
    if !game.native_on(crate::game::nbm_r::ID) {
        return;
    }
    let Some(who) = game.native_acting_player() else {
        return;
    };
    if matches!(who.game_mode, 1 | 3) {
        return;
    }

    let chunk = crate::game::nbm_r::dosimetry::pos_to_chunk(who.position[0], who.position[2]);
    let chunk_rad = crate::game::nbm_r::get_chunk_radiation(&who.dimension, chunk.0, chunk.1);

    let mut hazmat_pieces = 0;
    for stack in game.armor.iter().flatten() {
        if crate::game::nbm_r::content::is_hazmat_piece(&stack.item) {
            hazmat_pieces += 1;
        }
    }

    let mut items: Vec<(String, u32)> = Vec::new();
    for stack in game.inventory.iter().flatten() {
        let emission = crate::game::nbm_r::content::item_radiation_emission(&stack.item);
        if emission > 0.0 && stack.count > 0 {
            items.push((stack.item.to_string(), stack.count));
        }
    }
    if let Some(stack) = &game.offhand {
        let emission = crate::game::nbm_r::content::item_radiation_emission(&stack.item);
        if emission > 0.0 && stack.count > 0 {
            items.push((stack.item.to_string(), stack.count));
        }
    }

    let items_ref: Vec<(&str, u32)> = items.iter().map(|(id, cnt)| (id.as_str(), *cnt)).collect();
    crate::game::nbm_r::record_player_exposure(
        who.uuid,
        chunk_rad,
        &items_ref,
        hazmat_pieces,
        0.05,
    );
}
