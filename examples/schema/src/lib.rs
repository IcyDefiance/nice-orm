use nice_orm::*;

entity!(Account {
	#[entity_field(primary_key, identity_generation = "always")]
	id: i32,
	username: String,
	password: String,
});
