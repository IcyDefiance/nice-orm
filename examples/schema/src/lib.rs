use nice_orm::*;

entity!(Account {
	#[entity_field(primary_key, generated_as_identity = "always")]
	id: i32,
	username: String,
	password: String,
});
