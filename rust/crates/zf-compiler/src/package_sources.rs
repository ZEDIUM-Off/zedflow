//! Pure validation of Rust's explicit filesystem inputs in frozen packages.
//!
//! This is deliberately a token inspection, not Rust compilation: sources may be
//! fragments consumed by `include!`, and configuration-disabled code and macro
//! bodies must also be inspected. It never expands macros, reads files or resolves
//! Cargo dependencies. Arbitrary procedural macro expansion and hermetic export
//! remain the export compiler's responsibility.

use anyhow::{Context, Result, bail, ensure};
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use std::collections::BTreeSet;
use zf_flows::package::{PackageNode, PackageSnapshot};

const MAX_SOURCE_DEPTH: usize = 128;
const MAX_SOURCE_CONTEXTS: usize = 16_384;

/// Check every declared Rust source in every frozen dependency, including unused
/// sources, and recursively inspect code loaded with `include!` or `#[path]`.
/// References must be literal, portable package-relative paths to declared bytes.
/// Both sides of configuration branches are inspected without evaluating `cfg`.
///
/// # Errors
/// Rejects invalid snapshots, malformed token streams, absolute/traversing or
/// undeclared paths, dynamic filesystem references, ambiguous or cyclic modules,
/// macro-generated external modules, and assembler macros whose filesystem inputs
/// cannot be established here. Macro aliases of filesystem primitives must use
/// their explicit names instead. This validates explicit source references, not
/// arbitrary code or procedural macro behavior.
pub fn validate_package_sources(snapshot: &PackageSnapshot) -> Result<()> {
    snapshot.validate()?;
    for (revision, node) in &snapshot.packages {
        let manifest = node.manifest()?;
        let mut validator = SourceValidator {
            node,
            visited: BTreeSet::new(),
            active: BTreeSet::new(),
            inspected_files: BTreeSet::new(),
        };
        let result: Result<()> = (|| {
            validator.file(&manifest.entry, "", 0)?;
            for file in node.files.keys().filter(|file| file.ends_with(".rs")) {
                if !validator.inspected_files.contains(file) {
                    validator.file(file, &default_module_dir(file), 0)?;
                }
            }
            Ok(())
        })();
        result.with_context(|| {
            format!(
                "package {} ({revision}) Rust source validation",
                manifest.id
            )
        })?;
    }
    Ok(())
}

struct SourceValidator<'a> {
    node: &'a PackageNode,
    visited: BTreeSet<(String, String)>,
    active: BTreeSet<String>,
    inspected_files: BTreeSet<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TokenContext {
    Source,
    Macro,
    Import,
}

#[derive(Default)]
struct ModulePaths {
    unconditional: Option<String>,
    conditional: Vec<String>,
}

impl SourceValidator<'_> {
    fn file(&mut self, file: &str, module_dir: &str, depth: usize) -> Result<()> {
        ensure!(
            depth < MAX_SOURCE_DEPTH,
            "Rust source nesting limit exceeded"
        );
        ensure!(
            !self.active.contains(file),
            "cyclic Rust source reference: {file}"
        );
        if !self.visited.insert((file.into(), module_dir.into())) {
            return Ok(());
        }
        ensure!(
            self.visited.len() <= MAX_SOURCE_CONTEXTS,
            "too many Rust source contexts"
        );
        self.active.insert(file.into());
        self.inspected_files.insert(file.into());
        let bytes = self
            .node
            .files
            .get(file)
            .with_context(|| format!("undeclared Rust source: {file}"))?;
        let source = std::str::from_utf8(bytes)
            .with_context(|| format!("Rust source is not UTF-8: {file}"))?;
        // A crate's shebang is not a Rust token. Inner attributes start with #![.
        let source = source.strip_prefix('\u{feff}').unwrap_or(source);
        let source = if source.starts_with("#!") && !source.starts_with("#![") {
            source.split_once('\n').map_or("", |(_, rest)| rest)
        } else {
            source
        };
        let tokens = source
            .parse::<TokenStream>()
            .map_err(|error| anyhow::anyhow!("invalid Rust tokens in {file}: {error}"))?;
        self.tokens(
            tokens,
            file,
            module_dir,
            parent(file),
            TokenContext::Source,
            depth + 1,
        )
        .with_context(|| format!("Rust source {file}"))?;
        self.active.remove(file);
        Ok(())
    }

    fn tokens(
        &mut self,
        stream: TokenStream,
        file: &str,
        module_dir: &str,
        path_base: &str,
        context: TokenContext,
        depth: usize,
    ) -> Result<()> {
        ensure!(
            depth < MAX_SOURCE_DEPTH,
            "Rust token nesting limit exceeded"
        );
        let tokens: Vec<_> = stream.into_iter().collect();
        let mut paths = ModulePaths::default();
        let mut in_import = context == TokenContext::Import;
        let mut i = 0;
        while i < tokens.len() {
            if punct(&tokens[i], '#') {
                let attribute_index =
                    i + 1 + usize::from(tokens.get(i + 1).is_some_and(|t| punct(t, '!')));
                if let Some(TokenTree::Group(group)) = tokens.get(attribute_index)
                    && group.delimiter() == Delimiter::Bracket
                {
                    Self::attribute(group.stream(), false, &mut paths, depth + 1)?;
                    self.tokens(
                        group.stream(),
                        file,
                        module_dir,
                        path_base,
                        context,
                        depth + 1,
                    )?;
                    i = attribute_index + 1;
                    continue;
                }
            }
            if let TokenTree::Ident(identifier) = &tokens[i] {
                let name = identifier.to_string();
                let name = name.strip_prefix("r#").unwrap_or(&name);
                if identifier == "use" {
                    in_import = true;
                }
                let include_primitive = matches!(name, "include" | "include_str" | "include_bytes");
                let filesystem_primitive =
                    include_primitive || matches!(name, "asm" | "global_asm");
                let invoked = tokens.get(i + 1).is_some_and(|token| punct(token, '!'));
                if filesystem_primitive && !invoked {
                    ensure!(
                        context != TokenContext::Macro
                            && !(in_import
                                && tokens.get(i + 1).is_some_and(|token| ident(token, "as"))),
                        "filesystem macro {name} aliases or macro forwarding cannot be frozen; use an explicit invocation"
                    );
                }
                if include_primitive && invoked {
                    let Some(TokenTree::Group(arguments)) = tokens.get(i + 2) else {
                        bail!("dynamic {name}! reference");
                    };
                    let relative = literal_path(arguments.stream())
                        .with_context(|| format!("{name}! requires one literal path"))?;
                    let target = resolve_path(parent(file), &relative)?;
                    self.require_file(&target)?;
                    if name == "include" {
                        self.file(&target, parent(&target), depth + 1)?;
                    }
                    i += 3;
                    continue;
                }
                if matches!(name, "asm" | "global_asm")
                    && tokens.get(i + 1).is_some_and(|t| punct(t, '!'))
                {
                    bail!(
                        "{name}! assembler filesystem inputs cannot be frozen by package source validation"
                    );
                }
                if identifier == "mod" {
                    ensure!(
                        context != TokenContext::Macro,
                        "external or inline modules in macro token bodies require expansion before their paths can be frozen"
                    );
                    let Some(TokenTree::Ident(module)) = tokens.get(i + 1) else {
                        bail!("dynamic Rust module name");
                    };
                    let module = module.to_string();
                    let module = module.strip_prefix("r#").unwrap_or(&module);
                    if tokens.get(i + 2).is_some_and(|t| punct(t, ';')) {
                        let mut targets = Vec::new();
                        if let Some(path) = paths.unconditional.take() {
                            targets.push((resolve_path(path_base, &path)?, true));
                        } else {
                            let stem = join(module_dir, module);
                            let candidates = [format!("{stem}.rs"), format!("{stem}/mod.rs")];
                            let found: Vec<_> = candidates
                                .iter()
                                .filter(|path| self.node.files.contains_key(*path))
                                .collect();
                            ensure!(
                                found.len() == 1,
                                "module {module} requires exactly one declared frozen source: {} or {}",
                                candidates[0],
                                candidates[1]
                            );
                            targets.push((found[0].clone(), false));
                        }
                        for path in paths.conditional.drain(..) {
                            targets.push((resolve_path(path_base, &path)?, true));
                        }
                        for (target, explicit) in targets {
                            self.require_file(&target)?;
                            let next_dir = if explicit {
                                parent(&target).into()
                            } else {
                                default_module_dir(&target)
                            };
                            self.file(&target, &next_dir, depth + 1)?;
                        }
                        paths = ModulePaths::default();
                        i += 3;
                        continue;
                    }
                    if let Some(TokenTree::Group(body)) = tokens.get(i + 2)
                        && body.delimiter() == Delimiter::Brace
                    {
                        let mut directories =
                            vec![if let Some(path) = paths.unconditional.take() {
                                resolve_path(path_base, &path)?
                            } else {
                                join(module_dir, module)
                            }];
                        for path in paths.conditional.drain(..) {
                            directories.push(resolve_path(path_base, &path)?);
                        }
                        for directory in directories {
                            self.tokens(
                                body.stream(),
                                file,
                                &directory,
                                &directory,
                                TokenContext::Source,
                                depth + 1,
                            )?;
                        }
                        paths = ModulePaths::default();
                        i += 3;
                        continue;
                    }
                    bail!("dynamic Rust module declaration: {module}");
                }
            }
            if let TokenTree::Group(group) = &tokens[i] {
                // Any macro body may forward tokens or emit modules in another
                // scope. Includes keep their definition-file location, whereas
                // external module resolution cannot be guessed before expansion.
                let macro_body = context == TokenContext::Macro
                    || i.checked_sub(1).is_some_and(|n| punct(&tokens[n], '!'))
                    || (i >= 2 && punct(&tokens[i - 2], '!'));
                self.tokens(
                    group.stream(),
                    file,
                    module_dir,
                    path_base,
                    if macro_body {
                        TokenContext::Macro
                    } else if in_import {
                        TokenContext::Import
                    } else {
                        TokenContext::Source
                    },
                    depth + 1,
                )?;
                if group.delimiter() == Delimiter::Brace {
                    paths = ModulePaths::default();
                }
            } else if punct(&tokens[i], ';') {
                in_import = false;
                paths = ModulePaths::default();
            }
            i += 1;
        }
        Ok(())
    }

    fn attribute(
        stream: TokenStream,
        conditional: bool,
        paths: &mut ModulePaths,
        depth: usize,
    ) -> Result<()> {
        ensure!(
            depth < MAX_SOURCE_DEPTH,
            "Rust attribute nesting limit exceeded"
        );
        let tokens: Vec<_> = stream.into_iter().collect();
        if tokens.first().is_some_and(|token| ident(token, "path")) {
            ensure!(
                tokens.get(1).is_some_and(|token| punct(token, '=')),
                "dynamic #[path] reference"
            );
            let path = literal_path(tokens.into_iter().skip(2).collect())
                .context("#[path] requires one literal path")?;
            // Validate path spelling even if attached to a non-module item.
            resolve_path("", &path)?;
            if conditional {
                paths.conditional.push(path);
            } else {
                ensure!(
                    paths.unconditional.replace(path).is_none(),
                    "duplicate #[path] attributes"
                );
            }
        } else if tokens.first().is_some_and(|token| ident(token, "cfg_attr")) {
            let Some(TokenTree::Group(group)) = tokens.get(1) else {
                bail!("invalid cfg_attr");
            };
            let mut attributes = Vec::new();
            let mut current = TokenStream::new();
            for token in group.stream() {
                if punct(&token, ',') {
                    attributes.push(current);
                    current = TokenStream::new();
                } else {
                    current.extend([token]);
                }
            }
            attributes.push(current);
            for attribute in attributes.into_iter().skip(1) {
                Self::attribute(attribute, true, paths, depth + 1)?;
            }
        }
        Ok(())
    }

    fn require_file(&self, path: &str) -> Result<()> {
        ensure!(
            self.node.files.contains_key(path),
            "compile-time reference is not a declared frozen package file: {path}"
        );
        Ok(())
    }
}

fn literal_path(stream: TokenStream) -> Result<String> {
    let mut tokens: Vec<_> = stream.into_iter().collect();
    if tokens.last().is_some_and(|token| punct(token, ',')) {
        tokens.pop();
    }
    let literal = syn::parse2::<syn::LitStr>(tokens.into_iter().collect())
        .context("dynamic or non-string filesystem path is not supported")?;
    Ok(literal.value())
}

fn resolve_path(base: &str, relative: &str) -> Result<String> {
    ensure!(
        !relative.is_empty() && !relative.starts_with('/') && !relative.contains(['\\', ':', '\0']),
        "compile-time reference must be a portable relative path: {relative:?}"
    );
    let mut parts = Vec::new();
    for part in relative.split('/') {
        ensure!(
            part != "..",
            "compile-time path traversal is forbidden: {relative}"
        );
        ensure!(
            !part.is_empty(),
            "empty compile-time path component: {relative}"
        );
        if part != "." {
            parts.push(part);
        }
    }
    ensure!(
        !parts.is_empty(),
        "compile-time reference must name a file or module directory"
    );
    Ok(join(base, &parts.join("/")))
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}
fn join(base: &str, path: &str) -> String {
    if base.is_empty() {
        path.into()
    } else {
        format!("{base}/{path}")
    }
}
fn default_module_dir(file: &str) -> String {
    let name = file.rsplit('/').next().unwrap_or(file);
    if name == "mod.rs" {
        parent(file).into()
    } else {
        file.strip_suffix(".rs").unwrap_or(file).into()
    }
}
fn punct(token: &TokenTree, value: char) -> bool {
    matches!(token, TokenTree::Punct(punct) if punct.as_char() == value)
}
fn ident(token: &TokenTree, value: &str) -> bool {
    matches!(token, TokenTree::Ident(ident) if ident == value)
}
