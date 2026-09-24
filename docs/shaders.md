# Shaders

Overview of the render pipelines. Details live in:

- [pbr-shader.md](pbr-shader.md): planned default shader (glTF PBR).
- [cubemap-shader.md](cubemap-shader.md): skybox shader.

## Pipelines

All pipelines share one render pass, a `Depth24plusStencil8` depth buffer
and the canvas' preferred format. `Renderer::render` picks a pipeline per
mesh from its `MaterialType`.

| Pipeline | File | Used for | Culling | Status |
|---|---|---|---|---|
| Default | `src/core/shader.wgsl` | every mesh except the skybox | back faces | to be replaced by PBR |
| Cube map | `src/core/shader_cube.wgsl` | skybox | front faces | kept |

## Default shader (current)

**Unlit**: there are no normals and no lights. It branches on `MaterialType`:

| `MaterialType` | Output | Used by |
|---|---|---|
| `Color` (0) | `uniforms.color` | Device piano keys, glTF without textures |
| `VertexColor` (1) | per-vertex RGB | World ocean |
| `Textured` (2) | texel blended over `color` by texel alpha | glTF base color texture |

- **Vertex inputs:** position (slot 0), vertex color (slot 1), UV (slot 2).
  Unused slots are bound to empty buffers.
- **Group 0:** a 96-byte uniform holding the MVP matrix, color and material
  type.
- **Group 1:** a sampler and one 2D texture (a 1×1 placeholder when untextured).

### Limitations

- **No lighting:** shapes read as flat silhouettes. Adjacent piano keys merge
  into one colour, for example.
- **Wrong texture blend:** glTF says base color = factor × texture, but the
  shader blends by alpha instead.
- **Colour space:** textures are `rgba8unorm` and output isn't gamma-encoded,
  so any lighting math would be done in the wrong colour space.
- **Empty vertex buffers:** reading from them is out of bounds. WebGPU returns
  zeros, which happens to work but is fragile.
- **Duplicate uploads:** every glTF primitive uploads its own copy of the
  texture, even when primitives share an image.

## Cube map shader (current)

Draws the skybox by sampling a cube texture in the direction of each pixel,
using camera rotation only so the sky never moves with the camera. See
[cubemap-shader.md](cubemap-shader.md).

## Target

| Pipeline | Shader | Used for |
|---|---|---|
| **PBR** (new default) | glTF metallic-roughness, lit | every mesh except the skybox; `Color`, `VertexColor` and `Textured` become PBR presets |
| Cube map | unchanged | skybox, and later the environment source for PBR image-based lighting |
