#![deny(warnings)]
#![allow(clippy::from_over_into)]

mod builders;
mod composite_type;
mod error;
mod extensions;
mod field;
mod fields;
mod index;
mod internal_data_model;
mod internal_enum;
mod model;
mod order_by;
mod parent_container;
mod prisma_value_ext;
mod projections;
mod record;
mod relation;

pub mod pk;
pub mod prelude;

pub use builders::InternalDataModelBuilder;
pub use composite_type::*;
pub use datamodel::dml;
pub use error::*;
pub use field::*;
pub use fields::*;
pub use index::*;
pub use internal_data_model::*;
pub use internal_enum::*;
pub use model::*;
pub use order_by::*;
pub use prisma_value_ext::*;
pub use projections::*;
pub use record::*;
pub use relation::*;

// reexport
pub use prisma_value::*;

pub type Result<T> = std::result::Result<T, DomainError>;
