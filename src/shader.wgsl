struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
    //@location(1) tex_coords: vec2<f32>
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
    //@location(0) tex_coords: vec2<f32>
};

@group(0) @binding(0)
var<uniform> model_view_proj: mat4x4<f32>;

@vertex
fn vs_main(input: VertexInput ) -> VertexOutput {
    var output: VertexOutput;
    output.position = model_view_proj * vec4<f32>(input.position, 1.0);
    output.color = input.color;
    //result.tex_coords = input.tex_coords;
    return output;
}

@fragment
fn fs_main(output: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(output.color, 1.0);
}

