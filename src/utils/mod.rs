//! Small shared helpers.

use js_sys::Array;
use wasm_bindgen::JsValue;

/// Collects any iterable of JS-convertible values into a [`js_sys::Array`],
/// as required by many `web_sys` WebGPU descriptors.
pub fn iter_to_array<T>(iterable: impl IntoIterator<Item = T>) -> Array
where
  T: Into<JsValue>,
{
  iterable.into_iter().map(|v| v.into()).collect::<Array>()
}
