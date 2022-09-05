use phf::Map;

pub type Entities = &'static Map<&'static str, &'static EntityMeta>;

pub struct EntityMeta {
	pub table_name: &'static str,
	pub fields: Map<&'static str, FieldMeta>,
	pub primary_key: &'static [&'static str],
}

#[derive(Debug, Clone, Copy)]
pub struct FieldMeta {
	pub name: &'static str,
	pub ty: FieldType,
	pub optional: bool,
	pub generated_as_identity: Option<IdentityGeneration>,
}

#[derive(Debug, Clone, Copy)]
pub enum FieldType {
	I32,
	String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityGeneration {
	Always,
	ByDefault,
}
impl IdentityGeneration {
	pub fn from_name(name: &str) -> Self {
		match name {
			"ALWAYS" => Self::Always,
			"BY DEFAULT" => Self::ByDefault,
			_ => panic!("Unknown identity generation: {}", name),
		}
	}
}
