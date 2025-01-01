struct CullData {
    view: mat4x4<f32>,
    frustum: vec4<f32>,
    znear: f32,
    zfar: f32,
    object_count: u32,
}

// Individual object to draw
struct DrawObject {
    object_index: u32,
    batch_id: u32,
    radius: f32,
    unknown: u32,
    unknown2: u32,
}

// struct Entity {
//     index: u32,
//     gen: u16,
//     kind: u16,
// }

struct IndirectDrawCommand {
    index_count: u32,
    instance_count: atomic<u32>,
    first_index: u32,
    base_vertex: u32,
    first_instance: u32,
}

struct ObjectData {
    world_matrix: mat4x4<f32>,
    color: vec3<f32>,
    joint_offset: u32,
}

@group(0) @binding(0)
var<uniform> cull_data: CullData;

@group(0) @binding(1)
var<storage> object_data: array<ObjectData>;

@group(0) @binding(2)
var<storage, read> draws: array<DrawObject>;

@group(0) @binding(3)
var<storage, read_write> indirect_draws: array<IndirectDrawCommand>;

// Remaps a draw instance index to the object id
// Batch compaction
@group(0) @binding(4)
var<storage, read_write> object_id_indirection: array<u32>;

const invsq3: f32 = 0.57735026919f;
fn is_visible(draw: DrawObject) -> bool {
    let object = object_data[draw.object_index];

    var visible = true;

    let radius = draw.radius * length((object.world_matrix * vec4(invsq3, invsq3, invsq3, 0f)).xyz);

    let position = (object.world_matrix * vec4(0f, 0f, 0f, 1f)).xyz;
    let center = (cull_data.view * vec4(position, 1f)).xyz;
    visible = visible && center.z * cull_data.frustum[1] - abs(center.x) * cull_data.frustum[0] > -radius;
    visible = visible && center.z * cull_data.frustum[3] - abs(center.y) * cull_data.frustum[2] > -radius;

    visible = visible && center.z - radius < -cull_data.znear && center.z + radius > -cull_data.zfar;

    return visible;
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let draw_index = gid.x;

    if draw_index < cull_data.object_count {
        let draw = draws[draw_index];

        if is_visible(draw) {
            let object = object_data[draw.object_index];

            let instanceCount = atomicAdd(&indirect_draws[draw.batch_id].instance_count, 1u);

            // 1 indirect draw per batch
            let baseInstance = indirect_draws[draw.batch_id].first_instance;
            object_id_indirection[baseInstance + instanceCount] = draw.object_index;
        }
    }
}
