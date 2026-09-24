//! Engine core: rendering, scene graph, camera and shared app state.
//!
//! Everything else in the crate builds on these types.

pub mod app_state;
pub mod camera_controls;
pub mod geometry;
pub mod gltf;
pub mod group;
pub mod keyboard;
pub mod material;
pub mod mesh;
pub mod renderer;
pub mod scene;
pub mod textures;
pub mod viewport;

pub use app_state::AppState;
pub use geometry::Geometry;
pub use group::Group;
pub use keyboard::Keyboard;
pub use material::{Material, MaterialType, TextureSource};
pub use mesh::Mesh;
pub use renderer::{Color, Rect, Renderer};
pub use scene::Scene;
pub use viewport::Viewport;
