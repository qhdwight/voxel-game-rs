struct Voxel {
    density: f32;
};

struct VoxelBuffer {
    data: array<Voxel>;
};

struct VertexBuffer {
    data: array<vec3<f32>>;
};

struct IndexBuffer {
    data: array<u32>;
};

[[group(0), binding(0)]]
var<storage, read> in_voxels: VoxelBuffer;

[[group(0), binding(1)]]
var<storage, read_write> out_vertices: VertexBuffer;

[[group(0), binding(2)]]
var<storage, read_write> out_indices: IndexBuffer;

[[stage(compute), workgroup_size(8, 8, 8)]]
fn main([[builtin(global_invocation_id)]] invocation_id: vec3<u32>) {
    let voxel = in_voxels.data[invocation_id.x * 32u * 32u + invocation_id.y * 32u + invocation_id.z];
}
