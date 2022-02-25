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

fn get_flat_index(pos: vec3<i32>) -> u32 {
    return u32(pos.x + pos.y * 32 + pos.z * 32 * 32);
}

fn get_voxel_density(pos: vec3<i32>) -> f32 {
    var density: f32 = 0.0;
    if (pos.x >= 0u && pos.x < 32u
     && pos.y >= 0u && pos.y < 32u
     && pos.z >= 0u && pos.z < 32u) {
        density = in_voxels.data[get_flat_index(pos)].density;
    }
    return density;
}

fn interp_vertex(p1: vec3<f32>, p2: vec3<f32>, v1: f32, v2: f32) {
    let mu = (0.5 - v1) / (v2 - v1);
    return p1 + mu * (p2 - p1);
}

[[stage(compute), workgroup_size(8, 8, 8)]]
fn main([[builtin(global_invocation_id)]] invocation_id: vec3<u32>) {

    #import lookup.wgsl

    let voxel = in_voxels.data[get_flat_index()];
    let pos = vec3<i32>(invocation_id);

    {
        var cube_idx: u32 = 0u;
        var orient: u32 = 0u;
        let densities = array<f32, 8>();
        let positions = array<vec3<f32>, 8>();
        loop {
            orient = orient + 1u;

            let adj_pos = pos + smooth_adj_offsets[orient];
            positions[orient] = adj_pos;
            densities[orient] = get_voxel_density(adj_pos);

            orient = orient + 1u;
            if (orient >= 8u) {
                break;
            }
        }

        if (cube_idx == 0 || cube_idx == 255) {
            return;
        }

        var vertex_idx: u32 = 0u;
        let vertices = array<vec3<f32>, 12>();
        loop {
            if (edge_table[cube_idx] & (1u << vertex_idx) != 0u) {
                vertices[vertex_idx] = interp_vertex(
                    positions[vert_idx_1[vertex_idx]],
                    positions[vert_idx_2[vertex_idx]],
                    densities[vert_idx_1[vertex_idx]],
                    densities[vert_idx_2[vertex_idx]]
                );
            } else {
                vertices[vertex_idx] = vec3<f32>(0.0, 0.0, 0.0);
            }

            vertex_idx = vertex_idx + 1u;
            if (vertex_idx >= 12u) {
                break;
            }
        }

        let i: u32 = 0u;
        loop {
            var start_vert_idx = atomicAdd(&global_atomics.vertices_head, 3u);
            var start_indices_idx = atomicAdd(&global_atomics.indices_head, 3u);

            let v0 = vertices[triangle_table[cube_idx][i + 0u]];
            let v1 = vertices[triangle_table[cube_idx][i + 1u]];
            let v2 = vertices[triangle_table[cube_idx][i + 2u]];

            out_vertices.data[start_vert_idx + 0u] = v0;
            out_vertices.data[start_vert_idx + 1u] = v1;
            out_vertices.data[start_vert_idx + 2u] = v2;

            out_indices.data[start_indices_idx + 0u] = start_vert_idx + 0u;
            out_indices.data[start_indices_idx + 1u] = start_vert_idx + 1u;
            out_indices.data[start_indices_idx + 2u] = start_vert_idx + 2u;

            var normal = cross(v0 - v1, v0 - v2);
            out_normals.data[start_vert_idx + 0u] = normal;
            out_normals.data[start_vert_idx + 1u] = normal;
            out_normals.data[start_vert_idx + 2u] = normal;

            i = i + 3u;
            if (triangle_table[cube_idx][i] == -1) {
                break;
            }
        }
    }

    if (voxel.density > 0.5) {
        var dir: u32 = 0u;
        loop {
            var adj_pos = pos + block_adj_offsets[dir];
            var adj_density = get_voxel_density(pos);

            if (adj_density < 0.5) {
                var pos = vec3<f32>(invocation_id);

                var start_vert_idx = atomicAdd(&global_atomics.vertices_head, 4u);
                var start_indices_idx = atomicAdd(&global_atomics.indices_head, 6u);

                let v0 = block_faces[dir][0u];
                let v1 = block_faces[dir][1u];
                let v2 = block_faces[dir][2u];
                let v3 = block_faces[dir][3u];

                out_vertices.data[start_vert_idx + 0u] = pos + v0;
                out_vertices.data[start_vert_idx + 1u] = pos + v1;
                out_vertices.data[start_vert_idx + 2u] = pos + v2;
                out_vertices.data[start_vert_idx + 3u] = pos + v3;

                var normal = cross(v0 - v1, v0 - v2);
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
