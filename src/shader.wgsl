struct VertexInput {
  @location(0) position: vec3<f32>,
  @location(1) vertex_colors: vec3<f32>,
  @location(2) tex_coords: vec2<f32>,
};

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) vertex_colors: vec3<f32>,
  @location(1) tex_coords: vec2<f32>,
};

struct Uniforms {
  model_view_proj: mat4x4<f32>,
  color: vec4<f32>,
  material_type: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(1) @binding(0)
var tex_sampler: sampler;

@group(1) @binding(1)
var tex_diffuse: texture_2d<f32>;

@vertex
fn vs_main(input: VertexInput ) -> VertexOutput {
  var output: VertexOutput;
  output.position = uniforms.model_view_proj * vec4<f32>(input.position, 1.0);
  output.vertex_colors = input.vertex_colors;
  output.tex_coords = input.tex_coords;
  return output;
}

@fragment
fn fs_main(output: VertexOutput) -> @location(0) vec4<f32> {
  let texel = textureSample(tex_diffuse, tex_sampler, output.tex_coords);
  if uniforms.material_type == 1. {
      return vec4(output.vertex_colors,1.0);
  }
  if uniforms.material_type == 2. {
      let a = texel.a;
      let r = a * texel.r + (1 - a) * uniforms.color.r;
      let g = a * texel.g + (1 - a) * uniforms.color.g;
      let b = a * texel.b + (1 - a) * uniforms.color.b;
      return vec4(r,g,b,1.);
  }
  return uniforms.color;
}

