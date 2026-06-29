pub mod addons;
// pub mod numbers;

pub use indexmap;

pub const NLF_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Default)]
pub struct SchemaStore {
    pub schemas: Vec<Schema>
}

#[derive(Clone, Debug)]
pub struct Schema {
    pub id: usize,
    pub keys: indexmap::IndexSet<String>,
}