use std::f32::consts::{FRAC_PI_4, FRAC_PI_6, PI};

use bevy::prelude::*;

use crate::{PlayerInput, PlayerInputFlags};

pub enum MoveMode {
    Noclip,
    Ground,
}

#[derive(Component)]
pub struct PlayerController {
    pub move_mode: MoveMode,
    pub enabled: bool,
    pub walk_speed: f32,
    pub run_speed: f32,
    pub friction: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub velocity: Vec3,
}

impl Default for PlayerController {
    fn default() -> Self {
        Self {
            move_mode: MoveMode::Noclip,
            enabled: true,
            walk_speed: 10.0,
            run_speed: 30.0,
            friction: 0.5,
            pitch: FRAC_PI_4,
            yaw: -FRAC_PI_6,
            velocity: Vec3::ZERO,
        }
    }
}

pub fn player_controller_system(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut PlayerInput, &mut PlayerController), With<Camera>>,
) {
    let dt = time.delta_seconds();

    for (mut transform, input, mut controller) in query.iter_mut() {
        if !controller.enabled {
            continue;
        }

        match controller.move_mode {
            MoveMode::Noclip => {
                if input.movement == Vec3::ZERO {
                    let friction = controller.friction.clamp(0.0, 1.0);
                    controller.velocity *= 1.0 - friction;
                    if controller.velocity.length_squared() < 1e-6 {
                        controller.velocity = Vec3::ZERO;
                    }
                } else {
                    let max_speed = if input.flags.contains(PlayerInputFlags::Sprint) {
                        controller.run_speed
                    } else {
                        controller.walk_speed
                    };
                    controller.velocity = input.movement.normalize() * max_speed;
                }
                let right = transform.right();
                let fwd = transform.forward();
                transform.translation
                    += controller.velocity.x * dt * right
                    + controller.velocity.z * dt * Vec3::Z
                    + controller.velocity.y * dt * fwd;
            }

            MoveMode::Ground => {

            }
        }

        let (pitch, yaw) = (
            (controller.pitch - input.mouse.y * 0.5 * dt).clamp(
                0.001,
                PI - 0.001,
            ),
            controller.yaw - input.mouse.x * dt,
        );
        transform.rotation = Quat::from_euler(EulerRot::YZX, 0.0, yaw, pitch);
        controller.pitch = pitch;
        controller.yaw = yaw;
    }
}