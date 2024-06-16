#[derive(Debug)]
pub struct Movement {
  pub dx: f32,
  pub dy: f32,
}

impl Movement {
  pub fn new() -> Self {
    Self { dx: 0., dy: 0. }
  }
}
