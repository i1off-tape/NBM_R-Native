fn nbm_r_interaction(
    game: &mut crate::game::Game<'_>,
    button: i32,
    key_mapping: &str,
    hand: horizon_client::first_person::Hand,
    swing_hand: &mut bool,
    canceled: &mut bool,
) {
    let _ = key_mapping;
    if !game.native_on(crate::game::nbm_r::ID) {
        return;
    }

    // button 1 is key.use (right click)
    if button != 1 {
        return;
    }

    let held_item = match hand {
        horizon_client::first_person::Hand::Main => {
            game.inventory.get(game.selected).and_then(Option::as_ref)
        }
        horizon_client::first_person::Hand::Off => game.offhand.as_ref(),
    };

    let Some(stack) = held_item else {
        return;
    };

    if !crate::game::nbm_r::geiger_hud::is_geiger_counter_item(&stack.item) {
        return;
    }

    *canceled = true;
    *swing_hand = true;

    if let Some(p) = game.native_player() {
        let sound_name = crate::game::nbm_r::geiger_hud::SOUND_TECH_BOOP;
        game.native_play_local_sound(sound_name, [p.x, p.y, p.z], 1.0, 1.0, 0);

        let (chunk_rad, env_rad) = crate::game::nbm_r::calculate_ambient_radiation_at(
            &p.dimension,
            p.x,
            p.z,
            game.inventory
                .iter()
                .flatten()
                .chain(game.offhand.as_ref())
                .map(|s| (s.item.as_ref(), s.count)),
        );

        let player_dose = crate::game::nbm_r::get_player_dosimetry(0).dose;

        let hazmat_pieces = crate::game::nbm_r::count_hazmat_pieces(
            game.armor.iter().flatten().map(|s| s.item.as_ref()),
        );

        let summary = crate::game::nbm_r::geiger_hud::format_geiger_summary(
            chunk_rad,
            env_rad,
            player_dose,
            hazmat_pieces,
        );
        for line in summary.lines() {
            game.native_chat(line);
        }
    }
}
