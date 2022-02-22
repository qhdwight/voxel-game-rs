struct Voxel {
    density: f32;
};

struct VoxelBuffer {
    data: array<Voxel>;
};

struct MeshBuffer {
    vertices: array<vec3<f32>>;
    indices: array<u32>;
};

[[group(0), binding(0)]]
var<storage, read> in_voxels: VoxelBuffer;

[[group(0), binding(1)]]
var<storage, read_write> out_mesh: MeshBuffer;

[[stage(compute), workgroup_size(8, 8, 8)]]
fn main([[builtin(global_invocation_id)]] invocation_id: vec3<u32>) {
    let voxel = in_voxels.data[invocation_id.x * 32u * 32u + invocation_id.y * 32u + invocation_id.z];
}
