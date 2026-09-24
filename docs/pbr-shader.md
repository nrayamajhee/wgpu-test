# PBR shader

`src/core/pbr.wgsl`, the default pipeline for every mesh except the
skybox. It implements the glTF 2.0 metallic-roughness material, lit by scene
lights and the environment cube map. Overview in [shaders.md](shaders.md);
skybox in [cubemap-shader.md](cubemap-shader.md).

## Material inputs

| Input | Texture | Channels | Color space | Missing texture |
|---|---|---|---|---|
| Base color | `base_color_texture` × `color` × vertex color | RGBA | sRGB | white |
| Normal | `normal_texture`, XY × `normal_scale` | RGB (tangent space) | linear | flat |
| Roughness | `metallic_roughness_texture` × `roughness` | G | linear | white |
| Metallic | same texture × `metallic` | B | linear | white |
| Occlusion | `occlusion_texture`, mixed by `occlusion_strength` | R | linear | white |
| Emissive | `emissive_texture` × `emissive` | RGB | sRGB | white (factor defaults to black) |

Set these with `Material::new(color)` plus the `with_*` builders. The glTF
loader fills all of them from the file. `Material::vertex_color` and
`Material::textured` are presets built on the same shader.

## Vertex data

| Slot | Attribute | Fallback when missing |
|---|---|---|
| 0 | position | required |
| 1 | normal | computed (area-weighted face normals) |
| 2 | tangent (`w` = bitangent sign) | computed from UVs if a normal map is used, else zero |
| 3 | uv | zeros |
| 4 | color | white |

Every buffer is full-length, so the shader never reads out of bounds.

## Bind groups

| Group | Owner | Contents |
|---|---|---|
| 0 | `Renderer`, once per frame | `Frame` uniforms (view-projection, camera position, ambient, light counts, environment mip count, suns, point lights), environment sampler, environment cube |
| 1 | `Mesh` | `Object` uniforms (model matrix, base color, metallic/roughness/normal scale/occlusion strength, emissive), sampler, five textures |

The Rust sizes `FRAME_UNIFORM_SIZE` (496 bytes) and `PBR_UNIFORM_SIZE`
(112 bytes) must match the WGSL structs.

## How it works

**Vertex stage:** transforms position, normal and tangent to world space.
Transforms are uniform-scale, so the model matrix works for directions too.

**Fragment stage:**
1. **Sample:** all five maps are sampled up front (texture sampling needs
   uniform control flow), then multiplied by their factors.
2. **Normal:** builds a TBN frame from the interpolated normal and tangent
   and applies the normal map. A zero tangent skips normal mapping.
3. **Surface:** roughness is clamped to at least 0.04. Then
   `F0 = mix(0.04, albedo, metallic)`, and the diffuse color is
   `albedo × (1 − metallic)`.
4. **Direct light:** for each sun and point light, a Cook-Torrance BRDF:
   - **D:** GGX distribution.
   - **G:** Smith–Schlick-GGX.
   - **F:** Schlick Fresnel.
   - Diffuse is weighted by `(1 − F)(1 − metallic)`.
   - Point lights fall off with inverse square, windowed to zero at `range`.
5. **Indirect light**, multiplied by occlusion:
   - **Diffuse:** ambient plus the environment's smallest mip sampled along
     the normal, which approximates irradiance.
   - **Specular:** the environment sampled along the reflection vector, at a
     mip level of `roughness × max_mip`, so rougher surfaces see a blurrier
     sky. It's weighted by an analytic split-sum BRDF (Karis), so no lookup
     texture is needed.
6. **Emissive:** added after lighting.
7. **Output:** ACES tone map, returned as linear. The sRGB target encodes it.

## Approximations

- **Environment blur:** uses the box-filtered mip chain, not a proper GGX
  prefilter or irradiance convolution. Rough reflections are plausible but
  not energy-exact.
- **Shadows:** none.
- **Tangents:** the fallback generator is a simplified MikkTSpace, so normal
  maps may show seams where authoring tools expect exact MikkTSpace.
- **Alpha:** `alphaMode` is ignored and everything is opaque.

## Possible next steps

1. Prefilter the environment into proper GGX specular mips and an
   irradiance map (a compute pass at load time).
2. Shadow maps for the sun.
3. Alpha mask and blend modes.
4. glTF sampler settings and multiple UV sets.
