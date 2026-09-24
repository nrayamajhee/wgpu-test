//! The device: a two-octave piano keyboard. See [`Device`].

use nalgebra::{Similarity3, Translation3, UnitQuaternion};
use wasm_bindgen::JsValue;

use crate::core::{Color, Geometry, Group, Keyboard, Material, Mesh, Renderer, Scene};

/// One octave from C, left to right: `W` white / `B` black key, and its
/// hotkey (`KeyboardEvent.code`). Whites are on the home row, blacks on the
/// top-row key between their neighbours.
const OCTAVE: [(char, &str); 12] = [
  ('W', "KeyA"), // C
  ('B', "KeyW"), // C#
  ('W', "KeyS"), // D
  ('B', "KeyE"), // D#
  ('W', "KeyD"), // E
  ('W', "KeyF"), // F
  ('B', "KeyT"), // F#
  ('W', "KeyG"), // G
  ('B', "KeyY"), // G#
  ('W', "KeyH"), // A
  ('B', "KeyU"), // A#
  ('W', "KeyJ"), // B
];

/// Number of octaves. Octave 0 plays on the bare hotkeys, octave 1 with Shift held.
const OCTAVES: usize = 2;

/// White key `[width, height, depth]`.
const WHITE_SIZE: [f32; 3] = [0.9, 4., 0.5];
/// Black key `[width, height, depth]`.
const BLACK_SIZE: [f32; 3] = [0.55, 2.5, 0.5];
/// Horizontal distance between white key centres.
const WHITE_PITCH: f32 = 1.;
/// How far black keys sit in front of white keys (toward the camera).
const BLACK_OFFSET_Z: f32 = 0.3;

/// Resting white key color.
const WHITE: Color = Color::rgb(0.9, 0.9, 0.9);
/// Resting black key color.
const BLACK: Color = Color::rgb(0.05, 0.05, 0.05);
/// Color of a key whose hotkey is held.
const ACTIVE: Color = Color::rgb(0.8, 0.3, 0.1);

/// One piano key derived from [`OCTAVE`].
struct PianoKey {
  /// Scene node id.
  node: String,
  /// Black (sharp) key.
  black: bool,
  /// Horizontal centre, keyboard centred on `x = 0`.
  x: f32,
  /// Triggering `KeyboardEvent.code`.
  hotkey: &'static str,
  /// Octave index; octave 1 needs Shift held.
  octave: usize,
}

impl PianoKey {
  /// Resting color.
  fn color(&self) -> Color {
    if self.black {
      BLACK
    } else {
      WHITE
    }
  }
}

/// Builder and per-frame logic for the piano: tall, narrow cuboid keys
/// standing upright and facing +Z (the camera).
///
/// # Hotkeys
/// Each octave starts on C and uses the same keys:
///
/// | Note   | C | C# | D | D# | E | F | F# | G | G# | A | A# | B |
/// |--------|---|----|---|----|---|---|----|---|----|---|----|---|
/// | Hotkey | A | W  | S | E  | D | F | T  | G | Y  | H | U  | J |
///
/// Bare hotkeys play the lower octave; with Shift held, the upper octave.
pub struct Device;

impl Device {
  /// Scene node id of the whole keyboard; the camera follows this node.
  pub const NODE: &'static str = "device";

  /// All keys, left to right, with position, hotkey and octave.
  fn keys() -> impl Iterator<Item = PianoKey> {
    let white_count = OCTAVES * OCTAVE.iter().filter(|(c, _)| *c == 'W').count();
    let center = (white_count - 1) as f32 / 2.;
    // Whites placed so far.
    let mut whites = 0;
    (0..OCTAVES)
      .flat_map(|octave| OCTAVE.iter().map(move |&(c, hotkey)| (octave, c, hotkey)))
      .enumerate()
      .map(move |(i, (octave, c, hotkey))| {
        let black = c == 'B';
        // Blacks sit halfway between the previous and next white.
        let slot = if black {
          whites as f32 - 0.5
        } else {
          whites += 1;
          (whites - 1) as f32
        };
        PianoKey {
          node: format!("{}_key_{i}", Self::NODE),
          black,
          x: (slot - center) * WHITE_PITCH,
          hotkey,
          octave,
        }
      })
  }

  /// Returns the `"device"` group with one `"device_key_{i}"` child per key.
  /// White keys are centred on `y = 0`; black keys are top-aligned with them
  /// and sit slightly in front.
  ///
  /// # Errors
  /// If mesh creation fails.
  pub async fn new(renderer: &Renderer) -> Result<Group, JsValue> {
    let white = Geometry::cuboid(WHITE_SIZE);
    let black = Geometry::cuboid(BLACK_SIZE);
    let black_y = (WHITE_SIZE[1] - BLACK_SIZE[1]) / 2.;

    let mut device = Group::new(Self::NODE);
    for key in Self::keys() {
      let (geometry, y, z) = if key.black {
        (&black, black_y, BLACK_OFFSET_Z)
      } else {
        (&white, 0., 0.)
      };
      let mesh = Mesh::new(renderer, geometry, &Material::new(key.color())).await?;
      let transform = Similarity3::from_parts(
        Translation3::new(key.x, y, z),
        UnitQuaternion::identity(),
        1.,
      );
      device.add_child(
        Group::new(key.node)
          .with_mesh(mesh)
          .with_transform(transform),
      );
    }
    Ok(device)
  }

  /// Colors each key [`ACTIVE`] while its hotkey is held (with Shift for the
  /// upper octave), otherwise its resting color. Call once per frame.
  pub fn update(scene: &mut Scene, keyboard: &Keyboard) {
    let octave = if keyboard.shift() { 1 } else { 0 };
    for key in Self::keys() {
      let active = key.octave == octave && keyboard.is_pressed(key.hotkey);
      if let Some(mesh) = scene.get_mesh_mut(&key.node) {
        mesh.color = if active { ACTIVE } else { key.color() };
      }
    }
  }
}
