#!/usr/bin/env python3
"""Compile the real bundle on a tiny source fixture; no Horizon/assets required."""
import json
from pathlib import Path
import subprocess
import tempfile
import tomllib
import horizon


def run(command, **kwargs):
    result = subprocess.run(command, capture_output=True, text=True, **kwargs)
    if result.returncode:
        raise RuntimeError(result.stdout + result.stderr)
    return result


def snapshot(root):
    return {str(p.relative_to(root)): (horizon.digest(p.read_bytes()), p.stat().st_mtime_ns)
            for p in root.rglob('*') if p.is_file()}


def audit(original, stage, plan):
    run(['cargo', 'run', '--quiet', '--locked', '--offline', '-p', 'native-mod-source-audit',
         '--', str(original), str(stage), str(plan)], cwd=horizon.ROOT)


def main():
    root = horizon.ROOT
    (root/'target').mkdir(exist_ok=True)
    run(['cargo', 'build', '--locked', '--offline', '-p', 'horizon-mixins-cli'], cwd=horizon.MIXINS)
    cli = horizon.MIXINS/'target/debug/horizon-mixins'
    manifests = list((root/'mixins').glob('*/horizon-mixin.toml'))
    assert len(manifests) == 1, 'this template test expects one example bundle'
    manifest = tomllib.loads(manifests[0].read_text())
    with tempfile.TemporaryDirectory(prefix='template-test-', dir=root/'target') as scratch:
        scratch = Path(scratch)
        original, stage = scratch/'original', scratch/'stage'
        (original/'crates/horizon-client/src').mkdir(parents=True)
        (original/'Cargo.toml').write_text('[workspace]\nmembers=["crates/horizon-client"]\nresolver="2"\n')
        (original/'crates/horizon-client/Cargo.toml').write_text('[package]\nname="horizon-client"\nversion="0.1.0"\nedition="2024"\n')
        (original/'crates/horizon-client/src/lib.rs').write_text('''
extern crate self as horizon_client;
pub mod game;
pub mod hud {
    pub struct HudBatch;
}
pub mod extras_hud {
    pub struct Corners;
    impl Corners {
        pub fn place(&mut self, _corner: crate::extras::Corner, _w: f32, _h: f32) -> (f32, f32) {
            (0.0, 0.0)
        }
    }
}
pub mod extras {
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum Corner {
        TopLeft,
        TopRight,
        BottomLeft,
        BottomRight,
    }
}
''')
        (original/'crates/horizon-client/src/game.rs').write_text('''
use std::sync::Arc;

pub struct ItemStack {
    pub item: Arc<str>,
    pub count: u32,
}

pub struct NativeServerPlayer {
    pub uuid: u128,
    pub dimension: String,
    pub position: [f64; 3],
    pub game_mode: i32,
}

pub struct NativePlayer {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub dimension: String,
}

pub struct Settings {
    pub master_volume: f32,
    pub volumes: [f32; 11],
}

pub struct Game<'a> {
    pub positions: &'a mut Vec<[i32;3]>,
    pub inventory: Vec<Option<ItemStack>>,
    pub armor: [Option<ItemStack>; 4],
    pub offhand: Option<ItemStack>,
    pub settings: Settings,
}
impl Game<'_> {
    pub fn break_block(&mut self,pos:[i32;3])->i32 {self.positions.push(pos); 7}
    pub fn native_on(&self, _id: &str) -> bool { true }
    pub fn native_acting_player(&self) -> Option<NativeServerPlayer> { None }
    pub fn native_window(&self) -> bool { true }
    pub fn native_player(&self) -> Option<NativePlayer> { None }
    pub fn native_play_local_sound(&mut self, _sound: &str, _at: [f64; 3], _volume: f32, _pitch: f32, _delay: i32) -> bool { true }
    pub fn native_corner_shown(&self, _corner: crate::extras::Corner) -> bool { true }
    pub fn native_fill(&self, _batch: &mut crate::hud::HudBatch, _rect: [f32; 4], _argb: u32) {}
    pub fn native_text(&self, _batch: &mut crate::hud::HudBatch, _text: &str, _at: [f32; 2], _color: u32, _shadow: bool) {}
    pub fn native_text_width(&self, _text: &str) -> f32 { 10.0 }
}
pub mod native_hooks {
    use super::Game;
    impl Game<'_> {
        pub fn native_level_tick_post(&mut self, dimension: &str) { let _ = dimension; }
        pub fn native_player_tick(&mut self) {}
        pub fn native_chunk_load(&mut self, dimension: &str, chunk: [i32; 2]) { let _ = (dimension, chunk); }
        pub fn native_chunk_unload(&mut self, dimension: &str, chunk: [i32; 2]) { let _ = (dimension, chunk); }
        pub fn native_tick(&mut self) {}
        pub fn native_hud(&mut self, batch: &mut crate::hud::HudBatch, corners: &mut crate::extras_hud::Corners) {
            let _ = (batch, corners);
        }
    }
}
#[test] fn original_body_and_result_are_preserved() {
    let mut positions=Vec::new();
    let mut game=Game {
        positions:&mut positions,
        inventory: Vec::new(),
        armor: [None, None, None, None],
        offhand: None,
        settings: Settings {
            master_volume: 1.0,
            volumes: [1.0; 11],
        },
    };
    assert_eq!(game.break_block([1,2,3]),7);
    assert_eq!(game.break_block([-1,64,8]),7);
    assert_eq!(positions,vec![[1,2,3],[-1,64,8]]);
}
''')
        (original/'rust-toolchain.toml').write_bytes((root/'rust-toolchain.toml').read_bytes())
        (original/'.horizon-revision').write_text(horizon.PIN+'\n')
        run(['cargo', 'generate-lockfile', '--offline'], cwd=original)
        before = snapshot(original)
        base = ['--workspace', str(original), '--mods', str(root/'mixins')]
        report = json.loads(run([str(cli), 'check', *base, '--out', str(stage), '--trust-native-mods']).stdout)
        expected_injections = len(manifest.get('injection', []))
        assert report['injections'] == expected_injections and report['modules'] == 1
        assert 'passed' in report['cargo_stdout']
        first = snapshot(stage)
        second = json.loads(run([str(cli), 'check', *base, '--out', str(stage), '--trust-native-mods']).stdout)
        assert not second['stage_changed'] and snapshot(stage) == first
        assert snapshot(original) == before
        plan_path = scratch/'plan.json'
        run([str(cli), 'plan', *base, '--out', str(plan_path)])
        plan = json.loads(plan_path.read_text())
        assert plan['schema'] == horizon.COMPAT['mixin_schema']
        assert plan['injections'][0]['mod_id'] == manifest['mod']['id']
        audit(original, stage, plan_path)
        (original/'.horizon-revision').write_text('incompatible\n')
        bad = subprocess.run([str(cli), 'apply', *base, '--out', str(stage)], capture_output=True, text=True)
        assert bad.returncode and json.loads(bad.stderr)['code'] == 'E_REVISION'
        assert snapshot(stage) == first
        (original/'.horizon-revision').write_text(horizon.PIN+'\n')
        source = original/'crates/horizon-client/src/game.rs'
        source.write_text(source.read_text().replace('break_block', 'renamed_break_block'))
        bad = subprocess.run([str(cli), 'apply', *base, '--out', str(stage)], capture_output=True, text=True)
        assert bad.returncode and json.loads(bad.stderr)['code'] == 'E_TARGET_NOT_FOUND'
        assert snapshot(stage) == first
    print(json.dumps({'status': 'ok', 'compiled_runtime_and_original_body': True,
                      'only_planned_asts_changed': True, 'deterministic_bytes_and_mtimes': True,
                      'incompatible_revision_and_target_rejected': True}, indent=2))


if __name__ == '__main__':
    main()
