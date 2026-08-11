use crate::Component;
use crate::terminal_image::{get_capabilities, hyperlink, is_image_line};
use crate::utils::{apply_background_to_line, visible_width, wrap_text_with_ansi};
use markdown::mdast::{List, Node, Table};
use markdown::{Constructs, ParseOptions};
use std::cell::RefCell;
use std::sync::Arc;

type Style = Arc<dyn Fn(&str) -> String + Send + Sync>;
type Highlighter = Arc<dyn Fn(&str, Option<&str>) -> Vec<String> + Send + Sync>;

fn plain(text: &str) -> String {
    text.to_string()
}

#[derive(Clone)]
pub struct DefaultTextStyle {
    pub color: Option<Style>,
    pub bg_color: Option<Style>,
    pub bold: bool,
    pub italic: bool,
    pub strikethrough: bool,
    pub underline: bool,
}

impl Default for DefaultTextStyle {
    fn default() -> Self {
        Self {
            color: None,
            bg_color: None,
            bold: false,
            italic: false,
            strikethrough: false,
            underline: false,
        }
    }
}

#[derive(Clone)]
pub struct MarkdownTheme {
    pub heading: Style,
    pub link: Style,
    pub link_url: Style,
    pub code: Style,
    pub code_block: Style,
    pub code_block_border: Style,
    pub quote: Style,
    pub quote_border: Style,
    pub hr: Style,
    pub list_bullet: Style,
    pub bold: Style,
    pub italic: Style,
    pub strikethrough: Style,
    pub underline: Style,
    pub highlight_code: Option<Highlighter>,
    pub code_block_indent: String,
}

impl Default for MarkdownTheme {
    fn default() -> Self {
        let style = || Arc::new(plain as fn(&str) -> String) as Style;
        Self {
            heading: style(),
            link: style(),
            link_url: style(),
            code: style(),
            code_block: style(),
            code_block_border: style(),
            quote: style(),
            quote_border: style(),
            hr: style(),
            list_bullet: style(),
            bold: style(),
            italic: style(),
            strikethrough: style(),
            underline: style(),
            highlight_code: None,
            code_block_indent: "  ".into(),
        }
    }
}

#[derive(Clone, Default)]
pub struct MarkdownOptions {
    pub preserve_ordered_list_markers: bool,
    pub preserve_backslash_escapes: bool,
}

#[derive(Clone)]
struct Cache {
    text: String,
    width: usize,
    lines: Vec<String>,
}

pub struct Markdown {
    pub text: String,
    padding_x: usize,
    padding_y: usize,
    theme: MarkdownTheme,
    default_text_style: Option<DefaultTextStyle>,
    options: MarkdownOptions,
    cache: RefCell<Option<Cache>>,
}

impl Markdown {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            padding_x: 0,
            padding_y: 0,
            theme: MarkdownTheme::default(),
            default_text_style: None,
            options: MarkdownOptions::default(),
            cache: RefCell::new(None),
        }
    }

    pub fn with_padding(mut self, padding_x: usize, padding_y: usize) -> Self {
        self.padding_x = padding_x;
        self.padding_y = padding_y;
        self
    }

    pub fn theme_mut(&mut self) -> &mut MarkdownTheme {
        self.invalidate();
        &mut self.theme
    }

    pub fn options_mut(&mut self) -> &mut MarkdownOptions {
        self.invalidate();
        &mut self.options
    }

    pub fn set_default_text_style(&mut self, style: Option<DefaultTextStyle>) {
        self.default_text_style = style;
        self.invalidate();
    }

    pub fn default_text_style_mut(&mut self) -> &mut Option<DefaultTextStyle> {
        self.invalidate();
        &mut self.default_text_style
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.invalidate();
    }

    pub fn invalidate(&self) {
        *self.cache.borrow_mut() = None;
    }

    fn parse(&self, source: &str) -> Option<Node> {
        markdown::to_mdast(
            source,
            &ParseOptions {
                constructs: Constructs::gfm(),
                gfm_strikethrough_single_tilde: false,
                ..ParseOptions::default()
            },
        )
        .ok()
    }

    fn render_uncached(&self, width: usize) -> Vec<String> {
        if self.text.trim().is_empty() {
            return Vec::new();
        }
        let content_width = width.saturating_sub(self.padding_x * 2).max(1);
        let source = self.text.replace('\t', "   ");
        let Some(Node::Root(root)) = self.parse(&source) else {
            return Vec::new();
        };
        let mut rendered = Vec::new();
        for (index, node) in root.children.iter().enumerate() {
            let next = root.children.get(index + 1);
            rendered.extend(self.render_block(node, content_width, next, &source, None));
        }

        let wrapped: Vec<String> = rendered
            .into_iter()
            .flat_map(|line| {
                if is_image_line(&line) {
                    vec![line]
                } else {
                    wrap_text_with_ansi(&line, content_width)
                }
            })
            .collect();
        let left = " ".repeat(self.padding_x);
        let right = " ".repeat(self.padding_x);
        let bg = self
            .default_text_style
            .as_ref()
            .and_then(|style| style.bg_color.as_ref());
        let content: Vec<String> = wrapped
            .into_iter()
            .map(|line| {
                if is_image_line(&line) {
                    return line;
                }
                let line = format!("{left}{line}{right}");
                if let Some(bg) = bg {
                    apply_background_to_line(&line, width, |text| bg(text))
                } else {
                    let padding = width.saturating_sub(visible_width(&line));
                    format!("{line}{}", " ".repeat(padding))
                }
            })
            .collect();
        let empty = " ".repeat(width);
        let empty = if let Some(bg) = bg { bg(&empty) } else { empty };
        let mut result = Vec::with_capacity(content.len() + self.padding_y * 2);
        result.extend(std::iter::repeat_n(empty.clone(), self.padding_y));
        result.extend(content);
        result.extend(std::iter::repeat_n(empty, self.padding_y));
        if result.is_empty() {
            vec![String::new()]
        } else {
            result
        }
    }

    fn style_default(&self, text: &str) -> String {
        let Some(style) = &self.default_text_style else {
            return text.to_string();
        };
        let mut result = style
            .color
            .as_ref()
            .map_or_else(|| text.to_string(), |f| f(text));
        if style.bold {
            result = (self.theme.bold)(&result);
        }
        if style.italic {
            result = (self.theme.italic)(&result);
        }
        if style.strikethrough {
            result = (self.theme.strikethrough)(&result);
        }
        if style.underline {
            result = (self.theme.underline)(&result);
        }
        result
    }

    fn style_prefix(style: &Style) -> String {
        let sentinel = "\0";
        let styled = style(sentinel);
        styled
            .find(sentinel)
            .map_or_else(String::new, |at| styled[..at].to_string())
    }

    fn default_prefix(&self) -> String {
        let sentinel = "\0";
        let styled = self.style_default(sentinel);
        styled
            .find(sentinel)
            .map_or_else(String::new, |at| styled[..at].to_string())
    }

    fn raw<'a>(&self, node: &Node, source: &'a str) -> Option<&'a str> {
        let position = node.position()?;
        source.get(position.start.offset..position.end.offset)
    }

    fn render_inline(
        &self,
        nodes: &[Node],
        source: &str,
        context: Option<(&Style, String)>,
    ) -> String {
        let owned_default = Arc::new({
            let style = self.default_text_style.clone();
            let theme = self.theme.clone();
            move |text: &str| {
                let Some(style) = &style else {
                    return text.to_string();
                };
                let mut result = style
                    .color
                    .as_ref()
                    .map_or_else(|| text.to_string(), |f| f(text));
                if style.bold {
                    result = (theme.bold)(&result);
                }
                if style.italic {
                    result = (theme.italic)(&result);
                }
                if style.strikethrough {
                    result = (theme.strikethrough)(&result);
                }
                if style.underline {
                    result = (theme.underline)(&result);
                }
                result
            }
        }) as Style;
        let default_prefix = self.default_prefix();
        let (apply, prefix) = context.unwrap_or((&owned_default, default_prefix));
        let mut result = String::new();
        for node in nodes {
            match node {
                Node::Text(text) => {
                    let value = if self.options.preserve_backslash_escapes {
                        self.raw(node, source).unwrap_or(&text.value)
                    } else {
                        &text.value
                    };
                    result.push_str(
                        &value
                            .split('\n')
                            .map(|part| apply(part))
                            .collect::<Vec<_>>()
                            .join("\n"),
                    );
                }
                Node::Strong(strong) => {
                    result.push_str(&(self.theme.bold)(&self.render_inline(
                        &strong.children,
                        source,
                        Some((apply, prefix.clone())),
                    )));
                    result.push_str(&prefix);
                }
                Node::Emphasis(emphasis) => {
                    result.push_str(&(self.theme.italic)(&self.render_inline(
                        &emphasis.children,
                        source,
                        Some((apply, prefix.clone())),
                    )));
                    result.push_str(&prefix);
                }
                Node::Delete(delete) => {
                    result.push_str(&(self.theme.strikethrough)(&self.render_inline(
                        &delete.children,
                        source,
                        Some((apply, prefix.clone())),
                    )));
                    result.push_str(&prefix);
                }
                Node::InlineCode(code) => {
                    result.push_str(&(self.theme.code)(&code.value));
                    result.push_str(&prefix);
                }
                Node::Link(link) => {
                    let text =
                        self.render_inline(&link.children, source, Some((apply, prefix.clone())));
                    let styled = (self.theme.link)(&(self.theme.underline)(&text));
                    if get_capabilities().hyperlinks {
                        result.push_str(&hyperlink(&styled, &link.url));
                    } else if text == link.url
                        || link.url.strip_prefix("mailto:") == Some(text.as_str())
                    {
                        result.push_str(&styled);
                    } else {
                        result.push_str(&styled);
                        result.push_str(&(self.theme.link_url)(&format!(" ({})", link.url)));
                    }
                    result.push_str(&prefix);
                }
                Node::Break(_) => result.push('\n'),
                Node::Html(html) => result.push_str(&apply(&html.value)),
                Node::Image(image) => result.push_str(&apply(&image.alt)),
                other => {
                    if let Some(children) = other.children() {
                        result.push_str(&self.render_inline(
                            children,
                            source,
                            Some((apply, prefix.clone())),
                        ));
                    } else {
                        result.push_str(&apply(&other.to_string()));
                    }
                }
            }
        }
        while !prefix.is_empty() && result.ends_with(&prefix) {
            result.truncate(result.len() - prefix.len());
        }
        result
    }

    fn render_block(
        &self,
        node: &Node,
        width: usize,
        next: Option<&Node>,
        source: &str,
        inline_context: Option<(&Style, String)>,
    ) -> Vec<String> {
        let add_spacing = |lines: &mut Vec<String>| {
            if next.is_some() {
                lines.push(String::new())
            }
        };
        match node {
            Node::Heading(heading) => {
                let style: Style = if heading.depth == 1 {
                    let heading = self.theme.heading.clone();
                    let bold = self.theme.bold.clone();
                    let underline = self.theme.underline.clone();
                    Arc::new(move |text| heading(&bold(&underline(text))))
                } else {
                    let heading = self.theme.heading.clone();
                    let bold = self.theme.bold.clone();
                    Arc::new(move |text| heading(&bold(text)))
                };
                let prefix = Self::style_prefix(&style);
                let text = self.render_inline(&heading.children, source, Some((&style, prefix)));
                let mut lines = vec![if heading.depth >= 3 {
                    format!(
                        "{}{}",
                        style(&format!("{} ", "#".repeat(heading.depth as usize))),
                        text
                    )
                } else {
                    text
                }];
                add_spacing(&mut lines);
                lines
            }
            Node::Paragraph(paragraph) => {
                let mut lines =
                    vec![self.render_inline(&paragraph.children, source, inline_context)];
                if next.is_some_and(|n| !matches!(n, Node::List(_))) {
                    lines.push(String::new());
                }
                lines
            }
            Node::Code(code) => {
                let mut lines = vec![(self.theme.code_block_border)(&format!(
                    "```{}",
                    code.lang.as_deref().unwrap_or("")
                ))];
                let mut value = code.value.as_str();
                if let Some(raw) = self.raw(node, source) {
                    if let Some(last) = raw.lines().last() {
                        let marker = raw
                            .chars()
                            .take_while(|c| *c == '`' || *c == '~')
                            .collect::<String>();
                        if !last.is_empty()
                            && marker.len() >= 3
                            && last.len() < marker.len()
                            && last.chars().all(|c| c == marker.chars().next().unwrap())
                        {
                            value = value
                                .strip_suffix(last)
                                .unwrap_or(value)
                                .trim_end_matches('\n');
                        }
                    }
                }
                let code_lines = self.theme.highlight_code.as_ref().map_or_else(
                    || {
                        value
                            .split('\n')
                            .map(|line| (self.theme.code_block)(line))
                            .collect()
                    },
                    |highlight| highlight(value, code.lang.as_deref()),
                );
                lines.extend(
                    code_lines
                        .into_iter()
                        .map(|line| format!("{}{line}", self.theme.code_block_indent)),
                );
                lines.push((self.theme.code_block_border)("```"));
                add_spacing(&mut lines);
                lines
            }
            Node::List(list) => self.render_list(list, 0, width, source, inline_context),
            Node::Table(table) => self.render_table(table, width, next, source, inline_context),
            Node::Blockquote(quote) => {
                let quote_style: Style = {
                    let quote = self.theme.quote.clone();
                    let italic = self.theme.italic.clone();
                    Arc::new(move |text| quote(&italic(text)))
                };
                let prefix = Self::style_prefix(&quote_style);
                let mut inner = Vec::new();
                let quote_width = width.saturating_sub(2).max(1);
                for (index, child) in quote.children.iter().enumerate() {
                    inner.extend(self.render_block(
                        child,
                        quote_width,
                        quote.children.get(index + 1),
                        source,
                        Some((&quote_style, prefix.clone())),
                    ));
                }
                while inner.last().is_some_and(String::is_empty) {
                    inner.pop();
                }
                let mut lines = Vec::new();
                for line in inner {
                    let reapplied = if prefix.is_empty() {
                        quote_style(&line)
                    } else {
                        quote_style(&line.replace("\x1b[0m", &format!("\x1b[0m{prefix}")))
                    };
                    lines.extend(
                        wrap_text_with_ansi(&reapplied, quote_width)
                            .into_iter()
                            .map(|line| format!("{}{line}", (self.theme.quote_border)("│ "))),
                    );
                }
                add_spacing(&mut lines);
                lines
            }
            Node::ThematicBreak(_) => {
                let mut lines = vec![(self.theme.hr)(&"─".repeat(width.min(80)))];
                add_spacing(&mut lines);
                lines
            }
            Node::Html(html) => vec![self.style_default(html.value.trim())],
            Node::Text(_)
            | Node::Strong(_)
            | Node::Emphasis(_)
            | Node::Delete(_)
            | Node::InlineCode(_)
            | Node::Link(_) => {
                vec![self.render_inline(std::slice::from_ref(node), source, inline_context)]
            }
            _ => node.children().map_or_else(Vec::new, |children| {
                children
                    .iter()
                    .flat_map(|child| {
                        self.render_block(child, width, None, source, inline_context.clone())
                    })
                    .collect()
            }),
        }
    }

    fn source_marker(&self, item: &Node, source: &str, ordered: bool) -> Option<String> {
        let raw = self.raw(item, source)?.trim_start();
        let token = raw.split_whitespace().next()?;
        if ordered {
            let digits = token.trim_end_matches(['.', ')']);
            (!digits.is_empty()
                && digits.len() < 10
                && digits.chars().all(|c| c.is_ascii_digit())
                && token.len() == digits.len() + 1)
                .then(|| format!("{token} "))
        } else {
            matches!(token, "-" | "+" | "*").then(|| format!("{token} "))
        }
    }

    fn render_list(
        &self,
        list: &List,
        depth: usize,
        width: usize,
        source: &str,
        context: Option<(&Style, String)>,
    ) -> Vec<String> {
        let mut lines = Vec::new();
        let indent = "    ".repeat(depth);
        let start = list.start.unwrap_or(1);
        for (index, item_node) in list.children.iter().enumerate() {
            let Node::ListItem(item) = item_node else {
                continue;
            };
            let bullet = if self.options.preserve_ordered_list_markers {
                self.source_marker(item_node, source, list.ordered)
            } else {
                None
            }
            .unwrap_or_else(|| {
                if list.ordered {
                    format!("{}. ", start + index as u32)
                } else {
                    "- ".into()
                }
            });
            let task = item.checked.map_or(String::new(), |checked| {
                format!("[{}] ", if checked { "x" } else { " " })
            });
            let marker = format!("{bullet}{task}");
            let first = format!("{indent}{}", (self.theme.list_bullet)(&marker));
            let continuation = format!("{indent}{}", " ".repeat(visible_width(&marker)));
            let item_width = width.saturating_sub(visible_width(&first)).max(1);
            let mut any = false;
            for child in &item.children {
                if let Node::List(nested) = child {
                    lines.extend(self.render_list(
                        nested,
                        depth + 1,
                        width,
                        source,
                        context.clone(),
                    ));
                    any = true;
                    continue;
                }
                for line in self.render_block(child, item_width, None, source, context.clone()) {
                    for wrapped in wrap_text_with_ansi(&line, item_width) {
                        lines.push(format!(
                            "{}{wrapped}",
                            if any { &continuation } else { &first }
                        ));
                        any = true;
                    }
                }
            }
            if !any {
                lines.push(first);
            }
            if list.spread && index + 1 < list.children.len() {
                lines.push(String::new());
            }
        }
        lines
    }

    fn render_table(
        &self,
        table: &Table,
        width: usize,
        next: Option<&Node>,
        source: &str,
        context: Option<(&Style, String)>,
    ) -> Vec<String> {
        let rows: Vec<Vec<String>> = table
            .children
            .iter()
            .filter_map(|row| match row {
                Node::TableRow(row) => Some(
                    row.children
                        .iter()
                        .filter_map(|cell| match cell {
                            Node::TableCell(cell) => {
                                Some(self.render_inline(&cell.children, source, context.clone()))
                            }
                            _ => None,
                        })
                        .collect(),
                ),
                _ => None,
            })
            .collect();
        let columns = rows.first().map_or(0, Vec::len);
        if columns == 0 {
            return Vec::new();
        }
        let overhead = 3 * columns + 1;
        if width.saturating_sub(overhead) < columns {
            let mut lines = self
                .raw(&Node::Table(table.clone()), source)
                .map_or_else(Vec::new, |raw| wrap_text_with_ansi(raw, width));
            if next.is_some() {
                lines.push(String::new());
            }
            return lines;
        }
        let available = width - overhead;
        let mut natural = vec![0; columns];
        let mut minimum = vec![1; columns];
        for row in &rows {
            for (column, cell) in row.iter().enumerate().take(columns) {
                natural[column] = natural[column].max(visible_width(cell));
                minimum[column] = minimum[column].max(
                    cell.split_whitespace()
                        .map(visible_width)
                        .max()
                        .unwrap_or(0)
                        .min(30),
                );
            }
        }
        let mut widths = minimum.clone();
        if widths.iter().sum::<usize>() > available {
            widths.fill(1);
        }
        while widths.iter().sum::<usize>() < available {
            let Some(column) = (0..columns)
                .filter(|&i| widths[i] < natural[i])
                .max_by_key(|&i| natural[i] - widths[i])
            else {
                break;
            };
            widths[column] += 1;
        }
        let border = |left: char, middle: char, right: char| {
            format!(
                "{left}─{}─{right}",
                widths
                    .iter()
                    .map(|w| "─".repeat(*w))
                    .collect::<Vec<_>>()
                    .join(&format!("─{middle}─"))
            )
        };
        let separator = border('├', '┼', '┤');
        let mut lines = vec![border('┌', '┬', '┐')];
        for (row_index, row) in rows.iter().enumerate() {
            let cells: Vec<Vec<String>> = (0..columns)
                .map(|i| wrap_text_with_ansi(row.get(i).map_or("", String::as_str), widths[i]))
                .collect();
            let height = cells.iter().map(Vec::len).max().unwrap_or(1);
            for line_index in 0..height {
                let parts: Vec<String> = cells
                    .iter()
                    .enumerate()
                    .map(|(column, cell)| {
                        let text = cell.get(line_index).map_or("", String::as_str);
                        let padded = format!(
                            "{text}{}",
                            " ".repeat(widths[column].saturating_sub(visible_width(text)))
                        );
                        if row_index == 0 {
                            (self.theme.bold)(&padded)
                        } else {
                            padded
                        }
                    })
                    .collect();
                lines.push(format!("│ {} │", parts.join(" │ ")));
            }
            if row_index + 1 < rows.len() {
                lines.push(separator.clone());
            }
        }
        lines.push(border('└', '┴', '┘'));
        if next.is_some() {
            lines.push(String::new());
        }
        lines
    }
}

impl Component for Markdown {
    fn render(&self, width: usize) -> Vec<String> {
        if let Some(cache) = self.cache.borrow().as_ref() {
            if cache.text == self.text && cache.width == width {
                return cache.lines.clone();
            }
        }
        let lines = self.render_uncached(width);
        *self.cache.borrow_mut() = Some(Cache {
            text: self.text.clone(),
            width,
            lines: lines.clone(),
        });
        lines
    }
}
