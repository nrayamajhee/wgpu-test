# PBR shader plan

Replace the current default shader (`src/core/shader.wgsl`) with a
physically based shader that supports the glTF 2.0 metallic-roughness
material, and make it the default pipeline. The current shader and its
limitations are summarised in [shaders.md](shaders.md). The skybox keeps its
own shader; see [cubemap-shader.md](cubemap-shader.md).

## Proposal: `MaterialType::Pbr`

One pipeline and one shader with no material branches. The old material
types become PBR parameter presets, so existing callers keep working:

| Old | PBR equivalent |
|---|---|
| `Color` | `base_color_factor = color`, all textures default |
| `VertexColor` | base color × `COLOR_0` vertex attribute (glTF semantics) |
| `Textured` | base color texture × factor |

An `unlit` flag (glTF `KHR_materials_unlit`) keeps the current flat look
where it's wanted.

### Material inputs (glTF metallic-roughness)

| Input | Texture | Channels | Colour space | Default (1×1) |
|---|---|---|---|---|
| Albedo / base color | `baseColorTexture` × `baseColorFactor` | RGBA | sRGB | white |
| Normal map | `normalTexture` × `scale` | RGB (tangent space) | linear | (0.5, 0.5, 1) flat |
| Roughness | `metallicRoughnessTexture` × `roughnessFactor` | G | linear | white |
| Metallic mask | same texture × `metallicFactor` | B | linear | white |
| Occlusion *(optional)* | `occlusionTexture` × `strength` | R | linear | white |
| Emissive *(optional)* | `emissiveTexture` × `emissiveFactor` | RGB | sRGB | black |

glTF packs roughness and metallic into one texture. If a source has separate
roughness and metallic maps, pack them into G/B at load time so the shader
only ever handles one layout.

Every slot is always bound, using the 1×1 default when a map is missing.
That keeps a single bind group layout and a single pipeline, with no
`if has_texture` branches in the shader.

### Vertex data

| Attribute | Source |
|---|---|
| position | as now |
| normal | glTF `NORMAL`; genmesh normals; otherwise averaged face normals |
| tangent (`vec4`, w = handedness) | glTF `TANGENT`; otherwise generated with MikkTSpace (`mikktspace` crate) |
| uv0 | glTF `TEXCOORD_0`; otherwise zeros |
| color | glTF `COLOR_0`; otherwise white |

Every attribute gets a real buffer, filled with defaults when the source
doesn't provide it. That fixes the empty-buffer issue.

### Bind groups

| Group | Frequency | Contents |
|---|---|---|
| 0 | per frame | view-projection, camera position, sun direction/colour, ambient colour |
| 1 | per material | factors (base color, metallic, roughness, normal scale, emissive, flags) + textures + sampler |
| 2 | per object | model matrix, normal matrix |

Splitting by update frequency means materials can be shared between meshes
and uploaded once. Transforms are uniform-scale `Similarity3`s, so the
normal matrix is just the rotation part.

### How the shader works

**Vertex stage**
1. Transform position, normal and tangent to world space. Rebuild the
   bitangent as `cross(N, T) * T.w`.
2. Pass world position, TBN, UV and color to the fragment stage.

**Fragment stage**
1. **Sample:** read all maps and multiply by their factors. Base color is
   also multiplied by vertex color. sRGB textures are decoded by the texture
   format (`rgba8unorm-srgb`), so all math is in linear space.
2. **Normal:** `N = normalize(TBN * (sample * 2 - 1) * vec3(scale, scale, 1))`.
3. **Surface parameters:** `roughness` is clamped to at least 0.04 to avoid
   specular singularities. Set `F0 = mix(0.04, albedo, metallic)` and
   `diffuse = albedo * (1 - metallic)`.
4. **Direct light (sun):** a Cook-Torrance BRDF:
   - **D:** GGX / Trowbridge-Reitz normal distribution.
   - **G:** Smith–Schlick-GGX geometry term.
   - **F:** Schlick Fresnel.
   - **Result:** `(kD * diffuse / π + D·G·F / (4·NdotL·NdotV)) * light * NdotL`,
     where `kD = 1 - F`.
5. **Ambient:** a constant colour × `diffuse` × occlusion for now. This is
   later replaced by image-based lighting (below).
6. **Emissive:** add it after lighting.
7. **Output:** tone map (ACES or Reinhard), then encode to sRGB. Either
   configure the canvas with an `-srgb` view format or apply gamma in the
   shader. Skip lighting and tone mapping when `unlit` is set.

### Loader changes (`Geometry::from_gltf`)

- **Attributes:** read normals, tangents, the texture's own `texCoord` set,
  and colors.
- **Material:** read the metallic-roughness, normal, occlusion and emissive
  textures plus their factors.
- **Sampler:** honour the glTF sampler (wrap and filter modes).
- **Texture cache:** cache GPU textures by image index so primitives share
  uploads, and mark each texture sRGB or linear based on its slot.
- **Alpha:** `alphaMode` (`MASK` cutoff, `BLEND`) is out of scope initially,
  and treated as `OPAQUE`.

## Phases

1. **Lighting:** normals, a sun and a Lambert-only PBR pipeline using
   factors. Make it the default pipeline and map the old material types onto
   it.
2. **Textures:** base color and metallic-roughness textures, sRGB-correct
   formats, and tone mapping.
3. **Normal mapping:** tangents, including MikkTSpace fallback generation.
4. **Extras:** occlusion, emissive and the texture cache. Generate mipmaps
   too; WebGPU has no built-in mip generation, so roughness and normal maps
   alias without them.
5. **Image-based lighting:** use the skybox cubemap to precompute a diffuse
   irradiance map, a specular prefiltered map and a BRDF lookup texture.

Verify each phase against the Khronos glTF sample models (`MetalRoughSpheres`,
`NormalTangentTest`, `DamagedHelmet`) placed in `gltf/`.
