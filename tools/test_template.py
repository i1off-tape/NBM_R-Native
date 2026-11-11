#!/usr/bin/env python3
"""Compile the real bundle on a tiny source fixture; no Horizon/assets required."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile
import tomllib
import horizon
import validate_content


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
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--horizon', type=Path, default=horizon.ROOT.parent / 'horizon')
    parser.parse_known_args()
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
pub mod first_person {
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum Hand {
        Main,
        Off,
    }
}
pub mod mod_api {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum EquipmentSlot {
        MainHand,
        OffHand,
        Head,
        Chest,
        Legs,
        Feet,
        Body,
        Saddle,
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
    pub name: String,
    pub dimension: String,
    pub position: [f64; 3],
    pub game_mode: i32,
    pub health: f32,
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

static RECORDED_COMMANDS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
static RECORDED_SOUNDS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
static RECORDED_CHAT: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
static RECORDED_EFFECTS: std::sync::Mutex<Vec<(String, i32, i32)>> = std::sync::Mutex::new(Vec::new());
static RECORDED_DAMAGE: std::sync::Mutex<Vec<(String, f32)>> = std::sync::Mutex::new(Vec::new());

pub struct MockPlayer {
    pub health: f32,
}
impl MockPlayer {
    pub fn set_health(&mut self, h: f32, _absorption: bool) -> Result<(), ()> {
        self.health = h;
        Ok(())
    }
}

pub struct MockUi {
    pub death_cause: Option<String>,
}

pub struct Game<'a> {
    pub positions: &'a mut Vec<[i32;3]>,
    pub inventory: Vec<Option<ItemStack>>,
    pub selected: usize,
    pub armor: [Option<ItemStack>; 4],
    pub offhand: Option<ItemStack>,
    pub settings: Settings,
    pub player: Option<MockPlayer>,
    pub ui: MockUi,
}
impl Game<'_> {
    pub fn break_block(&mut self,pos:[i32;3])->i32 {self.positions.push(pos); 7}
    pub fn native_on(&self, _id: &str) -> bool { true }
    pub fn native_acting_player(&self) -> Option<NativeServerPlayer> { None }
    pub fn native_window(&self) -> bool { true }
    pub fn native_player(&self) -> Option<NativePlayer> {
        Some(NativePlayer {
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            dimension: "minecraft:overworld".to_string(),
        })
    }
    pub fn native_play_local_sound(&mut self, sound: &str, _at: [f64; 3], _volume: f32, _pitch: f32, _delay: i32) -> bool {
        RECORDED_SOUNDS.lock().unwrap().push(sound.to_string());
        true
    }
    pub fn player_menu_sound(&mut self, _sound: &str) {}
    pub fn native_chat(&mut self, text: &str) {
        RECORDED_CHAT.lock().unwrap().push(text.to_string());
    }
    pub fn native_run_command(&mut self, cmd: &str) -> bool {
        RECORDED_COMMANDS.lock().unwrap().push(cmd.to_string());
        true
    }
    pub fn give_player_effect(&mut self, effect: &'static str, ticks: i32, amplifier: i32) {
        RECORDED_EFFECTS.lock().unwrap().push((effect.to_string(), ticks, amplifier));
    }
    pub fn hurt_player_by(&mut self, damage_type: &str, amount: f32) {
        RECORDED_DAMAGE.lock().unwrap().push((damage_type.to_string(), amount));
    }
    pub fn send_command(&mut self, cmd: &str) {
        RECORDED_COMMANDS.lock().unwrap().push(cmd.to_string());
    }
    pub fn native_server_players(&self) -> Vec<NativeServerPlayer> {
        Vec::new()
    }
    pub fn native_player_stacks(&self, _uuid: u128) -> Option<Vec<Option<ItemStack>>> {
        None
    }
    pub fn native_player_equipment(
        &self,
        _uuid: u128,
        slot: horizon_client::mod_api::EquipmentSlot,
    ) -> Option<ItemStack> {
        match slot {
            horizon_client::mod_api::EquipmentSlot::Head => self.armor[0].as_ref().map(|s| ItemStack { item: s.item.clone(), count: s.count }),
            horizon_client::mod_api::EquipmentSlot::Chest => self.armor[1].as_ref().map(|s| ItemStack { item: s.item.clone(), count: s.count }),
            horizon_client::mod_api::EquipmentSlot::Legs => self.armor[2].as_ref().map(|s| ItemStack { item: s.item.clone(), count: s.count }),
            horizon_client::mod_api::EquipmentSlot::Feet => self.armor[3].as_ref().map(|s| ItemStack { item: s.item.clone(), count: s.count }),
            _ => None,
        }
    }
    pub fn native_set_player_status(
        &mut self,
        _uuid: u128,
        health: Option<f32>,
        _food: Option<(i32, f32, f32)>,
        _xp: Option<(i32, f32, i32)>,
    ) -> bool {
        if let Some(h) = health {
            if let Some(p) = self.player.as_mut() {
                let _ = p.set_health(h, false);
            }
        }
        true
    }
    pub fn native_corner_shown(&self, _corner: crate::extras::Corner) -> bool { true }
    pub fn native_fill(&self, _batch: &mut crate::hud::HudBatch, _rect: [f32; 4], _argb: u32) {}
    pub fn native_text(&self, _batch: &mut crate::hud::HudBatch, _text: &str, _at: [f32; 2], _color: u32, _shadow: bool) {}
    pub fn native_text_width(&self, _text: &str) -> f32 { 10.0 }
}
pub mod native_hooks {
    use super::Game;
    impl Game<'_> {
        pub fn native_tick(&mut self) {}
        pub fn native_hud(&mut self, batch: &mut crate::hud::HudBatch, corners: &mut crate::extras_hud::Corners) {
            let _ = (batch, corners);
        }
        pub fn native_interaction_key(
            &mut self,
            button: i32,
            key_mapping: &str,
            hand: horizon_client::first_person::Hand,
            swing_hand: &mut bool,
            canceled: &mut bool,
        ) {
            let _ = (button, key_mapping, hand, swing_hand, canceled);
        }
    }
}
#[test] fn original_body_and_result_are_preserved() {
    let mut positions=Vec::new();
    let mut game=Game {
        positions:&mut positions,
        inventory: Vec::new(),
        selected: 0,
        armor: [None, None, None, None],
        offhand: None,
        settings: Settings {
            master_volume: 1.0,
            volumes: [1.0; 11],
        },
        player: Some(MockPlayer { health: 20.0 }),
        ui: MockUi { death_cause: None },
    };
    assert_eq!(game.break_block([1,2,3]),7);
    assert_eq!(game.break_block([-1,64,8]),7);
    assert_eq!(positions,vec![[1,2,3],[-1,64,8]]);
}
#[test] fn test_nbm_r_tick_dispatch() {
    let _lock = nbm_r::TEST_MUTEX.lock().unwrap();
    let mut positions = Vec::new();
    let mut game = Game {
        positions: &mut positions,
        inventory: Vec::new(),
        selected: 0,
        armor: [None, None, None, None],
        offhand: None,
        settings: Settings {
            master_volume: 1.0,
            volumes: [1.0; 11],
        },
        player: Some(MockPlayer { health: 20.0 }),
        ui: MockUi { death_cause: None },
    };

    // 1. Healthy player (dose = 0.0)
    nbm_r::set_player_dose(0, 0.0);
    RECORDED_COMMANDS.lock().unwrap().clear();
    RECORDED_EFFECTS.lock().unwrap().clear();
    RECORDED_DAMAGE.lock().unwrap().clear();
    game.native_tick();
    assert!(RECORDED_COMMANDS.lock().unwrap().is_empty(), "Healthy player must not trigger commands");
    assert!(RECORDED_EFFECTS.lock().unwrap().is_empty(), "Healthy player must not receive effects");
    assert!(RECORDED_DAMAGE.lock().unwrap().is_empty(), "Healthy player must not receive damage");

    // 2. Sick player (dose = 500.0, Stage 2: Nausea, Slowness, Weakness)
    nbm_r::set_player_dose(0, 500.0);
    RECORDED_COMMANDS.lock().unwrap().clear();
    RECORDED_EFFECTS.lock().unwrap().clear();
    for _ in 0..40 {
        game.native_tick();
        if !RECORDED_EFFECTS.lock().unwrap().is_empty() {
            break;
        }
    }
    let cmds = RECORDED_COMMANDS.lock().unwrap().clone();
    assert!(!cmds.is_empty(), "Sick player must receive status effect commands");
    assert!(cmds.iter().any(|c| c.contains("minecraft:nausea")), "Expected nausea command");
    assert!(cmds.iter().any(|c| c.contains("minecraft:slowness")), "Expected slowness command");
    assert!(cmds.iter().any(|c| c.contains("minecraft:weakness")), "Expected weakness command");

    let effects = RECORDED_EFFECTS.lock().unwrap().clone();
    assert!(!effects.is_empty(), "Sick player must receive direct status effects");
    assert!(effects.iter().any(|(id, _, _)| id == "minecraft:nausea"), "Expected direct nausea effect");
    assert!(effects.iter().any(|(id, _, _)| id == "minecraft:slowness"), "Expected direct slowness effect");
    assert!(effects.iter().any(|(id, _, _)| id == "minecraft:weakness"), "Expected direct weakness effect");

    // 3. Lethal player (dose = 1200.0 >= 1000.0)
    nbm_r::set_player_dose(0, 1200.0);
    RECORDED_COMMANDS.lock().unwrap().clear();
    RECORDED_DAMAGE.lock().unwrap().clear();
    game.native_tick();
    let lethal_cmds = RECORDED_COMMANDS.lock().unwrap().clone();
    assert!(lethal_cmds.iter().any(|c| c == "kill" || c == "kill @p"), "Lethal dose must dispatch kill command");
    let lethal_dmg = RECORDED_DAMAGE.lock().unwrap().clone();
    assert_eq!(lethal_dmg, vec![("minecraft:generic_kill".to_string(), 1000.0)], "Lethal dose must inflict lethal damage");
    assert_eq!(game.player.as_ref().unwrap().health, 0.0, "Player health must be set to 0.0 on lethal radiation");
    assert_eq!(game.ui.death_cause.as_deref(), Some("death.attack.genericKill"), "Death cause must be genericKill");
    assert_eq!(nbm_r::get_player_dosimetry(0).dose, 0.0, "Player dose must reset to 0 after lethal resolution");
}
#[test] fn test_nbm_r_interaction_dispatch() {
    let _lock = nbm_r::TEST_MUTEX.lock().unwrap();
    let mut positions = Vec::new();
    let geiger_item = ItemStack { item: Arc::from("ntm:geiger_counter"), count: 1 };
    let stick_item = ItemStack { item: Arc::from("minecraft:stick"), count: 1 };

    let mut game = Game {
        positions: &mut positions,
        inventory: vec![Some(geiger_item), Some(stick_item)],
        selected: 0,
        armor: [None, None, None, None],
        offhand: None,
        settings: Settings {
            master_volume: 1.0,
            volumes: [1.0; 11],
        },
        player: Some(MockPlayer { health: 20.0 }),
        ui: MockUi { death_cause: None },
    };

    RECORDED_SOUNDS.lock().unwrap().clear();
    RECORDED_CHAT.lock().unwrap().clear();

    let mut swing = false;
    let mut canceled = false;

    // 1. Right click with geiger counter held in main hand (button = 1)
    game.native_interaction_key(1, "key.use", horizon_client::first_person::Hand::Main, &mut swing, &mut canceled);
    assert!(canceled, "Right-click with geiger must be canceled");
    assert!(swing, "Right-click with geiger must swing hand");
    let sounds = RECORDED_SOUNDS.lock().unwrap().clone();
    assert_eq!(sounds, vec!["nbm_r:item.techBoop"], "Must play techBoop on right click");
    let chat = RECORDED_CHAT.lock().unwrap().clone();
    assert_eq!(chat.len(), 5, "Must output exactly 5 lines of geiger summary to chat");
    assert!(chat[0].contains("Geiger Counter"), "Line 0 must be header");
    assert!(chat[1].contains("Chunk Rad:"), "Line 1 must be chunk rad");
    assert!(chat[2].contains("Env Rad:"), "Line 2 must be env rad");
    assert!(chat[3].contains("Player Rad:"), "Line 3 must be player rad");
    assert!(chat[4].contains("Player Res:"), "Line 4 must be player res");

    // 2. Right click with non-geiger item (selected = 1, stick)
    game.selected = 1;
    RECORDED_SOUNDS.lock().unwrap().clear();
    RECORDED_CHAT.lock().unwrap().clear();
    let mut swing2 = false;
    let mut canceled2 = false;
    game.native_interaction_key(1, "key.use", horizon_client::first_person::Hand::Main, &mut swing2, &mut canceled2);
    assert!(!canceled2, "Non-geiger right click must not be canceled");
    assert!(RECORDED_SOUNDS.lock().unwrap().is_empty());
    assert!(RECORDED_CHAT.lock().unwrap().is_empty());

    // 3. Left click (attack, button = 0) with geiger counter
    game.selected = 0;
    RECORDED_SOUNDS.lock().unwrap().clear();
    RECORDED_CHAT.lock().unwrap().clear();
    let mut swing3 = false;
    let mut canceled3 = false;
    game.native_interaction_key(0, "key.attack", horizon_client::first_person::Hand::Main, &mut swing3, &mut canceled3);
    assert!(!canceled3, "Left click attack with geiger must not be intercepted");
    assert!(RECORDED_SOUNDS.lock().unwrap().is_empty());
    assert!(RECORDED_CHAT.lock().unwrap().is_empty());
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
    val = validate_content.validate_content()
    assert val['status'] == 'ok', f"Content validation failed: {val['errors']}"
    print(json.dumps({'status': 'ok', 'compiled_runtime_and_original_body': True,
                      'only_planned_asts_changed': True, 'deterministic_bytes_and_mtimes': True,
                      'incompatible_revision_and_target_rejected': True,
                      'content_declarations_valid': True}, indent=2))


if __name__ == '__main__':
    main()
