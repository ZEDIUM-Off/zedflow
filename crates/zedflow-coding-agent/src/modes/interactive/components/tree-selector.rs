//! Session-tree navigation state, ported from Pi's tree selector.

use std::collections::{HashMap, HashSet};

use crate::session_manager::{SessionTreeEntry, SessionTreeEntryBase, SessionTreeNode};
use zedflow_agent::types::AgentMessage;
use zedflow_ai::types::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    Default,
    NoTools,
    UserOnly,
    LabeledOnly,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeEntryKind {
    UserMessage,
    AssistantMessage,
    ToolResult,
    Other,
    Setting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeItem {
    pub id: String,
    pub parent_id: Option<String>,
    pub text: String,
    pub label: Option<String>,
    pub kind: TreeEntryKind,
    pub children: Vec<TreeItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeSelectorAction {
    None,
    Select(String),
    Cancel,
}

#[derive(Debug, Clone)]
pub struct TreeSelectorState {
    items: Vec<TreeItem>,
    visible: Vec<usize>,
    selected: usize,
    current_leaf_id: Option<String>,
    filter_mode: FilterMode,
    query: String,
    folded: HashSet<String>,
}

impl TreeSelectorState {
    #[must_use]
    pub fn new(
        roots: Vec<TreeItem>,
        current_leaf_id: Option<String>,
        initial_selected_id: Option<&str>,
    ) -> Self {
        let mut items = Vec::new();
        flatten(&roots, &mut items);
        let mut state = Self {
            items,
            visible: Vec::new(),
            selected: 0,
            current_leaf_id,
            filter_mode: FilterMode::Default,
            query: String::new(),
            folded: HashSet::new(),
        };
        state.apply_filter();
        let target = initial_selected_id
            .map(str::to_owned)
            .or_else(|| state.current_leaf_id.clone());
        state.select_nearest(target.as_deref());
        state
    }

    #[must_use]
    pub fn from_session_tree(
        roots: &[SessionTreeNode],
        current_leaf_id: Option<String>,
        initial_selected_id: Option<&str>,
    ) -> Self {
        Self::new(
            roots.iter().map(tree_item).collect(),
            current_leaf_id,
            initial_selected_id,
        )
    }

    #[must_use]
    pub fn visible_items(&self) -> impl Iterator<Item = &TreeItem> {
        self.visible.iter().map(|index| &self.items[*index])
    }

    #[must_use]
    pub fn selected_item(&self) -> Option<&TreeItem> {
        self.visible
            .get(self.selected)
            .map(|index| &self.items[*index])
    }

    pub fn move_up(&mut self) {
        if !self.visible.is_empty() {
            self.selected = if self.selected == 0 {
                self.visible.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn move_down(&mut self) {
        if !self.visible.is_empty() {
            self.selected = (self.selected + 1) % self.visible.len();
        }
    }

    pub fn page_up(&mut self, page_size: usize) {
        self.selected = self.selected.saturating_sub(page_size);
    }

    pub fn page_down(&mut self, page_size: usize) {
        self.selected = (self.selected + page_size).min(self.visible.len().saturating_sub(1));
    }

    pub fn set_filter_mode(&mut self, mode: FilterMode) {
        self.filter_mode = mode;
        self.folded.clear();
        self.apply_filter();
    }

    pub fn cycle_filter(&mut self, forward: bool) {
        const MODES: [FilterMode; 5] = [
            FilterMode::Default,
            FilterMode::NoTools,
            FilterMode::UserOnly,
            FilterMode::LabeledOnly,
            FilterMode::All,
        ];
        let index = MODES
            .iter()
            .position(|mode| *mode == self.filter_mode)
            .unwrap_or(0);
        self.set_filter_mode(
            MODES[(index + if forward { 1 } else { MODES.len() - 1 }) % MODES.len()],
        );
    }

    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.folded.clear();
        self.apply_filter();
    }

    pub fn cancel_search_or_picker(&mut self) -> TreeSelectorAction {
        if self.query.is_empty() {
            TreeSelectorAction::Cancel
        } else {
            self.query.clear();
            self.folded.clear();
            self.apply_filter();
            TreeSelectorAction::None
        }
    }

    pub fn toggle_fold_selected(&mut self) {
        let Some(item) = self.selected_item() else {
            return;
        };
        if item.children.is_empty() {
            return;
        }
        let id = item.id.clone();
        if !self.folded.remove(&id) {
            self.folded.insert(id);
        }
        self.apply_filter();
    }

    #[must_use]
    pub fn select(&self) -> TreeSelectorAction {
        self.selected_item()
            .map_or(TreeSelectorAction::None, |item| {
                TreeSelectorAction::Select(item.id.clone())
            })
    }

    fn select_nearest(&mut self, target: Option<&str>) {
        let parents = self
            .items
            .iter()
            .map(|item| (item.id.as_str(), item.parent_id.as_deref()))
            .collect::<HashMap<_, _>>();
        let mut target = target;
        while let Some(id) = target {
            if let Some(index) = self
                .visible
                .iter()
                .position(|item| self.items[*item].id == id)
            {
                self.selected = index;
                return;
            }
            target = parents.get(id).copied().flatten();
        }
        self.selected = self.visible.len().saturating_sub(1);
    }

    fn apply_filter(&mut self) {
        let selected_id = self.selected_item().map(|item| item.id.clone());
        let tokens = self
            .query
            .to_lowercase()
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let mut hidden_by_fold = HashSet::new();
        for item in &self.items {
            if item.parent_id.as_ref().is_some_and(|parent| {
                self.folded.contains(parent) || hidden_by_fold.contains(parent)
            }) {
                hidden_by_fold.insert(item.id.clone());
            }
        }
        self.visible = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                if hidden_by_fold.contains(&item.id) {
                    return false;
                }
                let passes = match self.filter_mode {
                    FilterMode::Default => item.kind != TreeEntryKind::Setting,
                    FilterMode::NoTools => {
                        item.kind != TreeEntryKind::Setting
                            && item.kind != TreeEntryKind::ToolResult
                    }
                    FilterMode::UserOnly => item.kind == TreeEntryKind::UserMessage,
                    FilterMode::LabeledOnly => item.label.is_some(),
                    FilterMode::All => true,
                };
                let text = format!(
                    "{} {} {}",
                    item.id,
                    item.label.as_deref().unwrap_or_default(),
                    item.text
                )
                .to_lowercase();
                passes && tokens.iter().all(|token| text.contains(token))
            })
            .map(|(index, _)| index)
            .collect();
        self.select_nearest(selected_id.as_deref());
    }
}

fn flatten(roots: &[TreeItem], result: &mut Vec<TreeItem>) {
    for root in roots {
        result.push(root.clone());
        flatten(&root.children, result);
    }
}

fn tree_item(node: &SessionTreeNode) -> TreeItem {
    let base = entry_base(&node.entry);
    let kind = match &node.entry {
        SessionTreeEntry::Message(message) => match &message.message {
            AgentMessage::Llm(Message::User(_)) => TreeEntryKind::UserMessage,
            AgentMessage::Llm(Message::Assistant(_)) => TreeEntryKind::AssistantMessage,
            AgentMessage::Llm(Message::ToolResult(_)) => TreeEntryKind::ToolResult,
            AgentMessage::Custom(_) => TreeEntryKind::Other,
        },
        SessionTreeEntry::Label(_)
        | SessionTreeEntry::Custom(_)
        | SessionTreeEntry::ModelChange(_)
        | SessionTreeEntry::ThinkingLevelChange(_)
        | SessionTreeEntry::SessionInfo(_) => TreeEntryKind::Setting,
        _ => TreeEntryKind::Other,
    };
    TreeItem {
        id: base.id.clone(),
        parent_id: base.parent_id.clone(),
        text: format!("{:?}", node.entry),
        label: node.label.clone(),
        kind,
        children: node.children.iter().map(tree_item).collect(),
    }
}

fn entry_base(entry: &SessionTreeEntry) -> &SessionTreeEntryBase {
    match entry {
        SessionTreeEntry::Message(value) => &value.base,
        SessionTreeEntry::ThinkingLevelChange(value) => &value.base,
        SessionTreeEntry::ModelChange(value) => &value.base,
        SessionTreeEntry::ActiveToolsChange(value) => &value.base,
        SessionTreeEntry::Compaction(value) => &value.base,
        SessionTreeEntry::BranchSummary(value) => &value.base,
        SessionTreeEntry::Custom(value) => &value.base,
        SessionTreeEntry::CustomMessage(value) => &value.base,
        SessionTreeEntry::Label(value) => &value.base,
        SessionTreeEntry::SessionInfo(value) => &value.base,
        SessionTreeEntry::Leaf(value) => &value.base,
    }
}
