//! Scene content. Each builder returns a [`Group`](crate::core::Group)
//! for [`Scene::add_group`](crate::core::Scene::add_group).

pub mod backdrop;
pub mod device;
pub mod world;

pub use backdrop::Backdrop;
pub use device::Device;
pub use world::World;
