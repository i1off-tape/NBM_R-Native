fn nbm_r_chunk_load(game: &mut crate::game::Game<'_>, dimension: &str, chunk: [i32; 2]) {
    if !game.native_on(crate::game::nbm_r::ID) {
        return;
    }
    crate::game::nbm_r::on_chunk_load(dimension, chunk);
}
