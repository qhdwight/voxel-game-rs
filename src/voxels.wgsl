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

[[stage(compute), workgroup_size(32, 32, 32)]]
fn main([[builtin(global_invocation_id)]] invocation_id: vec3<u32>) {
    let voxel = inVoxels[invocation_id.x * 32 * 32 + invocation_id.y * 32 + invocation_id.z];
}
