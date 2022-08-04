use nice_orm::*;

entity!(Account {
	#[entity_field(primary_key)]
	id: i32,
	username: String,
	password: String,
});
