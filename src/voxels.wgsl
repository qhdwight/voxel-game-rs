struct Voxel {
    density: f32;
};

struct VoxelBuffer {
    data: array<Voxel>;
};

struct MeshBuffer {
    vertices: array<vec3<f32>>;
};

[[group(0), binding(0)]]
var<storage, read> inVoxels: VoxelBuffer;

[[group(0), binding(1)]]
var<storage, read_write> outMesh: MeshBuffer;

[[stage(compute), workgroup_size(32, 1, 1)]]
fn main([[builtin(global_invocation_id)]] invocation_id: vec3<u32>) {
    let voxel = inVoxels.data[invocation_id.x * 32u * 32u + invocation_id.y * 32u + invocation_id.z];
}
