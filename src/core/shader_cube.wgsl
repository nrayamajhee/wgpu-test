// Skybox. See docs/cubemap-shader.md.
//
// The matrix has no camera translation, so the mesh stays centred on the eye.
// Depth is forced to 1 (far plane): the sky is drawn only where nothing else is.

@group(0) @binding(0)
var<uniform> view_proj: mat4x4<f32>;

@group(1) @binding(0)
var tex_sampler: sampler;

@group(1) @binding(1)
var tex_cube: texture_cube<f32>;

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) direction: vec3<f32>,
};

@vertex
fn vs_main(@location(0) position: vec3<f32>) -> VertexOutput {
  var out: VertexOutput;
  // z = w gives depth 1 after the perspective divide.
  out.position = (view_proj * vec4(position, 1.0)).xyww;
  out.direction = position;
  return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
  // Sample the top mip: without explicit LOD, derivatives across the
  // cube's face seams would pick blurry mips.
  return textureSampleLevel(tex_cube, tex_sampler, in.direction, 0.0);
}
