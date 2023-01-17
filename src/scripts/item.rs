use std::time::Duration;

use bevy::utils::HashMap;

use engine::{ItemName, ItemProps, ItemStateName, ItemStateProps};

fn main() {
    let props = ItemProps {
        name: ItemName::from("knife"),
        move_factor: 1.0,
        states: HashMap::from([
            (ItemStateName::from("idle"), ItemStateProps {
                duration: Duration::from_millis(100),
                is_persistent: true,
            })
        ]),
        equip_states: Default::default(),
    };
    let toml = toml::to_string(&props).unwrap();
    println!("{}", toml);
}