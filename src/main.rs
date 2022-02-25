use std::mem::size_of;

use bevy::{
    core::{cast_slice, Pod, Zeroable},
    core_pipeline::node::MAIN_PASS_DEPENDENCIES,
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
    render::{
        mesh::{Indices, VertexAttributeValues},
        render_graph::{self, RenderGraph},
        render_resource::*,
        RenderApp,
        renderer::{RenderContext, RenderDevice, RenderQueue},
        RenderStage,
    },
    window::WindowDescriptor,
};
use enumflags2::{bitflags, BitFlags};

use qgame::BufferVec;

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
        .add_plugin(LogDiagnosticsPlugin::default())
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .add_startup_system(setup)
        .run();
}

fn setup(
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
    points: BufferVec<Vec2>,
    heights: BufferVec<f32>,
    voxels: Buffer,
    vertices: BufferVec<Vec4>,
    normals: BufferVec<Vec4>,
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

fn make_compute_bind_group_layout_entry(binding: u32, read_only: bool) -> BindGroupLayoutEntry {
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

        let mut points: BufferVec<Vec2> = BufferVec::new(BufferUsages::STORAGE);
        points.reserve(CHUNK_SZ_2, render_device);
        let mut heights: BufferVec<f32> = BufferVec::new(BufferUsages::STORAGE | BufferUsages::MAP_READ);
        heights.reserve(CHUNK_SZ_2, render_device);
        let voxels = render_device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (CHUNK_SZ_3 * size_of::<Voxel>()) as BufferAddress,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut vertices: BufferVec<Vec4> = BufferVec::new(BufferUsages::STORAGE | BufferUsages::MAP_READ);
        vertices.reserve(CHUNK_SZ_3 * 4 * 6, render_device);
        let mut normals: BufferVec<Vec4> = BufferVec::new(BufferUsages::STORAGE | BufferUsages::MAP_READ);
        normals.reserve(CHUNK_SZ_3 * 4 * 6, render_device);
        let mut indices: BufferVec<u32> = BufferVec::new(BufferUsages::STORAGE | BufferUsages::MAP_READ);
        indices.reserve(CHUNK_SZ_3 * 6 * 6, render_device);
        let mut atomics: BufferVec<u32> = BufferVec::new(BufferUsages::STORAGE | BufferUsages::MAP_READ);
        atomics.reserve(2, render_device);

        let shader_source = include_str!("../assets/simplex.wgsl");
        let shader = render_device.create_shader_module(&ShaderModuleDescriptor {
            label: Some("simplex shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });
        let simplex_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("simplex bind group layout"),
                entries: &[
                    make_compute_bind_group_layout_entry(0, true),
                    make_compute_bind_group_layout_entry(1, false),
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
                    make_compute_bind_group_layout_entry(0, true),
                    make_compute_bind_group_layout_entry(1, false),
                    make_compute_bind_group_layout_entry(2, false),
                    make_compute_bind_group_layout_entry(3, false),
                    make_compute_bind_group_layout_entry(4, false),
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

        world.insert_resource(Buffers { points, heights, voxels, vertices, normals, indices, atomics });

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
    // use std::time::Instant;
    // let now = Instant::now();

    for (mesh, mut voxels) in query.iter_mut() {
        buffers.atomics.clear();
        buffers.atomics.push(0);
        buffers.atomics.push(0);

        let time = time.time_since_startup().as_secs_f32();
        buffers.points.clear();
        for x in 0..CHUNK_SZ {
            for y in 0..CHUNK_SZ {
                buffers.points.push(Vec2::new(x as f32 + time, y as f32 + time));
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
                    BindGroupEntry { binding: 0, resource: buffers.voxels.as_entire_binding() },
                    BindGroupEntry { binding: 1, resource: buffers.vertices.buffer().unwrap().as_entire_binding() },
                    BindGroupEntry { binding: 2, resource: buffers.normals.buffer().unwrap().as_entire_binding() },
                    BindGroupEntry { binding: 3, resource: buffers.indices.buffer().unwrap().as_entire_binding() },
                    BindGroupEntry { binding: 4, resource: buffers.atomics.buffer().unwrap().as_entire_binding() },
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
                        let mut density= 0.0;
                        if height > 1.0 {
                            density = 1.0;
                        } else if height > 0.0 {
                            density = height;
                        }
                        // voxels.0[x + y * CHUNK_SZ + z * CHUNK_SZ_2] = Voxel {
                        //     flags: if z == (noise01 * 4.0) as usize { 1 } else { 0},
                        //     density: 0.0
                        // };
                        voxels.0[x + y * CHUNK_SZ + z * CHUNK_SZ_2] = Voxel {
                            flags: 0,
                            density
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
            pass.dispatch((CHUNK_SZ / 8) as u32, (CHUNK_SZ / 8) as u32, (CHUNK_SZ / 8) as u32);
        }
        render_queue.submit(vec![command_encoder.finish()]);

        buffers.atomics.read_buffer(2, render_device.as_ref());
        let vertex_count = buffers.atomics.as_slice()[0] as usize;
        let index_count = buffers.atomics.as_slice()[1] as usize;
        buffers.vertices.read_buffer(vertex_count, render_device.as_ref());
        buffers.normals.read_buffer(vertex_count, render_device.as_ref());
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
            // for _ in 0..vertex_count / 4 {
            //     uvs.push([0.0, 0.0]);
            //     uvs.push([1.0, 0.0]);
            //     uvs.push([1.0, 1.0]);
            //     uvs.push([0.0, 1.0]);
            // }
            for _ in 0..vertex_count / 3 {
                uvs.push([0.0, 0.0]);
                uvs.push([1.0, 0.0]);
                uvs.push([0.0, 1.0]);
            }
        }
    }

    // let elapsed = now.elapsed();
    // println!("Elapsed: {:.2?}", elapsed);
}
