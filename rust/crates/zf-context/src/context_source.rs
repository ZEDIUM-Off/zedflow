//! Readable Rust builders are the context strategy source. Only these builders
//! and literal arguments are accepted; parsing never invokes Rust or a macro.
use super::context::*;
use quote::ToTokens;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use syn::{
    Expr, Item, Lit, Stmt, Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
};
use zf_core::{diagnostics::Diagnostic, types::DataType};

pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;
const HEADER: &str = "// @zedflow-context 1";
const HEADER_V2: &str = "// @zedflow-context 2";
const IMPORTS: &str =
    "use zf_context::context::*;\nuse zf_core::types::DataType;\nuse std::collections::BTreeMap;\n";
type Parsed<T> = Result<T, Diagnostic>;

// A record contributes four Rust groups for one context type level. Keep this
// syntax budget separate from the language's 64-level semantic limit. The latter
// is checked after decoding, before a program can be accepted or evaluated.
const MAX_SYNTAX_DEPTH: usize = 512;
const MAX_SYNTAX_CHAIN: usize = 4100;
const INLINE_SYNTAX_DEPTH: usize = 16;
const SOURCE_PARSER_STACK_BYTES: usize = 64 * 1024 * 1024;

/// Lex before invoking syn's recursive expression parser. proc_macro2's lexer
/// and token-stream destruction use explicit stacks; strings, raw strings,
/// character literals and nested comments therefore cannot forge our counters.
/// Commas and semicolons separate siblings, rather than consuming nesting depth.
fn source_complexity(source: &str) -> Result<usize, Vec<Diagnostic>> {
    use proc_macro2::{TokenStream, TokenTree};
    struct Frame {
        tokens: proc_macro2::token_stream::IntoIter,
        inherited_operators: usize,
        inherited_accesses: usize,
        operators: usize,
        accesses: usize,
    }
    let tokens = source
        .parse::<TokenStream>()
        .map_err(|error| vec![syntax(syn::Error::new(error.span(), error.to_string()))])?;
    let mut frames = vec![Frame {
        tokens: tokens.into_iter(),
        inherited_operators: 0,
        inherited_accesses: 0,
        operators: 0,
        accesses: 0,
    }];
    let mut maximum = 0;
    while !frames.is_empty() {
        let depth = frames.len() - 1;
        let frame = frames.last_mut().expect("A nonempty stack has a frame");
        let Some(token) = frame.tokens.next() else {
            frames.pop();
            continue;
        };
        match &token {
            TokenTree::Punct(punct) => match punct.as_char() {
                ',' | ';' => {
                    frame.operators = 0;
                    frame.accesses = 0;
                }
                '.' => frame.accesses += 1,
                ':' => {}
                _ => frame.operators += 1,
            },
            // These expressions can nest without a delimiter or punctuation:
            // `return return ...`, or an arbitrarily long `else if` chain.
            TokenTree::Ident(ident)
                if matches!(
                    ident.to_string().as_str(),
                    "return"
                        | "if"
                        | "else"
                        | "loop"
                        | "while"
                        | "for"
                        | "match"
                        | "break"
                        | "yield"
                        | "unsafe"
                        | "async"
                        | "try"
                        | "const"
                        | "move"
                        | "let"
                ) =>
            {
                frame.operators += 1;
            }
            TokenTree::Group(_) | TokenTree::Ident(_) | TokenTree::Literal(_) => {}
        }
        let operators = frame.inherited_operators + frame.operators;
        let accesses = frame.inherited_accesses + frame.accesses;
        let next_depth = depth + usize::from(matches!(token, TokenTree::Group(_)));
        if next_depth + operators > MAX_SYNTAX_DEPTH || accesses > MAX_SYNTAX_CHAIN {
            return Err(vec![Diagnostic::new(
                "context_source_depth",
                "$source",
                "Rust source exceeds its syntax nesting or builder-chain limit",
            )]);
        }
        maximum = maximum.max(next_depth + operators + accesses);
        if let TokenTree::Group(group) = token {
            frames.push(Frame {
                tokens: group.stream().into_iter(),
                inherited_operators: operators,
                inherited_accesses: accesses,
                operators: 0,
                accesses: 0,
            });
        }
    }
    Ok(maximum)
}

/// Only owned domain values cross the worker boundary. In particular, syn ASTs
/// and their recursive destructors stay on the same bounded parsing stack.
pub fn parse_on_source_stack<T: Send>(
    source: &str,
    parse: impl FnOnce(&str) -> Result<T, Vec<Diagnostic>> + Send,
) -> Result<T, Vec<Diagnostic>> {
    if source_complexity(source)? <= INLINE_SYNTAX_DEPTH {
        return parse(source);
    }
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("context-source-parser".into())
            .stack_size(SOURCE_PARSER_STACK_BYTES)
            .spawn_scoped(scope, || parse(source))
            .map_err(|error| {
                vec![Diagnostic::new(
                    "context_source_parser",
                    "$source",
                    format!("Could not start the context source parser: {error}"),
                )]
            })?;
        worker.join().map_err(|_| {
            vec![Diagnostic::new(
                "context_source_parser",
                "$source",
                "The context source parser failed",
            )]
        })?
    })
}

pub fn generate(strategy: &ContextStrategy) -> Result<String, Vec<Diagnostic>> {
    validate_structure(strategy)?;
    let (header, constructor) = if strategy.version == 2 {
        (HEADER_V2, "new_v2")
    } else {
        (HEADER, "new")
    };
    let mut out = format!(
        "{header}\n{IMPORTS}\npub fn strategy() -> ContextStrategy {{\n    ContextStrategy::{constructor}({}, {})",
        string(&strategy.id),
        string(&strategy.name)
    );
    for (name, data_type) in &strategy.types {
        out.push_str(&format!(
            "\n        .define_type({}, {})",
            string(name),
            type_source(data_type)
        ));
    }
    for (name, data_type) in &strategy.requirements {
        out.push_str(&format!(
            "\n        .require({}, {})",
            string(name),
            type_source(data_type)
        ));
    }
    for capability in &strategy.capabilities {
        out.push_str(&format!(
            "\n        .capability(ContextCapability::new({}, {}, {}))",
            string(&capability.id),
            type_source(&capability.input),
            type_source(&capability.output)
        ));
    }
    out.push_str(&format!(
        "\n        .with_program({})\n}}\n",
        blocks_source(&strategy.program, 8)
    ));
    if out.len() > MAX_SOURCE_BYTES {
        return Err(vec![Diagnostic::new(
            "source_size",
            "$source",
            "Context source exceeds 1 MiB",
        )]);
    }
    Ok(out)
}

pub fn parse(source: &str) -> Result<ContextStrategy, Vec<Diagnostic>> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(vec![Diagnostic::new(
            "source_size",
            "$source",
            "Context source exceeds 1 MiB",
        )]);
    }
    let version = match source.lines().next() {
        Some(HEADER) => 1,
        Some(HEADER_V2) => 2,
        _ => 0,
    };
    if version == 0 {
        return Err(vec![Diagnostic::new(
            "context_header",
            "$source",
            "Expected // @zedflow-context 1 or 2 on the first line",
        )]);
    }
    parse_on_source_stack(source, |source| {
        let file = syn::parse_file(source).map_err(|error| vec![syntax(error)])?;
        let result = parse_file(&file).map_err(|error| vec![error])?;
        if result.version != version {
            return Err(vec![Diagnostic::new(
                "context_version",
                "$source",
                "Context header and constructor version disagree",
            )]);
        }
        validate_structure(&result)?;
        Ok(result)
    })
}

pub fn generate_library(library: &ContextLibrary) -> Result<String, Vec<Diagnostic>> {
    validate_library_structure(library)?;
    let mut source = format!(
        "// @zedflow-context-library 1\n{IMPORTS}\npub fn library() -> ContextLibrary {{\n    ContextLibrary::new()"
    );
    for (method, functions) in [
        ("projection", &library.projections),
        ("subprogram", &library.subprograms),
    ] {
        for (name, function) in functions {
            let parameters = format!(
                "BTreeMap::from([{}])",
                function
                    .parameters
                    .iter()
                    .map(|(name, ty)| format!("({}.into(), {})", string(name), type_source(ty)))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            source.push_str(&format!(
                "\n        .{method}({}, ContextFunction::new({}, {}, {}))",
                string(name),
                parameters,
                type_source(&function.output),
                expression_source(&function.body)
            ));
        }
    }
    source.push_str("\n}\n");
    if source.len() > MAX_SOURCE_BYTES {
        return Err(vec![Diagnostic::new(
            "source_size",
            "$source",
            "Context library exceeds 1 MiB",
        )]);
    }
    Ok(source)
}

pub fn parse_library(source: &str) -> Result<ContextLibrary, Vec<Diagnostic>> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(vec![Diagnostic::new(
            "source_size",
            "$source",
            "Context library exceeds 1 MiB",
        )]);
    }
    if source.lines().next() != Some("// @zedflow-context-library 1") {
        return Err(vec![Diagnostic::new(
            "context_header",
            "$source",
            "Expected // @zedflow-context-library 1",
        )]);
    }
    parse_on_source_stack(source, |source| {
        let file = syn::parse_file(source).map_err(|e| vec![syntax(e)])?;
        let expression =
            return_expression(&file, "library", "ContextLibrary").map_err(|e| vec![e])?;
        let library = parse_library_builder(expression).map_err(|e| vec![e])?;
        validate_library_structure(&library)?;
        Ok(library)
    })
}

pub fn generate_types(types: &zf_core::types::TypeRegistry) -> Result<String, Vec<Diagnostic>> {
    for ty in types.values() {
        zf_core::types::validate_type(ty, types)?;
    }
    let values = types
        .iter()
        .map(|(name, ty)| format!("({}.into(), {})", string(name), type_source(ty)))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        "// @zedflow-types 1\n{IMPORTS}\npub fn types() -> BTreeMap<String, DataType> {{\n    BTreeMap::from([{values}])\n}}\n"
    );
    if source.len() > MAX_SOURCE_BYTES {
        return Err(vec![Diagnostic::new(
            "source_size",
            "$source",
            "Type registry exceeds 1 MiB",
        )]);
    }
    Ok(source)
}

pub fn parse_types(source: &str) -> Result<zf_core::types::TypeRegistry, Vec<Diagnostic>> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(vec![Diagnostic::new(
            "source_size",
            "$source",
            "Type registry exceeds 1 MiB",
        )]);
    }
    if source.lines().next() != Some("// @zedflow-types 1") {
        return Err(vec![Diagnostic::new(
            "context_header",
            "$source",
            "Expected // @zedflow-types 1",
        )]);
    }
    parse_on_source_stack(source, |source| {
        let file = syn::parse_file(source).map_err(|e| vec![syntax(e)])?;
        let expression =
            return_expression(&file, "types", "BTreeMap<String, DataType>").map_err(|e| vec![e])?;
        let types = parse_pairs(expression, 0, parse_root_type).map_err(|e| vec![e])?;
        for ty in types.values() {
            zf_core::types::validate_type(ty, &types)?;
        }
        Ok(types)
    })
}

fn parse_library_builder(expr: &Expr) -> Parsed<ContextLibrary> {
    let mut value = expr;
    let mut methods = vec![];
    while let Expr::MethodCall(method) = value {
        if !method.attrs.is_empty() || method.turbofish.is_some() || methods.len() >= 4096 {
            return Err(unsupported(method, "Unsupported library builder"));
        }
        methods.push(method);
        value = &method.receiver;
    }
    call(value, "ContextLibrary::new", 0)?;
    let mut result = ContextLibrary::default();
    for method in methods.into_iter().rev() {
        if method.args.len() != 2 {
            return Err(unsupported(
                method,
                "Library entry requires name and function",
            ));
        }
        let name = text(&method.args[0])?;
        let args = call(&method.args[1], "ContextFunction::new", 3)?;
        let function = ContextFunction::new(
            parse_pairs(args[0], 0, parse_root_type)?,
            parse_type(args[1], 0)?,
            parse_expression(args[2], 0)?,
        );
        let entries = match method.method.to_string().as_str() {
            "projection" => &mut result.projections,
            "subprogram" => &mut result.subprograms,
            _ => return Err(unsupported(method, "Unsupported library method")),
        };
        if entries.insert(name, function).is_some() {
            return Err(unsupported(method, "Duplicate library entry"));
        }
    }
    Ok(result)
}

fn string(value: &str) -> String {
    syn::LitStr::new(value, proc_macro2::Span::call_site())
        .to_token_stream()
        .to_string()
}
fn type_source(value: &DataType) -> String {
    match value {
        DataType::Boolean => "DataType::Boolean".into(),
        DataType::Number => "DataType::Number".into(),
        DataType::Text => "DataType::Text".into(),
        DataType::List { item } => {
            format!("DataType::List {{ item: Box::new({}) }}", type_source(item))
        }
        DataType::Record { fields } => format!(
            "DataType::Record {{ fields: BTreeMap::from([{}]) }}",
            fields
                .iter()
                .map(|(name, ty)| format!("({}.into(), {})", string(name), type_source(ty)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        DataType::Media { media_type } => format!(
            "DataType::Media {{ media_type: {}.into() }}",
            string(media_type)
        ),
        DataType::Named { name } => format!("DataType::Named {{ name: {}.into() }}", string(name)),
    }
}
pub fn json_source(value: &Value) -> String {
    match value {
        Value::Number(number) => format!(
            "{number}{}",
            if number.is_i64() {
                "i64"
            } else if number.is_u64() {
                "u64"
            } else {
                "f64"
            }
        ),
        Value::String(value) => string(value),
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(json_source)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Object(values) => format!(
            "{{{}}}",
            values
                .iter()
                .map(|(key, value)| format!("{}: {}", string(key), json_source(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => value.to_string(),
    }
}
fn expression_source(value: &ContextExpr) -> String {
    match value {
        ContextExpr::List { item_type, items } => format!(
            "ContextExpr::list({}, vec![{}])",
            type_source(item_type),
            items
                .iter()
                .map(expression_source)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ContextExpr::Construct { name, value } => format!(
            "ContextExpr::construct({}, {})",
            string(name),
            expression_source(value)
        ),
        ContextExpr::Variable { name } => format!("ContextExpr::variable({})", string(name)),
        ContextExpr::Filter {
            value,
            item,
            condition,
        } => format!(
            "ContextExpr::filter({}, {}, {})",
            expression_source(value),
            string(item),
            predicate_source(condition)
        ),
        ContextExpr::Sort {
            value,
            item,
            key,
            descending,
        } => format!(
            "ContextExpr::sort({}, {}, {}, {descending})",
            expression_source(value),
            string(item),
            expression_source(key)
        ),
        ContextExpr::Take { value, count } => format!(
            "ContextExpr::take({}, {count}usize)",
            expression_source(value)
        ),
        ContextExpr::Truncate { value, count } => format!(
            "ContextExpr::truncate({}, {count}usize)",
            expression_source(value)
        ),
        ContextExpr::Map { value, item, body } => format!(
            "ContextExpr::map({}, {}, {})",
            expression_source(value),
            string(item),
            expression_source(body)
        ),
        ContextExpr::GroupBy { value, item, key } => format!(
            "ContextExpr::group_by({}, {}, {})",
            expression_source(value),
            string(item),
            expression_source(key)
        ),
        ContextExpr::Dedup { value, item, key } => format!(
            "ContextExpr::dedup({}, {}, {})",
            expression_source(value),
            string(item),
            expression_source(key)
        ),
        ContextExpr::Record { fields } => {
            format!("ContextExpr::record({})", expressions_source(fields))
        }
        ContextExpr::Template { template, values } => format!(
            "ContextExpr::template({}, {})",
            string(template),
            expressions_source(values)
        ),
        ContextExpr::ToJson { value } => {
            format!("ContextExpr::to_json({})", expression_source(value))
        }
        ContextExpr::Measure { value, unit } => format!(
            "ContextExpr::measure({}, MeasureUnit::{unit:?})",
            expression_source(value)
        ),
        ContextExpr::Call {
            catalog,
            name,
            arguments,
        } => format!(
            "ContextExpr::call(LibraryKind::{catalog:?}, {}, {})",
            string(name),
            expressions_source(arguments)
        ),
        ContextExpr::Resource { name } => format!("ContextExpr::resource({})", string(name)),
        ContextExpr::Field { value, field } => format!(
            "ContextExpr::field({}, {})",
            expression_source(value),
            string(field)
        ),
        ContextExpr::Project { value, fields } => format!(
            "ContextExpr::project({}, &[{}])",
            expression_source(value),
            fields
                .iter()
                .map(|field| string(field))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ContextExpr::Literal { data_type, value } => format!(
            "ContextExpr::literal({}, serde_json::json!({}))",
            type_source(data_type),
            json_source(value)
        ),
    }
}
fn expressions_source(values: &BTreeMap<String, ContextExpr>) -> String {
    format!(
        "BTreeMap::from([{}])",
        values
            .iter()
            .map(|(name, value)| format!("({}.into(), {})", string(name), expression_source(value)))
            .collect::<Vec<_>>()
            .join(", ")
    )
}
fn predicate_source(value: &ContextPredicate) -> String {
    match value {
        ContextPredicate::Compare {
            left,
            operator,
            right,
        } => format!(
            "ContextPredicate::compare({}, Comparison::{operator:?}, {})",
            expression_source(left),
            expression_source(right)
        ),
        ContextPredicate::Contains { value, item } => format!(
            "ContextPredicate::contains({}, {})",
            expression_source(value),
            expression_source(item)
        ),
        ContextPredicate::Present { value } => {
            format!("ContextPredicate::present({})", expression_source(value))
        }
        ContextPredicate::Eq { left, right } => format!(
            "ContextPredicate::equal({}, {})",
            expression_source(left),
            expression_source(right)
        ),
        ContextPredicate::And { items } | ContextPredicate::Or { items } => format!(
            "ContextPredicate::{}(vec![{}])",
            if matches!(value, ContextPredicate::And { .. }) {
                "all"
            } else {
                "any"
            },
            items
                .iter()
                .map(predicate_source)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ContextPredicate::Not { item } => {
            format!("ContextPredicate::negate({})", predicate_source(item))
        }
    }
}
fn blocks_source(blocks: &[ContextBlock], indent: usize) -> String {
    if blocks.is_empty() {
        return "vec![]".into();
    }
    let mut out = String::from("vec![\n");
    for block in blocks {
        out.push_str(&" ".repeat(indent + 4));
        out.push_str(&match block {
            ContextBlock::ForEach {id, value, item, items} => format!("ContextBlock::for_each({}, {}, {}, {})", string(id), expression_source(value), string(item), blocks_source(items, indent + 4)),
            ContextBlock::Group {id, label, items} => format!("ContextBlock::group({}, {}, {})", string(id), string(label), blocks_source(items, indent + 4)),
            ContextBlock::Emit {id, role, format, value} => format!("ContextBlock::emit({}, FragmentRole::{role:?}, FragmentFormat::{format:?},\n{}{}\n{})", string(id), " ".repeat(indent + 8), expression_source(value), " ".repeat(indent + 4)),
            ContextBlock::If {id, condition, then, otherwise} => format!("ContextBlock::branch({},\n{}{},\n{}{},\n{}{}\n{})", string(id), " ".repeat(indent + 8), predicate_source(condition), " ".repeat(indent + 8), blocks_source(then, indent + 8), " ".repeat(indent + 8), blocks_source(otherwise, indent + 8), " ".repeat(indent + 4)),
        });
        out.push_str(",\n");
    }
    out.push_str(&" ".repeat(indent));
    out.push(']');
    out
}

fn syntax(error: syn::Error) -> Diagnostic {
    let location = error.span().start();
    Diagnostic::new(
        "context_syntax",
        format!("$source:{}:{}", location.line, location.column + 1),
        error.to_string(),
    )
}
fn unsupported(value: &impl Spanned, message: &str) -> Diagnostic {
    let location = value.span().start();
    Diagnostic::new(
        "context_source",
        format!("$source:{}:{}", location.line, location.column + 1),
        message,
    )
}
fn parse_file(file: &syn::File) -> Parsed<ContextStrategy> {
    strategy(return_expression(file, "strategy", "ContextStrategy")?)
}
fn return_expression<'a>(file: &'a syn::File, name: &str, return_type: &str) -> Parsed<&'a Expr> {
    if !file.attrs.is_empty() || file.shebang.is_some() {
        return Err(unsupported(
            file,
            "File attributes and shebangs are not supported",
        ));
    }
    let expected = syn::parse_file(IMPORTS).map_err(syntax)?;
    if file.items.len() != expected.items.len() + 1 {
        return Err(unsupported(
            file,
            "Only the documented imports and strategy() function are allowed",
        ));
    }
    for (item, expected) in file.items.iter().zip(&expected.items) {
        if item.to_token_stream().to_string() != expected.to_token_stream().to_string() {
            return Err(unsupported(item, "Unsupported import or declaration"));
        }
    }
    let Some(Item::Fn(function)) = file.items.last() else {
        return Err(unsupported(file, "Expected strategy() function"));
    };
    let expected: syn::ItemFn =
        syn::parse_str(&format!("pub fn {name}() -> {return_type} {{}} ")).map_err(syntax)?;
    if !function.attrs.is_empty()
        || function.sig.to_token_stream().to_string() != expected.sig.to_token_stream().to_string()
        || function.vis.to_token_stream().to_string() != "pub"
    {
        return Err(unsupported(
            function,
            "Expected pub fn strategy() -> ContextStrategy",
        ));
    }
    let [Stmt::Expr(expression, None)] = function.block.stmts.as_slice() else {
        return Err(unsupported(
            function,
            "The function must return a single strategy builder; statements and arbitrary code are forbidden",
        ));
    };
    Ok(expression)
}
fn path(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(value)
            if value.attrs.is_empty()
                && value.qself.is_none()
                && value.path.leading_colon.is_none()
                && value.path.segments.iter().all(|s| s.arguments.is_none()) =>
        {
            Some(
                value
                    .path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::"),
            )
        }
        _ => None,
    }
}
fn call<'a>(expr: &'a Expr, name: &str, count: usize) -> Parsed<Vec<&'a Expr>> {
    if let Expr::Call(value) = expr
        && value.attrs.is_empty()
        && path(&value.func).as_deref() == Some(name)
        && value.args.len() == count
    {
        return Ok(value.args.iter().collect());
    }
    Err(unsupported(
        expr,
        &format!("Expected {name} with {count} literal arguments"),
    ))
}
fn text(expr: &Expr) -> Parsed<String> {
    if let Expr::Lit(value) = expr
        && value.attrs.is_empty()
        && let Lit::Str(value) = &value.lit
    {
        return Ok(value.value());
    }
    Err(unsupported(expr, "Expected a string literal"))
}
fn owned_text(expr: &Expr) -> Parsed<String> {
    if let Expr::MethodCall(value) = expr
        && value.attrs.is_empty()
        && value.method == "into"
        && value.turbofish.is_none()
        && value.args.is_empty()
    {
        return text(&value.receiver);
    }
    Err(unsupported(
        expr,
        "Expected a string literal followed by .into()",
    ))
}
fn vec_items(expr: &Expr) -> Parsed<Vec<Expr>> {
    if let Expr::Macro(value) = expr
        && value.attrs.is_empty()
        && value.mac.path.is_ident("vec")
        && matches!(value.mac.delimiter, syn::MacroDelimiter::Bracket(_))
    {
        struct Items(Vec<Expr>);
        impl Parse for Items {
            fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
                Ok(Self(
                    Punctuated::<Expr, Token![,]>::parse_terminated(input)?
                        .into_iter()
                        .collect(),
                ))
            }
        }
        return syn::parse2::<Items>(value.mac.tokens.clone())
            .map(|items| items.0)
            .map_err(syntax);
    }
    Err(unsupported(
        expr,
        "Expected vec![...] with explicit entries",
    ))
}
fn strategy(expr: &Expr) -> Parsed<ContextStrategy> {
    let mut value = expr;
    let mut methods = vec![];
    while let Expr::MethodCall(method) = value {
        if !method.attrs.is_empty() || method.turbofish.is_some() || methods.len() >= 4096 {
            return Err(unsupported(method, "Unsupported strategy builder"));
        }
        methods.push(method);
        value = &method.receiver;
    }
    let constructor = if let Expr::Call(call) = value {
        path(&call.func)
    } else {
        None
    };
    let mut result = match constructor.as_deref() {
        Some("ContextStrategy::new_v2") => {
            let args = call(value, "ContextStrategy::new_v2", 2)?;
            ContextStrategy::new_v2(&text(args[0])?, &text(args[1])?)
        }
        _ => {
            let args = call(value, "ContextStrategy::new", 2)?;
            ContextStrategy::new(&text(args[0])?, &text(args[1])?)
        }
    };
    let mut has_program = false;
    for method in methods.into_iter().rev() {
        let args: Vec<_> = method.args.iter().collect();
        match (method.method.to_string().as_str(), args.as_slice()) {
            ("define_type", [name, data_type]) if !has_program => {
                if result
                    .types
                    .insert(text(name)?, parse_type(data_type, 0)?)
                    .is_some()
                {
                    return Err(unsupported(method, "Duplicate named type definition"));
                }
            }
            ("require", [name, data_type]) if !has_program => {
                let name = text(name)?;
                if result
                    .requirements
                    .insert(name, parse_type(data_type, 0)?)
                    .is_some()
                {
                    return Err(unsupported(method, "Duplicate resource requirement"));
                }
            }
            ("capability", [value]) if !has_program => {
                let args = call(value, "ContextCapability::new", 3)?;
                result.capabilities.push(ContextCapability::new(
                    &text(args[0])?,
                    parse_type(args[1], 0)?,
                    parse_type(args[2], 0)?,
                ));
            }
            ("with_program", [value]) if !has_program => {
                result.program = parse_blocks(value, 0)?;
                has_program = true;
            }
            _ => {
                return Err(unsupported(
                    method,
                    "Unsupported, repeated or misplaced strategy builder method",
                ));
            }
        }
    }
    if !has_program {
        return Err(unsupported(expr, "A strategy must declare its program"));
    }
    Ok(result)
}
fn limit(expr: &Expr, depth: usize) -> Parsed<()> {
    if depth > 64 {
        Err(unsupported(
            expr,
            "Context source nesting exceeds 64 levels",
        ))
    } else {
        Ok(())
    }
}
fn parse_type(expr: &Expr, depth: usize) -> Parsed<DataType> {
    limit(expr, depth)?;
    match path(expr).as_deref() {
        Some("DataType::Boolean") => return Ok(DataType::Boolean),
        Some("DataType::Number") => return Ok(DataType::Number),
        Some("DataType::Text") => return Ok(DataType::Text),
        _ => {}
    }
    let Expr::Struct(value) = expr else {
        return Err(unsupported(expr, "Expected a DataType constructor"));
    };
    if !value.attrs.is_empty()
        || value.qself.is_some()
        || value.rest.is_some()
        || value.fields.len() != 1
    {
        return Err(unsupported(expr, "Unsupported DataType constructor"));
    }
    let field = &value.fields[0];
    if field.colon_token.is_none() || !field.attrs.is_empty() {
        return Err(unsupported(expr, "DataType fields must be explicit"));
    }
    let name = value.path.to_token_stream().to_string().replace(' ', "");
    match (
        name.as_str(),
        field.member.to_token_stream().to_string().as_str(),
    ) {
        ("DataType::List", "item") => Ok(DataType::List {
            item: Box::new(parse_type(call(&field.expr, "Box::new", 1)?[0], depth + 1)?),
        }),
        ("DataType::Media", "media_type") => Ok(DataType::Media {
            media_type: owned_text(&field.expr)?,
        }),
        ("DataType::Named", "name") => Ok(DataType::Named {
            name: owned_text(&field.expr)?,
        }),
        ("DataType::Record", "fields") => {
            let args = call(&field.expr, "BTreeMap::from", 1)?;
            let Expr::Array(values) = args[0] else {
                return Err(unsupported(
                    expr,
                    "Record fields must be a literal array of pairs",
                ));
            };
            if !values.attrs.is_empty() {
                return Err(unsupported(expr, "Array attributes are forbidden"));
            }
            let mut fields = BTreeMap::new();
            for pair in &values.elems {
                let Expr::Tuple(pair) = pair else {
                    return Err(unsupported(pair, "Expected a field name/type pair"));
                };
                if !pair.attrs.is_empty() || pair.elems.len() != 2 {
                    return Err(unsupported(pair, "Expected a field name/type pair"));
                }
                if fields
                    .insert(
                        owned_text(&pair.elems[0])?,
                        parse_type(&pair.elems[1], depth + 1)?,
                    )
                    .is_some()
                {
                    return Err(unsupported(pair, "Duplicate record field"));
                }
            }
            Ok(DataType::Record { fields })
        }
        _ => Err(unsupported(expr, "Unsupported DataType constructor")),
    }
}

// A schema's nesting is independent of the expression or registry containing it.
// Match validate_type's root depth even when parsing a name/schema pair.
fn parse_root_type(expr: &Expr, _container_depth: usize) -> Parsed<DataType> {
    parse_type(expr, 0)
}
fn parse_expression(expr: &Expr, depth: usize) -> Parsed<ContextExpr> {
    limit(expr, depth)?;
    let Expr::Call(value) = expr else {
        return Err(unsupported(expr, "Expected a ContextExpr constructor"));
    };
    match path(&value.func).as_deref() {
        Some("ContextExpr::construct") => {
            let args = call(expr, "ContextExpr::construct", 2)?;
            Ok(ContextExpr::construct(
                &text(args[0])?,
                parse_expression(args[1], depth + 1)?,
            ))
        }
        Some("ContextExpr::list") => {
            let args = call(expr, "ContextExpr::list", 2)?;
            Ok(ContextExpr::list(
                parse_type(args[0], 0)?,
                vec_items(args[1])?
                    .iter()
                    .map(|item| parse_expression(item, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?,
            ))
        }
        Some("ContextExpr::variable") => Ok(ContextExpr::variable(&text(
            call(expr, "ContextExpr::variable", 1)?[0],
        )?)),
        Some("ContextExpr::filter") => {
            let args = call(expr, "ContextExpr::filter", 3)?;
            Ok(ContextExpr::filter(
                parse_expression(args[0], depth + 1)?,
                &text(args[1])?,
                parse_predicate(args[2], depth + 1)?,
            ))
        }
        Some("ContextExpr::sort") => {
            let args = call(expr, "ContextExpr::sort", 4)?;
            let Expr::Lit(boolean) = args[3] else {
                return Err(unsupported(args[3], "Expected literal boolean"));
            };
            let Lit::Bool(boolean_value) = &boolean.lit else {
                return Err(unsupported(args[3], "Expected literal boolean"));
            };
            if !boolean.attrs.is_empty() {
                return Err(unsupported(args[3], "Literal attributes are forbidden"));
            }
            Ok(ContextExpr::sort(
                parse_expression(args[0], depth + 1)?,
                &text(args[1])?,
                parse_expression(args[2], depth + 1)?,
                boolean_value.value,
            ))
        }
        Some(name @ ("ContextExpr::take" | "ContextExpr::truncate")) => {
            let args = call(expr, name, 2)?;
            let Expr::Lit(number) = args[1] else {
                return Err(unsupported(args[1], "Expected usize literal"));
            };
            let Lit::Int(value) = &number.lit else {
                return Err(unsupported(args[1], "Expected usize literal"));
            };
            if !number.attrs.is_empty() || value.suffix() != "usize" {
                return Err(unsupported(args[1], "Expected explicit usize literal"));
            }
            let expression = parse_expression(args[0], depth + 1)?;
            let count = value.base10_parse().map_err(syntax)?;
            Ok(if name == "ContextExpr::truncate" {
                ContextExpr::truncate(expression, count)
            } else {
                ContextExpr::take(expression, count)
            })
        }
        Some(name @ ("ContextExpr::map" | "ContextExpr::group_by" | "ContextExpr::dedup")) => {
            let args = call(expr, name, 3)?;
            let value = parse_expression(args[0], depth + 1)?;
            let item = text(args[1])?;
            let body = parse_expression(args[2], depth + 1)?;
            Ok(match name {
                "ContextExpr::map" => ContextExpr::map(value, &item, body),
                "ContextExpr::group_by" => ContextExpr::group_by(value, &item, body),
                _ => ContextExpr::dedup(value, &item, body),
            })
        }
        Some("ContextExpr::record") => Ok(ContextExpr::record(parse_expressions(
            call(expr, "ContextExpr::record", 1)?[0],
            depth + 1,
        )?)),
        Some("ContextExpr::template") => {
            let args = call(expr, "ContextExpr::template", 2)?;
            Ok(ContextExpr::template(
                &text(args[0])?,
                parse_expressions(args[1], depth + 1)?,
            ))
        }
        Some("ContextExpr::to_json") => Ok(ContextExpr::to_json(parse_expression(
            call(expr, "ContextExpr::to_json", 1)?[0],
            depth + 1,
        )?)),
        Some("ContextExpr::measure") => {
            let args = call(expr, "ContextExpr::measure", 2)?;
            let unit = match path(args[1]).as_deref() {
                Some("MeasureUnit::Bytes") => MeasureUnit::Bytes,
                Some("MeasureUnit::Items") => MeasureUnit::Items,
                Some("MeasureUnit::Media") => MeasureUnit::Media,
                _ => return Err(unsupported(args[1], "Unknown measurement unit")),
            };
            Ok(ContextExpr::measure(
                parse_expression(args[0], depth + 1)?,
                unit,
            ))
        }
        Some("ContextExpr::call") => {
            let args = call(expr, "ContextExpr::call", 3)?;
            let catalog = match path(args[0]).as_deref() {
                Some("LibraryKind::Projection") => LibraryKind::Projection,
                Some("LibraryKind::Subprogram") => LibraryKind::Subprogram,
                _ => return Err(unsupported(args[0], "Unknown context catalogue")),
            };
            Ok(ContextExpr::call(
                catalog,
                &text(args[1])?,
                parse_expressions(args[2], depth + 1)?,
            ))
        }
        Some("ContextExpr::resource") => Ok(ContextExpr::resource(&text(
            call(expr, "ContextExpr::resource", 1)?[0],
        )?)),
        Some("ContextExpr::field") => {
            let args = call(expr, "ContextExpr::field", 2)?;
            Ok(ContextExpr::field(
                parse_expression(args[0], depth + 1)?,
                &text(args[1])?,
            ))
        }
        Some("ContextExpr::project") => {
            let args = call(expr, "ContextExpr::project", 2)?;
            let Expr::Reference(reference) = args[1] else {
                return Err(unsupported(
                    expr,
                    "Projection fields must be a borrowed literal array",
                ));
            };
            if !reference.attrs.is_empty() || reference.mutability.is_some() {
                return Err(unsupported(expr, "Invalid projection fields"));
            }
            let Expr::Array(fields) = reference.expr.as_ref() else {
                return Err(unsupported(
                    expr,
                    "Projection fields must be a literal array",
                ));
            };
            if !fields.attrs.is_empty() {
                return Err(unsupported(expr, "Invalid projection fields"));
            }
            Ok(ContextExpr::Project {
                value: Box::new(parse_expression(args[0], depth + 1)?),
                fields: fields.elems.iter().map(text).collect::<Parsed<_>>()?,
            })
        }
        Some("ContextExpr::literal") => {
            let args = call(expr, "ContextExpr::literal", 2)?;
            let Expr::Macro(value) = args[1] else {
                return Err(unsupported(
                    expr,
                    "Expected serde_json::json! with literal JSON",
                ));
            };
            if !value.attrs.is_empty()
                || value.mac.path.to_token_stream().to_string() != "serde_json :: json"
                || !matches!(value.mac.delimiter, syn::MacroDelimiter::Paren(_))
            {
                return Err(unsupported(
                    expr,
                    "Only the literal serde_json::json! macro is allowed",
                ));
            }
            Ok(ContextExpr::literal(
                parse_type(args[0], 0)?,
                syn::parse2::<JsonLiteral>(value.mac.tokens.clone())
                    .map_err(syntax)?
                    .0,
            ))
        }
        _ => Err(unsupported(
            expr,
            "Arbitrary expressions and calls are forbidden",
        )),
    }
}
fn parse_pairs<T>(
    expr: &Expr,
    depth: usize,
    parse_value: fn(&Expr, usize) -> Parsed<T>,
) -> Parsed<BTreeMap<String, T>> {
    limit(expr, depth)?;
    let args = call(expr, "BTreeMap::from", 1)?;
    let Expr::Array(array) = args[0] else {
        return Err(unsupported(expr, "Expected literal array of pairs"));
    };
    if !array.attrs.is_empty() {
        return Err(unsupported(expr, "Array attributes are forbidden"));
    }
    let mut values = BTreeMap::new();
    for pair in &array.elems {
        let Expr::Tuple(pair) = pair else {
            return Err(unsupported(pair, "Expected name/value pair"));
        };
        if !pair.attrs.is_empty() || pair.elems.len() != 2 {
            return Err(unsupported(pair, "Expected name/value pair"));
        }
        if values
            .insert(
                owned_text(&pair.elems[0])?,
                parse_value(&pair.elems[1], depth + 1)?,
            )
            .is_some()
        {
            return Err(unsupported(pair, "Duplicate argument name"));
        }
    }
    Ok(values)
}
fn parse_expressions(expr: &Expr, depth: usize) -> Parsed<BTreeMap<String, ContextExpr>> {
    parse_pairs(expr, depth, parse_expression)
}
fn parse_predicate(expr: &Expr, depth: usize) -> Parsed<ContextPredicate> {
    limit(expr, depth)?;
    let Expr::Call(value) = expr else {
        return Err(unsupported(expr, "Expected a ContextPredicate constructor"));
    };
    match path(&value.func).as_deref() {
        Some("ContextPredicate::compare") => {
            let args = call(expr, "ContextPredicate::compare", 3)?;
            let operator = match path(args[1]).as_deref() {
                Some("Comparison::Lt") => Comparison::Lt,
                Some("Comparison::Lte") => Comparison::Lte,
                Some("Comparison::Gt") => Comparison::Gt,
                Some("Comparison::Gte") => Comparison::Gte,
                Some("Comparison::Ne") => Comparison::Ne,
                _ => return Err(unsupported(args[1], "Unknown comparison operator")),
            };
            Ok(ContextPredicate::compare(
                parse_expression(args[0], depth + 1)?,
                operator,
                parse_expression(args[2], depth + 1)?,
            ))
        }
        Some("ContextPredicate::contains") => {
            let args = call(expr, "ContextPredicate::contains", 2)?;
            Ok(ContextPredicate::contains(
                parse_expression(args[0], depth + 1)?,
                parse_expression(args[1], depth + 1)?,
            ))
        }
        Some("ContextPredicate::present") => Ok(ContextPredicate::present(parse_expression(
            call(expr, "ContextPredicate::present", 1)?[0],
            depth + 1,
        )?)),
        Some("ContextPredicate::equal") => {
            let args = call(expr, "ContextPredicate::equal", 2)?;
            Ok(ContextPredicate::equal(
                parse_expression(args[0], depth + 1)?,
                parse_expression(args[1], depth + 1)?,
            ))
        }
        Some(name @ ("ContextPredicate::all" | "ContextPredicate::any")) => {
            let items = vec_items(call(expr, name, 1)?[0])?
                .iter()
                .map(|value| parse_predicate(value, depth + 1))
                .collect::<Parsed<_>>()?;
            Ok(if name.ends_with("all") {
                ContextPredicate::all(items)
            } else {
                ContextPredicate::any(items)
            })
        }
        Some("ContextPredicate::negate") => Ok(ContextPredicate::negate(parse_predicate(
            call(expr, "ContextPredicate::negate", 1)?[0],
            depth + 1,
        )?)),
        _ => Err(unsupported(expr, "Unsupported context predicate")),
    }
}
fn parse_blocks(expr: &Expr, depth: usize) -> Parsed<Vec<ContextBlock>> {
    limit(expr, depth)?;
    vec_items(expr)?
        .iter()
        .map(|expr| {
            let Expr::Call(value) = expr else {
                return Err(unsupported(expr, "Expected a ContextBlock constructor"));
            };
            match path(&value.func).as_deref() {
                Some("ContextBlock::group") => {
                    let args = call(expr, "ContextBlock::group", 3)?;
                    Ok(ContextBlock::group(
                        &text(args[0])?,
                        &text(args[1])?,
                        parse_blocks(args[2], depth + 1)?,
                    ))
                }
                Some("ContextBlock::for_each") => {
                    let args = call(expr, "ContextBlock::for_each", 4)?;
                    Ok(ContextBlock::for_each(
                        &text(args[0])?,
                        parse_expression(args[1], depth + 1)?,
                        &text(args[2])?,
                        parse_blocks(args[3], depth + 1)?,
                    ))
                }
                Some("ContextBlock::emit") => {
                    let args = call(expr, "ContextBlock::emit", 4)?;
                    let role = match path(args[1]).as_deref() {
                        Some("FragmentRole::Instruction") => FragmentRole::Instruction,
                        Some("FragmentRole::Data") => FragmentRole::Data,
                        _ => return Err(unsupported(args[1], "Invalid fragment role")),
                    };
                    let format = match path(args[2]).as_deref() {
                        Some("FragmentFormat::Text") => FragmentFormat::Text,
                        Some("FragmentFormat::Json") => FragmentFormat::Json,
                        Some("FragmentFormat::Media") => FragmentFormat::Media,
                        Some("FragmentFormat::AdkMessages") => FragmentFormat::AdkMessages,
                        _ => return Err(unsupported(args[2], "Invalid fragment representation")),
                    };
                    Ok(ContextBlock::emit(
                        &text(args[0])?,
                        role,
                        format,
                        parse_expression(args[3], depth + 1)?,
                    ))
                }
                Some("ContextBlock::branch") => {
                    let args = call(expr, "ContextBlock::branch", 4)?;
                    Ok(ContextBlock::branch(
                        &text(args[0])?,
                        parse_predicate(args[1], depth + 1)?,
                        parse_blocks(args[2], depth + 1)?,
                        parse_blocks(args[3], depth + 1)?,
                    ))
                }
                _ => Err(unsupported(expr, "Unsupported context block")),
            }
        })
        .collect()
}

struct JsonLiteral(Value);
pub fn parse_json_tokens(tokens: proc_macro2::TokenStream) -> Parsed<Value> {
    syn::parse2::<JsonLiteral>(tokens)
        .map(|literal| literal.0)
        .map_err(syntax)
}
impl Parse for JsonLiteral {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        json_literal(input, 0).map(Self)
    }
}
fn json_literal(input: ParseStream<'_>, depth: usize) -> syn::Result<Value> {
    if depth > 64 {
        return Err(input.error("JSON nesting exceeds 64 levels"));
    }
    if input.peek(syn::token::Brace) {
        let content;
        syn::braced!(content in input);
        let mut result = Map::new();
        while !content.is_empty() {
            let name: syn::LitStr = content.parse()?;
            content.parse::<Token![:]>()?;
            if result
                .insert(name.value(), json_literal(&content, depth + 1)?)
                .is_some()
            {
                return Err(content.error("Duplicate JSON key"));
            }
            if !content.is_empty() {
                content.parse::<Token![,]>()?;
            }
        }
        return Ok(Value::Object(result));
    }
    if input.peek(syn::token::Bracket) {
        let content;
        syn::bracketed!(content in input);
        let mut result = vec![];
        while !content.is_empty() {
            result.push(json_literal(&content, depth + 1)?);
            if !content.is_empty() {
                content.parse::<Token![,]>()?;
            }
        }
        return Ok(Value::Array(result));
    }
    if input.peek(syn::LitStr) {
        return input
            .parse::<syn::LitStr>()
            .map(|value| Value::String(value.value()));
    }
    if input.peek(syn::LitBool) {
        return input
            .parse::<syn::LitBool>()
            .map(|value| Value::Bool(value.value));
    }
    let negative = if input.peek(Token![-]) {
        input.parse::<Token![-]>()?;
        true
    } else {
        false
    };
    if input.peek(syn::LitInt) || input.peek(syn::LitFloat) {
        let literal: Lit = input.parse()?;
        let signed = |digits: &str| format!("{}{digits}", if negative { "-" } else { "" });
        let invalid = || input.error("JSON number has an unsupported suffix, sign or range");
        return match literal {
            Lit::Int(value) if value.suffix() == "i64" => signed(value.base10_digits())
                .parse::<i64>()
                .map(Value::from)
                .map_err(|_| invalid()),
            Lit::Int(value) if value.suffix() == "u64" && !negative => value
                .base10_parse::<u64>()
                .map(Value::from)
                .map_err(|_| invalid()),
            Lit::Int(value) if value.suffix().is_empty() => signed(value.base10_digits())
                .parse::<i32>()
                .map(Value::from)
                .map_err(|_| invalid()),
            Lit::Float(value) if value.suffix().is_empty() || value.suffix() == "f64" => {
                signed(value.base10_digits())
                    .parse::<serde_json::Number>()
                    .map(Value::Number)
                    .map_err(|_| invalid())
            }
            _ => Err(invalid()),
        };
    }
    if !negative && input.peek(syn::Ident) && input.parse::<syn::Ident>()? == "null" {
        return Ok(Value::Null);
    }
    Err(input.error("Only JSON literals are allowed; expressions are forbidden"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nested_type(depth: usize) -> DataType {
        let mut value = DataType::Text;
        for index in (0..depth).rev() {
            value = DataType::Record {
                fields: BTreeMap::from([(format!("level_{index}"), value)]),
            };
        }
        value
    }

    #[test]
    fn all_source_readers_preserve_sixty_four_type_levels() {
        let deep = nested_type(64);
        let strategy = ContextStrategy::new_v2("deep-source", "Deep source")
            .define_type("Deep", deep.clone())
            .require("deep", deep.clone())
            .capability(ContextCapability::new(
                "inspect",
                deep.clone(),
                deep.clone(),
            ));
        let source = generate(&strategy).unwrap();
        assert!(source_complexity(&source).unwrap() > INLINE_SYNTAX_DEPTH);
        assert_eq!(parse(&source).unwrap(), strategy);

        let types = BTreeMap::from([("Deep".into(), deep.clone())]);
        let source = generate_types(&types).unwrap();
        assert_eq!(parse_types(&source).unwrap(), types);

        let library = ContextLibrary::new().projection(
            "identity",
            ContextFunction::new(
                BTreeMap::from([("value".into(), deep.clone())]),
                deep,
                ContextExpr::variable("value"),
            ),
        );
        let source = generate_library(&library).unwrap();
        assert_eq!(parse_library(&source).unwrap(), library);
    }

    #[test]
    fn semantic_type_depth_remains_bounded_after_syntax_preflight() {
        let strategy =
            ContextStrategy::new_v2("too-deep", "Too deep").require("deep", nested_type(64));
        let valid = generate(&strategy).unwrap();
        let invalid = valid.replace("DataType::Text", &type_source(&nested_type(1)));
        assert!(source_complexity(&invalid).is_ok());
        let errors = parse(&invalid).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("64 levels"))
        );
        assert!(
            generate(
                &ContextStrategy::new_v2("too-deep", "Too deep").require("deep", nested_type(65))
            )
            .is_err()
        );
    }

    #[test]
    fn supported_block_depth_and_wide_programs_are_not_syntax_bombs() {
        let mut blocks = Vec::new();
        for index in 0..64 {
            blocks = vec![ContextBlock::group(
                &format!("group_{index}"),
                "Group",
                blocks,
            )];
        }
        let strategy = ContextStrategy::new_v2("deep-blocks", "Deep blocks").with_program(blocks);
        assert_eq!(parse(&generate(&strategy).unwrap()).unwrap(), strategy);

        let strategy = ContextStrategy::new_v2("wide-blocks", "Wide blocks").with_program(
            (0..4096)
                .map(|index| ContextBlock::group(&format!("group_{index}"), "Group", Vec::new()))
                .collect(),
        );
        let source = generate(&strategy).unwrap();
        assert!(source_complexity(&source).unwrap() <= INLINE_SYNTAX_DEPTH);
        assert_eq!(parse(&source).unwrap(), strategy);
    }

    #[test]
    fn the_existing_builder_chain_limit_is_preserved() {
        let mut strategy = ContextStrategy::new_v2("wide-types", "Wide types");
        for index in 0..4095 {
            strategy = strategy.require(&format!("value_{index}"), DataType::Text);
        }
        let source = generate(&strategy).unwrap();
        assert!(source_complexity(&source).unwrap() > INLINE_SYNTAX_DEPTH);
        assert_eq!(parse(&source).unwrap(), strategy);
    }

    #[test]
    fn preflight_uses_rust_tokens_instead_of_characters_inside_literals() {
        let literal = "{[(/* // )]}".repeat(1000);
        let strategy = ContextStrategy::new_v2("literal", &literal);
        let source = generate(&strategy).unwrap();
        let raw = source.replace(&string(&literal), &format!("r###\"{literal}\"###"));
        let commented = raw.replacen(
            IMPORTS,
            &format!(
                "/* {} */\n// {}\n{IMPORTS}",
                "/* [( */".repeat(1000),
                "{[(".repeat(1000)
            ),
            1,
        );
        assert!(source_complexity(&commented).unwrap() <= INLINE_SYNTAX_DEPTH);
        assert_eq!(parse(&commented).unwrap(), strategy);
        assert_eq!(
            source_complexity("'(' ')' '[' ']' '{' '}' b'(' br#\"(((\"#").unwrap(),
            0
        );
    }

    #[test]
    fn malicious_syntax_is_rejected_before_recursive_parsing() {
        for expression in [
            format!("{}0{}", "(".repeat(2048), ")".repeat(2048)),
            format!("{}true", "!".repeat(2048)),
            format!("value{}", ".field".repeat(MAX_SYNTAX_CHAIN + 1)),
            format!("{}0", "return ".repeat(2048)),
            format!("{}{{}}", "if true {} else ".repeat(2048)),
        ] {
            let source = format!(
                "{HEADER_V2}\n{IMPORTS}\npub fn strategy() -> ContextStrategy {{ {expression} }}"
            );
            assert_eq!(parse(&source).unwrap_err()[0].code, "context_source_depth");
            let library = source.replace(HEADER_V2, "// @zedflow-context-library 1");
            assert_eq!(
                parse_library(&library).unwrap_err()[0].code,
                "context_source_depth"
            );
            let types = source.replace(HEADER_V2, "// @zedflow-types 1");
            assert_eq!(
                parse_types(&types).unwrap_err()[0].code,
                "context_source_depth"
            );
        }
        assert!(source_complexity(&"(".repeat(100_000)).is_err());
    }
}
