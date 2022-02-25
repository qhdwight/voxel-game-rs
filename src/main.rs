use std::mem::size_of;

use bevy::{
    core::{cast_slice, Pod, Zeroable},
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    prelude::*,
    render::{
        mesh::{Indices, VertexAttributeValues},
        render_resource::*,
        renderer::{RenderDevice, RenderQueue},
    },
    window::WindowDescriptor,
};
use enumflags2::{bitflags, BitFlags};

use qgame::{BufferVec, EDGE_TABLE, TRI_TABLE};

const CHUNK_SZ: usize = 32;
const CHUNK_SZ_2: usize = CHUNK_SZ * CHUNK_SZ;
const CHUNK_SZ_3: usize = CHUNK_SZ * CHUNK_SZ * CHUNK_SZ;

#[derive(Component)]
struct Voxels(Vec<Voxel>);

impl Default for Voxels {
    #[inline]
    fn default() -> Voxels {
        let mut vec = Vec::with_capacity(CHUNK_SZ_3);
        vec.resize(CHUNK_SZ_3, Voxel::default());
        Voxels { 0: vec }
    }
}

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(WindowDescriptor {
            title: String::from("QGame"),
            ..Default::default()
        })
        .add_plugins(DefaultPlugins)
        .add_plugin(VoxelsPlugin)
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .add_startup_system(setup)
        .add_system(update_text)
        .run();
}

#[derive(Component)]
struct TextChanges;

fn setup(
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
    mesh.set_indices(Some(Indices::U32(Vec::new())));
    mesh.set_attribute(Mesh::ATTRIBUTE_POSITION, VertexAttributeValues::Float32x3(Vec::new()));
    mesh.set_attribute(Mesh::ATTRIBUTE_NORMAL, VertexAttributeValues::Float32x3(Vec::new()));
    mesh.set_attribute(Mesh::ATTRIBUTE_UV_0, VertexAttributeValues::Float32x2(Vec::new()));
    let mesh = meshes.add(mesh);
    let material = materials.add(StandardMaterial {
        base_color: Color::RED,
        ..Default::default()
    });

    commands.spawn_bundle(PerspectiveCameraBundle {
        transform: Transform::from_xyz(-16.0, -16.0, 32.0).looking_at(Vec3::new(16.0, 16.0, 0.0), Vec3::Z),
        // transform: Transform::from_xyz(-6.0, 6.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..Default::default()
    });

    commands.spawn_bundle(PointLightBundle {
        point_light: PointLight {
            intensity: 2000.0,
            shadows_enabled: true,
            ..Default::default()
        },
        transform: Transform::from_xyz(38.0, -34.0, 40.0),
        ..Default::default()
    });

    commands.spawn_bundle(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 2000.0,
            shadows_enabled: true,
            ..Default::default()
        },
        transform: Transform::from_xyz(-38.0, 34.0, 40.0),
        ..Default::default()
    });

    commands.spawn().insert(Voxels::default()).insert_bundle(PbrBundle {
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

#[bitflags]
#[repr(u32)]
#[derive(Copy, Clone, PartialEq)]
enum VoxelProps {
    IsBlock,
}

struct VoxelFlags(BitFlags<VoxelProps>);

impl Default for VoxelFlags {
    #[inline]
    fn default() -> VoxelFlags { VoxelFlags::zeroed() }
}

unsafe impl Zeroable for VoxelFlags {
    #[inline]
    fn zeroed() -> Self { unsafe { std::mem::zeroed() } }
}

impl Copy for VoxelFlags {}

impl Clone for VoxelFlags { fn clone(&self) -> Self { *self } }

unsafe impl Pod for VoxelFlags {}

#[derive(Copy, Clone, Default, Pod, Zeroable)]
#[repr(C)]
struct Voxel {
    flags: u32,
    density: f32,
}

struct Buffers {
    edge_table: Buffer,
    tri_table: Buffer,
    points: BufferVec<Vec2>,
    heights: BufferVec<f32>,
    voxels: Buffer,
    vertices: BufferVec<Vec4>,
    normals: BufferVec<Vec4>,
    uvs: BufferVec<Vec2>,
    indices: BufferVec<u32>,
    atomics: BufferVec<u32>,
}

struct BindingGroups {
    simplex: BindGroup,
    voxels: BindGroup,
}

struct VoxelsPipeline {
    simplex_pipeline: ComputePipeline,
    simplex_layout: BindGroupLayout,
    voxels_pipeline: ComputePipeline,
    voxels_layout: BindGroupLayout,
}

struct VoxelsPlugin;

impl Plugin for VoxelsPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<VoxelsPipeline>()
            .add_system_set(SystemSet::new()
                .with_system(marching_cubes));
    }
}

fn make_compute_uniform_bind_group_layout_entry(binding: u32) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn make_compute_storage_bind_group_layout_entry(binding: u32, read_only: bool) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

impl FromWorld for VoxelsPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.get_resource::<RenderDevice>().unwrap();

        let edge_table = render_device.create_buffer_with_data(&BufferInitDescriptor { label: Some("edge table"), contents: cast_slice(EDGE_TABLE), usage: BufferUsages::UNIFORM });
        let tri_table = render_device.create_buffer_with_data(&BufferInitDescriptor { label: Some("tri table"), contents: cast_slice(TRI_TABLE), usage: BufferUsages::UNIFORM });
        let points: BufferVec<Vec2> = BufferVec::with_capacity(BufferUsages::STORAGE, CHUNK_SZ_2, render_device);
        let heights: BufferVec<f32> = BufferVec::with_capacity(BufferUsages::STORAGE | BufferUsages::MAP_READ, CHUNK_SZ_2, render_device);
        let voxels = render_device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (CHUNK_SZ_3 * size_of::<Voxel>()) as BufferAddress,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let vertices: BufferVec<Vec4> = BufferVec::with_capacity(BufferUsages::STORAGE | BufferUsages::MAP_READ, CHUNK_SZ_3 * 4 * 6, render_device);
        let uvs: BufferVec<Vec2> = BufferVec::with_capacity(BufferUsages::STORAGE | BufferUsages::MAP_READ, CHUNK_SZ_3 * 4 * 6, render_device);
        let normals: BufferVec<Vec4> = BufferVec::with_capacity(BufferUsages::STORAGE | BufferUsages::MAP_READ, CHUNK_SZ_3 * 4 * 6, render_device);
        let indices: BufferVec<u32> = BufferVec::with_capacity(BufferUsages::STORAGE | BufferUsages::MAP_READ, CHUNK_SZ_3 * 6 * 6, render_device);
        let atomics: BufferVec<u32> = BufferVec::with_capacity(BufferUsages::STORAGE | BufferUsages::MAP_READ, 2, render_device);

        let shader_source = include_str!("../assets/simplex.wgsl");
        let shader = render_device.create_shader_module(&ShaderModuleDescriptor {
            label: Some("simplex shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });
        let simplex_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("simplex bind group layout"),
                entries: &[
                    make_compute_storage_bind_group_layout_entry(0, true),
                    make_compute_storage_bind_group_layout_entry(1, false),
                ],
            });
        let pipeline_layout = render_device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("simplex pipeline layout"),
            bind_group_layouts: &[&simplex_layout],
            push_constant_ranges: &[],
        });
        let simplex_pipeline = render_device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("simplex pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });

        let shader_source = include_str!("../assets/voxels.wgsl");
        let shader = render_device.create_shader_module(&ShaderModuleDescriptor {
            label: Some("voxels shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });
        let voxels_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("voxels bind group layout"),
                entries: &[
                    make_compute_uniform_bind_group_layout_entry(0),
                    make_compute_uniform_bind_group_layout_entry(1),
                    make_compute_storage_bind_group_layout_entry(2, true),
                    make_compute_storage_bind_group_layout_entry(3, false),
                    make_compute_storage_bind_group_layout_entry(4, false),
                    make_compute_storage_bind_group_layout_entry(5, false),
                    make_compute_storage_bind_group_layout_entry(6, false),
                    make_compute_storage_bind_group_layout_entry(7, false),
                ],
            });
        let pipeline_layout = render_device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("voxels pipeline layout"),
            bind_group_layouts: &[&voxels_layout],
            push_constant_ranges: &[],
        });
        let voxels_pipeline = render_device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("voxels pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });

        world.insert_resource(Buffers { edge_table, tri_table, points, heights, voxels, vertices, normals, uvs, indices, atomics });

        VoxelsPipeline {
            simplex_pipeline,
            simplex_layout,
            voxels_pipeline,
            voxels_layout,
        }
    }
}

fn marching_cubes(
    mut query: Query<(&Handle<Mesh>, &mut Voxels)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut buffers: ResMut<Buffers>,
    time: Res<Time>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    pipeline: Res<VoxelsPipeline>,
) {
    // let now = std::time::Instant::now();

    for (mesh, mut voxels) in query.iter_mut() {
        buffers.atomics.clear();
        buffers.atomics.push(0);
        buffers.atomics.push(0);

        let time = time.time_since_startup().as_secs_f32();
        buffers.points.clear();
        for x in 0..CHUNK_SZ {
            for y in 0..CHUNK_SZ {
                buffers.points.push(0.1 * Vec2::new(x as f32 + time, y as f32 + time));
            }
        }

        let binding_groups = BindingGroups {
            simplex: render_device.create_bind_group(&BindGroupDescriptor {
                label: Some("simplex binding"),
                layout: &pipeline.simplex_layout,
                entries: &[
                    BindGroupEntry { binding: 0, resource: buffers.points.buffer().unwrap().as_entire_binding() },
                    BindGroupEntry { binding: 1, resource: buffers.heights.buffer().unwrap().as_entire_binding() }
                ],
            }),
            voxels: render_device.create_bind_group(&BindGroupDescriptor {
                label: Some("voxels binding"),
                layout: &pipeline.voxels_layout,
                entries: &[
                    BindGroupEntry { binding: 0, resource: buffers.edge_table.as_entire_binding() },
                    BindGroupEntry { binding: 1, resource: buffers.tri_table.as_entire_binding() },
                    BindGroupEntry { binding: 2, resource: buffers.voxels.as_entire_binding() },
                    BindGroupEntry { binding: 3, resource: buffers.atomics.buffer().unwrap().as_entire_binding() },
                    BindGroupEntry { binding: 4, resource: buffers.vertices.buffer().unwrap().as_entire_binding() },
                    BindGroupEntry { binding: 5, resource: buffers.normals.buffer().unwrap().as_entire_binding() },
                    BindGroupEntry { binding: 6, resource: buffers.indices.buffer().unwrap().as_entire_binding() },
                    BindGroupEntry { binding: 7, resource: buffers.uvs.buffer().unwrap().as_entire_binding() },
                ],
            }),
        };

        if !buffers.points.is_empty() {
            let mut command_encoder = render_device.create_command_encoder(&CommandEncoderDescriptor { label: Some("simplex command encoder") });
            {
                let mut pass = command_encoder.begin_compute_pass(&ComputePassDescriptor::default());
                pass.set_pipeline(&pipeline.simplex_pipeline);
                pass.set_bind_group(0, &binding_groups.simplex, &[]);
                pass.dispatch((CHUNK_SZ / 32) as u32, (CHUNK_SZ / 32) as u32, 1);
            }
            render_queue.submit(vec![command_encoder.finish()]);

            buffers.heights.read_buffer(CHUNK_SZ_2, render_device.as_ref());
            for z in 0..CHUNK_SZ {
                for y in 0..CHUNK_SZ {
                    for x in 0..CHUNK_SZ {
                        let noise01 = (buffers.heights.as_slice()[x + y * CHUNK_SZ] + 1.0) * 0.5;
                        let height = noise01 * 4.0 + 8.0 - (z as f32);
                        let mut density = 0.0;
                        if height > 1.0 {
                            density = 1.0;
                        } else if height > 0.0 {
                            density = height;
                        }
                        // voxels.0[x + y * CHUNK_SZ + z * CHUNK_SZ_2] = Voxel {
                        //     flags: if z == (noise01 * 4.0) as usize { 1 } else { 0 },
                        //     density: 0.0,
                        // };
                        voxels.0[x + y * CHUNK_SZ + z * CHUNK_SZ_2] = Voxel {
                            flags: 0,
                            density,
                        };
                    }
                }
            }
            // buffers.points.clear();
        }

        buffers.points.write_buffer(render_device.as_ref(), render_queue.as_ref());
        let range = 0..size_of::<Voxel>() * voxels.0.len();
        let bytes: &[u8] = cast_slice(&voxels.0);
        render_queue.write_buffer(&buffers.voxels, 0, &bytes[range]);
        buffers.atomics.write_buffer(render_device.as_ref(), render_queue.as_ref());

        let mut command_encoder = render_device.create_command_encoder(&CommandEncoderDescriptor { label: Some("voxel command encoder") });
        {
            let mut pass = command_encoder.begin_compute_pass(&ComputePassDescriptor::default());
            pass.set_pipeline(&pipeline.voxels_pipeline);
            pass.set_bind_group(0, &binding_groups.voxels, &[]);
            let dispatch_size = (CHUNK_SZ / 8) as u32;
            pass.dispatch(dispatch_size, dispatch_size, dispatch_size);
        }
        render_queue.submit(vec![command_encoder.finish()]);

        buffers.atomics.read_buffer(2, render_device.as_ref());
        let vertex_count = buffers.atomics.as_slice()[0] as usize;
        let index_count = buffers.atomics.as_slice()[1] as usize;
        buffers.vertices.read_buffer(vertex_count, render_device.as_ref());
        buffers.normals.read_buffer(vertex_count, render_device.as_ref());
        buffers.uvs.read_buffer(vertex_count, render_device.as_ref());
        buffers.indices.read_buffer(index_count, render_device.as_ref());

        let mesh = meshes.get_mut(mesh).unwrap();

        if let Some(Indices::U32(indices)) = mesh.indices_mut() {
            indices.resize(index_count, 0);
            indices.copy_from_slice(buffers.indices.as_slice());
        }
        if let Some(VertexAttributeValues::Float32x3(vertices)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION) {
            vertices.clear();
            vertices.reserve(vertex_count);
            for v in buffers.vertices.iter() {
                vertices.push([v[0], v[1], v[2]]);
            }
        }
        if let Some(VertexAttributeValues::Float32x3(normals)) = mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL) {
            normals.clear();
            normals.reserve(vertex_count);
            for v in buffers.normals.iter() {
                normals.push([v[0], v[1], v[2]]);
            }
        }
        if let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0) {
            uvs.clear();
            uvs.reserve(vertex_count);
            for v in buffers.uvs.iter() {
                uvs.push([v[0], v[1]]);
            }
        }
    }

    // println!("Elapsed: {:.2?}", now.elapsed());
}

fn update_text(
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
        if let Some(frame_time_diagnostic) = diagnostics.get(FrameTimeDiagnosticsPlugin::FRAME_TIME)
        {
            if let Some(frame_time_avg) = frame_time_diagnostic.average() {
                frame_time = frame_time_avg;
            }
        }

        text.sections[0].value = format!("{:.1} fps, {:.3} ms/frame", fps, frame_time * 1000.0);
    }
}
