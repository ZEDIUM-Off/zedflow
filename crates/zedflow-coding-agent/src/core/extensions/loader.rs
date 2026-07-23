use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::{Mutex, OnceLock},
};

use super::super::source_info::{SourceOrigin, SourceScope, create_synthetic_source_info};
use super::types::{Extension, ExtensionFactory, ExtensionRuntime, LoadExtensionsResult};

static CACHE: OnceLock<Mutex<HashMap<String, Extension>>> = OnceLock::new();
fn cache() -> &'static Mutex<HashMap<String, Extension>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn clear_extension_cache() {
    if let Ok(mut cache) = cache().lock() {
        cache.clear();
    }
}

#[must_use]
pub fn create_extension_runtime() -> ExtensionRuntime {
    ExtensionRuntime::default()
}

pub fn load_extension_from_factory(
    name: impl Into<String>,
    factory: ExtensionFactory,
    runtime: &mut ExtensionRuntime,
) -> Result<Extension, String> {
    let name = name.into();
    factory(runtime).map_err(|error| error.message)?;
    let source = create_synthetic_source_info(
        name.clone(),
        "inline",
        Some(SourceScope::Temporary),
        Some(SourceOrigin::TopLevel),
        None,
    );
    let extension = Extension {
        name: name.clone(),
        source,
    };
    if let Ok(mut values) = cache().lock() {
        values.insert(name, extension.clone());
    }
    Ok(extension)
}

#[must_use]
pub fn load_extensions(paths: &[impl AsRef<Path>]) -> LoadExtensionsResult {
    let mut result = LoadExtensionsResult::default();
    for path in paths {
        let path = path.as_ref();
        if !path.exists() {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or("extension")
            .to_owned();
        let source = create_synthetic_source_info(
            path.display().to_string(),
            "local",
            Some(SourceScope::Project),
            Some(SourceOrigin::TopLevel),
            path.parent().map(|v| v.display().to_string()),
        );
        result.extensions.push(Extension { name, source });
    }
    result
}

#[must_use]
pub fn load_extensions_cached(paths: &[impl AsRef<Path>]) -> LoadExtensionsResult {
    let mut result = LoadExtensionsResult::default();
    for path in paths {
        let key = path.as_ref().display().to_string();
        if let Ok(values) = cache().lock() {
            if let Some(extension) = values.get(&key) {
                result.extensions.push(extension.clone());
                continue;
            }
        }
        let loaded = load_extensions(std::slice::from_ref(path));
        for extension in loaded.extensions {
            if let Ok(mut values) = cache().lock() {
                values.insert(key.clone(), extension.clone());
            }
            result.extensions.push(extension);
        }
        result.errors.extend(loaded.errors);
    }
    result
}

#[must_use]
pub fn discover_and_load_extensions(cwd: impl AsRef<Path>) -> LoadExtensionsResult {
    let dir = cwd.as_ref().join(".pi/extensions");
    let Ok(entries) = fs::read_dir(dir) else {
        return LoadExtensionsResult::default();
    };
    let paths: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    load_extensions(&paths)
}
