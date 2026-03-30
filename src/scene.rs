use std::collections::HashMap;

use crate::Mesh;
use nalgebra::{vector, Similarity, Similarity3};
use rapier3d::{
  dynamics::RigidBodyHandle,
  geometry::BroadPhaseMultiSap,
  prelude::{
    CCDSolver, Collider, ColliderHandle, ColliderSet, ImpulseJointSet, IntegrationParameters,
    IslandManager, MultibodyJointSet, NarrowPhase, PhysicsPipeline, RigidBody, RigidBodySet,
  },
};

pub struct SceneNode {
  pub id: String,
  pub parent: Option<String>,
  pub children: Vec<String>,
}

/// A self-contained tree of nodes with their associated meshes and transforms,
/// ready to be inserted into a Scene.
pub struct NodeBundle {
  pub nodes: Vec<SceneNode>,
  pub transforms: HashMap<String, Similarity3<f32>>,
  pub meshes: HashMap<String, Mesh>,
}

pub struct Scene {
  nodes: HashMap<String, SceneNode>,
  transforms: HashMap<String, Similarity3<f32>>,
  meshes: HashMap<String, Mesh>,
  r_handles: HashMap<String, RigidBodyHandle>,
  c_handles: HashMap<String, ColliderHandle>,
  rigid_body_set: RigidBodySet,
  collider_set: ColliderSet,
  integration_parameters: IntegrationParameters,
  physics_pipeline: PhysicsPipeline,
  island_manager: IslandManager,
  broad_phase: BroadPhaseMultiSap,
  narrow_phase: NarrowPhase,
  impulse_joint_set: ImpulseJointSet,
  multibody_joint_set: MultibodyJointSet,
  ccd_solver: CCDSolver,
}

impl Default for Scene {
  fn default() -> Self {
    Self::new()
  }
}

impl Scene {
  pub fn new() -> Self {
    Self {
      nodes: HashMap::new(),
      transforms: HashMap::new(),
      meshes: HashMap::new(),
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

  fn insert_node(&mut self, name: &str, parent: Option<&str>) {
    let node = SceneNode {
      id: name.to_owned(),
      parent: parent.map(|p| p.to_owned()),
      children: Vec::new(),
    };
    self.nodes.insert(name.to_owned(), node);
    if let Some(parent_id) = parent {
      if let Some(parent_node) = self.nodes.get_mut(parent_id) {
        parent_node.children.push(name.to_owned());
      }
    }
  }

  pub fn insert_bundle(&mut self, bundle: NodeBundle) {
    for node in bundle.nodes {
      if let Some(parent_id) = &node.parent {
        if let Some(parent_node) = self.nodes.get_mut(parent_id) {
          parent_node.children.push(node.id.clone());
        }
      }
      self.nodes.insert(node.id.clone(), node);
    }
    self.transforms.extend(bundle.transforms);
    self.meshes.extend(bundle.meshes);
  }

  pub fn add_node(&mut self, name: &str, transform: Similarity3<f32>) {
    self.insert_node(name, None);
    self.transforms.insert(name.to_owned(), transform);
  }

  pub fn add_static(&mut self, name: &str, mesh: Mesh, transform: Similarity3<f32>) {
    self.insert_node(name, None);
    self.transforms.insert(name.to_owned(), transform);
    self.meshes.insert(name.to_owned(), mesh);
  }

  pub fn add_child(
    &mut self,
    name: &str,
    parent: &str,
    mesh: Mesh,
    local_transform: Similarity3<f32>,
  ) {
    self.insert_node(name, Some(parent));
    self.transforms.insert(name.to_owned(), local_transform);
    self.meshes.insert(name.to_owned(), mesh);
  }

  pub fn add_child_node(&mut self, name: &str, parent: &str, local_transform: Similarity3<f32>) {
    self.insert_node(name, Some(parent));
    self.transforms.insert(name.to_owned(), local_transform);
  }

  pub fn add(&mut self, name: &str, mesh: Mesh, body: RigidBody) {
    self.add_w_scale(name, mesh, body, 1.)
  }

  pub fn add_w_scale(&mut self, name: &str, mesh: Mesh, body: RigidBody, scale: f32) {
    let handle = self.rigid_body_set.insert(body);
    let transform = Similarity::from_isometry(*self.rigid_body_set[handle].position(), scale);
    self.insert_node(name, None);
    self.transforms.insert(name.to_owned(), transform);
    self.meshes.insert(name.to_owned(), mesh);
    self.r_handles.insert(name.to_owned(), handle);
  }

  pub fn add_w_scale_collider(
    &mut self,
    name: &str,
    mesh: Option<Mesh>,
    body: RigidBody,
    collider: Collider,
    scale: f32,
  ) {
    let r_handle = self.rigid_body_set.insert(body);
    let c_handle =
      self
        .collider_set
        .insert_with_parent(collider, r_handle, &mut self.rigid_body_set);
    let transform = Similarity::from_isometry(*self.rigid_body_set[r_handle].position(), scale);
    self.insert_node(name, None);
    self.transforms.insert(name.to_owned(), transform);
    if let Some(mesh) = mesh {
      self.meshes.insert(name.to_owned(), mesh);
    }
    self.r_handles.insert(name.to_owned(), r_handle);
    self.c_handles.insert(name.to_owned(), c_handle);
  }

  pub fn meshes(&self) -> Vec<(&Mesh, Similarity3<f32>)> {
    let mut result = Vec::new();
    let roots: Vec<&str> = self
      .nodes
      .values()
      .filter(|n| n.parent.is_none())
      .map(|n| n.id.as_str())
      .collect();
    for root in roots {
      self.collect_meshes(root, &Similarity3::identity(), &mut result);
    }
    result
  }

  fn collect_meshes<'a>(
    &'a self,
    id: &str,
    parent_world: &Similarity3<f32>,
    out: &mut Vec<(&'a Mesh, Similarity3<f32>)>,
  ) {
    let local = self
      .transforms
      .get(id)
      .copied()
      .unwrap_or_else(Similarity3::identity);
    let world = *parent_world * local;
    if let Some(mesh) = self.meshes.get(id) {
      out.push((mesh, world));
    }
    if let Some(node) = self.nodes.get(id) {
      for child in &node.children {
        self.collect_meshes(child, &world, out);
      }
    }
  }

  pub fn sync_transforms(&mut self) {
    let updates: Vec<(String, Similarity3<f32>)> = self
      .r_handles
      .iter()
      .filter_map(|(id, handle)| {
        let body = self.rigid_body_set.get(*handle)?;
        let scaling = self.transforms.get(id).map(|t| t.scaling()).unwrap_or(1.);
        Some((
          id.clone(),
          Similarity::from_isometry(*body.position(), scaling),
        ))
      })
      .collect();

    for (id, transform) in updates {
      self.transforms.insert(id, transform);
    }
  }

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

  pub fn get_body(&self, key: &str) -> Option<&RigidBody> {
    let handle = self.r_handles.get(key)?;
    self.rigid_body_set.get(*handle)
  }

  pub fn get_body_mut(&mut self, key: &str) -> Option<&mut RigidBody> {
    let handle = self.r_handles.get(key)?;
    self.rigid_body_set.get_mut(*handle)
  }
}
