use std::iter::zip;

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

const CHUNK_SZ: usize = 32;

#[derive(Component)]
struct Voxels(Vec<Voxel>);

impl Default for Voxels {
    #[inline]
    fn default() -> Voxels {
        Voxels { 0: Vec::with_capacity(CHUNK_SZ * CHUNK_SZ * CHUNK_SZ) }
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
    let mesh = meshes.add(Mesh::new(PrimitiveTopology::TriangleList));
    let material = materials.add(StandardMaterial {
        base_color: Color::DARK_GREEN,
        ..Default::default()
    });

    commands.spawn_bundle(PerspectiveCameraBundle {
        // transform: Transform::from_xyz(-48.0, 48.0, 48.0).looking_at(Vec3::ZERO, Vec3::Y),
        transform: Transform::from_xyz(-6.0, 6.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..Default::default()
    });

    commands.spawn_bundle(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: true,
            ..Default::default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..Default::default()
    });

    commands.spawn().insert(Voxels::default()).insert_bundle(PbrBundle {
        mesh: mesh.clone(),
        material: material.clone(),
        ..Default::default()
    });
}

#[derive(Copy, Clone, Pod, Zeroable, Default)]
#[repr(C)]
struct Voxel {
    density: f32,
}

struct Buffers {
    points: BufferVec<Vec2>,
    heights: BufferVec<f32>,
    voxels: BufferVec<Voxel>,
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
        points.reserve(CHUNK_SZ * CHUNK_SZ, render_device);
        points.clear();
        for x in 0..CHUNK_SZ {
            for y in 0..CHUNK_SZ {
                points.push(Vec2::new(x as f32, y as f32));
            }
        }
        let CHUNK_SZ_3 = CHUNK_SZ * CHUNK_SZ * CHUNK_SZ;
        let mut heights: BufferVec<f32> = BufferVec::new(BufferUsages::STORAGE | BufferUsages::MAP_READ);
        heights.reserve(CHUNK_SZ * CHUNK_SZ, render_device);
        let mut voxels: BufferVec<Voxel> = BufferVec::new(BufferUsages::STORAGE);
        voxels.reserve(CHUNK_SZ_3, render_device);
        let mut vertices: BufferVec<Vec4> = BufferVec::new(BufferUsages::STORAGE | BufferUsages::MAP_READ);
        vertices.reserve(CHUNK_SZ_3 * 8, render_device);
        let mut normals: BufferVec<Vec4> = BufferVec::new(BufferUsages::STORAGE | BufferUsages::MAP_READ);
        normals.reserve(CHUNK_SZ_3 * 8, render_device);
        let mut indices: BufferVec<u32> = BufferVec::new(BufferUsages::STORAGE | BufferUsages::MAP_READ);
        indices.reserve(CHUNK_SZ_3 * 6 * 6, render_device);
        let mut atomics: BufferVec<u32> = BufferVec::new(BufferUsages::STORAGE | BufferUsages::MAP_READ);
        atomics.reserve(2, render_device);

        let shader_source = include_str!("simplex.wgsl");
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

        let shader_source = include_str!("voxels.wgsl");
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

fn read_buffer<T: Pod>(buf_vec: &BufferVec<T>, count: usize, device: &RenderDevice) -> Vec<T> {
    let buf = buf_vec.buffer().unwrap();
    let slice = &buf.slice(..);
    device.map_buffer(slice, MapMode::Read);
    let count_bytes = std::mem::size_of::<T>() * count;
    let vec = cast_slice(&slice.get_mapped_range()[..count_bytes]).to_vec();
    buf.unmap();
    vec
}

fn marching_cubes(
    mut query: Query<(&Handle<Mesh>, &Voxels)>,
    mut meshes: ResMut<Assets<Mesh>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    pipeline: Res<VoxelsPipeline>,
    mut buffers: ResMut<Buffers>,
) {
    for (mut mesh, voxels) in query.iter_mut() {
        buffers.points.write_buffer(render_device.as_ref(), render_queue.as_ref());
        buffers.voxels.write_buffer(render_device.as_ref(), render_queue.as_ref());
        buffers.atomics.clear();
        buffers.atomics.push(0);
        buffers.atomics.push(0);
        buffers.atomics.write_buffer(render_device.as_ref(), render_queue.as_ref());

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
                    BindGroupEntry { binding: 0, resource: buffers.voxels.buffer().unwrap().as_entire_binding() },
                    BindGroupEntry { binding: 1, resource: buffers.vertices.buffer().unwrap().as_entire_binding() },
                    BindGroupEntry { binding: 2, resource: buffers.normals.buffer().unwrap().as_entire_binding() },
                    BindGroupEntry { binding: 3, resource: buffers.indices.buffer().unwrap().as_entire_binding() },
                    BindGroupEntry { binding: 4, resource: buffers.atomics.buffer().unwrap().as_entire_binding() },
                ],
            }),
        };

        let mut heights: Option<Vec<f32>> = None;
        if !buffers.points.is_empty() {
            let mut command_encoder = render_device.create_command_encoder(&CommandEncoderDescriptor { label: Some("simplex command encoder") });
            {
                let mut pass = command_encoder.begin_compute_pass(&ComputePassDescriptor::default());
                pass.set_pipeline(&pipeline.simplex_pipeline);
                pass.set_bind_group(0, &binding_groups.simplex, &[]);
                pass.dispatch(buffers.points.len() as u32, 1, 1);
            }
            render_queue.submit(vec![command_encoder.finish()]);
            heights = Some(read_buffer(&buffers.heights, buffers.heights.len(), render_device.as_ref()));
        }
        if let Some(heights) = heights {
            buffers.voxels.clear();
            buffers.voxels.push(Voxel { density: 1.0 });
            buffers.voxels.push(Voxel { density: 1.0 });
            // for x in 0..CHUNK_SZ {
            //     for y in 0..CHUNK_SZ {
            //         buffers.voxels.push(Voxel { density: 1.0 });
            //     }
            // }
        }

        let mut command_encoder = render_device.create_command_encoder(&CommandEncoderDescriptor { label: Some("voxel command encoder") });
        {
            let mut pass = command_encoder.begin_compute_pass(&ComputePassDescriptor::default());
            pass.set_pipeline(&pipeline.voxels_pipeline);
            pass.set_bind_group(0, &binding_groups.voxels, &[]);
            pass.dispatch(CHUNK_SZ as u32, CHUNK_SZ as u32, CHUNK_SZ as u32);
        }
        render_queue.submit(vec![command_encoder.finish()]);

        let atomics = read_buffer(&buffers.atomics, 2, render_device.as_ref());
        let vertices = read_buffer(&buffers.vertices, atomics[0] as usize, render_device.as_ref());
        let normals = read_buffer(&buffers.normals, atomics[0] as usize, render_device.as_ref());
        let indices = read_buffer(&buffers.indices, atomics[1] as usize, render_device.as_ref());

        let vertices: Vec<[f32; 3]> = vertices.iter().map(|v| [v[0], v[1], v[2]]).collect();
        let normals: Vec<[f32; 3]> = normals.iter().map(|n| [n[0], n[1], n[2]]).collect();

        let mut uvs = Vec::<[f32; 2]>::with_capacity(vertices.len());
        for _ in 0..vertices.len() / 4 {
            uvs.push([0.0, 0.0]);
            uvs.push([1.0, 0.0]);
            uvs.push([1.0, 1.0]);
            uvs.push([0.0, 1.0]);
        }

        let mut mesh = meshes.get_mut(mesh).unwrap();
        mesh.set_indices(Some(Indices::U32(indices)));
        mesh.set_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
        mesh.set_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.set_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    }
}
