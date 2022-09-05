use phf::Map;

pub type Entities = &'static Map<&'static str, &'static EntityMeta>;

pub struct EntityMeta {
	pub table_name: &'static str,
	pub fields: Map<&'static str, FieldMeta>,
	pub primary_key: &'static [&'static str],
}

pub struct FieldMeta {
	pub name: &'static str,
	pub ty: FieldType,
	pub optional: bool,
	pub generated_as_identity: Option<GeneratedWhen>,
}

#[derive(Debug, Clone, Copy)]
pub enum FieldType {
	I32,
	String,
}

pub enum GeneratedWhen {
	Always,
	ByDefault,
}
