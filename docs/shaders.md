# Shaders

Overview of the render pipelines. Details live in:

- [pbr-shader.md](pbr-shader.md): the default lit shader (glTF PBR).
- [cubemap-shader.md](cubemap-shader.md): the skybox shader.

## Pipelines

| Pipeline | File | Used for | Culling | Depth |
|---|---|---|---|---|
| PBR | `src/core/pbr.wgsl` | every mesh except the skybox | back faces | `Less`, writes |
| Cube map | `src/core/shader_cube.wgsl` | skybox | front faces | `LessEqual`, no writes, drawn at depth 1 |
| Mipmap | `src/core/mipmap.wgsl` | internal: generates texture mip levels | none | none |

`Renderer::render` picks the pipeline per mesh from its `MaterialType`
(`Pbr` or `CubeMap`). Both drawing pipelines share one render pass and a
`Depth24plusStencil8` depth buffer.

## Color pipeline

- **Inputs:** color textures (base color, emissive, skybox) are
  `rgba8unorm-srgb`, so sampling returns linear values. Data textures
  (normal, metallic-roughness, occlusion) are `rgba8unorm`. All `Color`
  values in Rust are linear.
- **Lighting:** the PBR shader computes linear HDR radiance, then tone maps
  it with ACES.
- **Output:** the canvas is rendered through an `-srgb` view format, so the
  GPU gamma-encodes on write. Shaders never apply gamma themselves.

## Textures

- **Cache:** `Renderer::texture` caches uploads by source and color space.
  Materials that share a URL or an embedded image (`Rc<[u8]>`) share one GPU
  texture.
- **Mipmaps:** every sampled texture gets a full mip chain, generated on the
  GPU by the mipmap pipeline. The shared sampler is trilinear with 8×
  anisotropy.
- **Defaults:** a missing map binds a cached 1×1 texture: white, or flat
  (128, 128, 255) for normal maps.

## Lights

Lights live on scene nodes (`Group::with_light`) and are gathered every
frame into one uniform block (see `src/lights`):

| Light | Placement | Limit |
|---|---|---|
| `Ambient` | none; all ambients are summed | unlimited |
| `Sun` | direction rotated by the node | 4 |
| `PointLight` | node's world position; glTF range falloff | 8 |

Lights beyond the limits are ignored.

## Current scene

| Mesh | Material |
|---|---|
| Skybox (`Backdrop`) | cube map; also the environment every PBR surface reflects |
| Ocean (`World`) | vertex-color albedo, roughness 0.25 |
| Piano keys (`Device`) | ivory / black lacquer dielectric, roughness 0.2; pressed keys turn orange and glow |
| Case (`Device`) | gray metal, roughness 0.35 |

Lit by a warm sun and a faint blue ambient (`World`), plus a point lamp in
front of the piano (`Device`).
