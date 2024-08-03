@group(0) @binding(0)
var<uniform> dimensions: vec3<u32>;

@group(0) @binding(1)
var input: texture_depth_multisampled_2d;

@group(0) @binding(2)
var output: texture_storage_2d<r32float, write>;

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let width = dimensions.x;
    let height = dimensions.y;
    let sample_count = dimensions.z;

    var total = 1.0;

    let coords = vec2(id.x, id.y);

    for (var i = 0u; i < sample_count; i++) {
        let value = textureLoad(input, coords, i32(i));
        total = min(total, value);
    }

    textureStore(output, coords, vec4(total));
}
