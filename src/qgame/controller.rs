use std::f32::consts::{FRAC_PI_4, FRAC_PI_6, PI};

use bevy::prelude::*;
use bevy_rapier3d::{
    na::Point3,
    prelude::*,
    rapier::parry::query::RayCast,
};

use crate::{PlayerInput, PlayerInputFlags};

pub enum MoveMode {
    Noclip,
    Ground,
}

#[derive(Component)]
pub struct PlayerController {
    pub move_mode: MoveMode,
    pub enabled: bool,
    pub fly_speed: f32,
    pub fast_fly_speed: f32,
    pub gravity: f32,
    pub walk_speed: f32,
    pub run_speed: f32,
    pub fwd_speed: f32,
    pub side_speed: f32,
    pub friction: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub velocity: Vec3,
    pub ground_tick: u8,
    pub stop_speed: f32,
}

impl Default for PlayerController {
    fn default() -> Self {
        Self {
            move_mode: MoveMode::Noclip,
            enabled: true,
            fly_speed: 10.0,
            fast_fly_speed: 30.0,
            gravity: 9.8,
            walk_speed: 10.0,
            run_speed: 30.0,
            fwd_speed: 0.0,
            side_speed: 10.0,
            friction: 0.5,
            pitch: FRAC_PI_4,
            yaw: -FRAC_PI_6,
            velocity: Vec3::ZERO,
            ground_tick: 0,
            stop_speed: 1.0,
        }
    }
}

fn friction(lateral_speed: f32, friction: f32, stop_speed: f32, dt: f32, velocity: &mut Vec3) {
    let control = f32::max(lateral_speed, stop_speed);
    let drop = control * friction * dt;
    let new_speed = f32::max((lateral_speed - drop) / lateral_speed, 0.0);
    velocity.x *= new_speed;
    velocity.y *= new_speed;
}

fn accelerate(wish_dir: Vec3, wish_speed: f32, accel: f32, dt: f32, velocity: &mut Vec3) {
    let vel_proj = Vec3::dot(*velocity, wish_dir);
    let add_speed = wish_speed - vel_proj;
    if add_speed <= 0.0 { return; }

    let accel_speed = f32::min(accel * wish_speed * dt, add_speed);
    let wish_dir = wish_dir * accel_speed;
    velocity.x += wish_dir.x;
    velocity.y += wish_dir.y;
}

pub fn player_controller_system(
    time: Res<Time>,
    query_pipeline: Res<QueryPipeline>, collider_query: QueryPipelineColliderComponentsQuery,
    mut query: Query<
        (&mut Transform, &mut PlayerInput, &mut PlayerController, &ColliderShapeComponent, &mut RigidBodyVelocityComponent),
        With<Camera>
    >,
) {
    let dt = time.delta_seconds();

    for (mut transform, input, mut controller, collider, mut rb_vel) in query.iter_mut() {
        if !controller.enabled {
            continue;
        }

        let right = transform.right();
        let fwd = transform.forward();

        match controller.move_mode {
            MoveMode::Noclip => {
                if input.movement == Vec3::ZERO {
                    let friction = controller.friction.clamp(0.0, 1.0);
                    controller.velocity *= 1.0 - friction;
                    if controller.velocity.length_squared() < 1e-6 {
                        controller.velocity = Vec3::ZERO;
                    }
                } else {
                    let fly_speed = if input.flags.contains(PlayerInputFlags::Sprint) {
                        controller.fly_speed
                    } else {
                        controller.fast_fly_speed
                    };
                    controller.velocity = input.movement.normalize() * fly_speed;
                }
                transform.translation = transform.translation
                    + controller.velocity.x * dt * right
                    + controller.velocity.z * dt * Vec3::Z
                    + controller.velocity.y * dt * fwd;
            }

            MoveMode::Ground => {
                if let Some(capsule) = collider.as_capsule() {
                    // let capsule: Capsule = capsule;
                    // let transform: Transform = transform;

                    let init_vel = controller.velocity;
                    let end_vel = init_vel;
                    let lateral_speed = (init_vel.x * init_vel.x + init_vel.y * init_vel.y).sqrt();

                    let pos = transform.translation;

                    let collider_set = QueryPipelineColliderComponentsSet(&collider_query);

                    let shape = Capsule::new(capsule.segment.a, capsule.segment.b, capsule.radius * 1.0625);
                    let shape_pos = (transform.translation, transform.rotation).into();
                    let shape_vel = Vec3::new(0.0, 0.0, -0.1).into();
                    let max_toi = 4.0;
                    let groups = InteractionGroups::all();
                    let filter = None;

                    if let Some((handle, hit)) = query_pipeline.cast_shape(
                        &collider_set, &shape_pos, &shape_vel, &shape, max_toi, groups, filter,
                    ) {
                        println!("Hit the entity {:?} with the configuration: {:?}", handle.entity(), hit);
                    }

                    let mut wish_dir = input.movement.y * controller.fwd_speed * fwd + input.movement.x * controller.side_speed * right;
                    let mut wish_speed = wish_dir.length();
                    wish_dir /= wish_speed; // normalize

                    let max_speed = if input.flags.contains(PlayerInputFlags::Sprint) {
                        controller.run_speed
                    } else {
                        controller.walk_speed
                    };

                    wish_speed = f32::min(wish_speed, max_speed);
                }
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