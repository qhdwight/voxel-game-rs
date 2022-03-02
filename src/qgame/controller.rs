use std::f32::consts::{FRAC_PI_4, FRAC_PI_6, PI};

use bevy::prelude::*;

use crate::{PlayerInput, PlayerInputFlags};

#[derive(Component)]
pub struct CameraController {
    pub enabled: bool,
    pub walk_speed: f32,
    pub run_speed: f32,
    pub friction: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub velocity: Vec3,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
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

pub fn camera_controller(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut PlayerInput, &mut CameraController), With<Camera>>,
) {
    let dt = time.delta_seconds();

    for (mut transform, input, mut options) in query.iter_mut() {
        if !options.enabled {
            continue;
        }

        let input: Mut<PlayerInput> = input;

        // Apply movement update
        if input.movement != Vec3::ZERO {
            let max_speed = if input.flags.contains(PlayerInputFlags::Sprint) {
                options.run_speed
            } else {
                options.walk_speed
            };
            options.velocity = input.movement.normalize() * max_speed;
        } else {
            let friction = options.friction.clamp(0.0, 1.0);
            options.velocity *= 1.0 - friction;
            if options.velocity.length_squared() < 1e-6 {
                options.velocity = Vec3::ZERO;
            }
        }
        let forward = transform.forward();
        let right = transform.right();
        transform.translation += options.velocity.x * dt * right
            + options.velocity.z * dt * Vec3::Z
            + options.velocity.y * dt * forward;

        // Apply look update
        let (pitch, yaw) = (
            (options.pitch - input.mouse.y * 0.5 * dt).clamp(
                0.001,
                PI - 0.001,
            ),
            options.yaw - input.mouse.x * dt,
        );
        transform.rotation = Quat::from_euler(EulerRot::YZX, 0.0, yaw, pitch);
        options.pitch = pitch;
        options.yaw = yaw;
    }
}