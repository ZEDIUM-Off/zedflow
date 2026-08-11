use std::{
    cell::{Cell, RefCell},
    sync::Arc,
};

use crate::Component;
use crate::terminal_image::{
    ImageDimensions, ImageProtocol, ImageRenderOptions, allocate_image_id, get_capabilities,
    get_cell_dimensions, get_image_dimensions, image_fallback, render_image,
};

type Style = Arc<dyn Fn(&str) -> String + Send + Sync>;

#[derive(Clone)]
pub struct ImageTheme {
    pub fallback_color: Style,
}

impl Default for ImageTheme {
    fn default() -> Self {
        Self {
            fallback_color: Arc::new(str::to_owned),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ImageOptions {
    pub max_width_cells: Option<u32>,
    pub max_height_cells: Option<u32>,
    pub filename: Option<String>,
    pub image_id: Option<u32>,
}

pub struct Image {
    pub base64_data: String,
    pub mime_type: String,
    pub dimensions: ImageDimensions,
    pub options: ImageOptions,
    theme: ImageTheme,
    image_id: Cell<Option<u32>>,
    cache: RefCell<Option<(usize, Vec<String>)>>,
}

impl Image {
    pub fn new(base64_data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        let base64_data = base64_data.into();
        let mime_type = mime_type.into();
        let dimensions =
            get_image_dimensions(&base64_data, &mime_type).unwrap_or(ImageDimensions {
                width: 800,
                height: 600,
            });
        Self::with_options(base64_data, mime_type, ImageOptions::default(), dimensions)
    }

    pub fn with_options(
        base64_data: impl Into<String>,
        mime_type: impl Into<String>,
        options: ImageOptions,
        dimensions: ImageDimensions,
    ) -> Self {
        Self {
            base64_data: base64_data.into(),
            mime_type: mime_type.into(),
            dimensions,
            image_id: Cell::new(options.image_id),
            options,
            theme: ImageTheme::default(),
            cache: RefCell::new(None),
        }
    }

    pub fn set_theme(&mut self, theme: ImageTheme) {
        self.theme = theme;
        self.invalidate();
    }

    pub fn image_id(&self) -> Option<u32> {
        self.image_id.get()
    }
}

impl Component for Image {
    fn render(&self, width: usize) -> Vec<String> {
        if let Some((cached_width, lines)) = self.cache.borrow().as_ref() {
            if *cached_width == width {
                return lines.clone();
            }
        }

        let max_width = (width.saturating_sub(2) as u32)
            .min(self.options.max_width_cells.unwrap_or(60))
            .max(1);
        let cell = get_cell_dimensions();
        let max_height = self.options.max_height_cells.unwrap_or_else(|| {
            (max_width * cell.width.max(1))
                .div_ceil(cell.height.max(1))
                .max(1)
        });
        let caps = get_capabilities();
        if caps.images == Some(ImageProtocol::Kitty) && self.image_id.get().is_none() {
            self.image_id.set(Some(allocate_image_id()));
        }
        let rendered = render_image(
            &self.base64_data,
            self.dimensions,
            &ImageRenderOptions {
                max_width_cells: Some(max_width),
                max_height_cells: Some(max_height),
                image_id: self.image_id.get(),
                move_cursor: Some(false),
                ..Default::default()
            },
        );
        let lines = match rendered {
            None => vec![(self.theme.fallback_color)(&image_fallback(
                &self.mime_type,
                Some(self.dimensions),
                self.options.filename.as_deref(),
            ))],
            Some(rendered) if caps.images == Some(ImageProtocol::Kitty) => {
                if let Some(id) = rendered.image_id {
                    self.image_id.set(Some(id));
                }
                let mut lines = vec![rendered.sequence];
                lines.resize(rendered.rows as usize, String::new());
                lines
            }
            Some(rendered) => {
                let mut lines = vec![String::new(); rendered.rows.saturating_sub(1) as usize];
                let up = (rendered.rows > 1)
                    .then(|| format!("\x1b[{}A", rendered.rows - 1))
                    .unwrap_or_default();
                lines.push(up + &rendered.sequence);
                lines
            }
        };
        *self.cache.borrow_mut() = Some((width, lines.clone()));
        lines
    }

    fn invalidate(&mut self) {
        *self.cache.borrow_mut() = None;
    }
}
