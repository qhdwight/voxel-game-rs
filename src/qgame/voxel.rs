use std::{
    iter::once,
    mem::size_of,
};

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
use wgpu::MaintainBase::Wait;

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

#[derive(Resource)]
pub struct VoxelsPipeline {
    simplex_pipeline: ComputePipeline,
    voxels_pipeline: ComputePipeline,
}

#[derive(Resource)]
pub struct VoxelBuffers {
    // Place edge table and triangle table in uniform buffer
    // They are too large to have inline in the shader
    edge_table: Buffer,
    tri_table: Buffer,
    points: BufVec<Vec2>,
    heights: BufVec<f32>,
    voxels: Buffer,
    voxels_staging: Buffer,
    vertices: BufVec<Vec4>,
    normals: BufVec<Vec4>,
    uvs: BufVec<Vec2>,
    indices: BufVec<u32>,
    atomics: BufVec<u32>,
    atomics_staging: Buffer,
}

struct BindingGroups {
    simplex: BindGroup,
    voxels: BindGroup,
}

pub struct VoxelsPlugin;

impl Plugin for VoxelsPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(PreUpdate, (
                init_pipeline_system.run_if(not(resource_exists::<VoxelsPipeline>())),
                voxel_polygonize_system.run_if(resource_exists::<VoxelsPipeline>()),
            ));
    }
}

fn init_pipeline_system(mut commands: Commands, render_device: Res<RenderDevice>) {
    let edge_table = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("edge table buffer"),
        contents: cast_slice(EDGE_TABLE),
        usage: BufferUsages::STORAGE,
    });
    let tri_table = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("tri table buffer"),
        contents: cast_slice(TRI_TABLE),
        usage: BufferUsages::STORAGE,
    });
    let points: BufVec<Vec2> = BufVec::with_capacity(false, CHUNK_SZ_2, render_device.as_ref());
    let heights: BufVec<f32> = BufVec::with_capacity(true, CHUNK_SZ_2, render_device.as_ref());
    let voxels = render_device.create_buffer(&BufferDescriptor {
        label: Some("voxels buffer"),
        size: (CHUNK_SZ_3 * size_of::<Voxel>()) as BufferAddress,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let voxels_staging = render_device.create_buffer(&BufferDescriptor {
        label: Some("voxels staging buffer"),
        size: (CHUNK_SZ_3 * size_of::<Voxel>()) as BufferAddress,
        usage: BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let vertices: BufVec<Vec4> = BufVec::with_capacity(true, CHUNK_SZ_3 * 4 * 6, render_device.as_ref());
    let uvs: BufVec<Vec2> = BufVec::with_capacity(true, CHUNK_SZ_3 * 4 * 6, render_device.as_ref());
    let normals: BufVec<Vec4> = BufVec::with_capacity(true, CHUNK_SZ_3 * 4 * 6, render_device.as_ref());
    let indices: BufVec<u32> = BufVec::with_capacity(true, CHUNK_SZ_3 * 6 * 6, render_device.as_ref());
    let atomics: BufVec<u32> = BufVec::with_capacity(true, 2, render_device.as_ref());
    let atomics_staging = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("atomics staging buffer"),
        contents: cast_slice(&[0u32, 0u32]),
        usage: BufferUsages::COPY_SRC,
    });

    // let simplex_shader = asset_server.load("shaders/simplex.wgsl");
    let shader_source = include_str!("../../assets/shaders/simplex.wgsl");
    let shader = render_device.create_shader_module(ShaderModuleDescriptor {
        label: Some("simplex shader"),
        source: ShaderSource::Wgsl(shader_source.into()),
    });
    // TODO:arch update to Bevy compute creation when they allow PipelineCache to be used in main world
    let simplex_pipeline = render_device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("simplex pipeline"),
        layout: None,
        module: &shader,
        entry_point: "main",
    });

    // let voxel_shader = asset_server.load("shaders/voxels.wgsl");
    let shader_source = include_str!("../../assets/shaders/voxels.wgsl");
    let shader = render_device.create_shader_module(ShaderModuleDescriptor {
        label: Some("voxels shader"),
        source: ShaderSource::Wgsl(shader_source.into()),
    });
    let voxels_pipeline = render_device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("voxels pipeline"),
        layout: None,
        module: &shader,
        entry_point: "main",
    });

    commands.insert_resource(VoxelBuffers { edge_table, tri_table, points, heights, voxels, voxels_staging, vertices, normals, uvs, indices, atomics, atomics_staging });
    commands.insert_resource(VoxelsPipeline { simplex_pipeline, voxels_pipeline });
}

pub fn _sync_added_chunks_system(
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
    mut query: Query<(Entity, &Handle<Mesh>, &mut Chunk)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut buffers: ResMut<VoxelBuffers>,
    time: Res<Time>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    pipeline: Res<VoxelsPipeline>,
) {
    // let now = std::time::Instant::now();

    for (entity, mesh, mut chunk) in query.iter_mut() {
        buffers.atomics.clear();
        buffers.atomics.push(0);
        buffers.atomics.push(0);

        let time = time.elapsed().as_secs_f32();
        buffers.points.clear();
        for x in 0..CHUNK_SZ {
            for y in 0..CHUNK_SZ {
                buffers.points.push(0.05 * Vec2::new(x as f32 + time, y as f32 + time));
            }
        }

        let binding_groups = BindingGroups {
            simplex: render_device.create_bind_group(
                "simplex binding",
                &pipeline.simplex_pipeline.get_bind_group_layout(0).into(),
                &BindGroupEntries::sequential((
                    buffers.points.buffer().as_entire_binding(),
                    buffers.heights.buffer().as_entire_binding(),
                )),
            ),
            voxels: render_device.create_bind_group(
                "voxels binding",
                &pipeline.voxels_pipeline.get_bind_group_layout(0).into(),
                &BindGroupEntries::sequential((
                    buffers.edge_table.as_entire_binding(),
                    buffers.tri_table.as_entire_binding(),
                    buffers.voxels.as_entire_binding(),
                    buffers.atomics.buffer().as_entire_binding(),
                    buffers.vertices.buffer().as_entire_binding(),
                    buffers.normals.buffer().as_entire_binding(),
                    buffers.indices.buffer().as_entire_binding(),
                    buffers.uvs.buffer().as_entire_binding(),
                )),
            ),
        };

        if !buffers.points.is_empty() {
            let mut command_encoder = render_device.create_command_encoder(&CommandEncoderDescriptor { label: Some("simplex command encoder") });
            buffers.points.encode_write(render_queue.as_ref(), &mut command_encoder);
            {
                let mut pass = command_encoder.begin_compute_pass(&ComputePassDescriptor::default());
                pass.set_pipeline(&pipeline.simplex_pipeline);
                pass.set_bind_group(0, &binding_groups.simplex, &[]);
                pass.dispatch_workgroups((CHUNK_SZ / 32) as u32, (CHUNK_SZ / 32) as u32, 1);
            }
            buffers.heights.encode_read(CHUNK_SZ_2, &mut command_encoder);
            render_queue.submit(once(command_encoder.finish()));
            buffers.heights.map_buffer(CHUNK_SZ_2);
            render_device.poll(Wait);
            buffers.heights.read_and_unmap_buffer(CHUNK_SZ_2);

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

        let mut command_encoder = render_device.create_command_encoder(&CommandEncoderDescriptor { label: Some("voxel 1 command encoder") });
        render_queue.write_buffer(&buffers.voxels_staging, 0, &cast_slice(&chunk.voxels)[..]);
        command_encoder.copy_buffer_to_buffer(&buffers.voxels_staging, 0, &buffers.voxels, 0, (CHUNK_SZ_3 * size_of::<Voxel>()) as BufferAddress);
        command_encoder.copy_buffer_to_buffer(&buffers.atomics_staging, 0, &buffers.atomics.buffer, 0, (2 * size_of::<u32>()) as BufferAddress);
        {
            let mut pass = command_encoder.begin_compute_pass(&ComputePassDescriptor::default());
            pass.set_pipeline(&pipeline.voxels_pipeline);
            pass.set_bind_group(0, &binding_groups.voxels, &[]);
            let dispatch_size = (CHUNK_SZ / 8) as u32;
            pass.dispatch_workgroups(dispatch_size, dispatch_size, dispatch_size);
        }
        buffers.atomics.encode_read(2, &mut command_encoder);
        render_queue.submit(once(command_encoder.finish()));
        buffers.atomics.map_buffer(2);
        render_device.poll(Wait);
        buffers.atomics.read_and_unmap_buffer(2);
        let vertex_count = buffers.atomics.as_slice()[0] as usize;
        let index_count = buffers.atomics.as_slice()[1] as usize;

        if vertex_count == 0 {
            continue;
        }

        let mut command_encoder = render_device.create_command_encoder(&CommandEncoderDescriptor { label: Some("voxel 2 command encoder") });
        buffers.vertices.encode_read(vertex_count, &mut command_encoder);
        buffers.normals.encode_read(vertex_count, &mut command_encoder);
        buffers.uvs.encode_read(vertex_count, &mut command_encoder);
        buffers.indices.encode_read(index_count, &mut command_encoder);
        render_queue.submit(once(command_encoder.finish()));
        buffers.vertices.map_buffer(vertex_count);
        buffers.normals.map_buffer(vertex_count);
        buffers.uvs.map_buffer(vertex_count);
        buffers.indices.map_buffer(index_count);
        render_device.poll(Wait);

        buffers.vertices.read_and_unmap_buffer(vertex_count);
        buffers.normals.read_and_unmap_buffer(vertex_count);
        buffers.uvs.read_and_unmap_buffer(vertex_count);
        buffers.indices.read_and_unmap_buffer(index_count);

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

        // TODO:perf inefficient
        commands.entity(entity).insert(Collider::from_bevy_mesh(mesh, &ComputedColliderShape::TriMesh).unwrap());
    }

    // println!("Elapsed: {:.2?}", now.elapsed());
}