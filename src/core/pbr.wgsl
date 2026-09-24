// glTF metallic-roughness PBR. See docs/pbr-shader.md.
//
// Direct light: Cook-Torrance (GGX, Smith-Schlick, Schlick Fresnel) from
// suns and point lights. Indirect light: ambient + environment cube map
// (split-sum approximation). Output is linear; the sRGB render target encodes it.

const PI: f32 = 3.14159265;
// Must match MAX_SUNS / MAX_POINT_LIGHTS in src/lights/mod.rs.
const MAX_SUNS: u32 = 4u;
const MAX_POINT_LIGHTS: u32 = 8u;

struct Sun {
  direction: vec4<f32>, // xyz: direction light travels (world)
  color: vec4<f32>,     // rgb: color * intensity
};

struct PointLight {
  position: vec4<f32>,  // xyz: world position, w: range
  color: vec4<f32>,     // rgb: color * intensity
};

// Group 0: per frame, owned by the renderer.
struct Frame {
  view_proj: mat4x4<f32>,
  camera_position: vec4<f32>,
  ambient: vec4<f32>,     // rgb: sum of ambient lights
  counts: vec4<f32>,      // x: suns, y: point lights, z: environment max mip
  suns: array<Sun, MAX_SUNS>,
  points: array<PointLight, MAX_POINT_LIGHTS>,
};

// Group 1: per mesh.
struct Object {
  model: mat4x4<f32>,     // uniform scale only
  base_color: vec4<f32>,
  params: vec4<f32>,      // metallic, roughness, normal_scale, occlusion_strength
  emissive: vec4<f32>,    // rgb: emissive factor
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var env_sampler: sampler;
@group(0) @binding(2) var environment: texture_cube<f32>;

@group(1) @binding(0) var<uniform> object: Object;
@group(1) @binding(1) var tex_sampler: sampler;
@group(1) @binding(2) var base_color_texture: texture_2d<f32>;         // sRGB
@group(1) @binding(3) var normal_texture: texture_2d<f32>;             // tangent space
@group(1) @binding(4) var metallic_roughness_texture: texture_2d<f32>; // G roughness, B metallic
@group(1) @binding(5) var occlusion_texture: texture_2d<f32>;          // R
@group(1) @binding(6) var emissive_texture: texture_2d<f32>;           // sRGB

struct VertexInput {
  @location(0) position: vec3<f32>,
  @location(1) normal: vec3<f32>,
  @location(2) tangent: vec4<f32>,  // w: bitangent sign; zero = no normal mapping
  @location(3) uv: vec2<f32>,
  @location(4) color: vec3<f32>,
};

struct VertexOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(0) world_position: vec3<f32>,
  @location(1) normal: vec3<f32>,
  @location(2) tangent: vec4<f32>,
  @location(3) uv: vec2<f32>,
  @location(4) color: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
  let world = object.model * vec4(in.position, 1.0);
  var out: VertexOutput;
  out.clip_position = frame.view_proj * world;
  out.world_position = world.xyz;
  // Uniform scale: the model matrix rotates directions, scale is normalized away.
  out.normal = (object.model * vec4(in.normal, 0.0)).xyz;
  out.tangent = vec4((object.model * vec4(in.tangent.xyz, 0.0)).xyz, in.tangent.w);
  out.uv = in.uv;
  out.color = in.color;
  return out;
}

// GGX / Trowbridge-Reitz normal distribution. `a` = roughness².
fn distribution_ggx(n_dot_h: f32, a: f32) -> f32 {
  let a2 = a * a;
  let d = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
  return a2 / (PI * d * d);
}

// Smith geometry term with Schlick-GGX, k for direct lighting.
fn geometry_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
  let r = roughness + 1.0;
  let k = r * r / 8.0;
  let g_v = n_dot_v / (n_dot_v * (1.0 - k) + k);
  let g_l = n_dot_l / (n_dot_l * (1.0 - k) + k);
  return g_v * g_l;
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
  return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

// Analytic fit of the split-sum environment BRDF (Karis, "Physically Based
// Shading on Mobile"); replaces a BRDF lookup texture.
fn env_brdf(f0: vec3<f32>, roughness: f32, n_dot_v: f32) -> vec3<f32> {
  let c0 = vec4(-1.0, -0.0275, -0.572, 0.022);
  let c1 = vec4(1.0, 0.0425, 1.04, -0.04);
  let r = roughness * c0 + c1;
  let a004 = min(r.x * r.x, exp2(-9.28 * n_dot_v)) * r.x + r.y;
  let ab = vec2(-1.04, 1.04) * a004 + r.zw;
  return f0 * ab.x + ab.y;
}

// Outgoing radiance toward `v` from light arriving along `l` with `radiance`.
fn direct(
  n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, radiance: vec3<f32>,
  albedo: vec3<f32>, metallic: f32, roughness: f32, f0: vec3<f32>,
) -> vec3<f32> {
  let n_dot_l = max(dot(n, l), 0.0);
  if n_dot_l <= 0.0 {
    return vec3(0.0);
  }
  let h = normalize(v + l);
  let n_dot_v = max(dot(n, v), 1e-4);
  let f = fresnel_schlick(max(dot(h, v), 0.0), f0);
  let d = distribution_ggx(max(dot(n, h), 0.0), roughness * roughness);
  let g = geometry_smith(n_dot_v, n_dot_l, roughness);
  let specular = d * g * f / (4.0 * n_dot_v * n_dot_l + 1e-4);
  let k_d = (1.0 - f) * (1.0 - metallic);
  return (k_d * albedo / PI + specular) * radiance * n_dot_l;
}

// glTF KHR_lights_punctual range falloff: inverse square, windowed to 0 at range.
fn attenuation(distance: f32, range: f32) -> f32 {
  let window = clamp(1.0 - pow(distance / range, 4.0), 0.0, 1.0);
  return window * window / max(distance * distance, 1e-4);
}

// ACES filmic tone mapping (Narkowicz fit): HDR -> [0, 1].
fn tone_map(x: vec3<f32>) -> vec3<f32> {
  return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3(0.0), vec3(1.0));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
  // Sample every map up front: textureSample needs uniform control flow.
  let base_sample = textureSample(base_color_texture, tex_sampler, in.uv);
  let normal_sample = textureSample(normal_texture, tex_sampler, in.uv).xyz;
  let mr_sample = textureSample(metallic_roughness_texture, tex_sampler, in.uv);
  let occlusion_sample = textureSample(occlusion_texture, tex_sampler, in.uv).r;
  let emissive_sample = textureSample(emissive_texture, tex_sampler, in.uv).rgb;

  let base = base_sample * object.base_color * vec4(in.color, 1.0);
  let albedo = base.rgb;
  let metallic = clamp(object.params.x * mr_sample.b, 0.0, 1.0);
  let roughness = clamp(object.params.y * mr_sample.g, 0.04, 1.0);
  let occlusion = mix(1.0, occlusion_sample, object.params.w);

  // Normal: perturb by the normal map in tangent space, if a tangent exists.
  let geometric_n = normalize(in.normal);
  let t = in.tangent.xyz - geometric_n * dot(geometric_n, in.tangent.xyz);
  let has_tangent = dot(t, t) > 1e-12;
  let tangent = normalize(select(vec3(1.0, 0.0, 0.0), t, has_tangent));
  let bitangent = cross(geometric_n, tangent) * select(1.0, in.tangent.w, has_tangent);
  let local = (normal_sample * 2.0 - 1.0) * vec3(object.params.z, object.params.z, 1.0);
  let mapped_n = normalize(mat3x3(tangent, bitangent, geometric_n) * local);
  let n = select(geometric_n, mapped_n, has_tangent);

  let v = normalize(frame.camera_position.xyz - in.world_position);
  let n_dot_v = max(dot(n, v), 1e-4);
  let f0 = mix(vec3(0.04), albedo, metallic);
  let diffuse_color = albedo * (1.0 - metallic);

  // Direct lighting.
  var color = vec3(0.0);
  for (var i = 0u; i < min(u32(frame.counts.x), MAX_SUNS); i++) {
    let sun = frame.suns[i];
    color += direct(n, v, -normalize(sun.direction.xyz), sun.color.rgb,
                    albedo, metallic, roughness, f0);
  }
  for (var i = 0u; i < min(u32(frame.counts.y), MAX_POINT_LIGHTS); i++) {
    let light = frame.points[i];
    let to_light = light.position.xyz - in.world_position;
    let distance = length(to_light);
    let radiance = light.color.rgb * attenuation(distance, light.position.w);
    color += direct(n, v, to_light / distance, radiance, albedo, metallic, roughness, f0);
  }

  // Indirect lighting, darkened by occlusion.
  // Environment: rougher surfaces read blurrier mips. The smallest mip
  // approximates diffuse irradiance around the normal.
  let max_mip = frame.counts.z;
  let r = reflect(-v, n);
  let env_specular = textureSampleLevel(environment, env_sampler, r, roughness * max_mip).rgb;
  let env_diffuse = textureSampleLevel(environment, env_sampler, n, max_mip).rgb;
  let indirect = (env_diffuse + frame.ambient.rgb) * diffuse_color
    + env_specular * env_brdf(f0, roughness, n_dot_v);
  color += indirect * occlusion;

  color += emissive_sample * object.emissive.rgb;

  return vec4(tone_map(color), base.a);
}
