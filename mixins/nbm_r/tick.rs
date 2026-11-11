fn nbm_r_tick(game: &mut crate::game::Game<'_>) {
    if !game.native_on(crate::game::nbm_r::ID) {
        return;
    }

    // 1. Level chunk diffusion and decay
    crate::game::nbm_r::on_level_tick("minecraft:overworld", false);

    // 2. Server-side authoritative player dosimetry, exposure, and sickness handling
    let server_players = game.native_server_players();
    for sp in &server_players {
        if sp.health <= 0.0 {
            crate::game::nbm_r::on_player_death(sp.uuid);
            let _ = game.native_run_command(&format!("effect clear {}", sp.name));
            continue;
        }

        let (cx, cz) = crate::game::nbm_r::dosimetry::pos_to_chunk(sp.position[0], sp.position[2]);
        let chunk_rad = crate::game::nbm_r::get_chunk_radiation(&sp.dimension, cx, cz);

        let mut items_vec: Vec<(&str, u32)> = Vec::new();
        let stacks_storage;
        if let Some(stacks) = game.native_player_stacks(sp.uuid) {
            stacks_storage = stacks;
            for stack in stacks_storage.iter().flatten() {
                if stack.count > 0 {
                    items_vec.push((stack.item.as_ref(), stack.count));
                }
            }
        }

        let mut hazmat_pieces = 0;
        for slot in [
            horizon_client::mod_api::EquipmentSlot::Head,
            horizon_client::mod_api::EquipmentSlot::Chest,
            horizon_client::mod_api::EquipmentSlot::Legs,
            horizon_client::mod_api::EquipmentSlot::Feet,
        ] {
            if let Some(stack) = game.native_player_equipment(sp.uuid, slot) {
                if crate::game::nbm_r::content::is_hazmat_piece(&stack.item) {
                    hazmat_pieces += 1;
                }
            }
        }
        if hazmat_pieces == 0 && server_players.len() == 1 {
            hazmat_pieces = crate::game::nbm_r::count_hazmat_pieces(
                game.armor.iter().flatten().map(|s| s.item.as_ref()),
            );
        }

        crate::game::nbm_r::record_player_exposure(
            sp.uuid,
            chunk_rad,
            &items_vec,
            hazmat_pieces,
            0.05,
        );

        let eval = crate::game::nbm_r::tick_player_sickness(sp.uuid);
        if eval.damage.is_some() {
            crate::game::nbm_r::on_player_death(sp.uuid);
            let _ = game.native_run_command(&format!("effect clear {}", sp.name));
            let _ = game.native_run_command(&format!("kill {}", sp.name));
        } else if eval.stage.is_sick() {
            let ticks = crate::game::nbm_r::get_player_dosimetry(sp.uuid).ticks_alive;
            // Refreshes active status effects every 40 ticks (2 seconds) per MAPPING.md:51
            if ticks % 40 == 1 {
                for effect in &eval.effects {
                    let cmd = effect.to_command(&sp.name);
                    let _ = game.native_run_command(&cmd);
                }
            }
        }
    }

    // 3. Client-side local player dosimetry and exposure update & sickness evaluation
    if let Some(p) = game.native_player() {
        // When server is driving this session, server authoritatively handles dosimetry and sickness.
        // Client only drives dosimetry in standalone / mock testing mode (server_players.is_empty()).
        if server_players.is_empty() {
            let chunk = crate::game::nbm_r::dosimetry::pos_to_chunk(p.x, p.z);
            let chunk_rad = crate::game::nbm_r::get_chunk_radiation(&p.dimension, chunk.0, chunk.1);

            let hazmat_pieces = crate::game::nbm_r::count_hazmat_pieces(
                game.armor.iter().flatten().map(|s| s.item.as_ref()),
            );

            let items_iter = game
                .inventory
                .iter()
                .flatten()
                .chain(game.offhand.as_ref())
                .map(|s| (s.item.as_ref(), s.count));
            let items_vec: Vec<(&str, u32)> = items_iter.collect();

            crate::game::nbm_r::record_player_exposure(
                0,
                chunk_rad,
                &items_vec,
                hazmat_pieces,
                0.05,
            );

            let eval = crate::game::nbm_r::tick_player_sickness(0);
            if eval.damage.is_some() {
                crate::game::nbm_r::on_player_death(0);
                if let Some(p) = game.player.as_mut() {
                    let _ = p.set_health(0.0, false);
                }
                game.ui.death_cause = Some("death.attack.genericKill".into());
                game.hurt_player_by("minecraft:generic_kill", 1000.0);
                let _ = game.native_run_command(crate::game::nbm_r::sickness::LETHAL_COMMAND);
            } else if eval.stage.is_sick() {
                let ticks = crate::game::nbm_r::get_player_dosimetry(0).ticks_alive;
                if ticks % 40 == 1 {
                    for effect in &eval.effects {
                        game.give_player_effect(effect.id, effect.duration_ticks, effect.amplifier as i32);
                        let cmd = effect.to_command("@p");
                        let _ = game.native_run_command(&cmd);
                    }
                }
            }
        }
    }

    // 4. Geiger counter audio scheduling and click dispatch
    if game.native_window() {
        if let Some(p) = game.native_player() {
            let has_geiger = crate::game::nbm_r::check_has_geiger_counter(
                game.inventory
                    .iter()
                    .flatten()
                    .chain(game.offhand.as_ref())
                    .filter(|s| s.count > 0)
                    .map(|s| s.item.as_ref()),
            );

            if has_geiger {
                let (_, ambient_rad) = crate::game::nbm_r::calculate_ambient_radiation_at(
                    &p.dimension,
                    p.x,
                    p.z,
                    game.inventory
                        .iter()
                        .flatten()
                        .chain(game.offhand.as_ref())
                        .map(|s| (s.item.as_ref(), s.count)),
                );

                let master_vol = game.settings.master_volume;
                let player_vol = game.settings.volumes.get(7).copied().unwrap_or(1.0);
                let effective_vol =
                    crate::game::nbm_r::geiger_hud::effective_audio_volume(1.0, master_vol, player_vol);
                if effective_vol > 0.0 {
                    let sound_tier = crate::game::nbm_r::lock_geiger_audio(|state| {
                        state.step_tick(ambient_rad)
                    });

                    if let Some(tier) = sound_tier {
                        let sound_name = crate::game::nbm_r::geiger_hud::geiger_sound_event(tier);
                        game.native_play_local_sound(
                            sound_name,
                            [p.x, p.y, p.z],
                            effective_vol,
                            1.0,
                            0,
                        );
                    }
                }
            }
        }
    }
}
