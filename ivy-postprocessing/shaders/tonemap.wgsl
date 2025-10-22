struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) clip_position: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var result: VertexOutput;
    let x = i32(vertex_index) / 2;
    let y = i32(vertex_index) & 1;
    let uv = vec2<f32>(
        f32(x) * 2.0,
        f32(y) * 2.0
    );
    result.position = vec4<f32>(
        uv.x * 2.0 - 1.0,
        1.0 - uv.y * 2.0,
        1.0, 1.0
    );
    result.clip_position = result.position;
    result.uv = uv;
    return result;
}

struct ColorGradingUniforms {
    lift: vec3<f32>,
    exposure: f32,
    gamma: vec3<f32>,
    contrast: f32,
    gain: vec3<f32>,
    saturation: f32,
    temperature: f32,
    tint: f32,
};

@group(0) @binding(0)
var source_texture: texture_2d<f32>;

@group(0) @binding(1)
var default_sampler: sampler;

@group(1) @binding(0)
var<uniform> grading: ColorGradingUniforms;

fn reinhard(x: f32) -> f32 {
    return x / (1f + x);
}

const L_white: f32 = 4f;


fn convert_rgb_xyz(rgb: vec3<f32>) -> vec3<f32> {
	// Reference(s):
	// - RGB/XYZ Matrices
	//   https://web.archive.org/web/20191027010220/http://www.brucelindbloom.com/index.html?Eqn_RGB_XYZ_Matrix.html
    var xyz: vec3<f32>;
    xyz.x = dot(vec3(0.4124564, 0.3575761, 0.1804375), rgb);
    xyz.y = dot(vec3(0.2126729, 0.7151522, 0.0721750), rgb);
    xyz.z = dot(vec3(0.0193339, 0.1191920, 0.9503041), rgb);
    return xyz;
}

fn convert_xyz_rgb(xyz: vec3<f32>) -> vec3<f32> {
    var rgb: vec3<f32>;
    rgb.x = dot(vec3(3.2404542, -1.5371385, -0.4985314), xyz);
    rgb.y = dot(vec3(-0.9692660, 1.8760108, 0.0415560), xyz);
    rgb.z = dot(vec3(0.0556434, -0.2040259, 1.0572252), xyz);
    return rgb;
}

fn convert_xyz_yxy(xyz: vec3<f32>) -> vec3<f32> {
	// Reference(s):
	// - XYZ to xyY
	//   https://web.archive.org/web/20191027010144/http://www.brucelindbloom.com/index.html?Eqn_XYZ_to_xyY.html
    let inv = 1.0 / dot(xyz, vec3(1.0, 1.0, 1.0));
    return vec3(xyz.y, xyz.x * inv, xyz.y * inv);
}

fn convert_yxy_xyz(Yxy: vec3<f32>) -> vec3<f32> {
	// Reference(s):
	// - xyY to XYZ
	//   https://web.archive.org/web/20191027010036/http://www.brucelindbloom.com/index.html?Eqn_xyY_to_XYZ.html
    var xyz: vec3<f32>;
    xyz.x = Yxy.x * Yxy.y / Yxy.z;
    xyz.y = Yxy.x;
    xyz.z = Yxy.x * (1.0 - Yxy.y - Yxy.z) / Yxy.z;
    return xyz;
}

fn convert_rgb_yxy(rgb: vec3<f32>) -> vec3<f32> {
    return convert_xyz_yxy(convert_rgb_xyz(rgb));
}

fn convert_yxy_rgb(Yxy: vec3<f32>) -> vec3<f32> {
    return convert_xyz_rgb(convert_yxy_xyz(Yxy));
}

fn reinhard_2(x: f32) -> f32 {
    return (x * (1.0 + x / (L_white * L_white))) / (1.0 + x);
}

fn apply_color_grading(color: vec3<f32>) -> vec3<f32> {
    // Exposure adjustment (EV stops)
    var result = color * pow(2.0, grading.exposure);

    // Professional Lift/Gamma/Gain color correction
    result = result + grading.lift;
    result = pow(max(result, vec3(0.0001)), 1.0 / grading.gamma);
    result = result * grading.gain;

    // Saturation adjustment using luminance
    let luminance = dot(result, vec3(0.2126, 0.7152, 0.0722));
    result = mix(vec3(luminance), result, grading.saturation);

    // Contrast adjustment
    result = (result - 0.5) * grading.contrast + 0.5;

    // White balance (temperature/tint)
    let temp_shift = vec3(-grading.temperature * 0.1, 0.0, grading.temperature * 0.1);
    let tint_shift = vec3(0.0, grading.tint * 0.1, -grading.tint * 0.1);
    result = result + temp_shift + tint_shift;

    return clamp(result, vec3(0.0), vec3(1.0));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = textureSample(source_texture, default_sampler, in.uv).rgb;

    // Apply tonemapping (Reinhard)
    var yxy = convert_rgb_yxy(color);
    let lum = 0.05;
    let lp = yxy.x / (9.6 * lum + 0.0001);
    yxy.x = reinhard_2(lp);
    color = convert_yxy_rgb(yxy);

    // Apply color grading
    color = apply_color_grading(color);

    return vec4(color, 1.0);
}
 
