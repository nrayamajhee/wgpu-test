struct VertexInput {
  @location(0) position: vec3<f32>,
  @location(1) vertex_colors: vec3<f32>,
  @location(2) tex_coords: vec2<f32>,
};

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) vertex_colors: vec3<f32>,
  @location(1) tex_coords: vec2<f32>,
  @location(2) @interpolate(flat) color: vec4<f32>,
  @location(3) @interpolate(flat) material_type: u32,
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
  return output;
}

@group(1) @binding(0)
var tex_sampler: sampler;

@group(1) @binding(1)
var tex_diffuse: texture_2d<f32>;


@fragment
fn fs_main(output: VertexOutput) -> @location(0) vec4<f32> {
  let texel = textureSample(tex_diffuse, tex_sampler, output.tex_coords);
  switch output.material_type {
    case 1 {
      return vec4(output.vertex_colors,1.0);
    }
    case 2 {
      return texel;
    }
    case 0, default {
      return output.color;
    }
  } 
}

