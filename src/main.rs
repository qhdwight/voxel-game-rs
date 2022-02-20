use bevy::{
    core_pipeline::node::MAIN_PASS_DEPENDENCIES,
    prelude::*,
    render::{
        render_graph::{self, RenderGraph},
        render_resource::*,
        renderer::{RenderContext, RenderDevice},
        RenderApp, RenderStage,
    },
    window::WindowDescriptor,
};
use bevy::render::renderer::RenderQueue;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(WindowDescriptor {
            // vsync: false,
            ..Default::default()
        })
        .add_plugins(DefaultPlugins)
        .add_plugin(SimplexComputePlugin)
        .add_startup_system(setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn_bundle(PerspectiveCameraBundle::new_3d());
}

pub struct SimplexComputePlugin;

impl Plugin for SimplexComputePlugin {
    fn build(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .init_resource::<SimplexPipeline>()
            .add_system_to_stage(RenderStage::Prepare, prepare_buffer)
            .add_system_to_stage(RenderStage::Queue, queue_bind_group);

        let buffer: BufferVec<Vec3> = BufferVec::new(BufferUsages::STORAGE | BufferUsages::COPY_DST);
        render_app.insert_resource(SimplexBuffer(buffer));

        let mut render_graph = render_app.world.get_resource_mut::<RenderGraph>().unwrap();
        render_graph.add_node("simplex", DispatchSimplex {});
        render_graph
            .add_node_edge("simplex", MAIN_PASS_DEPENDENCIES)
            .unwrap();
    }
}

struct SimplexBuffer(BufferVec<Vec3>);

struct SimplexBindGroup(BindGroup);

fn prepare_buffer(
    mut simplex_buffer: ResMut<SimplexBuffer>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    simplex_buffer.0.write_buffer(render_device.as_ref(), render_queue.as_ref());
}

fn queue_bind_group(
    mut commands: Commands,
    pipeline: Res<SimplexPipeline>,
    simplex_buffer: Res<SimplexBuffer>,
    render_device: Res<RenderDevice>,
) {
    match simplex_buffer.0.buffer() {
        Some(buffer) => {
            let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
                label: Some("points binding"),
                layout: &pipeline.bind_group_layout,
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding { buffer: &buffer, offset: 0, size: None }),
                }],
            });
            commands.insert_resource(SimplexBindGroup(bind_group));
        }
        None => {}
    }
}

pub struct SimplexPipeline {
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
                label: None,
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
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
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = render_device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: None,
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

struct DispatchSimplex {}

impl render_graph::Node for DispatchSimplex {
    fn run(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let pipeline = world.get_resource::<SimplexPipeline>().unwrap();
        let buffer = &world.get_resource::<SimplexBuffer>().unwrap().0;

        if !buffer.is_empty() {
            let bind_group = &world.get_resource::<SimplexBindGroup>().unwrap().0;
            let mut pass = render_context
                .command_encoder
                .begin_compute_pass(&ComputePassDescriptor::default());
            pass.set_pipeline(&pipeline.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch(buffer.len() as u32, 1, 1);
        }

        Ok(())
    }
}

