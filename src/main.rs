use std::f32::consts::TAU;

use bevy::{
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    prelude::*,
    render::{
        mesh::{Indices, VertexAttributeValues},
        render_resource::*,
    },
    window::WindowDescriptor,
};
use bevy_rapier3d::{
    na::Point3,
    prelude::*,
};

use qgame::*;

mod qgame;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(WindowDescriptor {
            title: String::from("QGame"),
            ..Default::default()
        })
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 0.25,
        })
        .add_plugins(DefaultPlugins)
        .add_plugin(RapierPhysicsPlugin::<NoUserData>::default())
        .add_plugin(RapierRenderPlugin)
        .add_plugin(VoxelsPlugin)
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .add_asset::<Config>()
        .init_asset_loader::<ConfigAssetLoader>()
        .add_startup_system(setup_system)
        .add_system_to_stage(CoreStage::PreUpdate, player_input_system)
        .add_system(cursor_grab_system)
        .add_system(update_text_system)
        .add_system(manage_inventory_system)
        .add_system_set(SystemSet::new()
            .with_system(player_look_system)
            .with_system(player_controller_system))
        .add_system_to_stage(CoreStage::PostUpdate, sync_camera_system)
        .run();
}

#[derive(Component)]
struct TextChanges;

fn setup_system(
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // println!("{}", toml::to_string(&Config::default()).unwrap());

    let config: Handle<Config> = asset_server.load("default_config.toml");
    commands.insert_resource(config);

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
    mesh.set_indices(Some(Indices::U32(Vec::with_capacity(4096))));
    mesh.set_attribute(Mesh::ATTRIBUTE_POSITION, VertexAttributeValues::Float32x3(Vec::with_capacity(4096)));
    mesh.set_attribute(Mesh::ATTRIBUTE_NORMAL, VertexAttributeValues::Float32x3(Vec::with_capacity(4096)));
    mesh.set_attribute(Mesh::ATTRIBUTE_UV_0, VertexAttributeValues::Float32x2(Vec::with_capacity(4096)));
    let mesh = meshes.add(mesh);
    let material = materials.add(StandardMaterial {
        base_color: Color::RED,
        ..Default::default()
    });

    commands.spawn_bundle(PerspectiveCameraBundle::new_3d());

    commands.spawn()
        .insert_bundle(ColliderBundle {
            shape: ColliderShape::capsule(Point3::new(0.0, 0.0, 0.5), Point3::new(0.0, 0.0, 1.5), 0.5).into(),
            collider_type: ColliderType::Solid.into(),
            material: ColliderMaterial { friction: 0.7, restitution: 0.3, ..Default::default() }.into(),
            mass_properties: ColliderMassProps::Density(2.0).into(),
            ..Default::default()
        })
        .insert(ColliderDebugRender::with_id(0))
        .insert_bundle(RigidBodyBundle {
            body_type: RigidBodyType::KinematicPositionBased.into(),
            position: Vec3::new(-16.0, 32.0, -16.0).into(),
            ..Default::default()
        })
        .insert(PlayerInput {
            pitch: -TAU / 12.0,
            yaw: TAU * 5.0 / 8.0,
            ..Default::default()
        })
        .insert(PlayerController {
            ..Default::default()
        });

    // commands.spawn_bundle(PointLightBundle {
    //     point_light: PointLight {
    //         intensity: 2000.0,
    //         shadows_enabled: true,
    //         ..Default::default()
    //     },
    //     transform: Transform::from_xyz(38.0, -34.0, 40.0),
    //     ..Default::default()
    // });

    commands.spawn_bundle(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 2000.0,
            shadows_enabled: true,
            ..Default::default()
        },
        transform: Transform::from_xyz(-38.0, 40.0, 34.0),
        ..Default::default()
    });

    {
        let mesh = meshes.add(Mesh::from(bevy::prelude::shape::Cube { size: 1.0 }));
        let material = materials.add(StandardMaterial {
            base_color: Color::PINK,
            ..Default::default()
        });
        commands.spawn_bundle(PbrBundle {
            mesh: mesh.clone(),
            material: material.clone(),
            transform: Transform::from_xyz(-18.0, 32.0, -18.0),
            ..Default::default()
        });
    }

    commands.spawn().insert(Map::default());

    commands.spawn()
        .insert(Chunk::new(IVec3::ZERO))
        // .insert_bundle(ColliderBundle {
        //     shape: ColliderShape::trimesh(Vec::new(), Vec::new()).into(),
        //     collider_type: ColliderType::Solid.into(),
        //     position: Vec3::new(0.0, 0.0, 0.0).into(),
        //     material: ColliderMaterial { friction: 0.7, restitution: 0.3, ..Default::default() }.into(),
        //     mass_properties: ColliderMassProps::Density(2.0).into(),
        //     ..Default::default()
        // })
        .insert(ColliderDebugRender::with_id(1))
        .insert_bundle(PbrBundle {
            mesh: mesh.clone(),
            material: material.clone(),
            ..Default::default()
        });

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
                    ..Default::default()
                },
                ..Default::default()
            },
            text: Text {
                sections: vec![
                    TextSection {
                        value: "".to_string(),
                        style: TextStyle {
                            font: font.clone(),
                            font_size: 16.0,
                            color: Color::WHITE,
                        },
                    },
                ],
                alignment: Default::default(),
            },
            ..Default::default()
        })
        .insert(TextChanges);
}

pub struct Buffers {
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

fn update_text_system(
    time: Res<Time>,
    diagnostics: Res<Diagnostics>,
    mut query: Query<&mut Text, With<TextChanges>>,
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

        text.sections[0].value = format!("{:.1} fps, {:.3} ms/frame", fps, frame_time * 1000.0);
    }
}
