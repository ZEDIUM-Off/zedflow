#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceScope {
    User,
    Project,
    Temporary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceOrigin {
    Package,
    TopLevel,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathMetadata {
    pub source: String,
    pub scope: SourceScope,
    pub origin: SourceOrigin,
    pub base_dir: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceInfo {
    pub path: String,
    pub source: String,
    pub scope: SourceScope,
    pub origin: SourceOrigin,
    pub base_dir: Option<String>,
}

pub fn create_source_info(path: impl Into<String>, metadata: PathMetadata) -> SourceInfo {
    SourceInfo {
        path: path.into(),
        source: metadata.source,
        scope: metadata.scope,
        origin: metadata.origin,
        base_dir: metadata.base_dir,
    }
}

pub fn create_synthetic_source_info(
    path: impl Into<String>,
    source: impl Into<String>,
    scope: Option<SourceScope>,
    origin: Option<SourceOrigin>,
    base_dir: Option<String>,
) -> SourceInfo {
    SourceInfo {
        path: path.into(),
        source: source.into(),
        scope: scope.unwrap_or(SourceScope::Temporary),
        origin: origin.unwrap_or(SourceOrigin::TopLevel),
        base_dir,
    }
}
