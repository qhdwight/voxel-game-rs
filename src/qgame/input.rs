use std::f32::consts::FRAC_PI_2;
use std::str::from_utf8;

use bevy::{
    asset::{AssetLoader, LoadContext, LoadedAsset},
    input::mouse::MouseMotion,
    prelude::*,
    reflect::TypePath,
    reflect::TypeUuid,
    utils::BoxedFuture,
};
use bevy::window::CursorGrabMode;
use flagset::{flags, FlagSet};
use serde::{Deserialize, Serialize};

flags! {
    pub enum PlayerInputFlags: u32 {
        Jump,
        Sprint,
        Fly,
        Fire,
        Reload
    }
}

#[derive(Component, Default, Debug)]
pub struct PlayerInput {
    pub movement: Vec3,
    pub flags: FlagSet<PlayerInputFlags>,
    pub yaw: f32,
    pub pitch: f32,
    pub wanted_item_slot: Option<u8>,
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize, TypeUuid, TypePath)]
#[uuid = "9ac18a62-063a-4fa1-9575-d295ce69997b"]
pub struct Config {
    pub sensitivity: f32,
    pub key_forward: KeyCode,
    pub key_back: KeyCode,
    pub key_left: KeyCode,
    pub key_right: KeyCode,
    pub key_up: KeyCode,
    pub key_down: KeyCode,
    pub key_sprint: KeyCode,
    pub key_jump: KeyCode,
    pub key_fly: KeyCode,
    pub key_crouch: KeyCode,
    pub key_fire: KeyCode,
    pub key_reload: KeyCode,
}

#[derive(Resource)]
pub struct ConfigState {
    pub(crate) handle: Handle<Config>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            key_forward: KeyCode::W,
            key_back: KeyCode::S,
            key_left: KeyCode::A,
            key_right: KeyCode::D,
            key_up: KeyCode::Q,
            key_down: KeyCode::E,
            key_sprint: KeyCode::ShiftLeft,
            key_jump: KeyCode::Space,
            key_fly: KeyCode::F,
            key_crouch: KeyCode::ControlLeft,
            key_fire: KeyCode::Q,
            sensitivity: 0.5,
            key_reload: KeyCode::R,
        }
    }
}

fn get_pressed(key_input: &Res<Input<KeyCode>>, key: KeyCode) -> f32 {
    if key_input.pressed(key) {
        1.0
    } else {
        0.0
    }
}

fn get_axis(key_input: &Res<Input<KeyCode>>, key_pos: KeyCode, key_neg: KeyCode) -> f32 {
    get_pressed(key_input, key_pos) - get_pressed(key_input, key_neg)
}

pub fn cursor_grab_sys(
    mut windows: Query<&mut Window>,
    btn: Res<Input<MouseButton>>,
    key: Res<Input<KeyCode>>,
) {
    let mut window = windows.single_mut();
    if btn.just_pressed(MouseButton::Left) {
        window.cursor.grab_mode = CursorGrabMode::Locked;
        window.cursor.visible = false;
    }
    if key.just_pressed(KeyCode::Escape) {
        window.cursor.grab_mode = CursorGrabMode::None;
        window.cursor.visible = true;
    }
}

pub fn player_input_system(
    key_input: Res<Input<KeyCode>>,
    config: Res<Assets<Config>>,
    config_state: Res<ConfigState>,
    mut window: Query<&mut Window>,
    mut mouse_events: EventReader<MouseMotion>,
    mut query: Query<&mut PlayerInput>)
{
    if let Some(config) = config.get(&config_state.handle) {
        for mut player_input in query.iter_mut() {
            let window = window.single_mut();
            if window.focused {
                let mut mouse_delta = Vec2::ZERO;
                for mouse_event in mouse_events.iter() {
                    mouse_delta += mouse_event.delta;
                }
                mouse_delta *= config.sensitivity;

                player_input.pitch = (player_input.pitch - mouse_delta.y).clamp(
                    -FRAC_PI_2 + 0.001953125,
                    FRAC_PI_2 - 0.001953125,
                );
                player_input.yaw = player_input.yaw - mouse_delta.x;
            }

            player_input.movement = Vec3::new(
                get_axis(&key_input, config.key_right, config.key_left),
                get_axis(&key_input, config.key_up, config.key_down),
                get_axis(&key_input, config.key_forward, config.key_back),
            );
            player_input.flags.clear();
            if key_input.pressed(config.key_sprint) { player_input.flags |= PlayerInputFlags::Sprint; }
            if key_input.pressed(config.key_jump) { player_input.flags |= PlayerInputFlags::Jump; }
            if key_input.pressed(config.key_fire) { player_input.flags |= PlayerInputFlags::Fire; }
            if key_input.pressed(config.key_reload) { player_input.flags |= PlayerInputFlags::Reload; }
            if key_input.just_pressed(config.key_fly) { player_input.flags |= PlayerInputFlags::Fly; }
            if key_input.pressed(KeyCode::Key1) { player_input.wanted_item_slot = Some(0); }
            if key_input.pressed(KeyCode::Key2) { player_input.wanted_item_slot = Some(1); }
            if key_input.pressed(KeyCode::Key3) { player_input.wanted_item_slot = Some(2); }
        }
    }
}

#[derive(Default)]
pub struct ConfigAssetLoader;

impl AssetLoader for ConfigAssetLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<(), anyhow::Error>> {
        Box::pin(async move {
            let asset: Config = toml::from_str(from_utf8(bytes)?)?;
            load_context.set_default_asset(LoadedAsset::new(asset));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["config.toml"]
    }
}