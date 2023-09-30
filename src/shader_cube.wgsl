struct VertexInput {
  @location(0) position: vec3<f32>,
  @location(1) tex_coords: vec2<f32>,
};

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) tex_coords: vec2<f32>,
  @location(1) frag_position: vec3<f32>,
};

@group(0) @binding(0)
var<uniform> view_proj: mat4x4<f32>;

@group(1) @binding(0)
var tex_sampler: sampler;

@group(1) @binding(1)
var tex_cube: texture_cube<f32>;

@vertex
fn vs_main(input: VertexInput ) -> VertexOutput {
  var output: VertexOutput;
  output.position = view_proj * vec4<f32>(input.position, 1.0);
  output.tex_coords = input.tex_coords;
  output.frag_position = input.position;
  return output;
}

@fragment
fn fs_main(output: VertexOutput) -> @location(0) vec4<f32> {
  return textureSample(tex_cube, tex_sampler, output.frag_position);
}

