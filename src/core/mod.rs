pub mod types;

mod symbol;

pub use symbol::{PredefinedSymbols, Symbol, SymbolRegistry};

#[derive(Debug, Clone, Copy)]
pub struct ConstId(pub(crate) usize);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConstValue {
    String(String),
}
