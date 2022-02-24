struct Voxel {
    density: f32;
};

struct VoxelBuffer {
    data: array<Voxel>;
};

struct VertexBuffer {
    data: array<vec3<f32>>;
};

struct NormalBuffer {
    data: array<vec3<f32>>;
};

struct IndexBuffer {
    data: array<u32>;
};

struct Atomics {
    vertices_head: atomic<u32>;
    indices_head: atomic<u32>;
};

[[group(0), binding(0)]]
var<storage, read> in_voxels: VoxelBuffer;

[[group(0), binding(1)]]
var<storage, read_write> out_vertices: VertexBuffer;

[[group(0), binding(2)]]
var<storage, read_write> out_normals: NormalBuffer;

[[group(0), binding(3)]]
var<storage, read_write> out_indices: IndexBuffer;

[[group(0), binding(4)]]
var<storage, read_write> global_atomics: Atomics;

[[stage(compute), workgroup_size(8, 8, 8)]]
fn main([[builtin(global_invocation_id)]] invocation_id: vec3<u32>) {

    var faces = array<array<vec3<f32>, 4>, 6>(
        array<vec3<f32>, 4>(
            vec3<f32>(0.5, -0.5, -0.5),
            vec3<f32>(0.5, 0.5, -0.5),
            vec3<f32>(0.5, 0.5, 0.5),
            vec3<f32>(0.5, -0.5, 0.5),
        ),
        array<vec3<f32>, 4>(
            vec3<f32>(-0.5, -0.5, 0.5),
            vec3<f32>(-0.5, 0.5, 0.5),
            vec3<f32>(-0.5, 0.5, -0.5),
            vec3<f32>(-0.5, -0.5, -0.5)
        ),
        array<vec3<f32>, 4>(
            vec3<f32>(-0.5, 0.5, 0.5),
            vec3<f32>(0.5, 0.5, 0.5),
            vec3<f32>(0.5, 0.5, -0.5),
            vec3<f32>(-0.5, 0.5, -0.5)
        ),
        array<vec3<f32>, 4>(
            vec3<f32>(-0.5, -0.5, -0.5),
            vec3<f32>(0.5, -0.5, -0.5),
            vec3<f32>(0.5, -0.5, 0.5),
            vec3<f32>(-0.5, -0.5, 0.5)
        ),
       array<vec3<f32>, 4>(
            vec3<f32>(0.5, -0.5, 0.5),
            vec3<f32>(0.5, 0.5, 0.5),
            vec3<f32>(-0.5, 0.5, 0.5),
            vec3<f32>(-0.5, -0.5, 0.5)
       ),
       array<vec3<f32>, 4>(
            vec3<f32>(-0.5, -0.5, -0.5),
            vec3<f32>(-0.5, 0.5, -0.5),
            vec3<f32>(0.5, 0.5, -0.5),
            vec3<f32>(0.5, -0.5, -0.5)
       ),
    );
    var adj_offsets = array<vec3<i32>, 6>(
        vec3<i32>(1, 0, 0),
        vec3<i32>(-1, 0, 0),
        vec3<i32>(0, 1, 0),
        vec3<i32>(0, -1, 0),
        vec3<i32>(0, 0, 1),
        vec3<i32>(0, 0, -1),
    );

    let voxel = in_voxels.data[invocation_id.x + invocation_id.y * 32u + invocation_id.z * 32u * 32u];
    if (voxel.density > 0.5) {
        var dir: u32 = 0u;
        loop {
            var adj_pos = vec3<i32>(invocation_id) + adj_offsets[dir];
            var flat_idx = u32(adj_pos.x + adj_pos.y * 32 + adj_pos.z * 32 * 32);
            var adj_density = 0.0;
            if (invocation_id.x > 0u && invocation_id.x < 31u
             && invocation_id.y > 0u && invocation_id.y < 31u
             && invocation_id.z > 0u && invocation_id.z < 31u) {
                adj_density = in_voxels.data[flat_idx].density;
            }
            if (adj_density < 0.5) {
                var pos = vec3<f32>(invocation_id);

                var start_vert_idx = atomicAdd(&global_atomics.vertices_head, 4u);
                var start_indices_idx = atomicAdd(&global_atomics.indices_head, 6u);

                out_vertices.data[start_vert_idx + 0u] = pos + faces[dir][0u];
                out_vertices.data[start_vert_idx + 1u] = pos + faces[dir][1u];
                out_vertices.data[start_vert_idx + 2u] = pos + faces[dir][2u];
                out_vertices.data[start_vert_idx + 3u] = pos + faces[dir][3u];

                var normal = cross(faces[dir][0u] - faces[dir][1u], faces[dir][0u] - faces[dir][2u]);
                out_normals.data[start_vert_idx + 0u] = normal;
                out_normals.data[start_vert_idx + 1u] = normal;
                out_normals.data[start_vert_idx + 2u] = normal;
                out_normals.data[start_vert_idx + 3u] = normal;

                out_indices.data[start_indices_idx + 0u] = start_vert_idx + 0u;
                out_indices.data[start_indices_idx + 1u] = start_vert_idx + 1u;
                out_indices.data[start_indices_idx + 2u] = start_vert_idx + 2u;
                out_indices.data[start_indices_idx + 3u] = start_vert_idx + 0u;
                out_indices.data[start_indices_idx + 4u] = start_vert_idx + 2u;
                out_indices.data[start_indices_idx + 5u] = start_vert_idx + 3u;
            }

            dir = dir + 1u;
            if (dir >= 6u) {
                break;
            }
        }
    }
}
