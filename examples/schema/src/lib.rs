use nice_orm::*;

entity!(
	pub Account {
		#[entity_field(primary_key)]
		id: i32,
	}
);
