extern crate core;

use std::{
    f32::consts::TAU,
    fmt::Write,
};

use bevy::{
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    prelude::*,
    render::{
        mesh::{Indices, VertexAttributeValues},
        render_resource::*,
    },
    window::WindowDescriptor,
};
use bevy_rapier3d::prelude::*;

use qgame::*;

mod qgame;

#[derive(Component)]
struct TopRightText;

#[derive(Component)]
struct PlayerHudText;

pub struct DefaultMaterials {
    pub gun_material: Handle<StandardMaterial>,
}

#[derive(Clone, Hash, Debug, PartialEq, Eq, SystemLabel)]
pub enum Modify {
    Set,
    Equip,
    Item,
    Look,
    Move,
    Pickup,
}

#[derive(Clone, Hash, Debug, PartialEq, Eq, SystemLabel)]
pub enum Render {
    Set,
    Look,
}

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(WindowDescriptor {
            title: String::from("QGame"),
            ..default()
        })
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 0.25,
        })
        .add_plugins(DefaultPlugins)
        .insert_resource(RapierConfiguration {
            ..default()
        })
        .add_plugin(RapierPhysicsPlugin::<NoUserData>::default())
        .add_plugin(RapierDebugRenderPlugin::default())
        .add_plugin(VoxelsPlugin)
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .add_plugin(InventoryPlugin)
        .add_asset::<Config>()
        .init_asset_loader::<ConfigAssetLoader>()
        .add_startup_system(setup_sys)
        .add_startup_system(spawn_ui_sys)
        .add_startup_system(spawn_voxel_sys)
        .add_startup_system(spawn_player_sys)
        .add_system_set_to_stage(CoreStage::PreUpdate, SystemSet::new()
            .with_system(player_input_system),
        )
        .add_system(cursor_grab_sys)
        .add_system(update_fps_text_sys)
        .add_system_set(SystemSet::new()
            .label(Modify::Set)
            .with_system(player_look_sys
                .label(Modify::Look))
            .with_system(player_move_sys
                .label(Modify::Move).after(Modify::Look))
            .with_system(modify_equip_state_sys
                .label(Modify::Equip).after(Modify::Move))
            .with_system(modify_item_sys
                .label(Modify::Item).after(Modify::Equip))
            .with_system(item_pickup_sys
                .label(Modify::Pickup).after(Modify::Item))
        )
        .add_system_set(SystemSet::new()
            .label(Render::Set).after(Modify::Set)
            .with_system(item_pickup_animate_sys)
            .with_system(render_player_camera_sys
                .label(Render::Look))
            .with_system(render_inventory_sys
                .after(Render::Look))
            .with_system(update_hud_system)
        )
        .run();
}

fn setup_sys(
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // println!("{}", toml::to_string(&Config::default()).unwrap());

    let config: Handle<Config> = asset_server.load("default.config.toml");
    commands.insert_resource(config);

    // commands.spawn_bundle(PointLightBundle {
    //     point_light: PointLight {
    //         intensity: 2000.0,
    //         shadows_enabled: true,
    //         ..default()
    //     },
    //     transform: Transform::from_xyz(38.0, -34.0, 40.0),
    //     ..default()
    // });

    commands.spawn_bundle(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 2000.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(-38.0, 40.0, 34.0),
        ..default()
    });

    // {
    //     let mesh = meshes.add(Mesh::from(bevy::prelude::shape::Cube { size: 1.0 }));
    //     let material = materials.add(StandardMaterial {
    //         base_color: Color::PINK,
    //         ..default()
    //     });
    //     commands.spawn()
    //         .insert_bundle(PbrBundle {
    //             mesh: mesh.clone(),
    //             material: material.clone(),
    //             transform: Transform::from_xyz(-18.0, 32.0, -18.0),
    //             ..default()
    //         })
    //         .insert_bundle(ColliderBundle {
    //             shape: ColliderShape::cuboid(1.0, 1.0, 1.0).into(),
    //             collider_type: ColliderType::Solid.into(),
    //             position: Vec3::new(-18.0, 32.0, -18.0).into(),
    //             ..default()
    //         });
    // }

    let gun_material = materials.add(StandardMaterial {
        base_color: Color::DARK_GRAY,
        metallic: 0.05,
        perceptual_roughness: 0.1,
        ..default()
    });

    // let rifle_handle = asset_server.load("models/rifle.gltf#Mesh0/Primitive0");
    // commands.spawn()
    //     .insert(GlobalTransform::default())
    //     .with_children(|parent| {
    //         parent.spawn_bundle(PbrBundle {
    //             mesh: rifle_handle.clone(),
    //             material: gun_material.clone(),
    //             ..default()
    //         })
    //             .insert(ItemPickupVisual::default());
    //     })
    //     .insert(Collider::ball(0.5))
    //     .insert(Sensor(true))
    //     .insert(Transform::from_xyz(0.0, 20.0, 8.0))
    //     .insert(ItemPickup { item_name: ItemName::from("rifle") });

    commands.insert_resource(DefaultMaterials { gun_material });

    asset_server.watch_for_changes().unwrap()
}

fn spawn_ui_sys(asset_server: Res<AssetServer>, mut commands: Commands) {
    let font = asset_server.load("fonts/FiraMono-Medium.ttf");
    commands.spawn_bundle(UiCameraBundle::default());

    commands
        .spawn_bundle(TextBundle {
            style: Style {
                align_self: AlignSelf::FlexEnd,
                position_type: PositionType::Absolute,
                position: Rect {
                    top: Val::Px(5.0),
                    right: Val::Px(5.0),
                    ..default()
                },
                ..default()
            },
            text: Text {
                sections: vec![
                    TextSection {
                        value: "".to_string(),
                        style: TextStyle { font: font.clone(), font_size: 16.0, color: Color::WHITE },
                    },
                ],
                alignment: Default::default(),
            },
            ..default()
        })
        .insert(TopRightText);

    commands
        .spawn_bundle(TextBundle {
            style: Style {
                align_self: AlignSelf::FlexEnd,
                position_type: PositionType::Absolute,
                position: Rect {
                    bottom: Val::Px(5.0),
                    left: Val::Px(5.0),
                    ..default()
                },
                ..default()
            },
            text: Text {
                sections: vec![
                    TextSection {
                        value: "".to_string(),
                        style: TextStyle { font: font.clone(), font_size: 12.0, color: Color::ANTIQUE_WHITE },
                    },
                ],
                alignment: Default::default(),
            },
            ..default()
        })
        .insert(PlayerHudText);
}

fn spawn_voxel_sys(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
    mesh.set_indices(Some(Indices::U32(Vec::with_capacity(4096))));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, VertexAttributeValues::Float32x3(Vec::with_capacity(4096)));
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, VertexAttributeValues::Float32x3(Vec::with_capacity(4096)));
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, VertexAttributeValues::Float32x2(Vec::with_capacity(4096)));
    let mesh_handle = meshes.add(mesh);
    let ground_mat_handle = materials.add(StandardMaterial {
        base_color: Color::DARK_GREEN,
        ..default()
    });
    commands.spawn().insert(Map::default());
    commands.spawn()
        .insert(Chunk::new(IVec3::ZERO))
        // .insert(AsyncCollider::Mesh(mesh_handle.clone()))
        .insert_bundle(PbrBundle {
            mesh: mesh_handle.clone(),
            material: ground_mat_handle.clone(),
            ..default()
        });
}

fn spawn_player_sys(mut commands: Commands) {
    let inv = Inventory::default();
    commands.spawn()
        .insert(Collider::capsule(Vec3::Y * 0.5, Vec3::Y * 1.5, 0.5))
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(Velocity::zero())
        .insert(RigidBody::Dynamic)
        .insert(Sleeping::disabled())
        .insert(LockedAxes::ROTATION_LOCKED)
        .insert(MassProperties {
            mass: 1.0,
            ..default()
        })
        .insert(GravityScale(0.0))
        .insert(Ccd { enabled: true })
        .insert(Transform::from_xyz(4.0, 24.0, 4.0))
        .insert(LogicalPlayer(0))
        .insert(PlayerInput {
            pitch: -TAU / 12.0,
            yaw: TAU * 5.0 / 8.0,
            ..default()
        })
        .insert(PlayerController {
            ..default()
        })
        .insert(inv);

    commands.spawn()
        .insert_bundle(PerspectiveCameraBundle::new_3d())
        .insert(RenderPlayer(0));
}

pub struct Buffers {
    // Place edge table and triangle table in uniform buffer
    // They are too large to have inline in the shader
    edge_table: Buffer,
    tri_table: Buffer,
    points: BufVec<Vec2>,
    heights: BufVec<f32>,
    voxels: Buffer,
    vertices: BufVec<Vec4>,
    normals: BufVec<Vec4>,
    uvs: BufVec<Vec2>,
    indices: BufVec<u32>,
    atomics: BufVec<u32>,
}

struct BindingGroups {
    simplex: BindGroup,
    voxels: BindGroup,
}

fn update_fps_text_sys(
    time: Res<Time>,
    diagnostics: Res<Diagnostics>,
    mut query: Query<&mut Text, With<TopRightText>>,
) {
    for mut text in query.iter_mut() {
        let mut fps = 0.0;
        if let Some(fps_diagnostic) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(fps_avg) = fps_diagnostic.average() {
                fps = fps_avg;
            }
        }

        let mut frame_time = time.delta_seconds_f64();
        if let Some(frame_time_diagnostic) = diagnostics.get(FrameTimeDiagnosticsPlugin::FRAME_TIME) {
            if let Some(frame_time_avg) = frame_time_diagnostic.average() {
                frame_time = frame_time_avg;
            }
        }

        let text = &mut text.sections[0].value;
        text.clear();
        write!(text, "{:.1} fps, {:.3} ms/frame", fps, frame_time * 1000.0).unwrap();
    }
}

fn update_hud_system(
    mut text_query: Query<&mut Text, With<PlayerHudText>>,
    player_query: Query<&Transform, With<PerspectiveProjection>>,
    mut item_query: Query<&mut Item>,
    inv_query: Query<(&Inventory, &PlayerInput)>,
) {
    for mut text in text_query.iter_mut() {
        let text = &mut text.sections[0].value;
        text.clear();
        for transform in player_query.iter() {
            let p = transform.translation;
            write!(text, "Position {{ {:.2}, {:.2}, {:.2} }}", p.x, p.y, p.z).unwrap();
        }
        for (inv, input) in inv_query.iter() {
            write!(text, "\n{:?}", input).unwrap();
            write!(text, "\n{:?}", inv).unwrap();
            for i in 0..inv.item_ents.0.len() {
                if let Some(item_ent) = inv.item_ents.0[i] {
                    if let Ok(item) = item_query.get_mut(item_ent) {
                        write!(text, "\n{:?}", *item).unwrap();
                    }
                }
            }
        }
    }
}
