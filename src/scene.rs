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

pub struct Scene {
  ids: Vec<String>,
  meshes: Vec<Mesh>,
  r_handles: Vec<RigidBodyHandle>,
  c_handles: Vec<ColliderHandle>,
  scales: Vec<f32>,
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

impl Scene {
  pub fn new() -> Self {
    let integration_parameters = IntegrationParameters::default();
    let collider_set = ColliderSet::new();
    let physics_pipeline = PhysicsPipeline::new();
    let island_manager = IslandManager::new();
    let broad_phase = BroadPhaseMultiSap::new();
    let narrow_phase = NarrowPhase::new();
    let impulse_joint_set = ImpulseJointSet::new();
    let multibody_joint_set = MultibodyJointSet::new();
    let ccd_solver = CCDSolver::new();

    Self {
      ids: Vec::new(),
      meshes: Vec::new(),
      r_handles: Vec::new(),
      c_handles: Vec::new(),
      scales: Vec::new(),
      rigid_body_set: RigidBodySet::new(),
      collider_set,
      integration_parameters,
      physics_pipeline,
      island_manager,
      broad_phase,
      narrow_phase,
      impulse_joint_set,
      multibody_joint_set,
      ccd_solver,
    }
  }

  pub fn add(&mut self, name: &str, mesh: Mesh, body: RigidBody) {
    self.add_w_scale(name, mesh, body, 1.)
  }

  pub fn add_w_scale(&mut self, name: &str, mesh: Mesh, body: RigidBody, scale: f32) {
    let handle = self.rigid_body_set.insert(body);
    self.ids.push(name.to_owned());
    self.meshes.push(mesh);
    self.r_handles.push(handle);
    self.scales.push(scale);
  }

  pub fn add_w_scale_collider(
    &mut self,
    name: &str,
    mesh: Mesh,
    body: RigidBody,
    collider: Collider,
    scale: f32,
  ) {
    let r_handle = self.rigid_body_set.insert(body);
    let c_handle =
      self
        .collider_set
        .insert_with_parent(collider, r_handle, &mut self.rigid_body_set);
    self.ids.push(name.to_owned());
    self.meshes.push(mesh);
    self.r_handles.push(r_handle);
    self.c_handles.push(c_handle);
    self.scales.push(scale);
  }

  pub fn simiarities(&self) -> Vec<Similarity3<f32>> {
    self
      .r_handles
      .iter()
      .zip(self.scales.iter())
      .map(|(handle, scale)| {
        let body = self.rigid_body_set.get(*handle).unwrap();
        Similarity::from_isometry(*body.position(), *scale)
      })
      .collect()
  }

  pub fn meshes(&self) -> &Vec<Mesh> {
    &self.meshes
  }
  pub fn physics(&mut self) {
    self.physics_pipeline.step(
      // &vector![0., -9.8, 0.],
      &vector![0., -0., 0.],
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
    let key = self.ids.iter().position(|p| p == key)?;
    let handle = self.r_handles[key];
    let body = self.rigid_body_set.get(handle)?;
    Some(body)
  }
  pub fn get_body_mut(&mut self, key: &str) -> Option<&mut RigidBody> {
    let key = self.ids.iter().position(|p| p == key)?;
    let handle = self.r_handles[key];
    let body = self.rigid_body_set.get_mut(handle)?;
    Some(body)
  }
}
