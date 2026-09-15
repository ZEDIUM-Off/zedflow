//! Strict, executable Rust builders for bridge composition definitions.
use crate::composition::*;
use anyhow::{Context, Result, bail, ensure};
use quote::ToTokens;
use syn::{Expr, Item, Stmt};
use zf_context::context_source;
use zf_core::diagnostics::Diagnostic;

const HEADER: &str = "// @zedflow-bridge 1";

/// Lets context-package validation inspect bridges without depending on their
/// representation or executing any imported flow.
pub struct PackageBridgeValidator;

impl zf_context::context_package::BridgeArtifactValidator for PackageBridgeValidator {
    fn validate(
        &self,
        source: &str,
    ) -> Result<zf_context::context_package::BridgeDependencies, Vec<Diagnostic>> {
        let bridge = parse(source)?;
        Ok(zf_context::context_package::BridgeDependencies {
            requires: bridge.requires.into_iter().collect(),
            flows: bridge
                .imports
                .into_values()
                .map(|import| import.flow)
                .collect(),
        })
    }
}

fn text(value: &str) -> String {
    syn::LitStr::new(value, proc_macro2::Span::call_site())
        .to_token_stream()
        .to_string()
}
fn endpoint(value: &Endpoint) -> String {
    format!(
        "Endpoint::new({}, {})",
        text(&value.instance),
        text(&value.port)
    )
}
fn connection(value: &Connection) -> String {
    let mut out = format!(
        "Connection::new({}, {}, RouteMode::{:?}, InvocationKind::{:?})",
        endpoint(&value.from),
        endpoint(&value.to),
        value.mode,
        value.invocation
    );
    if let Some(name) = &value.tool_name {
        out.push_str(&format!(".tool({})", text(name)));
    }
    if let Some(condition) = &value.condition {
        out.push_str(&format!(
            ".when(serde_json::json!({}))",
            context_source::json_source(condition)
        ));
    }
    out
}
fn diagnostic(error: anyhow::Error) -> Vec<Diagnostic> {
    vec![Diagnostic::new(
        "bridge_source",
        "$source",
        format!("{error:#}"),
    )]
}

pub fn generate(bridge: &BridgeDefinition) -> Result<String, Vec<Diagnostic>> {
    let mut out = format!(
        "{HEADER}\nuse zf_flows::composition::*;\n\npub fn bridge() -> BridgeDefinition {{\n    BridgeDefinition::new()"
    );
    for required in &bridge.requires {
        out.push_str(&format!("\n        .require({})", text(required)));
    }
    for (alias, import) in &bridge.imports {
        out.push_str(&match &import.reuse {
            Some(instance) => format!(
                "\n        .reuse({}, {}, {})",
                text(alias),
                text(&import.flow),
                text(instance)
            ),
            None => format!("\n        .import({}, {})", text(alias), text(&import.flow)),
        });
    }
    for (id, value) in &bridge.connections {
        out.push_str(&format!(
            "\n        .connect({}, {})",
            text(id),
            connection(value)
        ));
    }
    for (id, value) in &bridge.bindings {
        if !value.permissions.read {
            return Err(diagnostic(anyhow::anyhow!(
                "Data binding {id} must grant read access"
            )));
        }
        out.push_str(&format!(
            "\n        .bind({}, {}, {}, DataPermissions::{}())",
            text(id),
            endpoint(&value.from),
            endpoint(&value.to),
            if value.permissions.write {
                "read_write"
            } else {
                "read_only"
            }
        ));
    }
    out.push_str("\n}\n");
    if out.len() > context_source::MAX_SOURCE_BYTES {
        return Err(diagnostic(anyhow::anyhow!("Bridge source exceeds 1 MiB")));
    }
    Ok(out)
}

pub fn parse(source: &str) -> Result<BridgeDefinition, Vec<Diagnostic>> {
    parse_inner(source).map_err(diagnostic)
}
fn parse_inner(source: &str) -> Result<BridgeDefinition> {
    ensure!(
        source.len() <= context_source::MAX_SOURCE_BYTES,
        "Bridge source exceeds 1 MiB"
    );
    ensure!(source.lines().next() == Some(HEADER), "Expected {HEADER}");
    let file = syn::parse_file(source)?;
    let function = file
        .items
        .iter()
        .find_map(|item| {
            if let Item::Fn(f) = item {
                Some(f)
            } else {
                None
            }
        })
        .context("Missing bridge function")?;
    let [Stmt::Expr(expression, None)] = function.block.stmts.as_slice() else {
        bail!("Bridge function must return one builder expression")
    };
    let mut chain = vec![];
    let mut root = expression;
    while let Expr::MethodCall(method) = root {
        ensure!(chain.len() < 4096, "Bridge builder exceeds 4096 operations");
        chain.push(method);
        root = &method.receiver;
    }
    call(root, "BridgeDefinition::new", 0)?;
    let mut bridge = BridgeDefinition::default();
    for method in chain.into_iter().rev() {
        let args: Vec<_> = method.args.iter().collect();
        match method.method.to_string().as_str() {
            "require" => {
                count(&args, 1)?;
                ensure!(
                    bridge.requires.insert(literal(args[0])?),
                    "Duplicate bridge requirement"
                );
            }
            "import" | "reuse" => {
                let reuse = method.method == "reuse";
                count(&args, if reuse { 3 } else { 2 })?;
                let value = FlowImport {
                    flow: literal(args[1])?,
                    reuse: if reuse { Some(literal(args[2])?) } else { None },
                };
                ensure!(
                    bridge.imports.insert(literal(args[0])?, value).is_none(),
                    "Duplicate import"
                );
            }
            "connect" => {
                count(&args, 2)?;
                ensure!(
                    bridge
                        .connections
                        .insert(literal(args[0])?, parse_connection(args[1])?)
                        .is_none(),
                    "Duplicate connection"
                );
            }
            "bind" => {
                count(&args, 4)?;
                let permissions = match path_of_call(args[3])?.as_str() {
                    "DataPermissions::read_only" => {
                        call(args[3], "DataPermissions::read_only", 0)?;
                        DataPermissions::read_only()
                    }
                    "DataPermissions::read_write" => {
                        call(args[3], "DataPermissions::read_write", 0)?;
                        DataPermissions::read_write()
                    }
                    _ => bail!("Invalid data permissions"),
                };
                let value = DataBinding {
                    from: parse_endpoint(args[1])?,
                    to: parse_endpoint(args[2])?,
                    permissions,
                };
                ensure!(
                    bridge.bindings.insert(literal(args[0])?, value).is_none(),
                    "Duplicate data binding"
                );
            }
            other => bail!("Unsupported bridge operation {other}"),
        }
    }
    // Exact normalized comparison also rejects attributes, extra items, changed
    // imports, arbitrary macro expressions and ignored Rust syntax.
    let canonical = generate(&bridge).map_err(|d| anyhow::anyhow!("{d:?}"))?;
    // Historical archives retain their original import. Compare against the
    // two explicit source formats; do not rewrite arbitrary source text.
    let legacy = canonical.replacen(
        "use zf_flows::composition::*;",
        "use zedflow_daemon::harness::composition::*;",
        1,
    );
    let actual = file.to_token_stream().to_string();
    ensure!(
        syn::parse_file(&canonical)?.to_token_stream().to_string() == actual
            || syn::parse_file(&legacy)?.to_token_stream().to_string() == actual,
        "Source contains unrecognized syntax or noncanonical builder order"
    );
    Ok(bridge)
}
fn count(args: &[&Expr], count: usize) -> Result<()> {
    ensure!(args.len() == count, "Expected {count} literal arguments");
    Ok(())
}
fn path(expr: &Expr) -> Result<String> {
    let Expr::Path(p) = expr else {
        bail!("Expected an enum or constructor path")
    };
    Ok(p.path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>()
        .join("::"))
}
fn path_of_call(expr: &Expr) -> Result<String> {
    let Expr::Call(c) = expr else {
        bail!("Expected constructor call")
    };
    path(&c.func)
}
fn call<'a>(expr: &'a Expr, name: &str, n: usize) -> Result<Vec<&'a Expr>> {
    let Expr::Call(c) = expr else {
        bail!("Expected {name}")
    };
    ensure!(path(&c.func)? == name, "Expected {name}");
    let args = c.args.iter().collect::<Vec<_>>();
    count(&args, n)?;
    Ok(args)
}
fn literal(expr: &Expr) -> Result<String> {
    if let Expr::Lit(l) = expr
        && let syn::Lit::Str(s) = &l.lit
    {
        return Ok(s.value());
    }
    bail!("Expected a literal string")
}
fn parse_endpoint(expr: &Expr) -> Result<Endpoint> {
    let args = call(expr, "Endpoint::new", 2)?;
    Ok(Endpoint::new(&literal(args[0])?, &literal(args[1])?))
}
fn parse_connection(expr: &Expr) -> Result<Connection> {
    let mut root = expr;
    let mut methods = vec![];
    while let Expr::MethodCall(m) = root {
        ensure!(methods.len() < 3, "Too many connection modifiers");
        methods.push(m);
        root = &m.receiver;
    }
    let args = call(root, "Connection::new", 4)?;
    let mode = match path(args[2])?.as_str() {
        "RouteMode::CallAwait" => RouteMode::CallAwait,
        "RouteMode::Launch" => RouteMode::Launch,
        "RouteMode::Handoff" => RouteMode::Handoff,
        _ => bail!("Unknown route mode"),
    };
    let invocation = match path(args[3])?.as_str() {
        "InvocationKind::Tool" => InvocationKind::Tool,
        "InvocationKind::Node" => InvocationKind::Node,
        "InvocationKind::Condition" => InvocationKind::Condition,
        "InvocationKind::Context" => InvocationKind::Context,
        _ => bail!("Unknown invocation kind"),
    };
    let mut connection = Connection::new(
        parse_endpoint(args[0])?,
        parse_endpoint(args[1])?,
        mode,
        invocation,
    );
    for m in methods.into_iter().rev() {
        ensure!(m.args.len() == 1, "Modifier needs one argument");
        match m.method.to_string().as_str() {
            "tool" => {
                ensure!(connection.tool_name.is_none(), "Duplicate tool name");
                connection.tool_name = Some(literal(&m.args[0])?);
            }
            "when" => {
                ensure!(connection.condition.is_none(), "Duplicate route predicate");
                let Expr::Macro(value) = &m.args[0] else {
                    bail!("Expected literal JSON predicate")
                };
                ensure!(
                    value.mac.path.to_token_stream().to_string() == "serde_json :: json",
                    "Only literal JSON macro is allowed"
                );
                connection.condition = Some(
                    context_source::parse_json_tokens(value.mac.tokens.clone())
                        .map_err(|d| anyhow::anyhow!("{d:?}"))?,
                );
            }
            _ => bail!("Unsupported connection modifier"),
        }
    }
    Ok(connection)
}
