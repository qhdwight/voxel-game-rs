extern crate core;

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
struct Voxels([Voxel; CHUNK_SZ * CHUNK_SZ * CHUNK_SZ]);

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(WindowDescriptor {
            title: String::from("QGame"),
            // vsync: falsew,
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

    commands.spawn_bundle(PbrBundle {
        mesh: mesh.clone(),
        material: material.clone(),
        ..Default::default()
    });

    commands.spawn_bundle(PerspectiveCameraBundle {
        transform: Transform::from_xyz(80.0, 80.0, 300.0),
        ..Default::default()
    });

    // commands.spawn(Voxels { 0 = Default::default() });
}

#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct Voxel {
    density: f32,
}

struct Buffers {
    points: BufferVec<Vec2>,
    heights: BufferVec<f32>,
    voxels: BufferVec<Voxel>,
    vertices: BufferVec<Vec3>,
    indices: BufferVec<u32>,
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
        points.reserve(1, render_device);
        let mut heights: BufferVec<f32> = BufferVec::new(BufferUsages::STORAGE | BufferUsages::MAP_READ);
        heights.reserve(1, render_device);
        let mut voxels: BufferVec<Voxel> = BufferVec::new(BufferUsages::STORAGE);
        voxels.reserve(CHUNK_SZ * CHUNK_SZ * CHUNK_SZ, render_device);
        let mut vertices: BufferVec<Vec3> = BufferVec::new(BufferUsages::STORAGE | BufferUsages::MAP_READ);
        vertices.reserve(1, render_device);
        let mut indices: BufferVec<u32> = BufferVec::new(BufferUsages::STORAGE | BufferUsages::MAP_READ);
        indices.reserve(1, render_device);

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

        world.insert_resource(Buffers { points, heights, voxels, vertices, indices });

        VoxelsPipeline {
            simplex_pipeline,
            simplex_layout,
            voxels_pipeline,
            voxels_layout,
        }
    }
}

fn marching_cubes(
    mut query: Query<(&Handle<Mesh>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    pipeline: Res<VoxelsPipeline>,
    mut buffers: ResMut<Buffers>,
) {
    // for (mut mesh) in query.iter_mut() {
    //     let mesh = meshes.get(mesh).unwrap();
    //     mesh.set_indices()
    // }

    buffers.points.clear();
    buffers.points.reserve(CHUNK_SZ * CHUNK_SZ, render_device.as_ref());
    for x in 0..CHUNK_SZ {
        for y in 0..CHUNK_SZ {
            buffers.points.push(Vec2::new(x as f32, y as f32));
        }
    }
    buffers.points.write_buffer(render_device.as_ref(), render_queue.as_ref());
    buffers.heights.reserve(CHUNK_SZ * CHUNK_SZ, render_device.as_ref());

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
                BindGroupEntry { binding: 2, resource: buffers.indices.buffer().unwrap().as_entire_binding() }
            ],
        }),
    };

    let height_buf_vec = &buffers.heights;
    if !buffers.points.is_empty() {
        let mut command_encoder = render_device.create_command_encoder(&CommandEncoderDescriptor { label: Some("command encoder") });
        {
            let mut pass = command_encoder.begin_compute_pass(&ComputePassDescriptor::default());
            pass.set_pipeline(&pipeline.simplex_pipeline);
            pass.set_bind_group(0, &binding_groups.simplex, &[]);
            pass.dispatch(buffers.points.len() as u32, 1, 1);
        }
        let height_buf = height_buf_vec.buffer().unwrap();
        let slice = &height_buf.slice(..);
        render_device.map_buffer(slice, MapMode::Read);
        let out_vec: Vec<f32> = cast_slice(&slice.get_mapped_range()).to_vec();
        height_buf.unmap();

        let commands = command_encoder.finish();
        render_queue.submit(vec![commands]);
    }

    let mut command_encoder = render_device.create_command_encoder(&CommandEncoderDescriptor { label: Some("command encoder") });
    {
        let mut pass = command_encoder.begin_compute_pass(&ComputePassDescriptor::default());
        pass.set_pipeline(&pipeline.voxels_pipeline);
        pass.set_bind_group(0, &binding_groups.voxels, &[]);
        pass.dispatch(buffers.points.len() as u32, 1, 1);
    }
    let commands = command_encoder.finish();
    render_queue.submit(vec![commands]);
}
