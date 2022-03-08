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

fn find_item<F: Fn(&Item) -> bool>(
    items: &Children, mut item_query: Query<&mut Item>, predicate: F,
) -> Option<usize> {
    for (idx, item_ent) in items.iter().enumerate() {
        let item = item_query.get(*item_ent).unwrap();
        if predicate(item) {
            return Some(idx);
        }
    }
    return None;
}

fn find_replacement(
    inv: &Inventory, items: &Children, mut item_query: Query<&mut Item>,
) -> Option<usize> {
    if inv.prev_equipped_idx.is_none() {
        return find_item(items, item_query, |item| item.state == IDLE_STATE);
    }
    return inv.prev_equipped_idx;
}

fn modify_equip_state(
    time: Res<Time>, wanted_item_idx: Option<usize>,
    inv: &mut Inventory, items: &Children, mut item_query: Query<&mut Item>,
) {
    let has_valid_wanted = inv.equipped_idx.is_some();
    let is_alr_unequipping = inv.equip_state == UNEQUIP_STATE;
    if has_valid_wanted && wanted_item_idx != inv.equipped_idx && !is_alr_unequipping {
        inv.equip_state = UNEQUIP_STATE.to_string();
        inv.equip_state_dur = Duration::ZERO;
    }
    if inv.equipped_idx.is_none() { return; }

    inv.equip_state_dur.saturating_add(time.delta());
    while inv.equip_state_dur > Duration::from_millis(100) {
        match inv.equip_state {
            EQUIP_STATE => {
                inv.equip_state = String::from(EQUIP_STATE);
            }
            UNEQUIP_STATE => {
                inv.equip_state = String::from(UNEQUIP_STATE);
            }
            _ => unimplemented!()
        }
        inv.equip_state_dur.saturating_sub(Duration::from_millis(100));
    }

    if inv.equip_state != UNEQUIP_STATE { return; }

    if has_valid_wanted {
        inv.prev_equipped_idx = inv.equipped_idx;
        inv.equipped_idx = wanted_item_idx;
    } else {
        inv.equipped_idx = find_replacement(inv, items, item_query);
    }
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