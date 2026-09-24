//! Scene graph and physics world.
//!
//! Nodes are keyed by unique string ids. Each node may have a local
//! transform, a [`Mesh`], a [`Light`], a rigid body and a collider. Content is added via
//! [`Group`]s; see [`Scene::add_group`].

use std::collections::HashMap;

use crate::core::{Group, Mesh};
use crate::lights::Light;
use nalgebra::{vector, Similarity3};
use rapier3d::{
  dynamics::RigidBodyHandle,
  geometry::BroadPhaseMultiSap,
  prelude::{
    CCDSolver, ColliderHandle, ColliderSet, ImpulseJointSet, IntegrationParameters, IslandManager,
    MultibodyJointSet, NarrowPhase, PhysicsPipeline, RigidBody, RigidBodySet,
  },
};
use wasm_bindgen::JsValue;

/// Parent/child links of a scene node. Data lives in [`Scene`]'s side tables.
pub struct SceneNode {
  /// Unique node id.
  pub id: String,
  /// Parent id; `None` for roots.
  pub parent: Option<String>,
  /// Child ids, in insertion order.
  pub children: Vec<String>,
}

/// The scene graph plus the Rapier physics world driving its bodies.
///
/// Per-node data is stored in id-keyed tables; only `transforms` is required.
pub struct Scene {
  /// Hierarchy of all nodes.
  nodes: HashMap<String, SceneNode>,
  /// Local transforms, relative to the parent node.
  transforms: HashMap<String, Similarity3<f32>>,
  /// Renderable meshes, drawn at the node's world transform.
  meshes: HashMap<String, Mesh>,
  /// Light sources, placed by the node's world transform.
  lights: HashMap<String, Light>,
  /// Rigid body of each physics-driven node.
  r_handles: HashMap<String, RigidBodyHandle>,
  /// Collider of each node that has one.
  c_handles: HashMap<String, ColliderHandle>,
  /// Rapier: all rigid bodies.
  rigid_body_set: RigidBodySet,
  /// Rapier: all colliders.
  collider_set: ColliderSet,
  /// Rapier: timestep and solver settings.
  integration_parameters: IntegrationParameters,
  /// Rapier: runs a simulation step.
  physics_pipeline: PhysicsPipeline,
  /// Rapier: tracks active/sleeping body islands.
  island_manager: IslandManager,
  /// Rapier: coarse collision detection.
  broad_phase: BroadPhaseMultiSap,
  /// Rapier: exact contact generation.
  narrow_phase: NarrowPhase,
  /// Rapier: impulse-based joints.
  impulse_joint_set: ImpulseJointSet,
  /// Rapier: reduced-coordinate joints.
  multibody_joint_set: MultibodyJointSet,
  /// Rapier: continuous collision detection.
  ccd_solver: CCDSolver,
}

impl Default for Scene {
  fn default() -> Self {
    Self::new()
  }
}

impl Scene {
  /// Creates an empty scene and physics world.
  pub fn new() -> Self {
    Self {
      nodes: HashMap::new(),
      transforms: HashMap::new(),
      meshes: HashMap::new(),
      lights: HashMap::new(),
      r_handles: HashMap::new(),
      c_handles: HashMap::new(),
      rigid_body_set: RigidBodySet::new(),
      collider_set: ColliderSet::new(),
      integration_parameters: IntegrationParameters::default(),
      physics_pipeline: PhysicsPipeline::new(),
      island_manager: IslandManager::new(),
      broad_phase: BroadPhaseMultiSap::new(),
      narrow_phase: NarrowPhase::new(),
      impulse_joint_set: ImpulseJointSet::new(),
      multibody_joint_set: MultibodyJointSet::new(),
      ccd_solver: CCDSolver::new(),
    }
  }

  /// Adds a group (and all its children) as a root of the scene.
  ///
  /// # Errors
  /// If any node id in the group already exists.
  pub fn add_group(&mut self, group: Group) -> Result<(), JsValue> {
    self.insert_group(group, None, Similarity3::identity())
  }

  /// Adds a group (and all its children) under an existing node.
  ///
  /// # Errors
  /// If `parent` doesn't exist or any node id in the group already exists.
  pub fn add_group_to(&mut self, parent: &str, group: Group) -> Result<(), JsValue> {
    if !self.nodes.contains_key(parent) {
      return Err(JsValue::from_str(&format!(
        "Scene::add_group_to: parent node '{parent}' does not exist"
      )));
    }
    let parent_world = self.world_transform(parent).unwrap_or_else(Similarity3::identity);
    self.insert_group(group, Some(parent), parent_world)
  }

  /// Recursively inserts `group` under `parent`, registering its mesh,
  /// light, body and collider. `parent_world` is the parent's world transform, used to
  /// convert world-space bodies/colliders to and from local space.
  fn insert_group(
    &mut self,
    group: Group,
    parent: Option<&str>,
    parent_world: Similarity3<f32>,
  ) -> Result<(), JsValue> {
    let Group {
      name,
      transform,
      mesh,
      body,
      collider,
      light,
      children,
    } = group;

    if self.nodes.contains_key(&name) {
      return Err(JsValue::from_str(&format!(
        "Scene: node '{name}' already exists"
      )));
    }

    let mut local = transform;
    let mut world = parent_world * local;

    let r_handle = body.map(|body| {
      let handle = self.rigid_body_set.insert(body);
      // Bodies live in world space: derive the local transform from them.
      world = Similarity3::from_isometry(*self.rigid_body_set[handle].position(), world.scaling());
      local = parent_world.inverse() * world;
      handle
    });

    let c_handle = collider.map(|mut collider| match r_handle {
      Some(r_handle) => {
        self
          .collider_set
          .insert_with_parent(collider, r_handle, &mut self.rigid_body_set)
      }
      None => {
        collider.set_position(world.isometry * collider.position());
        self.collider_set.insert(collider)
      }
    });

    self.nodes.insert(
      name.clone(),
      SceneNode {
        id: name.clone(),
        parent: parent.map(str::to_owned),
        children: Vec::new(),
      },
    );
    if let Some(parent) = parent.and_then(|p| self.nodes.get_mut(p)) {
      parent.children.push(name.clone());
    }
    self.transforms.insert(name.clone(), local);
    if let Some(mesh) = mesh {
      self.meshes.insert(name.clone(), mesh);
    }
    if let Some(light) = light {
      self.lights.insert(name.clone(), light);
    }
    if let Some(handle) = r_handle {
      self.r_handles.insert(name.clone(), handle);
    }
    if let Some(handle) = c_handle {
      self.c_handles.insert(name.clone(), handle);
    }

    for child in children {
      self.insert_group(child, Some(&name), world)?;
    }
    Ok(())
  }

  /// All meshes paired with their world transforms, for rendering.
  pub fn meshes(&self) -> Vec<(&Mesh, Similarity3<f32>)> {
    Self::collect(&self.meshes, self.world_transforms())
  }

  /// All lights paired with their world transforms, for rendering.
  pub fn lights(&self) -> Vec<(&Light, Similarity3<f32>)> {
    Self::collect(&self.lights, self.world_transforms())
  }

  /// Pairs each entry of `table` with its node's world transform.
  fn collect<'a, T>(
    table: &'a HashMap<String, T>,
    world: HashMap<&str, Similarity3<f32>>,
  ) -> Vec<(&'a T, Similarity3<f32>)> {
    table
      .iter()
      .filter_map(|(id, item)| Some((item, *world.get(id.as_str())?)))
      .collect()
  }

  /// World transform of every node, computed in one top-down pass.
  fn world_transforms(&self) -> HashMap<&str, Similarity3<f32>> {
    let mut out = HashMap::with_capacity(self.nodes.len());
    let mut stack: Vec<(&str, Similarity3<f32>)> = self
      .nodes
      .values()
      .filter(|n| n.parent.is_none())
      .map(|n| (n.id.as_str(), Similarity3::identity()))
      .collect();
    while let Some((id, parent_world)) = stack.pop() {
      let local = self
        .transforms
        .get(id)
        .copied()
        .unwrap_or_else(Similarity3::identity);
      let world = parent_world * local;
      out.insert(id, world);
      if let Some(node) = self.nodes.get(id) {
        stack.extend(node.children.iter().map(|c| (c.as_str(), world)));
      }
    }
    out
  }

  /// Number of ancestors of `id` (roots are 0).
  fn depth(&self, id: &str) -> usize {
    let mut depth = 0;
    let mut current = self.nodes.get(id).and_then(|n| n.parent.as_deref());
    while let Some(parent) = current {
      depth += 1;
      current = self.nodes.get(parent).and_then(|n| n.parent.as_deref());
    }
    depth
  }

  /// Copies rigid body positions back into node transforms.
  /// Parents are processed before children so nested bodies stay correct.
  pub fn sync_transforms(&mut self) {
    let mut ids: Vec<String> = self.r_handles.keys().cloned().collect();
    ids.sort_by_key(|id| self.depth(id));

    for id in ids {
      let Some(body) = self.r_handles.get(&id).and_then(|h| self.rigid_body_set.get(*h)) else {
        continue;
      };
      let parent_world = self
        .nodes
        .get(&id)
        .and_then(|n| n.parent.as_deref())
        .and_then(|p| self.world_transform(p))
        .unwrap_or_else(Similarity3::identity);
      let local_scale = self.transforms.get(&id).map(|t| t.scaling()).unwrap_or(1.);
      let world =
        Similarity3::from_isometry(*body.position(), parent_world.scaling() * local_scale);
      self.transforms.insert(id, parent_world.inverse() * world);
    }
  }

  /// Advances the physics simulation by one step (zero gravity).
  /// Call [`Scene::sync_transforms`] afterwards to update the scene graph.
  pub fn physics(&mut self) {
    self.physics_pipeline.step(
      &vector![0., 0., 0.],
      &self.integration_parameters,
      &mut self.island_manager,
      &mut self.broad_phase,
      &mut self.narrow_phase,
      &mut self.rigid_body_set,
      &mut self.collider_set,
      &mut self.impulse_joint_set,
      &mut self.multibody_joint_set,
      &mut self.ccd_solver,
      None,
      &(),
      &(),
    );
  }

  /// Transform of a node relative to its parent.
  pub fn local_transform(&self, key: &str) -> Option<Similarity3<f32>> {
    self.transforms.get(key).copied()
  }

  /// Transform of a node in world space.
  pub fn world_transform(&self, key: &str) -> Option<Similarity3<f32>> {
    let mut world = self.local_transform(key)?;
    let mut current = self.nodes.get(key)?.parent.as_deref();
    while let Some(parent) = current {
      world = self.local_transform(parent)? * world;
      current = self.nodes.get(parent)?.parent.as_deref();
    }
    Some(world)
  }

  /// Mutable mesh of node `key`, e.g. to change its color.
  pub fn get_mesh_mut(&mut self, key: &str) -> Option<&mut Mesh> {
    self.meshes.get_mut(key)
  }

  /// Rigid body attached to node `key`, if any.
  pub fn get_body(&self, key: &str) -> Option<&RigidBody> {
    let handle = self.r_handles.get(key)?;
    self.rigid_body_set.get(*handle)
  }

  /// Mutable rigid body attached to node `key`, e.g. to apply impulses.
  pub fn get_body_mut(&mut self, key: &str) -> Option<&mut RigidBody> {
    let handle = self.r_handles.get(key)?;
    self.rigid_body_set.get_mut(*handle)
  }
}
