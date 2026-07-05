mod symbol;
pub mod types;

pub use symbol::{PredefinedSymbols, Symbol, SymbolRegistry};

#[derive(Debug, Clone, Copy)]
pub struct ConstId(pub(crate) usize);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConstValue {
    String(String),
}
