use bevy::{
    core::cast_slice,
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
    window::{WindowDescriptor, WindowId},
    winit::WinitWindows,
};

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(WindowDescriptor {
            // vsync: false,
            ..Default::default()
        })
        .add_plugins(DefaultPlugins)
        .add_plugin(SimplexComputePlugin)
        .add_plugin(LogDiagnosticsPlugin::default())
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .add_startup_system(setup)
        .run();
}

fn setup(
    mut commands: Commands,
    windows: Res<WinitWindows>,
) {
    commands.spawn_bundle(PerspectiveCameraBundle {
        transform: Transform::from_xyz(80.0, 80.0, 300.0),
        ..Default::default()
    });

    let primary = windows.get_window(WindowId::primary()).unwrap();
    primary.set_title("QGame");
}

struct SimplexComputePlugin;

impl Plugin for SimplexComputePlugin {
    fn build(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .init_resource::<SimplexPipeline>()
            .add_system_to_stage(RenderStage::Prepare, prepare_buffer)
            .add_system_to_stage(RenderStage::Queue, queue_bind_group);

        let points: BufferVec<Vec2> = BufferVec::new(BufferUsages::STORAGE);
        let heights: BufferVec<f32> = BufferVec::new(BufferUsages::STORAGE | BufferUsages::MAP_READ);
        render_app.insert_resource(SimplexBuffer { points, heights });

        let mut render_graph = render_app.world.get_resource_mut::<RenderGraph>().unwrap();
        render_graph.add_node("simplex", DispatchSimplex {});
        render_graph
            .add_node_edge("simplex", MAIN_PASS_DEPENDENCIES)
            .unwrap();
    }
}

struct SimplexBuffer {
    points: BufferVec<Vec2>,
    heights: BufferVec<f32>,
}

struct SimplexBindGroup(BindGroup);

fn prepare_buffer(
    mut simplex_buffer: ResMut<SimplexBuffer>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    simplex_buffer.points.clear();
    simplex_buffer.points.reserve(8 * 8, render_device.as_ref());
    for x in 0..8 {
        for y in 0..8 {
            simplex_buffer.points.push(Vec2::new(x as f32, y as f32));
        }
    }
    simplex_buffer.points.write_buffer(render_device.as_ref(), render_queue.as_ref());
    simplex_buffer.heights.reserve(8 * 8, render_device.as_ref());
}

fn queue_bind_group(
    mut commands: Commands,
    pipeline: Res<SimplexPipeline>,
    simplex_buffer: Res<SimplexBuffer>,
    render_device: Res<RenderDevice>,
) {
    if !simplex_buffer.points.is_empty() {
        let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            label: Some("points binding"),
            layout: &pipeline.bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: simplex_buffer.points.buffer().unwrap().as_entire_binding(),
            }, BindGroupEntry {
                binding: 1,
                resource: simplex_buffer.heights.buffer().unwrap().as_entire_binding(),
            }],
        });
        commands.insert_resource(SimplexBindGroup(bind_group));
    }
}

struct SimplexPipeline {
    pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
}

impl FromWorld for SimplexPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.get_resource::<RenderDevice>().unwrap();

        let shader_source = include_str!("simplex.wgsl");
        let shader = render_device.create_shader_module(&ShaderModuleDescriptor {
            label: Some("simplex shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        let bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("simplex bind group layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }, BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let pipeline_layout = render_device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("simplex pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
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
            bind_group_layout,
        }
    }
}

struct DispatchSimplex;

impl render_graph::Node for DispatchSimplex {
    fn run(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let pipeline = world.get_resource::<SimplexPipeline>().unwrap();
        let simplex_buffer = world.get_resource::<SimplexBuffer>().unwrap();
        let height_buf_vec = &simplex_buffer.heights;
        let device = world.get_resource::<RenderDevice>().unwrap();

        if !simplex_buffer.points.is_empty() {
            let bind_group = &world.get_resource::<SimplexBindGroup>().unwrap().0;
            let mut pass = render_context
                .command_encoder
                .begin_compute_pass(&ComputePassDescriptor::default());
            pass.set_pipeline(&pipeline.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch(simplex_buffer.points.len() as u32, 1, 1);
            let height_buf = height_buf_vec.buffer().unwrap();
            let slice = &height_buf.slice(..);
            device.map_buffer(slice, MapMode::Read);
            let out_vec: Vec<f32> = cast_slice(&slice.get_mapped_range()).to_vec();
            height_buf.unmap()
        }

        // ??

        Ok(())
    }
}

