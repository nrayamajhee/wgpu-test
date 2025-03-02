#[derive(Debug)]
pub struct Movement {
  pub dx: isize,
  pub dy: isize,
}

impl Movement {
  pub fn new() -> Self {
    Self { dx: 0, dy: 0 }
  }
}
