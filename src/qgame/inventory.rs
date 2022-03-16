use std::{
    f32::consts::TAU,
    option::Option,
    time::Duration,
};
use std::ops::Deref;

use bevy::{
    prelude::*,
    utils::HashMap,
};
use bevy_rapier3d::prelude::*;

use crate::PlayerInput;

const EQUIPPING_STATE: EquipStateName = "equipping";
const EQUIPPED_STATE: EquipStateName = "equipped";
const UNEQUIPPING_STATE: EquipStateName = "unequipping";
const UNEQUIPPED_STATE: EquipStateName = "unequipped";
const IDLE_STATE: ItemStateName = "idle";
const RELOAD_STATE: ItemStateName = "reload";
const FIRE_STATE: ItemStateName = "fire";

type ItemName = &'static str;
type ItemStateName = &'static str;
type EquipStateName = &'static str;

pub struct ItemStateProps {
    pub duration: Duration,
    pub is_persistent: bool,
}

pub struct ItemProps {
    pub name: ItemName,
    pub move_factor: f32,
    pub states: HashMap<ItemStateName, ItemStateProps>,
    pub equip_states: HashMap<EquipStateName, ItemStateProps>,
}

pub struct WeaponProps {
    pub damage: u16,
    pub headshot_factor: f32,
}

pub struct GunProps {
    pub mag_size: u16,
    pub starting_ammo_in_reserve: u16,
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

#[derive(Component, Debug)]
pub struct Inventory {
    pub equipped_slot: Option<u8>,
    pub prev_equipped_slot: Option<u8>,
    pub equip_state_name: EquipStateName,
    pub equip_state_dur: Duration,
    pub item_ents: Items,
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            equipped_slot: None,
            prev_equipped_slot: None,
            equip_state_name: UNEQUIPPED_STATE.clone(),
            equip_state_dur: Duration::ZERO,
            item_ents: Items([None; 10]),
        }
    }
}

impl Inventory {
    // fn get_item_mut(&self, mut item_query: &mut Query<&mut Item>, slot: u8) -> Option<Mut<Item>> {
    //     match self.item_ents.0[slot as usize] {
    //         Some(item_ent) => {
    //             match item_query.get_mut(item_ent) {
    //                 Ok(item) => Some(item),
    //                 Err(_) => None,
    //             }
    //         }
    //         None => None,
    //     }
    // }
    //
    // pub fn get_item(&self, item_query: &Query<&Item>, slot: u8) -> Option<&Item> {
    //     match self.item_ents.0[slot as usize] {
    //         Some(item_ent) => {
    //             match item_query.get(item_ent) {
    //                 Ok(item) => Some(item),
    //                 Err(_) => None,
    //             }
    //         }
    //         None => None,
    //     }
    // }

    fn start_item_state(&self, mut item: Mut<Item>, state: ItemStateName, dur: Duration) {
        item.state_name = state;
        item.state_dur = dur;
        match item.state_name {
            _ => unimplemented!()
        }
    }

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
                Some(item_ent) => item_query.get(item_ent),
                None => Err(bevy::ecs::query::QueryEntityError::NoSuchEntity),
            }.ok();
            if predicate(item) {
                return Some(slot);
            }
        }
        None
    }

    pub fn insert_item(
        &mut self,
        inv_ent: Entity,
        commands: &mut Commands,
        mut item_query: &mut Query<&mut Item>,
        item_name: ItemName,
    ) {
        let open_slot = self.find_slot(item_query, |item| item.is_none());
        if let Some(open_slot) = open_slot {
            self.set_item(inv_ent, commands, item_query, item_name, open_slot);
        }
    }

    pub fn set_item(
        &mut self,
        inv_ent: Entity,
        commands: &mut Commands,
        mut item_query: &mut Query<&mut Item>,
        item_name: ItemName, slot: u8,
    ) -> &mut Self {
        let existing_item_ent = self.item_ents.0[slot as usize];
        if let Some(existing_item_ent) = existing_item_ent {
            commands.entity(existing_item_ent).despawn()
        }
        let item_ent = commands.spawn()
            .insert(Item {
                name: item_name,
                amount: 1,
                state_name: IDLE_STATE,
                state_dur: Duration::ZERO,
                inv_ent,
                inv_slot: slot,
            }).id();
        if self.equipped_slot.is_none() {
            self.equipped_slot = Some(slot);
            self.equip_state_dur = Duration::ZERO;
            self.equip_state_name = EQUIPPING_STATE;
        }
        self.item_ents.0[slot as usize] = Some(item_ent);
        self
    }
}

pub fn modify_equip_state_sys(
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut inv_query: Query<(&PlayerInput, &mut Inventory)>,
    mut item_query: Query<&mut Item>,
) {
    // let item_handles = asset_server.load_folder("items").unwrap();

    for (input, mut inv) in inv_query.iter_mut() {
        let input: &PlayerInput = input;
        let mut inv: Mut<'_, Inventory> = inv;

        let has_valid_wanted = input.wanted_item_slot.is_some()
            && inv.item_ents.0[input.wanted_item_slot.unwrap() as usize].is_some();

        // Handle unequipping current item
        let is_alr_unequipping = inv.equip_state_name == UNEQUIPPING_STATE;
        if has_valid_wanted && input.wanted_item_slot != inv.equipped_slot && !is_alr_unequipping {
            inv.equip_state_name = UNEQUIPPING_STATE;
            inv.equip_state_dur = Duration::ZERO;
        }
        if inv.equipped_slot.is_none() { return; }

        // Handle finishing equip status
        inv.equip_state_dur = inv.equip_state_dur.saturating_add(time.delta());
        while inv.equip_state_dur > Duration::from_millis(2000) {
            match inv.equip_state_name {
                EQUIPPING_STATE => {
                    inv.equip_state_name = EQUIPPED_STATE;
                }
                UNEQUIPPING_STATE => {
                    inv.equip_state_name = UNEQUIPPED_STATE;
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
        inv.equip_state_name = EQUIPPING_STATE;
    }
}

pub fn modify_item_sys(
    time: Res<Time>,
    mut item_query: Query<&mut Item>,
    inv_query: Query<&Inventory>,
) {
    for mut item in item_query.iter_mut() {
        let is_equipped = inv_query.get(item.inv_ent).unwrap().equipped_slot == Some(item.inv_slot);
        if is_equipped {
            item.state_dur = item.state_dur.saturating_add(time.delta());
            while item.state_dur > Duration::from_millis(2000) {
                match item.state_name {
                    IDLE_STATE | RELOAD_STATE | FIRE_STATE => {
                        item.state_name = IDLE_STATE;
                    }
                    _ => unimplemented!()
                }
                item.state_dur = item.state_dur.saturating_sub(Duration::from_millis(2000));
            }
        }
    }
}

pub fn item_pickup_sys(
    mut commands: Commands,
    // query_pipeline: Res<QueryPipeline>,
    // collider_query: QueryPipelineColliderComponentsQuery,
    // mut inv_query: Query<(&mut Inventory, &ColliderShapeComponent)>,
    mut intersection_events: EventReader<IntersectionEvent>,
    mut inv_query: Query<&mut Inventory>,
    mut item_query: Query<&mut Item>,
    mut pickup_query: Query<&mut ItemPickup>,
) {
    // TODO:design use shape cast instead of reading events?
    // let collider_set = QueryPipelineColliderComponentsSet(&collider_query);
    //
    // for (mut inv, player_collider) in inv_query.iter_mut() {
    //     let mut inv: Mut<'_, Inventory> = inv;
    //     let player_collider: &ColliderShapeComponent = player_collider;
    //
    //     query_pipeline.intersections_with_shape(&collider_set, )
    // }
    for intersection_event in intersection_events.iter() {
        let intersection: &IntersectionEvent = intersection_event;
        let ent1 = intersection.collider1.entity();
        let ent2 = intersection.collider2.entity();
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
                let mut pickup = pickup_query.get_mut(pickup_ent).unwrap();
                let mut inv = inv_query.get_mut(player_ent).unwrap();
                inv.insert_item(player_ent, &mut commands, &mut item_query, pickup.item_name);
                // commands.entity(pickup_ent).despawn_recursive();
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
        let height = f32::sin(time.time_since_startup().as_secs_f32()) * 0.125;
        transform.translation = Vec3::new(0.0, height, 0.0);
    }
}

