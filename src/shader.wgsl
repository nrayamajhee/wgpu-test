struct VertexInput {
  @location(0) position: vec3<f32>,
  @location(1) vertex_colors: vec3<f32>,
  @location(2) tex_coords: vec2<f32>
};

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) vertex_colors: vec3<f32>,
  @location(1) tex_coords: vec2<f32>
};

@group(0) @binding(0)
var<uniform> model_view_proj: mat4x4<f32>;

@vertex
fn vs_main(input: VertexInput ) -> VertexOutput {
  var output: VertexOutput;
  output.position = model_view_proj * vec4<f32>(input.position, 1.0);
  output.vertex_colors = input.vertex_colors;
  output.tex_coords = input.tex_coords;
  return output;
}

@group(1) @binding(0)
var tex_sampler: sampler;

@group(1) @binding(1)
var tex_diffuse: texture_2d<f32>;


@fragment
fn fs_main(output: VertexOutput) -> @location(0) vec4<f32> {
  return textureSample(tex_diffuse, tex_sampler, output.tex_coords);
}

