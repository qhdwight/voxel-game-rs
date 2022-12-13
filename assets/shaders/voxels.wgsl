struct Voxel {
    flags: u32,
    density: f32,
};

struct VoxelBuffer {
    data: array<Voxel>,
};

struct VertexBuffer {
    data: array<vec3<f32>>,
};

struct NormalBuffer {
    data: array<vec3<f32>>,
};

struct IndexBuffer {
    data: array<u32>,
};

struct UvBuffer {
    data: array<vec2<f32>>,
};

struct Atomics {
    vertices_head: atomic<u32>,
    indices_head: atomic<u32>,
};

struct EdgeTable {
    data: array<u32, 256>,
};

struct TriangleTable {
    data: array<array<i32, 16>, 256>,
};

@group(0) @binding(0)
var<storage, read_write> uniform_edge_table: EdgeTable;

@group(0) @binding(1)
var<storage, read_write> uniform_tri_table: TriangleTable;

@group(0) @binding(2)
var<storage, read> in_voxels: VoxelBuffer;

@group(0) @binding(3)
var<storage, read_write> global_atomics: Atomics;

@group(0) @binding(4)
var<storage, read_write> out_vertices: VertexBuffer;

@group(0) @binding(5)
var<storage, read_write> out_normals: NormalBuffer;

@group(0) @binding(6)
var<storage, read_write> out_indices: IndexBuffer;

@group(0) @binding(7)
var<storage, read_write> out_uvs: UvBuffer;


let chunk_sz = 32;

fn get_flat_index(pos: vec3<i32>) -> u32 {
    return u32(pos.x + pos.y * chunk_sz + pos.z * chunk_sz * chunk_sz);
}

fn get_voxel_density(pos: vec3<i32>) -> f32 {
    var density: f32 = 0.0;
    if (pos.x >= 0 && pos.x < chunk_sz
     && pos.y >= 0 && pos.y < chunk_sz
     && pos.z >= 0 && pos.z < chunk_sz) {
        density = in_voxels.data[get_flat_index(pos)].density;
    }
    return density;
}

fn interp_vertex(p1: vec3<f32>, p2: vec3<f32>, v1: f32, v2: f32) -> vec3<f32> {
    let mu = (0.5 - v1) / (v2 - v1);
    return p1 + mu * (p2 - p1);
}

@compute @workgroup_size(8, 8, 8)
fn main(@builtin(global_invocation_id) invocation_id: vec3<u32>) {

    let pos = vec3<i32>(invocation_id);
    let voxel = in_voxels.data[get_flat_index(pos)];

    if (voxel.flags == 0u) {

        let smooth_adj_offsets = array<vec3<i32>, 8>(
            vec3<i32>(0, 0, 1),
            vec3<i32>(1, 0, 1),
            vec3<i32>(1, 0, 0),
            vec3<i32>(0, 0, 0),
            vec3<i32>(0, 1, 1),
            vec3<i32>(1, 1, 1),
            vec3<i32>(1, 1, 0),
            vec3<i32>(0, 1, 0)
        );

        var cube_idx: u32 = 0u;
        var orient: u32 = 0u;
        let positions = array<vec3<f32>, 8>(
            vec3<f32>(pos + smooth_adj_offsets[0u]),
            vec3<f32>(pos + smooth_adj_offsets[1u]),
            vec3<f32>(pos + smooth_adj_offsets[2u]),
            vec3<f32>(pos + smooth_adj_offsets[3u]),
            vec3<f32>(pos + smooth_adj_offsets[4u]),
            vec3<f32>(pos + smooth_adj_offsets[5u]),
            vec3<f32>(pos + smooth_adj_offsets[6u]),
            vec3<f32>(pos + smooth_adj_offsets[7u]),
        );
        let densities = array<f32, 8>(
            get_voxel_density(pos + smooth_adj_offsets[0u]),
            get_voxel_density(pos + smooth_adj_offsets[1u]),
            get_voxel_density(pos + smooth_adj_offsets[2u]),
            get_voxel_density(pos + smooth_adj_offsets[3u]),
            get_voxel_density(pos + smooth_adj_offsets[4u]),
            get_voxel_density(pos + smooth_adj_offsets[5u]),
            get_voxel_density(pos + smooth_adj_offsets[6u]),
            get_voxel_density(pos + smooth_adj_offsets[7u]),
        );
        cube_idx = cube_idx | u32(densities[0u] < 0.5) * (1u << 0u);
        cube_idx = cube_idx | u32(densities[1u] < 0.5) * (1u << 1u);
        cube_idx = cube_idx | u32(densities[2u] < 0.5) * (1u << 2u);
        cube_idx = cube_idx | u32(densities[3u] < 0.5) * (1u << 3u);
        cube_idx = cube_idx | u32(densities[4u] < 0.5) * (1u << 4u);
        cube_idx = cube_idx | u32(densities[5u] < 0.5) * (1u << 5u);
        cube_idx = cube_idx | u32(densities[6u] < 0.5) * (1u << 6u);
        cube_idx = cube_idx | u32(densities[7u] < 0.5) * (1u << 7u);

        if (cube_idx == 0x00u || cube_idx == 0xffu) {
            return;
        }

        var vertices = array<vec3<f32>, 12>(
            f32((uniform_edge_table.data[cube_idx] & (1u <<  0u)) != 0u) * interp_vertex(positions[0u], positions[1u], densities[0u], densities[1u]),
            f32((uniform_edge_table.data[cube_idx] & (1u <<  1u)) != 0u) * interp_vertex(positions[1u], positions[2u], densities[1u], densities[2u]),
            f32((uniform_edge_table.data[cube_idx] & (1u <<  2u)) != 0u) * interp_vertex(positions[2u], positions[3u], densities[2u], densities[3u]),
            f32((uniform_edge_table.data[cube_idx] & (1u <<  3u)) != 0u) * interp_vertex(positions[3u], positions[0u], densities[3u], densities[0u]),
            f32((uniform_edge_table.data[cube_idx] & (1u <<  4u)) != 0u) * interp_vertex(positions[4u], positions[5u], densities[4u], densities[5u]),
            f32((uniform_edge_table.data[cube_idx] & (1u <<  5u)) != 0u) * interp_vertex(positions[5u], positions[6u], densities[5u], densities[6u]),
            f32((uniform_edge_table.data[cube_idx] & (1u <<  6u)) != 0u) * interp_vertex(positions[6u], positions[7u], densities[6u], densities[7u]),
            f32((uniform_edge_table.data[cube_idx] & (1u <<  7u)) != 0u) * interp_vertex(positions[7u], positions[4u], densities[7u], densities[4u]),
            f32((uniform_edge_table.data[cube_idx] & (1u <<  8u)) != 0u) * interp_vertex(positions[0u], positions[4u], densities[0u], densities[4u]),
            f32((uniform_edge_table.data[cube_idx] & (1u <<  9u)) != 0u) * interp_vertex(positions[1u], positions[5u], densities[1u], densities[5u]),
            f32((uniform_edge_table.data[cube_idx] & (1u << 10u)) != 0u) * interp_vertex(positions[2u], positions[6u], densities[2u], densities[6u]),
            f32((uniform_edge_table.data[cube_idx] & (1u << 11u)) != 0u) * interp_vertex(positions[3u], positions[7u], densities[3u], densities[7u]),
        );

        var tri_idx: u32 = 0u;
        loop {
            var start_vert_idx = atomicAdd(&global_atomics.vertices_head, 3u);
            var start_indices_idx = atomicAdd(&global_atomics.indices_head, 3u);

            let v0 = vertices[ uniform_tri_table.data[cube_idx][tri_idx + 0u] ];
            let v1 = vertices[ uniform_tri_table.data[cube_idx][tri_idx + 1u] ];
            let v2 = vertices[ uniform_tri_table.data[cube_idx][tri_idx + 2u] ];

            out_vertices.data[start_vert_idx + 0u] = v0;
            out_vertices.data[start_vert_idx + 1u] = v1;
            out_vertices.data[start_vert_idx + 2u] = v2;

            out_indices.data[start_indices_idx + 0u] = start_vert_idx + 0u;
            out_indices.data[start_indices_idx + 1u] = start_vert_idx + 1u;
            out_indices.data[start_indices_idx + 2u] = start_vert_idx + 2u;

            let normal = cross(v0 - v1, v0 - v2);
            out_normals.data[start_vert_idx + 0u] = normal;
            out_normals.data[start_vert_idx + 1u] = normal;
            out_normals.data[start_vert_idx + 2u] = normal;

            out_uvs.data[start_vert_idx + 0u] = vec2<f32>(0.0, 0.0);
            out_uvs.data[start_vert_idx + 1u] = vec2<f32>(1.0, 0.0);
            out_uvs.data[start_vert_idx + 2u] = vec2<f32>(0.0, 1.0);

            tri_idx = tri_idx + 3u;
            if (uniform_tri_table.data[cube_idx][tri_idx] == -1) {
                break;
            }
        }
    } else {

        var block_faces = array<array<vec3<f32>, 4>, 6>(
            array<vec3<f32>, 4>(
                vec3<f32>(0.5, -0.5, -0.5),
                vec3<f32>(0.5,  0.5, -0.5),
                vec3<f32>(0.5,  0.5,  0.5),
                vec3<f32>(0.5, -0.5,  0.5),
            ),
            array<vec3<f32>, 4>(
                vec3<f32>(-0.5, -0.5,  0.5),
                vec3<f32>(-0.5,  0.5,  0.5),
                vec3<f32>(-0.5,  0.5, -0.5),
                vec3<f32>(-0.5, -0.5, -0.5)
            ),
            array<vec3<f32>, 4>(
                vec3<f32>(-0.5, 0.5,  0.5),
                vec3<f32>( 0.5, 0.5,  0.5),
                vec3<f32>( 0.5, 0.5, -0.5),
                vec3<f32>(-0.5, 0.5, -0.5)
            ),
            array<vec3<f32>, 4>(
                vec3<f32>(-0.5, -0.5, -0.5),
                vec3<f32>( 0.5, -0.5, -0.5),
                vec3<f32>( 0.5, -0.5,  0.5),
                vec3<f32>(-0.5, -0.5,  0.5)
            ),
            array<vec3<f32>, 4>(
                vec3<f32>( 0.5, -0.5, 0.5),
                vec3<f32>( 0.5,  0.5, 0.5),
                vec3<f32>(-0.5,  0.5, 0.5),
                vec3<f32>(-0.5, -0.5, 0.5)
            ),
            array<vec3<f32>, 4>(
                vec3<f32>(-0.5, -0.5, -0.5),
                vec3<f32>(-0.5,  0.5, -0.5),
                vec3<f32>( 0.5,  0.5, -0.5),
                vec3<f32>( 0.5, -0.5, -0.5)
            ),
        );
        var block_adj_offsets = array<vec3<i32>, 6>(
            vec3<i32>( 1,  0,  0),
            vec3<i32>(-1,  0,  0),
            vec3<i32>( 0,  1,  0),
            vec3<i32>( 0, -1,  0),
            vec3<i32>( 0,  0,  1),
            vec3<i32>( 0,  0, -1),
        );

        var dir: u32 = 0u;  
        loop {
            let adj_pos = pos + block_adj_offsets[dir];
            let adj_density = get_voxel_density(pos);

            if (adj_density < 0.5) {
                var pos = vec3<f32>(invocation_id);

                let start_vert_idx = atomicAdd(&global_atomics.vertices_head, 4u);
                let start_indices_idx = atomicAdd(&global_atomics.indices_head, 6u);

                let v0 = block_faces[dir][0u];
                let v1 = block_faces[dir][1u];
                let v2 = block_faces[dir][2u];
                let v3 = block_faces[dir][3u];

                out_vertices.data[start_vert_idx + 0u] = pos + v0;
                out_vertices.data[start_vert_idx + 1u] = pos + v1;
                out_vertices.data[start_vert_idx + 2u] = pos + v2;
                out_vertices.data[start_vert_idx + 3u] = pos + v3;

                let normal = cross(v0 - v1, v0 - v2);
                out_normals.data[start_vert_idx + 0u] = normal;
                out_normals.data[start_vert_idx + 1u] = normal;
                out_normals.data[start_vert_idx + 2u] = normal;
                out_normals.data[start_vert_idx + 3u] = normal;

                out_uvs.data[start_vert_idx + 0u] = vec2<f32>(0.0, 0.0);
                out_uvs.data[start_vert_idx + 1u] = vec2<f32>(1.0, 0.0);
                out_uvs.data[start_vert_idx + 2u] = vec2<f32>(1.0, 1.0);
                out_uvs.data[start_vert_idx + 3u] = vec2<f32>(0.0, 1.0);

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
