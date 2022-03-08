use std::{
    option::Option,
    time::Duration,
};

use bevy::{
    prelude::*,
};

const EQUIP_STATE: &str = "equip_state";
const UNEQUIP_STATE: &str = "unequip_state";
const IDLE_STATE: &str = "idle_state";
const RELOAD_STATE: &str = "reload_state";
const FIRE_STATE: &str = "fire_state";

#[derive(Component)]
pub struct Item {
    pub name: String,
    pub amount: u16,
    pub state: String,
    pub state_dur: Duration,
}

#[derive(Component)]
pub struct Gun {
    pub ammo: u16,
    pub ammo_in_reserve: u16,
}

#[derive(Component)]
pub struct Inventory {
    pub equipped_idx: Option<usize>,
    pub prev_equipped_idx: Option<usize>,
    pub equip_state: String,
    pub equip_state_dur: Duration,
}

fn modify_equip_state(time: Res<Time>, inventory: &mut Inventory, wanted_item_idx: Option<usize>) {
    let has_valid_equip = inventory.equipped_idx.is_some();
    let is_alr_unequipping = inventory.equip_state == UNEQUIP_STATE;
    if has_valid_equip && wanted_item_idx != inventory.equipped_idx && !is_alr_unequipping {
        inventory.equip_state = UNEQUIP_STATE.to_string();
        inventory.equip_state_dur = Duration::ZERO;
    }
    if inventory.equipped_idx.is_none() { return; }
}

pub fn manage_inventory_system(
    mut commands: Commands,
    time: Res<Time>,
    mut inv_query: Query<(&mut Inventory, &Children)>,
    mut item_query: Query<&mut Item>,
) {
    for (mut inv, children) in inv_query.iter_mut() {
        for (item_idx, child) in children.iter().enumerate() {
            let item = item_query.get_mut(*child).unwrap();

            if let Some(equipped_idx) = inv.equipped_idx {
                if equipped_idx == item_idx {
                    item.state_dur.saturating_add(time.delta());
                }
            }
        }
    }
}