use bevy::{
    asset::{AssetLoader, LoadContext, LoadedAsset},
    input::mouse::MouseMotion,
    prelude::*,
    reflect::TypeUuid,
    utils::BoxedFuture,
};
use flagset::{flags, FlagSet};
use serde::{Deserialize, Serialize};

flags! {
    pub enum PlayerInputFlags: u32 {
        Jump,
        Sprint,
    }
}

#[derive(Component, Default)]
pub struct PlayerInput {
    pub movement: Vec3,
    pub mouse: Vec2,
    pub flags: FlagSet<PlayerInputFlags>,
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize, TypeUuid)]
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
    pub key_crouch: KeyCode,
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
            key_sprint: KeyCode::LShift,
            key_jump: KeyCode::Space,
            key_crouch: KeyCode::LControl,
            sensitivity: 0.5,
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

pub fn cursor_grab_system(
    mut windows: ResMut<Windows>,
    btn: Res<Input<MouseButton>>,
    key: Res<Input<KeyCode>>,
) {
    let window = windows.get_primary_mut().unwrap();
    if btn.just_pressed(MouseButton::Left) {
        window.set_cursor_lock_mode(true);
        window.set_cursor_visibility(false);
    }
    if key.just_pressed(KeyCode::Escape) {
        window.set_cursor_lock_mode(false);
        window.set_cursor_visibility(true);
    }
}

pub fn player_input_system(
    key_input: Res<Input<KeyCode>>,
    config: Res<Assets<Config>>,
    config_handle: Res<Handle<Config>>,
    mut mouse_events: EventReader<MouseMotion>,
    mut query: Query<&mut PlayerInput>)
{
    if let Some(config) = config.get(config_handle.as_ref()) {
        for mut player_input in query.iter_mut() {
            let mut mouse_delta = Vec2::ZERO;
            for mouse_event in mouse_events.iter() {
                mouse_delta += mouse_event.delta;
            }
            mouse_delta *= config.sensitivity;
            player_input.mouse = mouse_delta;

            player_input.movement = Vec3::new(
                get_axis(&key_input, config.key_right, config.key_left),
                get_axis(&key_input, config.key_forward, config.key_back),
                get_axis(&key_input, config.key_up, config.key_down),
            );
            player_input.flags.clear();
            if key_input.pressed(config.key_sprint) {
                player_input.flags |= PlayerInputFlags::Sprint;
            }
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
            let asset: Config = toml::from_slice(bytes)?;
            load_context.set_default_asset(LoadedAsset::new(asset));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["toml"]
    }
}