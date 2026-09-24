# Cube map (skybox) shader

`src/core/shader_cube.wgsl`, drawn with the cube map pipeline
(`Renderer::pipeline_cubemap`). Kept alongside the new PBR shader.
Overview in [shaders.md](shaders.md); PBR plan in [pbr-shader.md](pbr-shader.md).

## How it works

- **Mesh:** `things::Backdrop` builds an icosphere, scales it by 10000 and
  gives it `Material::cubemap` with six face images from `img/milkyway/`
  (order +x, −x, +y, −y, +z, −z).
- **Texture:** the six images are copied into one 6-layer `rgba8unorm`
  texture and bound as a `texture_cube` view (group 1, with a linear sampler).
- **Matrix:** group 0 holds a single matrix, `projection × camera rotation ×
  model` (`Viewport::view_cube`). Camera translation is left out, so the
  sphere is always centred on the eye and looks infinitely far away.
- **Vertex stage:** outputs the clip position and passes the object-space
  vertex position through.
- **Fragment stage:** samples the cube map with that position as a direction
  vector, so no UVs are needed.
- **Culling:** front faces are culled, because the camera is inside the
  sphere.

## Quirks

- **Unused UVs:** a `tex_coords` attribute is declared and bound, but the
  buffer is empty and the shader never reads it.
- **Depth:** the sphere writes depth at ~10000 units, so anything farther
  away is hidden behind the sky. The usual fix is to output `position.xyww`
  (depth = 1), use `LessEqual`, and draw the skybox last.
- **Colour space:** the textures are `rgba8unorm`. Once the PBR shader
  renders in linear space with sRGB output, switch them to `rgba8unorm-srgb`
  so the sky's colours stay the same.

## Role in PBR

The same cube map will be the environment source for image-based lighting
(phase 5 of the PBR plan): it gets convolved into irradiance and prefiltered
specular maps that the PBR shader samples for ambient light.
