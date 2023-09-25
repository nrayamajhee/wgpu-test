struct VertexInput {
  @location(0) position: vec3<f32>,
  @location(1) vertex_colors: vec3<f32>,
  @location(2) tex_coords: vec2<f32>,
};

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) vertex_colors: vec3<f32>,
  @location(1) tex_coords: vec2<f32>,
  @location(2) frag_position: vec3<f32>,
  @location(3) @interpolate(flat) color: vec4<f32>,
  @location(4) @interpolate(flat) material_type: u32,
};

@group(0) @binding(0)
var<uniform> model_view_proj: mat4x4<f32>;

struct Material {
  color: vec4<f32>,
  material_type: f32,
}

@group(2) @binding(0)
var<uniform> material: Material;

@vertex
fn vs_main(input: VertexInput ) -> VertexOutput {
  var output: VertexOutput;
  output.position = model_view_proj * vec4<f32>(input.position, 1.0);
  output.vertex_colors = input.vertex_colors;
  output.tex_coords = input.tex_coords;
  output.color = material.color;
  output.material_type = u32(material.material_type);
  output.frag_position = output.position.xyz;
  return output;
}

@group(1) @binding(0)
var tex_sampler: sampler;

@group(1) @binding(1)
var tex_diffuse: texture_2d<f32>;

@group(1) @binding(2)
var tex_cube: texture_cube<f32>;


@fragment
fn fs_main(output: VertexOutput) -> @location(0) vec4<f32> {
  let texel = textureSample(tex_diffuse, tex_sampler, output.tex_coords);
  let cube_texel = textureSample(tex_cube, tex_sampler, output.frag_position.xyz);
  switch output.material_type {
    case 1 {
      return vec4(output.vertex_colors,1.0);
    }
    case 2 {
      let a = texel.a;
      let r = a * texel.r + (1 - a) * output.color.r;
      let g = a * texel.g + (1 - a) * output.color.g;
      let b = a * texel.b + (1 - a) * output.color.b;
      return vec4(r,g,b,1.);
    }
    case 3 {
      return cube_texel;
    }
    case 0, default {
      return output.color;
    }
  } 
}

