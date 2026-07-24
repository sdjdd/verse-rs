use std::collections::HashMap;

use crate::{
    compiler::{
        compiler::Compiler,
        const_pool::ConstPool,
        lexer::{LexerError, Span, tokenize},
        parser::{ParseError, Parser},
        semantic::{SemanticAnalyzer, SemanticError, SemanticErrorKind},
    },
    core::{
        ConstValue, PredefinedSymbols, Symbol, SymbolRegistry,
        types::{PredefinedTypes, TypeRegistry},
    },
    vm::{self, Function},
};

pub mod ast;
pub mod compiler;
pub mod const_pool;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod semantic;

pub struct CompileError {
    pub span: Span,
    pub kind: CompileErrorKind,
}

pub enum CompileErrorKind {
    Lexing(LexerError),
    Parsing(ParseError),
    Semantic(SemanticErrorKind),
}

impl From<LexerError> for CompileError {
    fn from(value: LexerError) -> Self {
        Self {
            span: match &value {
                LexerError::InconsistentIndent(span) => span,
                LexerError::InvalidIndentSize(span) => span,
                LexerError::InvalidToken(span) => span,
            }
            .clone(),
            kind: CompileErrorKind::Lexing(value),
        }
    }
}

impl From<ParseError> for CompileError {
    fn from(value: ParseError) -> Self {
        Self {
            span: match &value {
                ParseError::InvalidExpression { span } => span,
                ParseError::UnexpectedToken { span, .. } => span,
            }
            .clone(),
            kind: CompileErrorKind::Parsing(value),
        }
    }
}

impl From<SemanticError> for CompileError {
    fn from(value: SemanticError) -> Self {
        Self {
            span: value.span,
            kind: CompileErrorKind::Semantic(value.kind),
        }
    }
}

pub struct CompileOutcome {
    pub const_table: Vec<ConstValue>,
    pub symbol_table: SymbolRegistry,
    pub type_registry: TypeRegistry,
    pub predefined_types: PredefinedTypes,
    pub global_symbol_slots: HashMap<Symbol, usize>,
    pub functions: Vec<Function>,
    pub classes: Vec<vm::Class>,
    pub entry: u32,
}

pub fn compile(src: &str) -> Result<CompileOutcome, Vec<CompileError>> {
    let tokens = match tokenize(src) {
        Ok(tokens) => tokens,
        Err(e) => return Err(vec![e.into()]),
    };

    let mut symbol_table = SymbolRegistry::new();
    let mut const_pool = ConstPool::new();
    let mut parser = Parser::new(src, &tokens, &mut symbol_table, &mut const_pool);

    let program = parser.parse();
    if !parser.errors.is_empty() {
        return Err(parser.errors.into_iter().map(|e| e.into()).collect());
    }

    let builtin_symbols = PredefinedSymbols::install(&mut symbol_table);
    let mut analyzer = SemanticAnalyzer::new(&mut symbol_table, builtin_symbols);

    let root_irs = analyzer.analyze(&program.expressions);
    if !analyzer.errors.is_empty() {
        return Err(analyzer.errors.into_iter().map(|e| e.into()).collect());
    }

    let mut type_registry = TypeRegistry::new();
    let predefined_types = PredefinedTypes::install(&mut type_registry);
    let mut compiler = Compiler::new(&mut type_registry, predefined_types);
    let global_symbol_slots = analyzer.get_global_symbol_slots();

    compiler.compile_classes(analyzer.classes.into_iter().map(|c| c.into()).collect());
    compiler.compile(root_irs);

    Ok(CompileOutcome {
        const_table: const_pool.into_vec(),
        global_symbol_slots,
        entry: (compiler.functions.len() - 1) as u32,
        functions: compiler.functions,
        classes: compiler.classes,
        symbol_table,
        type_registry,
        predefined_types,
    })
}
