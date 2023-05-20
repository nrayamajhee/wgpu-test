struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>
};

@group(0) @binding(0)
var<uniform> view_proj: mat4x4<f32>;

@group(0) @binding(1)
var<uniform> model: mat4x4<f32>;

@vertex
fn vs_main(
    vertex: VertexInput,
) -> VertexOutput {
    var result: VertexOutput;
    result.position = view_proj * model * vec4<f32>(vertex.position, 1.0);
    result.tex_coords = vertex.tex_coords;
    return result;
}

@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;

@group(1) @binding(1)
var s_diffuse: sampler;


@fragment
fn fs_main(result: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, result.tex_coords);
}

