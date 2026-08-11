//! Resource grouping and toggle actions for Pi's configuration selector.

use crate::package_manager::{ResolvedPaths, ResolvedResource, ResourceOrigin, SourceScope};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    Extensions,
    Skills,
    Prompts,
    Themes,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigScope {
    Global,
    Project,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceItem {
    pub path: PathBuf,
    pub enabled: bool,
    pub resource_type: ResourceType,
    pub group: String,
    pub display_name: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigAction {
    Toggle {
        scope: ConfigScope,
        path: PathBuf,
        enabled: bool,
    },
    Save,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSelector {
    pub items: Vec<ResourceItem>,
    pub selected: usize,
}
impl ConfigSelector {
    #[must_use]
    pub fn new(paths: &ResolvedPaths) -> Self {
        let mut items = Vec::new();
        add(&mut items, &paths.extensions, ResourceType::Extensions);
        add(&mut items, &paths.skills, ResourceType::Skills);
        add(&mut items, &paths.prompts, ResourceType::Prompts);
        add(&mut items, &paths.themes, ResourceType::Themes);
        items.sort_by(|a, b| {
            a.group
                .cmp(&b.group)
                .then(a.display_name.cmp(&b.display_name))
        });
        Self { items, selected: 0 }
    }
    pub fn move_selection(&mut self, delta: isize) {
        self.selected = self
            .selected
            .saturating_add_signed(delta)
            .min(self.items.len().saturating_sub(1));
    }
    pub fn toggle_selected(&mut self) -> Option<ConfigAction> {
        let item = self.items.get_mut(self.selected)?;
        item.enabled = !item.enabled;
        let scope = if item.group.contains("project") {
            ConfigScope::Project
        } else {
            ConfigScope::Global
        };
        Some(ConfigAction::Toggle {
            scope,
            path: item.path.clone(),
            enabled: item.enabled,
        })
    }
}

fn add(
    target: &mut Vec<ResourceItem>,
    resources: &[ResolvedResource],
    resource_type: ResourceType,
) {
    target.extend(resources.iter().map(|resource| {
        let scope = match resource.metadata.scope {
            SourceScope::User => "user",
            SourceScope::Project => "project",
            SourceScope::Temporary => "temporary",
        };
        let origin = match resource.metadata.origin {
            ResourceOrigin::Package => &resource.metadata.source,
            ResourceOrigin::TopLevel => "top-level",
        };
        let display_name = resource
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned();
        ResourceItem {
            path: resource.path.clone(),
            enabled: resource.enabled,
            resource_type,
            group: format!("{origin} ({scope})"),
            display_name,
        }
    }));
}
