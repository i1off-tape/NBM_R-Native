// NBM_R-Native: Native Rust Port of HBM's Nuclear Tech Mod for Horizon.
//
// This file serves as both the standalone Cargo library crate and the
// injected `crate::game::nbm_r` module inside the Horizon engine.

pub fn block_break_message(pos: [i32; 3]) -> String {
    format!(
        "[nbm_r] Breaking block at {}, {}, {}",
        pos[0], pos[1], pos[2]
    )
}

pub fn before_break(pos: [i32; 3]) {
    eprintln!("{}", block_break_message(pos));
}

/// Core radiation simulation logic (pure Rust, testable without game runtime).
pub mod radiation {
    /// Calculate dose attenuation based on shielding thickness and material factor.
    pub fn calculate_attenuation(raw_rads: f32, shielding_coeff: f32, thickness_blocks: f32) -> f32 {
        if raw_rads <= 0.0 {
            return 0.0;
        }
        let factor = (-shielding_coeff * thickness_blocks).exp();
        raw_rads * factor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_break_message() {
        assert_eq!(
            block_break_message([-3, 64, 12]),
            "[nbm_r] Breaking block at -3, 64, 12"
        );
    }

    #[test]
    fn test_radiation_attenuation() {
        let raw = 100.0;
        let lead_coeff = 0.5;
        let shielded = radiation::calculate_attenuation(raw, lead_coeff, 2.0);
        assert!(shielded < raw);
        assert!((shielded - 36.787).abs() < 0.01);
    }
}
