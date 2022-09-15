use crate::EntityExt;
use sqlx::{
	postgres::{PgArguments, PgTypeInfo},
	query,
	query::Query,
	Encode, Postgres, Type,
};
use std::{
	fmt::{self, Write},
	marker::PhantomData,
};

pub struct SqlBuilder {
	sql: String,
	param_types: Vec<PgTypeInfo>,
	param_idx: usize,
}
impl SqlBuilder {
	pub fn new() -> Self {
		Self { sql: String::new(), param_types: Vec::new(), param_idx: 0 }
	}

	pub fn push(&mut self, sql: &str) {
		self.sql.push_str(sql);
	}

	pub fn push_param(&mut self, type_info: PgTypeInfo) {
		self.param_types.push(type_info);
		write!(self.sql, "${}", self.param_idx).unwrap();
		self.param_idx += 1;
	}

	pub fn to_string(self) -> String {
		self.sql.clone()
	}

	pub fn to_query(&self) -> Query<Postgres, PgArguments> {
		query(&self.sql)
	}
}
impl Write for SqlBuilder {
	fn write_str(&mut self, s: &str) -> fmt::Result {
		self.sql.push_str(s);
		Ok(())
	}
}

pub trait Expression {
	fn push_to(&self, query: &mut SqlBuilder);
	fn bind_to<'a>(&'a self, query: Query<'a, Postgres, PgArguments>) -> Query<'a, Postgres, PgArguments>;
}
impl Expression for String {
	fn push_to(&self, query: &mut SqlBuilder) {
		query.push_param(Self::type_info());
	}

	fn bind_to<'a>(&'a self, query: Query<'a, Postgres, PgArguments>) -> Query<'a, Postgres, PgArguments> {
		query.bind(self).persistent(true)
	}
}

pub trait ExpressionExt: Sized {
	fn eq<'a, T: Encode<'a, Postgres>>(self, other: T) -> Eq<Self, T> {
		Eq(self, other)
	}
}
impl<T: Expression> ExpressionExt for T {}

pub trait Predicate: Expression {}

pub struct Field<T: EntityExt> {
	pub name: &'static str,
	pub phantom: PhantomData<T>,
}
impl<T: EntityExt> Field<T> {
	pub const fn new(name: &'static str) -> Self {
		Self { name, phantom: PhantomData }
	}
}
impl<T: EntityExt> Expression for Field<T> {
	fn push_to(&self, query: &mut SqlBuilder) {
		write!(query, "\"{}\".\"{}\"", T::META.table_name, self.name).unwrap();
	}

	fn bind_to<'a>(&'a self, query: Query<'a, Postgres, PgArguments>) -> Query<'a, Postgres, PgArguments> {
		query
	}
}

pub struct Eq<T, U>(T, U);
impl<T: Expression, U: Expression> Expression for Eq<T, U> {
	fn push_to(&self, query: &mut SqlBuilder) {
		self.0.push_to(query);
		query.push("=");
		self.1.push_to(query);
	}

	fn bind_to<'a>(&'a self, mut query: Query<'a, Postgres, PgArguments>) -> Query<'a, Postgres, PgArguments> {
		query = self.0.bind_to(query);
		self.1.bind_to(query)
	}
}
impl<T: Expression, U: Expression> Predicate for Eq<T, U> {}
