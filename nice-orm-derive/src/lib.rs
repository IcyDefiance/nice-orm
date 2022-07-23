extern crate proc_macro;

use convert_case::{Case, Casing};
use darling::FromField;
use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro_crate::{crate_name, FoundCrate};
use proc_macro_error::{abort, proc_macro_error};
use quote::{quote, ToTokens};
use syn::{
	braced,
	parse::{Nothing, Parse, ParseStream},
	parse_macro_input,
	punctuated::Punctuated,
	token::Brace,
	Field, Ident, Result, Token, Type, Visibility,
};

#[proc_macro]
#[proc_macro_error]
pub fn entity(input: TokenStream) -> TokenStream {
	let nice_orm = find_crate("nice-orm");

	let entities = parse_macro_input!(input as Entities);

	let mut outputs = vec![];
	let mut metas = vec![];

	for entity in entities.0 {
		let fields = entity.fields.iter().map(|field| {
			let ident = &field.ident;
			let ty = &field.ty;
			quote! { #ident: #ty }
		});
		let field_inits = entity.fields.iter().map(|field| {
			let ident = &field.ident;
			quote! { #ident: Default::default() }
		});
		let field_metas = entity
			.fields
			.iter()
			.map(|field| {
				let field_name = field.ident.as_ref().unwrap().to_string();
				let type_i32: Type = syn::parse_str("i32").unwrap();
				let type_string: Type = syn::parse_str("String").unwrap();
				let ty = if field.ty == type_i32 {
					quote! { #nice_orm::entity_meta::FieldType::I32 }
				} else if field.ty == type_string {
					quote! { #nice_orm::entity_meta::FieldType::String }
				} else {
					unreachable!()
				};
				quote! { #field_name => #nice_orm::entity_meta::FieldMeta { name: #field_name, ty: #ty, optional: false } }
			})
			.collect::<Vec<_>>();
		let primary_key = entity
			.fields
			.iter()
			.filter(|field| field.primary_key)
			.map(|field| field.ident.as_ref().unwrap().to_string())
			.collect::<Vec<_>>();

		let vis = entity.vis;
		let ident = entity.ident;
		let table_name = ident.to_string().to_case(Case::Snake);

		outputs.push(quote! {
			#vis struct #ident {
				__orm_loaded: bool,
				#(#fields),*
			}
			impl #ident {
				pub const META: &'static #nice_orm::entity_meta::EntityMeta = &#nice_orm::entity_meta::EntityMeta {
					table_name: #table_name,
					fields: #nice_orm::phf::phf_map! { #(#field_metas),* },
					primary_key: &[#(#primary_key),*],
				};

				pub fn new() -> Self {
					Self {
						__orm_loaded: false,
						#(#field_inits),*
					}
				}
			}
		});

		metas.push(quote! { #table_name => #ident::META });
	}

	quote! {
		#nice_orm::lazy_static::lazy_static! {
			pub static ref ENTITIES: #nice_orm::phf::Map<&'static str, &'static #nice_orm::entity_meta::EntityMeta>
				= #nice_orm::phf::phf_map! { #(#metas),* };
		}

		#(#outputs)*
	}
	.into()
}

struct Entities(Punctuated<Entity, Nothing>);
impl Parse for Entities {
	fn parse(input: ParseStream) -> Result<Self> {
		Ok(Entities(input.parse_terminated(Entity::parse)?))
	}
}

struct Entity {
	vis: Visibility,
	ident: Ident,
	_brace_token: Brace,
	fields: Vec<EntityField>,
}
impl Parse for Entity {
	fn parse(input: ParseStream) -> Result<Self> {
		let content;

		let vis = input.parse()?;
		let ident = input.parse()?;
		let _brace_token = braced!(content in input);
		let fields = content.parse_terminated::<_, Token![,]>(Field::parse_named)?;
		let fields = fields.into_iter().map(|field| EntityField::from_field(&field).unwrap()).collect();

		Ok(Entity { vis, ident, _brace_token, fields })
	}
}

#[derive(FromField)]
#[darling(attributes(entity_field))]
struct EntityField {
	ident: Option<Ident>,
	#[darling(and_then = "EntityField::validate_type")]
	ty: Type,
	#[darling(default)]
	primary_key: bool,
}
impl EntityField {
	// used by darling above
	#[allow(unused)]
	fn validate_type(ty: Type) -> darling::Result<Type> {
		let type_i32: Type = syn::parse_str("i32").unwrap();
		if ty == type_i32 { Ok(ty) } else { Err(darling::Error::custom("unsupported type")) }
	}
}

fn find_crate(name: &str) -> impl ToTokens {
	match crate_name(name) {
		Ok(x) => match x {
			FoundCrate::Itself => quote!(crate),
			FoundCrate::Name(name) => {
				let ident = Ident::new(&name, Span::call_site());
				quote!(#ident)
			},
		},
		Err(_) => abort!(Span::call_site(), "{} is not present in `Cargo.toml`", name),
	}
}
