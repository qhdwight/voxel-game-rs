use bevy::{
    core::{cast_slice, Pod, Zeroable},
    core_pipeline::node::MAIN_PASS_DEPENDENCIES,
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
    render::{
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
            // vsync: false,
            ..Default::default()
        })
        .add_plugins(DefaultPlugins)
        .add_plugin(VoxelsPlugin)
        .add_plugin(LogDiagnosticsPlugin::default())
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .add_startup_system(setup)
        .add_system(marching_cubes)
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

fn marching_cubes(
    pbr_bundle: Res<PbrBundle>,
) {


    let mesh = meshes.get_mut(&pbr_bundle.mesh).unwrap();
    // mesh.set_attribute()
}

struct VoxelsPlugin;

impl Plugin for VoxelsPlugin {
    fn build(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .init_resource::<SimplexPipeline>()
            .add_system_to_stage(RenderStage::Prepare, prepare_buffer)
            .add_system_to_stage(RenderStage::Queue, queue_bind_group);

        let points: BufferVec<Vec2> = BufferVec::new(BufferUsages::STORAGE);
        let heights: BufferVec<f32> = BufferVec::new(BufferUsages::STORAGE | BufferUsages::MAP_READ);
        let voxels: BufferVec<Voxel> = BufferVec::new(BufferUsages::STORAGE);
        let vertices: BufferVec<Vec3> = BufferVec::new(BufferUsages::STORAGE | BufferUsages::MAP_READ);
        let indices: BufferVec<u32> = BufferVec::new(BufferUsages::STORAGE | BufferUsages::MAP_READ);
        render_app.insert_resource(Buffers { points, heights, voxels, vertices, indices });

        let mut render_graph = render_app.world.get_resource_mut::<RenderGraph>().unwrap();
        render_graph.add_node("voxels", DispatchVoxels);
        render_graph.add_node_edge("voxels", MAIN_PASS_DEPENDENCIES).unwrap();
    }
}

struct DispatchVoxels;

impl render_graph::Node for DispatchVoxels {
    fn run(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let device = world.get_resource::<RenderDevice>().unwrap();
        let buffers = world.get_resource::<Buffers>().unwrap();

        let simplex_pipeline = world.get_resource::<SimplexPipeline>().unwrap();
        let height_buf_vec = &buffers.heights;
        if !buffers.points.is_empty() {
            let simplex_bind_group = &world.get_resource::<VoxelsBindingGroup>().unwrap().simplex;
            let mut pass = render_context
                .command_encoder
                .begin_compute_pass(&ComputePassDescriptor::default());
            pass.set_pipeline(&simplex_pipeline.pipeline);
            pass.set_bind_group(0, simplex_bind_group.as_ref().unwrap(), &[]);
            pass.dispatch(buffers.points.len() as u32, 1, 1);

            let height_buf = height_buf_vec.buffer().unwrap();
            let slice = &height_buf.slice(..);
            device.map_buffer(slice, MapMode::Read);
            let out_vec: Vec<f32> = cast_slice(&slice.get_mapped_range()).to_vec();
            height_buf.unmap()
        }

        let voxels_pipeline = world.get_resource::<VoxelsPipeline>().unwrap();
        let voxels_bind_group = &world.get_resource::<VoxelsBindingGroup>().unwrap().simplex;
        let mut pass = render_context
            .command_encoder
            .begin_compute_pass(&ComputePassDescriptor::default());
        pass.set_pipeline(&voxels_pipeline.pipeline);
        pass.set_bind_group(0, voxels_bind_group.as_ref().unwrap(), &[]);
        pass.dispatch(buffers.points.len() as u32, 1, 1);

        Ok(())
    }
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

struct VoxelsBindingGroup {
    simplex: Option<BindGroup>,
    voxels: BindGroup,
}

fn prepare_buffer(
    mut voxels_buffer: ResMut<Buffers>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    voxels_buffer.points.clear();
    voxels_buffer.points.reserve(CHUNK_SZ * CHUNK_SZ, render_device.as_ref());
    for x in 0..CHUNK_SZ {
        for y in 0..CHUNK_SZ {
            voxels_buffer.points.push(Vec2::new(x as f32, y as f32));
        }
    }
    voxels_buffer.points.write_buffer(render_device.as_ref(), render_queue.as_ref());
    voxels_buffer.heights.reserve(CHUNK_SZ * CHUNK_SZ, render_device.as_ref());
}

fn queue_bind_group(
    mut commands: Commands,
    pipeline: Res<SimplexPipeline>,
    voxels_buffer: Res<Buffers>,
    render_device: Res<RenderDevice>,
) {
    let binding_groups = VoxelsBindingGroup {
        simplex: if voxels_buffer.points.is_empty() {
            None
        } else {
            Option::from(render_device.create_bind_group(&BindGroupDescriptor {
                label: Some("simplex binding"),
                layout: &pipeline.layout,
                entries: &[
                    BindGroupEntry { binding: 0, resource: voxels_buffer.points.buffer().unwrap().as_entire_binding() },
                    BindGroupEntry { binding: 1, resource: voxels_buffer.heights.buffer().unwrap().as_entire_binding() }
                ],
            }))
        },
        voxels: render_device.create_bind_group(&BindGroupDescriptor {
            label: Some("voxels binding"),
            layout: &pipeline.layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: voxels_buffer.voxels.buffer().unwrap().as_entire_binding() },
                BindGroupEntry { binding: 1, resource: voxels_buffer.vertices.buffer().unwrap().as_entire_binding() },
                BindGroupEntry { binding: 2, resource: voxels_buffer.indices.buffer().unwrap().as_entire_binding() }
            ],
        }),
    };
    commands.insert_resource(binding_groups);
}

struct SimplexPipeline {
    pipeline: ComputePipeline,
    layout: BindGroupLayout,
}

struct VoxelsPipeline {
    pipeline: ComputePipeline,
    layout: BindGroupLayout,
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

impl FromWorld for SimplexPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.get_resource::<RenderDevice>().unwrap();
        let shader_source = include_str!("simplex.wgsl");
        let shader = render_device.create_shader_module(&ShaderModuleDescriptor {
            label: Some("simplex shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });
        let layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("simplex bind group layout"),
                entries: &[
                    make_compute_bind_group_layout_entry(0, true),
                    make_compute_bind_group_layout_entry(1, false),
                ],
            });
        let pipeline_layout = render_device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("simplex pipeline layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = render_device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("simplex pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });
        SimplexPipeline {
            pipeline,
            layout,
        }
    }
}

impl FromWorld for VoxelsPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.get_resource::<RenderDevice>().unwrap();
        let shader_source = include_str!("voxels.wgsl");
        let shader = render_device.create_shader_module(&ShaderModuleDescriptor {
            label: Some("voxels shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });
        let layout =
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
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = render_device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("voxels pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });
        VoxelsPipeline {
            pipeline,
            layout,
        }
    }
}
