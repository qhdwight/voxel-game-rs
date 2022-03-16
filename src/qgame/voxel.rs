use std::iter::once;
use std::mem::size_of;

use bevy::{
    core::{cast_slice, Pod, Zeroable},
    prelude::*,
    render::{
        mesh::{Indices, VertexAttributeValues},
        render_resource::*,
        renderer::{RenderDevice, RenderQueue},
    },
    utils::HashMap,
};

use crate::*;

// use flagset::{flags, FlagSet};

const CHUNK_SZ: usize = 32;
const CHUNK_SZ_2: usize = CHUNK_SZ * CHUNK_SZ;
const CHUNK_SZ_3: usize = CHUNK_SZ * CHUNK_SZ * CHUNK_SZ;

#[derive(Component)]
pub struct Chunk {
    pub position: IVec3,
    pub voxels: Vec<Voxel>,
}

#[derive(Component)]
pub struct Map {
    pub chunks: HashMap<IVec3, Entity>,
}

impl Default for Map {
    fn default() -> Self {
        Self {
            chunks: HashMap::default(),
        }
    }
}

impl Chunk {
    pub fn new(position: IVec3) -> Self {
        let mut voxels = Vec::with_capacity(CHUNK_SZ_3);
        voxels.resize(CHUNK_SZ_3, Voxel::default());
        Self { position, voxels }
    }
}

// flags! {
//     #[repr(u32)]
//     pub enum VoxelFlags: u32 {
//         IsBlock,
//     }
// }

#[derive(Copy, Clone, Default, Pod, Zeroable)]
#[repr(C)]
pub struct Voxel {
    flags: u32,
    density: f32,
}

pub struct VoxelsPipeline {
    simplex_pipeline: ComputePipeline,
    simplex_layout: BindGroupLayout,
    voxels_pipeline: ComputePipeline,
    voxels_layout: BindGroupLayout,
}

pub struct VoxelsPlugin;

impl Plugin for VoxelsPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<VoxelsPipeline>()
            .add_system_to_stage(CoreStage::PreUpdate, voxel_polygonize_system);
    }
}

impl FromWorld for VoxelsPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.get_resource::<RenderDevice>().unwrap();
        let asset_server = world.get_resource::<AssetServer>().unwrap();

        let edge_table = render_device.create_buffer_with_data(&BufferInitDescriptor { label: Some("edge table buffer"), contents: cast_slice(EDGE_TABLE), usage: BufferUsages::UNIFORM });
        let tri_table = render_device.create_buffer_with_data(&BufferInitDescriptor { label: Some("tri table buffer"), contents: cast_slice(TRI_TABLE), usage: BufferUsages::UNIFORM });
        let points: BufVec<Vec2> = BufVec::with_capacity(BufferUsages::STORAGE, CHUNK_SZ_2, render_device);
        let heights: BufVec<f32> = BufVec::with_capacity(BufferUsages::STORAGE | BufferUsages::MAP_READ, CHUNK_SZ_2, render_device);
        let voxels = render_device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("voxels buffer"),
            size: (CHUNK_SZ_3 * size_of::<Voxel>()) as BufferAddress,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let vertices: BufVec<Vec4> = BufVec::with_capacity(BufferUsages::STORAGE | BufferUsages::MAP_READ, CHUNK_SZ_3 * 4 * 6, render_device);
        let uvs: BufVec<Vec2> = BufVec::with_capacity(BufferUsages::STORAGE | BufferUsages::MAP_READ, CHUNK_SZ_3 * 4 * 6, render_device);
        let normals: BufVec<Vec4> = BufVec::with_capacity(BufferUsages::STORAGE | BufferUsages::MAP_READ, CHUNK_SZ_3 * 4 * 6, render_device);
        let indices: BufVec<u32> = BufVec::with_capacity(BufferUsages::STORAGE | BufferUsages::MAP_READ, CHUNK_SZ_3 * 6 * 6, render_device);
        let atomics: BufVec<u32> = BufVec::with_capacity(BufferUsages::STORAGE | BufferUsages::MAP_READ, 2, render_device);

        // let simplex_shader = asset_server.load("shaders/simplex.wgsl");
        let shader_source = include_str!("../../assets/shaders/simplex.wgsl");
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

        // let voxel_shader = asset_server.load("shaders/voxels.wgsl");
        let shader_source = include_str!("../../assets/shaders/voxels.wgsl");
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

pub fn sync_added_chunks_system(
    added_chunk_query: Query<(Entity, &Chunk), Added<Chunk>>,
    mut map_query: Query<&mut Map>,
) {
    for (chunk_entity, chunk) in added_chunk_query.iter() {
        for mut map in map_query.iter_mut() {
            map.chunks.insert(chunk.position, chunk_entity);
        }
    }
}

pub fn voxel_polygonize_system(
    mut commands: Commands,
    mut query: Query<(Entity, &Handle<Mesh>, Option<&mut ColliderShapeComponent>, &mut Chunk)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut buffers: ResMut<Buffers>,
    time: Res<Time>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    pipeline: Res<VoxelsPipeline>,
) {
    // let now = std::time::Instant::now();

    for (entity, mesh, mut collider, mut chunk) in query.iter_mut() {
        buffers.atomics.clear();
        buffers.atomics.push(0);
        buffers.atomics.push(0);

        let time = time.time_since_startup().as_secs_f32();
        buffers.points.clear();
        for x in 0..CHUNK_SZ {
            for y in 0..CHUNK_SZ {
                buffers.points.push(0.05 * Vec2::new(x as f32 + time, y as f32 + time));
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
            buffers.points.write_buffer(render_device.as_ref(), render_queue.as_ref());

            let mut command_encoder = render_device.create_command_encoder(&CommandEncoderDescriptor { label: Some("simplex command encoder") });
            {
                let mut pass = command_encoder.begin_compute_pass(&ComputePassDescriptor::default());
                pass.set_pipeline(&pipeline.simplex_pipeline);
                pass.set_bind_group(0, &binding_groups.simplex, &[]);
                pass.dispatch((CHUNK_SZ / 32) as u32, (CHUNK_SZ / 32) as u32, 1);
            }
            render_queue.submit(once(command_encoder.finish()));

            buffers.heights.read_buffer(CHUNK_SZ_2, render_device.as_ref());
            for z in 0..CHUNK_SZ {
                for y in 0..CHUNK_SZ {
                    for x in 0..CHUNK_SZ {
                        let noise01 = (buffers.heights.as_slice()[x + z * CHUNK_SZ] + 1.0) * 0.5;
                        let height = noise01 * 4.0 + 8.0 - (y as f32);
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
                        chunk.voxels[x + y * CHUNK_SZ + z * CHUNK_SZ_2] = Voxel {
                            flags: 0,
                            density,
                        };
                    }
                }
            }
        }

        let range = 0..size_of::<Voxel>() * chunk.voxels.len();
        let bytes: &[u8] = cast_slice(&chunk.voxels);
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
        render_queue.submit(once(command_encoder.finish()));

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
                uvs.push((*v).into());
            }
        }

        // // TODO:perf inefficient
        // if let Some(Indices::U32(indices)) = mesh.indices() {
        //     if let Some(VertexAttributeValues::Float32x3(vertices)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        //         let vertices = vertices.iter().map(|v| (*v).into()).collect();
        //         let indices = indices.chunks(3).map(|t| t.try_into().unwrap()).collect();
        //         let shape: ColliderShapeComponent = ColliderShape::trimesh(vertices, indices).into();
        //
        //         if collider.is_none() {
        //             commands.entity(entity)
        //                 .insert_bundle(ColliderBundle {
        //                     shape,
        //                     collider_type: ColliderType::Solid.into(),
        //                     position: Vec3::new(0.0, 0.0, 0.0).into(),
        //                     material: ColliderMaterial { friction: 0.7, restitution: 0.3, ..Default::default() }.into(),
        //                     mass_properties: ColliderMassProps::Density(2.0).into(),
        //                     ..Default::default()
        //                 });
        //         } else {
        //             commands.entity(entity).insert(shape);
        //         }
        //     }
        // }
    }

    // println!("Elapsed: {:.2?}", now.elapsed());
}