struct Voxel {
    flags: u32,
    density: f32,
};

struct Atomics {
    vertex_head: atomic<u32>,
    triangle_head: atomic<u32>,
};

@group(0) @binding(0)
var<storage, read> triangle_table: array<array<i32, 16>, 256>;
@group(0) @binding(1)
var<storage, read> block_faces_table: array<array<vec3<f32>, 4>, 6>;
@group(0) @binding(2)
var<storage, read> in_voxels: array<Voxel>;
@group(0) @binding(3)
var<storage, read_write> out_atomics: Atomics;
@group(0) @binding(4)
var<storage, read_write> out_vertices: array<vec3<f32>>;
@group(0) @binding(5)
var<storage, read_write> out_normals: array<vec3<f32>>;
@group(0) @binding(6)
var<storage, read_write> out_triangles: array<u32>;
@group(0) @binding(7)
var<storage, read_write> out_uvs: array<vec2<f32>>;

var<workgroup> workgroup_atomics: Atomics;
var<workgroup> workgroup_vertices: array<vec3<f32>, 768>;
var<workgroup> workgroup_normals: array<vec3<f32>, 768>;
var<workgroup> workgroup_triangles: array<u32, 768>;
var<workgroup> workgroup_uvs: array<vec2<f32>, 768>;

const chunk_sz = 32;

fn get_flat_index(pos: vec3<i32>) -> u32 {
    return u32(pos.x + pos.y * chunk_sz + pos.z * chunk_sz * chunk_sz);
}

fn get_voxel_density(pos: vec3<i32>) -> f32 {
    var density: f32 = 0.0;
    if (pos.x >= 0 && pos.x < chunk_sz
     && pos.y >= 0 && pos.y < chunk_sz
     && pos.z >= 0 && pos.z < chunk_sz) {
        density = in_voxels[get_flat_index(pos)].density;
    }
    return density;
}

fn interpolate_vertex(p1: vec3<f32>, p2: vec3<f32>, v1: f32, v2: f32) -> vec3<f32> {
    let mu = (0.5 - v1) / (v2 - v1);
    return p1 + mu * (p2 - p1);
}

@workgroup_size(4, 4, 4)
@compute fn main(@builtin(global_invocation_id) invocation_index: vec3<u32>,
                 @builtin(local_invocation_index) workgroup_index: u32) {

    if (workgroup_index == 0u) {
        atomicStore(&workgroup_atomics.vertex_head, 0u);
        atomicStore(&workgroup_atomics.triangle_head, 0u);
    }

    workgroupBarrier();

    let pos = vec3<i32>(invocation_index);
    let voxel = in_voxels[get_flat_index(pos)];

    if (voxel.flags == 0u) {

        var cube_index: u32 = 0u;
        let positions = array<vec3<f32>, 8>(
            vec3<f32>(pos + vec3<i32>(0, 0, 1)),
            vec3<f32>(pos + vec3<i32>(1, 0, 1)),
            vec3<f32>(pos + vec3<i32>(1, 0, 0)),
            vec3<f32>(pos + vec3<i32>(0, 0, 0)),
            vec3<f32>(pos + vec3<i32>(0, 1, 1)),
            vec3<f32>(pos + vec3<i32>(1, 1, 1)),
            vec3<f32>(pos + vec3<i32>(1, 1, 0)),
            vec3<f32>(pos + vec3<i32>(0, 1, 0)),
        );
        let densities = array<f32, 8>(
            get_voxel_density(pos + vec3<i32>(0, 0, 1)),
            get_voxel_density(pos + vec3<i32>(1, 0, 1)),
            get_voxel_density(pos + vec3<i32>(1, 0, 0)),
            get_voxel_density(pos + vec3<i32>(0, 0, 0)),
            get_voxel_density(pos + vec3<i32>(0, 1, 1)),
            get_voxel_density(pos + vec3<i32>(1, 1, 1)),
            get_voxel_density(pos + vec3<i32>(1, 1, 0)),
            get_voxel_density(pos + vec3<i32>(0, 1, 0)),
        );
        cube_index |= select(0u, 1u << 0u, densities[0u] < 0.5);
        cube_index |= select(0u, 1u << 1u, densities[1u] < 0.5);
        cube_index |= select(0u, 1u << 2u, densities[2u] < 0.5);
        cube_index |= select(0u, 1u << 3u, densities[3u] < 0.5);
        cube_index |= select(0u, 1u << 4u, densities[4u] < 0.5);
        cube_index |= select(0u, 1u << 5u, densities[5u] < 0.5);
        cube_index |= select(0u, 1u << 6u, densities[6u] < 0.5);
        cube_index |= select(0u, 1u << 7u, densities[7u] < 0.5);

        if (cube_index > 0x00u && cube_index < 0xffu) {
            var vertices = array<vec3<f32>, 12>(
                interpolate_vertex(positions[0u], positions[1u], densities[0u], densities[1u]),
                interpolate_vertex(positions[1u], positions[2u], densities[1u], densities[2u]),
                interpolate_vertex(positions[2u], positions[3u], densities[2u], densities[3u]),
                interpolate_vertex(positions[3u], positions[0u], densities[3u], densities[0u]),
                interpolate_vertex(positions[4u], positions[5u], densities[4u], densities[5u]),
                interpolate_vertex(positions[5u], positions[6u], densities[5u], densities[6u]),
                interpolate_vertex(positions[6u], positions[7u], densities[6u], densities[7u]),
                interpolate_vertex(positions[7u], positions[4u], densities[7u], densities[4u]),
                interpolate_vertex(positions[0u], positions[4u], densities[0u], densities[4u]),
                interpolate_vertex(positions[1u], positions[5u], densities[1u], densities[5u]),
                interpolate_vertex(positions[2u], positions[6u], densities[2u], densities[6u]),
                interpolate_vertex(positions[3u], positions[7u], densities[3u], densities[7u]),
            );

            var triangle_index: u32 = 0u;
            loop {
                let start_vertex_index = atomicAdd(&workgroup_atomics.vertex_head, 3u);
                let start_triangle_index = atomicAdd(&workgroup_atomics.triangle_head, 3u);

                let v0 = vertices[triangle_table[cube_index][triangle_index + 0u]];
                let v1 = vertices[triangle_table[cube_index][triangle_index + 1u]];
                let v2 = vertices[triangle_table[cube_index][triangle_index + 2u]];

                workgroup_vertices[start_vertex_index + 0u] = v0;
                workgroup_vertices[start_vertex_index + 1u] = v1;
                workgroup_vertices[start_vertex_index + 2u] = v2;

                workgroup_triangles[start_triangle_index + 0u] = start_vertex_index + 0u;
                workgroup_triangles[start_triangle_index + 1u] = start_vertex_index + 1u;
                workgroup_triangles[start_triangle_index + 2u] = start_vertex_index + 2u;

                let normal = normalize(cross(v1 - v0, v2 - v0));
                workgroup_normals[start_vertex_index + 0u] = normal;
                workgroup_normals[start_vertex_index + 1u] = normal;
                workgroup_normals[start_vertex_index + 2u] = normal;

                workgroup_uvs[start_vertex_index + 0u] = vec2<f32>(0.0, 0.0);
                workgroup_uvs[start_vertex_index + 1u] = vec2<f32>(1.0, 0.0);
                workgroup_uvs[start_vertex_index + 2u] = vec2<f32>(0.0, 1.0);

                triangle_index += 3u;
                if (triangle_table[cube_index][triangle_index] == -1) {
                    break;
                }
            }
        }

    } else {

        var block_adjacent_offsets = array<vec3<i32>, 6>(
            vec3<i32>( 1,  0,  0),
            vec3<i32>(-1,  0,  0),
            vec3<i32>( 0,  1,  0),
            vec3<i32>( 0, -1,  0),
            vec3<i32>( 0,  0,  1),
            vec3<i32>( 0,  0, -1),
        );

        var dir: u32 = 0u;
        loop {
            let adj_pos = pos + block_adjacent_offsets[dir];
            let adj_density = get_voxel_density(pos);

            if (adj_density < 0.5) {
                var pos = vec3<f32>(invocation_index);

                let start_vertex_index = atomicAdd(&workgroup_atomics.vertex_head, 4u);
                let start_triangle_index = atomicAdd(&workgroup_atomics.triangle_head, 6u);

                let v0 = block_faces_table[dir][0u];
                let v1 = block_faces_table[dir][1u];
                let v2 = block_faces_table[dir][2u];
                let v3 = block_faces_table[dir][3u];

                workgroup_vertices[start_vertex_index + 0u] = pos + v0;
                workgroup_vertices[start_vertex_index + 1u] = pos + v1;
                workgroup_vertices[start_vertex_index + 2u] = pos + v2;
                workgroup_vertices[start_vertex_index + 3u] = pos + v3;

                let normal = cross(v0 - v1, v0 - v2);
                workgroup_normals[start_vertex_index + 0u] = normal;
                workgroup_normals[start_vertex_index + 1u] = normal;
                workgroup_normals[start_vertex_index + 2u] = normal;
                workgroup_normals[start_vertex_index + 3u] = normal;

                workgroup_uvs[start_vertex_index + 0u] = vec2<f32>(0.0, 0.0);
                workgroup_uvs[start_vertex_index + 1u] = vec2<f32>(1.0, 0.0);
                workgroup_uvs[start_vertex_index + 2u] = vec2<f32>(1.0, 1.0);
                workgroup_uvs[start_vertex_index + 3u] = vec2<f32>(0.0, 1.0);

                workgroup_triangles[start_triangle_index + 0u] = start_vertex_index + 0u;
                workgroup_triangles[start_triangle_index + 1u] = start_vertex_index + 1u;
                workgroup_triangles[start_triangle_index + 2u] = start_vertex_index + 2u;
                workgroup_triangles[start_triangle_index + 3u] = start_vertex_index + 0u;
                workgroup_triangles[start_triangle_index + 4u] = start_vertex_index + 2u;
                workgroup_triangles[start_triangle_index + 5u] = start_vertex_index + 3u;
            }

            dir += 1u;
            if (dir >= 6u) {
                break;
            }
        }
    }

    workgroupBarrier();

    if (workgroup_index == 0u) {
        let workgroup_vertex_count = atomicLoad(&workgroup_atomics.vertex_head);
        let start_vertex_index = atomicAdd(&out_atomics.vertex_head, workgroup_vertex_count);
        var vertex_index: u32 = 0u;
        loop {
            let out_index = start_vertex_index + vertex_index;
            out_vertices[out_index + 0u] = workgroup_vertices[vertex_index + 0u];
            out_vertices[out_index + 1u] = workgroup_vertices[vertex_index + 1u];
            out_vertices[out_index + 2u] = workgroup_vertices[vertex_index + 2u];
            out_normals[out_index + 0u] = workgroup_normals[vertex_index + 0u];
            out_normals[out_index + 1u] = workgroup_normals[vertex_index + 1u];
            out_normals[out_index + 2u] = workgroup_normals[vertex_index + 2u];
            out_uvs[out_index + 0u] = workgroup_uvs[vertex_index + 0u];
            out_uvs[out_index + 1u] = workgroup_uvs[vertex_index + 1u];
            out_uvs[out_index + 2u] = workgroup_uvs[vertex_index + 2u];

//            out_triangles[out_index + 0u] = out_index + 0u;
//            out_triangles[out_index + 1u] = out_index + 1u;
//            out_triangles[out_index + 2u] = out_index + 2u;

            vertex_index += 3u;
            if (vertex_index >= workgroup_vertex_count) {
                break;
            }
        }

        let workgroup_triangle_count = atomicLoad(&workgroup_atomics.triangle_head);
        let start_triangle_index = atomicAdd(&out_atomics.triangle_head, workgroup_triangle_count);
        var triangle_index: u32 = 0u;
        loop {
            let out_index = start_triangle_index + triangle_index;
            out_triangles[out_index + 0u] = start_vertex_index + workgroup_triangles[triangle_index + 0u];
            out_triangles[out_index + 1u] = start_vertex_index + workgroup_triangles[triangle_index + 1u];
            out_triangles[out_index + 2u] = start_vertex_index + workgroup_triangles[triangle_index + 2u];

            triangle_index += 3u;
            if (triangle_index >= workgroup_triangle_count) {
                break;
            }
        }
    }
}
