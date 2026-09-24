// Downsamples one mip level into the next: a fullscreen triangle samples the
// previous level with linear filtering (a 2x2 box filter). See textures.rs.

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

// Vertices 0..3 form one triangle covering the whole target.
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
  let uv = vec2(f32((index << 1u) & 2u), f32(index & 2u));
  var out: VertexOutput;
  out.position = vec4(uv * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
  out.uv = uv;
  return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
  return textureSample(source, source_sampler, in.uv);
}
