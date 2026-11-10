fn nbm_r_level_tick(game: &mut crate::game::Game<'_>, dimension: &str) {
    if !game.native_on(crate::game::nbm_r::ID) {
        return;
    }
    crate::game::nbm_r::on_level_tick(dimension);
}
