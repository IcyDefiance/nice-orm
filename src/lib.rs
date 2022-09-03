pub mod entity_manager;
pub mod entity_meta;

pub use bevy_reflect;
pub use lazy_static;
pub use nice_orm_derive::*;
pub use phf;
pub use serde;

use bevy_reflect::Struct;
use entity_meta::EntityMeta;
use std::{
	any::{Any, TypeId},
	collections::hash_map::DefaultHasher,
	hash::{Hash, Hasher},
};

pub trait Entity: Struct {
	fn id(&self) -> Option<Box<dyn Key + Send + Sync>>;
	fn meta(&self) -> &'static EntityMeta;
	fn mark_loaded(&mut self);
}

pub trait Key {
	fn eq(&self, other: &dyn Key) -> bool;
	fn hash(&self) -> u64;
	fn as_any(&self) -> &dyn Any;
}
impl<T: Eq + Hash + 'static> Key for T {
	fn eq(&self, other: &dyn Key) -> bool {
		if let Some(other) = other.as_any().downcast_ref::<T>() {
			return self == other;
		}
		false
	}

	fn hash(&self) -> u64 {
		let mut h = DefaultHasher::new();
		// mix the typeid of T into the hash to make distinct types
		// provide distinct hashes
		Hash::hash(&(TypeId::of::<T>(), self), &mut h);
		h.finish()
	}

	fn as_any(&self) -> &dyn Any {
		self
	}
}
impl PartialEq for Box<dyn Key + Send + Sync> {
	fn eq(&self, other: &Self) -> bool {
		Key::eq(self.as_ref(), other.as_ref())
	}
}
impl Eq for Box<dyn Key + Send + Sync> {}
impl Hash for Box<dyn Key + Send + Sync> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		let key_hash = Key::hash(self.as_ref());
		state.write_u64(key_hash);
	}
}
