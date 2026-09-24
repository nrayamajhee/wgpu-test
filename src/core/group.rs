//! Detached, buildable scene content. See [`Group`].

use nalgebra::Similarity3;
use rapier3d::prelude::{Collider, RigidBody};

use crate::core::Mesh;

/// A named tree of scene content, built up front and inserted with
/// [`Scene::add_group`](crate::core::Scene::add_group) or
/// [`Scene::add_group_to`](crate::core::Scene::add_group_to).
///
/// Every part is optional, so one type covers all cases:
/// - empty node (transform only), e.g. a pivot for children
/// - static mesh
/// - rigid body, with or without a mesh and/or collider
/// - static collider without a body
/// - any nesting of the above via children
///
/// # Transforms
/// - `transform` is local to the parent.
/// - A `body`'s position is world space and overrides the transform's
///   translation/rotation; the transform's scale still applies to the mesh.
/// - A collider without a body is placed relative to the group's world
///   transform (scale is ignored).
///
/// # Example
/// ```ignore
/// Group::new("lithosphere")
///   .with_mesh(mesh)
///   .with_body(body)
///   .with_collider(collider)
///   .with_scale(1000.)
///   .with_child(Group::new("moon").with_mesh(moon_mesh));
/// ```
pub struct Group {
  /// Unique node id in the scene.
  pub name: String,
  /// Local transform relative to the parent.
  pub transform: Similarity3<f32>,
  /// Mesh drawn at this node.
  pub mesh: Option<Mesh>,
  /// Rigid body driving this node's transform.
  pub body: Option<RigidBody>,
  /// Collider, attached to `body` if present, otherwise static.
  pub collider: Option<Collider>,
  /// Nested groups.
  pub children: Vec<Group>,
}

impl Group {
  /// Empty group with an identity transform.
  pub fn new(name: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      transform: Similarity3::identity(),
      mesh: None,
      body: None,
      collider: None,
      children: Vec::new(),
    }
  }

  /// The node id this group will be inserted as.
  pub fn name(&self) -> &str {
    &self.name
  }

  /// Sets the local transform (replaces any scale set earlier).
  pub fn with_transform(mut self, transform: Similarity3<f32>) -> Self {
    self.transform = transform;
    self
  }

  /// Sets the uniform scale of the local transform.
  pub fn with_scale(mut self, scale: f32) -> Self {
    self.transform.set_scaling(scale);
    self
  }

  /// Attaches a mesh to draw.
  pub fn with_mesh(mut self, mesh: Mesh) -> Self {
    self.mesh = Some(mesh);
    self
  }

  /// Attaches a rigid body; it will drive this node's transform.
  pub fn with_body(mut self, body: RigidBody) -> Self {
    self.body = Some(body);
    self
  }

  /// Attaches a collider (to the body if one is set).
  pub fn with_collider(mut self, collider: Collider) -> Self {
    self.collider = Some(collider);
    self
  }

  /// Adds a child group (builder form).
  pub fn with_child(mut self, child: Group) -> Self {
    self.children.push(child);
    self
  }

  /// Adds a child group (in-place form, for loops).
  pub fn add_child(&mut self, child: Group) {
    self.children.push(child);
  }
}
