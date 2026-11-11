fn nbm_r_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    if !game.native_on(crate::game::nbm_r::ID) {
        return;
    }
    use horizon_client::extras::Corner;
    if !game.native_corner_shown(Corner::TopRight) {
        return;
    }
    let Some(p) = game.native_player() else {
        return;
    };

    let has_geiger = crate::game::nbm_r::check_has_geiger_counter(
        game.inventory
            .iter()
            .flatten()
            .chain(game.offhand.as_ref())
            .filter(|s| s.count > 0)
            .map(|s| s.item.as_ref()),
    );
    if !has_geiger {
        return;
    }

    let dose = crate::game::nbm_r::get_player_dosimetry(0).dose;

    let (_, ambient_rate) = crate::game::nbm_r::calculate_ambient_radiation_at(
        &p.dimension,
        p.x,
        p.z,
        game.inventory
            .iter()
            .flatten()
            .chain(game.offhand.as_ref())
            .map(|s| (s.item.as_ref(), s.count)),
    );

    let target_angle = crate::game::nbm_r::geiger_hud::target_needle_angle(ambient_rate);
    let needle_angle = crate::game::nbm_r::lock_geiger_hud(|state| {
        state.update(target_angle, 0.05);
        state.current_needle_angle
    });

    let w = 96.0;
    let h = 48.0;
    let (x, y) = corners.place(Corner::TopRight, w, h);

    // Bezel and dial background
    game.native_fill(batch, [x, y, w, h], 0xD01C201C);
    game.native_fill(batch, [x + 2.0, y + 2.0, w - 4.0, h - 4.0], 0xEE2A332C);

    // Header label
    game.native_text(batch, "DOSIMETER", [x + 4.0, y + 4.0], 0xAAAAAA, false);

    // Dial arc tick marks
    let cx = x + (w / 2.0);
    let cy = y + 28.0;
    let radius = 18.0;

    for &angle in &[-60.0f32, -30.0, 0.0, 30.0, 60.0] {
        let (tx, ty) =
            crate::game::nbm_r::geiger_hud::compute_needle_endpoint(cx, cy, radius, angle);
        game.native_fill(batch, [tx - 0.5, ty - 0.5, 2.0, 2.0], 0xFF888888);
    }

    // Needle line
    let (nx, ny) =
        crate::game::nbm_r::geiger_hud::compute_needle_endpoint(cx, cy, radius - 2.0, needle_angle);
    let steps = 10;
    for s in 0..=steps {
        let frac = s as f32 / steps as f32;
        let px = cx + (nx - cx) * frac;
        let py = cy + (ny - cy) * frac;
        game.native_fill(batch, [px - 0.5, py - 0.5, 1.5, 1.5], 0xFFFF3322);
    }
    // Pivot cap
    game.native_fill(batch, [cx - 1.5, cy - 1.5, 3.0, 3.0], 0xFFEEEEEE);

    // Dose digital readout
    let dose_text = crate::game::nbm_r::geiger_hud::format_absorbed_rad(dose);
    let text_w = game.native_text_width(&dose_text);
    let text_x = x + (w - text_w) / 2.0;
    let text_color = crate::game::nbm_r::geiger_hud::dose_status_color(dose);
    game.native_text(batch, &dose_text, [text_x, y + 34.0], text_color, true);
}
