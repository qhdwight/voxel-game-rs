use std::{
    f32::consts::TAU,
    option::Option,
    time::Duration,
};

use bevy::{
    asset::{AssetLoader, LoadContext, LoadedAsset},
    prelude::*,
    reflect::TypeUuid,
    utils::{BoxedFuture, HashMap},
};
use bevy_rapier3d::prelude::*;
use serde::{Deserialize, Serialize};
use smartstring::alias::String;

use crate::{PlayerInput, PlayerInputFlags};

const EQUIPPING_STATE: &str = "equipping";
const EQUIPPED_STATE: &str = "equipped";
const UNEQUIPPING_STATE: &str = "unequipping";
const UNEQUIPPED_STATE: &str = "unequipped";
const IDLE_STATE: &str = "idle";
const RELOAD_STATE: &str = "reload";
const FIRE_STATE: &str = "fire";

pub type ItemName = String;
pub type ItemStateName = String;
pub type EquipStateName = String;

#[derive(Serialize, Deserialize)]
pub struct ItemStateProps {
    pub duration: Duration,
    pub is_persistent: bool,
}

#[derive(Serialize, Deserialize, TypeUuid)]
#[uuid = "2cc54620-95c6-4522-b40e-0a4991ebae5f"]
pub struct ItemProps {
    pub name: ItemName,
    pub move_factor: f32,
    pub states: HashMap<ItemStateName, ItemStateProps>,
    pub equip_states: HashMap<EquipStateName, ItemStateProps>,
}

#[derive(Serialize, Deserialize, TypeUuid)]
#[uuid = "46e9c7af-27c2-4560-86e7-df48f9e84729"]
pub struct WeaponProps {
    pub damage: u16,
    pub headshot_factor: f32,
    pub item_props: ItemProps,
}

#[derive(Serialize, Deserialize, TypeUuid)]
#[uuid = "df56751c-7560-420d-b480-eb8fb6f9b9bf"]
pub struct GunProps {
    pub mag_size: u16,
    pub starting_ammo_in_reserve: u16,
    pub weapon_props: WeaponProps,
}

#[derive(Component, Debug)]
pub struct Item {
    pub name: ItemName,
    pub amount: u16,
    pub state_name: ItemStateName,
    pub state_dur: Duration,
    pub inv_ent: Entity,
    pub inv_slot: u8,
}

#[derive(Component)]
pub struct ItemPickup {
    pub item_name: ItemName,
}

#[derive(Component, Default)]
pub struct ItemPickupVisual;

#[derive(Component)]
pub struct Gun {
    pub ammo: u16,
    pub ammo_in_reserve: u16,
}

#[derive(Debug)]
pub struct Items(pub [Option<Entity>; 10]);

#[derive(Component)]
pub struct ItemVisual;

#[derive(Component, Debug)]
pub struct Inventory {
    pub equipped_slot: Option<u8>,
    pub prev_equipped_slot: Option<u8>,
    pub equip_state_name: EquipStateName,
    pub equip_state_dur: Duration,
    pub item_ents: Items,
}

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {}
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
            let asset: GunProps = toml::from_slice(bytes)?;
            load_context.set_default_asset(LoadedAsset::new(asset));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["config.toml"]
    }
}

// ██╗      ██████╗  ██████╗ ██╗ ██████╗
// ██║     ██╔═══██╗██╔════╝ ██║██╔════╝
// ██║     ██║   ██║██║  ███╗██║██║
// ██║     ██║   ██║██║   ██║██║██║
// ███████╗╚██████╔╝╚██████╔╝██║╚██████╗
// ╚══════╝ ╚═════╝  ╚═════╝ ╚═╝ ╚═════╝

pub fn modify_equip_state_sys(
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut inv_query: Query<(&PlayerInput, &mut Inventory)>,
    mut item_query: Query<&mut Item>,
) {
    for (input, mut inv) in inv_query.iter_mut() {
        let has_valid_wanted = input.wanted_item_slot.is_some()
            && inv.item_ents.0[input.wanted_item_slot.unwrap() as usize].is_some();

        // Handle unequipping current item
        let is_alr_unequipping = inv.equip_state_name == UNEQUIPPING_STATE;
        if has_valid_wanted && input.wanted_item_slot != inv.equipped_slot && !is_alr_unequipping {
            inv.equip_state_name = EquipStateName::from(UNEQUIPPING_STATE);
            inv.equip_state_dur = Duration::ZERO;
        }
        if inv.equipped_slot.is_none() { return; }
        if inv.equipped_slot.is_none() { return; }

        // Handle finishing equip state
        inv.equip_state_dur = inv.equip_state_dur.saturating_add(time.delta());
        while inv.equip_state_dur > Duration::from_millis(2000) {
            match inv.equip_state_name.as_str() {
                EQUIPPING_STATE => {
                    inv.equip_state_name = EquipStateName::from(EQUIPPED_STATE);
                }
                UNEQUIPPING_STATE => {
                    inv.equip_state_name = EquipStateName::from(UNEQUIPPED_STATE);
                }
                _ => {}
            }
            inv.equip_state_dur = inv.equip_state_dur.saturating_sub(Duration::from_millis(2000));
        }

        if inv.equip_state_name != UNEQUIPPED_STATE { return; }

        // We have unequipped the last slot, so we need to starting equipping the new slot
        if has_valid_wanted {
            inv.prev_equipped_slot = inv.equipped_slot;
            inv.equipped_slot = input.wanted_item_slot;
        } else {
            inv.equipped_slot = inv.find_replacement(&mut item_query);
        }
        inv.equip_state_name = EquipStateName::from(EQUIPPING_STATE);
    }
}

pub fn modify_item_sys(
    time: Res<Time>,
    mut item_query: Query<&mut Item>,
    player_query: Query<(&PlayerInput, &Inventory)>,
) {
    for mut item in item_query.iter_mut() {
        let (input, inv): (&PlayerInput, &Inventory) = player_query.get(item.inv_ent).unwrap();
        let is_equipped = inv.equipped_slot == Some(item.inv_slot);
        if is_equipped {
            item.modify(inv, input, &time);
            while item.state_dur > Duration::from_millis(2000) {
                match item.state_name.as_str() {
                    IDLE_STATE | RELOAD_STATE | FIRE_STATE => {
                        item.state_name = ItemStateName::from(IDLE_STATE);
                    }
                    _ => unimplemented!()
                }
                item.state_dur = item.state_dur.saturating_sub(Duration::from_millis(2000));
            }
        }
    }
}

pub fn item_pickup_sys(
    phys_ctx: Res<RapierContext>,
    mut commands: Commands,
    mut inv_query: Query<&mut Inventory>,
    mut item_query: Query<&mut Item>,
    mut pickup_query: Query<&mut ItemPickup>,
) {
    for (ent1, ent2, _inter) in phys_ctx.intersection_pairs() {
        let mut pickup_ent: Option<Entity> = None;
        let mut player_ent: Option<Entity> = None;
        if pickup_query.get(ent1).is_ok() && inv_query.get(ent2).is_ok() {
            pickup_ent = Some(ent1);
            player_ent = Some(ent2);
        } else if pickup_query.get(ent2).is_ok() && inv_query.get(ent1).is_ok() {
            pickup_ent = Some(ent2);
            player_ent = Some(ent1);
        }
        if let Some(pickup_ent) = pickup_ent {
            if let Some(player_ent) = player_ent {
                let pickup = pickup_query.get_mut(pickup_ent).unwrap();
                let mut inv = inv_query.get_mut(player_ent).unwrap();
                inv.push_item(player_ent, &mut commands, &mut item_query, &pickup.item_name);
                commands.entity(pickup_ent).despawn_recursive();
            }
        }
    }
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            equipped_slot: None,
            prev_equipped_slot: None,
            equip_state_name: EquipStateName::from(UNEQUIPPED_STATE),
            equip_state_dur: Duration::ZERO,
            item_ents: Items([None; 10]),
        }
    }
}

impl Item {
    fn start_state(&mut self, _inv: &Inventory, state: ItemStateName, dur: Duration) {
        self.state_name = state;
        self.state_dur = dur;
        match self.state_name.as_str() {
            FIRE_STATE => {
                println!("Boom!");
            }
            _ => {}
        }
    }

    fn can_fire(&mut self, inv: &Inventory, at_state_end: bool) -> bool {
        match (inv.equip_state_name.as_str(), self.state_name.as_str(), at_state_end) {
            (EQUIPPED_STATE, FIRE_STATE, true) | (EQUIPPED_STATE, IDLE_STATE, _) => true,
            _ => false,
        }
    }

    fn modify_status(&mut self, inv: &Inventory, input: &PlayerInput, time: &Res<Time>) {
        while self.state_dur > Duration::from_millis(2000) {
            // We have just finished a state
            self.end_status(inv, input, time);
            let next_state = self.next_state(inv, input);
            self.start_state(inv, next_state, self.state_dur - Duration::from_millis(2000));
        }
        self.state_dur = self.state_dur.saturating_add(time.delta());
    }

    fn next_state(&mut self, inv: &Inventory, input: &PlayerInput) -> ItemStateName {
        let do_fire = input.flags.contains(PlayerInputFlags::Fire) && self.can_fire(inv, true);
        match (self.state_name.as_str(), do_fire) {
            (FIRE_STATE, true) => ItemStateName::from(FIRE_STATE),
            _ => ItemStateName::from(IDLE_STATE)
        }
    }

    fn end_status(&mut self, _inv: &Inventory, _input: &PlayerInput, _time: &Res<Time>) {}

    fn modify(&mut self, inv: &Inventory, input: &PlayerInput, time: &Res<Time>) {
        if input.flags.contains(PlayerInputFlags::Fire) && self.can_fire(inv, false) {
            self.start_state(inv, ItemStateName::from(FIRE_STATE), Duration::ZERO);
        } else if input.flags.contains(PlayerInputFlags::Reload) {
            self.start_state(inv, ItemStateName::from(RELOAD_STATE), Duration::ZERO);
        }
        self.modify_status(inv, input, time);
    }
}

impl Inventory {
    fn find_replacement(&self, item_query: &mut Query<&mut Item>) -> Option<u8> {
        if self.prev_equipped_slot.is_none() {
            self.find_slot(item_query, |item| item.is_some())
        } else {
            self.prev_equipped_slot
        }
    }

    fn find_slot(
        &self, item_query: &mut Query<&mut Item>, predicate: impl Fn(Option<&Item>) -> bool,
    ) -> Option<u8> {
        for (slot, &item_ent) in self.item_ents.0.iter().enumerate() {
            let slot = slot as u8;
            let item = match item_ent {
                Some(item_ent) => item_query.get(item_ent).ok(),
                None => None,
            };
            if predicate(item) {
                return Some(slot);
            }
        }
        None
    }

    pub fn push_item(
        &mut self,
        inv_ent: Entity,
        commands: &mut Commands,
        item_query: &mut Query<&mut Item>,
        item_name: &ItemName,
    ) {
        let open_slot = self.find_slot(item_query, |item| item.is_none());
        if let Some(open_slot) = open_slot {
            self.set_item(inv_ent, commands, item_name, open_slot);
        }
    }

    pub fn set_item(
        &mut self,
        inv_ent: Entity,
        commands: &mut Commands,
        item_name: &ItemName, slot: u8,
    ) -> &mut Self {
        let existing_item_ent = self.item_ents.0[slot as usize];
        if let Some(existing_item_ent) = existing_item_ent {
            commands.entity(existing_item_ent).despawn()
        }
        let item_ent = commands.spawn(Item {
            name: item_name.clone(),
            amount: 1,
            state_name: ItemStateName::from(IDLE_STATE),
            state_dur: Duration::ZERO,
            inv_ent,
            inv_slot: slot,
        }).id();
        if self.equipped_slot.is_none() {
            self.equipped_slot = Some(slot);
            self.equip_state_dur = Duration::ZERO;
            self.equip_state_name = EquipStateName::from(EQUIPPING_STATE);
        }
        self.item_ents.0[slot as usize] = Some(item_ent);
        self
    }
}

// ██████╗ ███████╗███╗   ██╗██████╗ ███████╗██████╗
// ██╔══██╗██╔════╝████╗  ██║██╔══██╗██╔════╝██╔══██╗
// ██████╔╝█████╗  ██╔██╗ ██║██║  ██║█████╗  ██████╔╝
// ██╔══██╗██╔══╝  ██║╚██╗██║██║  ██║██╔══╝  ██╔══██╗
// ██║  ██║███████╗██║ ╚████║██████╔╝███████╗██║  ██║
// ╚═╝  ╚═╝╚══════╝╚═╝  ╚═══╝╚═════╝ ╚══════╝╚═╝  ╚═╝

pub fn render_inventory_sys(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    materials: Res<DefaultMaterials>,
    item_query: Query<&mut Item>,
    player_query: Query<&Inventory>,
    camera_query: Query<&Transform, With<PerspectiveProjection>>,
) {
    for inv in player_query.iter() {
        for item in inv.item_ents.0.iter() {
            if let Some(item_ent) = item {
                if let Ok(item) = item_query.get(*item_ent) {
                    let is_equipped = inv.equipped_slot == Some(item.inv_slot);
                    let mut transform = Transform::default();
                    let mesh_handle = asset_server.load(format!("models/{}.gltf#Mesh0/Primitive0", item.name).as_str());
                    if is_equipped {
                        transform = camera_query.single().mul_transform(Transform::from_xyz(0.4, -0.3, -1.0));
                    }
                    commands.entity(*item_ent).insert(PbrBundle {
                        mesh: mesh_handle.clone(),
                        material: materials.gun_material.clone(),
                        transform,
                        visibility: Visibility { is_visible: is_equipped },
                        ..default()
                    });
                }
            }
        }
    }
}

pub fn item_pickup_animate_sys(
    time: Res<Time>,
    mut pickup_query: Query<&mut Transform, With<ItemPickupVisual>>,
) {
    for mut transform in pickup_query.iter_mut() {
        let dr = TAU * time.delta_seconds() * 0.125;
        transform.rotate(Quat::from_axis_angle(Vec3::Y, dr));
        let height = f32::sin(time.elapsed().as_secs_f32()) * 0.125;
        transform.translation = Vec3::new(0.0, height, 0.0);
    }
}
