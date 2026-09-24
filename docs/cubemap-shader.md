# Cube map (skybox) shader

`src/core/shader_cube.wgsl`, drawn with the cube map pipeline
(`Renderer::pipeline_cubemap`). Overview in [shaders.md](shaders.md);
PBR in [pbr-shader.md](pbr-shader.md).

## How it works

- **Mesh:** `things::Backdrop` builds an icosphere, scales it by 10000 and
  gives it `Material::cubemap` with six face images from `img/milkyway/`
  (order +x, −x, +y, −y, +z, −z).
- **Texture:** the six images are copied into one 6-layer `rgba8unorm-srgb`
  texture with a full mip chain, bound as a `texture_cube` view.
- **Matrix:** group 0 holds a single matrix, `projection × camera rotation ×
  model` (`Viewport::view_cube`). Camera translation is left out, so the
  sphere is always centred on the eye.
- **Vertex stage:** outputs `position.xyww`, so every sky fragment lands at
  depth 1, the far plane. It passes the object-space position through as a
  direction.
- **Fragment stage:** samples the cube map along that direction at mip 0.
  Explicit LOD avoids blurry seams where screen-space derivatives jump
  between faces.
- **Depth state:** `LessEqual` with no depth writes, so the sky shows only
  where nothing else was drawn, regardless of draw order.
- **Culling:** front faces are culled, because the camera is inside the
  sphere.

## Role in PBR

The first cube map mesh in the scene becomes the **environment** for all
PBR surfaces. `Renderer::render` adopts its texture into PBR bind group 0.
PBR surfaces then sample its mip chain for reflections (mip by roughness)
and diffuse sky light (smallest mip). Until a cube map loads, the
environment is a black 1×1 cube.

## Fixed quirks

| Was | Now |
|---|---|
| Unused UV vertex attribute bound to an empty buffer | removed; position only |
| Sky written at ~10000 units depth, hiding distant geometry | `xyww` depth 1, `LessEqual`, no depth writes |
| `rgba8unorm` texture in a non-linear pipeline | `rgba8unorm-srgb` with an sRGB render target |
| No mipmaps | full mip chain, which also feeds PBR rough reflections |
