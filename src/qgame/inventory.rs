use std::{
    option::Option,
    time::Duration,
};

use bevy::{
    prelude::*,
};
use bevy::utils::HashMap;

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

#[derive(Component)]
pub struct Item {
    pub name: ItemName,
    pub amount: u16,
    pub state_name: ItemStateName,
    pub state_dur: Duration,
    pub inv_ent: Entity,
    pub inv_slot: u8,
}

#[derive(Component)]
pub struct Gun {
    pub ammo: u16,
    pub ammo_in_reserve: u16,
}

pub struct Items([Option<Entity>; 10]);

#[derive(Component)]
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
            equip_state_name: EQUIPPING_STATE.clone(),
            equip_state_dur: Duration::ZERO,
            item_ents: Items([None; 10]),
        }
    }
}

impl Inventory {
    fn start_item_state(&self, mut item: Mut<Item>, state: ItemStateName, dur: Duration) {
        item.state_name = state;
        item.state_dur = dur;
        match item.state_name {
            _ => unimplemented!()
        }
    }

    fn find_replacement(&self, mut item_query: &Query<&mut Item>) -> Option<u8> {
        if self.prev_equipped_slot.is_none() {
            self.find_item(item_query, |item| item.is_none())
        } else {
            self.prev_equipped_slot
        }
    }

    fn find_item(
        &self, mut item_query: &Query<&mut Item>, predicate: impl Fn(Option<&Item>) -> bool,
    ) -> Option<u8> {
        for (slot, &item_ent) in self.item_ents.0.iter().enumerate() {
            let item = if let Some(item_ent) = item_ent {
                Some(item_query.get(item_ent).unwrap())
            } else {
                None
            };
            if predicate(item) {
                return Some(slot as u8);
            }
        }
        None
    }

    fn insert_item(
        &mut self, inv_ent: Entity,
        commands: &mut Commands,
        mut item_query: &mut Query<&mut Item>,
        item_name: ItemName, slot: u8,
    ) {
        let existing_itme_ent = self.item_ents.0[slot as usize];
        if let Some(existing_itme_ent) = existing_itme_ent {
            commands.entity(existing_itme_ent).despawn()
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
    }
}

pub fn modify_equip_state_system(
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut inv_query: Query<(&PlayerInput, &mut Inventory)>,
    mut item_query: Query<&mut Item>,
) {
    let item_handles = asset_server.load_folder("items").unwrap();

    for (input, mut inv) in inv_query.iter_mut() {
        let input: &PlayerInput = input;
        let mut inv: Mut<'_, Inventory> = inv;

        let has_valid_wanted = inv.equipped_slot.is_some();
        let is_alr_unequipping = inv.equip_state_name == UNEQUIPPED_STATE;
        if has_valid_wanted && input.wanted_item_slot != inv.equipped_slot && !is_alr_unequipping {
            inv.equip_state_name = UNEQUIPPED_STATE;
            inv.equip_state_dur = Duration::ZERO;
        }
        if inv.equipped_slot.is_none() { return; }

        inv.equip_state_dur = inv.equip_state_dur.saturating_add(time.delta());
        while inv.equip_state_dur > Duration::from_millis(100) {
            match inv.equip_state_name {
                EQUIPPING_STATE => {
                    inv.equip_state_name = EQUIPPED_STATE;
                }
                UNEQUIPPING_STATE => {
                    inv.equip_state_name = UNEQUIPPED_STATE;
                }
                _ => {}
            }
            inv.equip_state_dur = inv.equip_state_dur.saturating_sub(Duration::from_millis(100));
        }

        if inv.equip_state_name != UNEQUIPPED_STATE { return; }

        if has_valid_wanted {
            inv.prev_equipped_slot = inv.equipped_slot;
            inv.equipped_slot = input.wanted_item_slot;
        } else {
            inv.equipped_slot = inv.find_replacement(&item_query);
        }
    }
}

pub fn modify_item_system(
    time: Res<Time>,
    mut item_query: Query<&mut Item>,
    inv_query: Query<&Inventory>,
) {
    for mut item in item_query.iter_mut() {
        let is_equipped = inv_query.get(item.inv_ent).unwrap().equipped_slot == Some(item.inv_slot);
        if is_equipped {
            item.state_dur = item.state_dur.saturating_add(time.delta());
            while item.state_dur > Duration::from_millis(100) {
                match item.state_name {
                    RELOAD_STATE | FIRE_STATE => {
                        item.state_name = IDLE_STATE;
                    }
                    _ => unimplemented!()
                }
                item.state_dur = item.state_dur.saturating_sub(Duration::from_millis(100));
            }
        }
    }
}

fn set_item_at_index() {}
