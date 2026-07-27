use crate::Component;
use crate::terminal_image::{
    ImageDimensions, ImageProtocol, ImageRenderOptions, allocate_image_id, get_capabilities,
    get_cell_dimensions, get_image_dimensions, image_fallback, render_image,
};

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
    image_id: u32,
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
        let image_id = options.image_id.unwrap_or_else(allocate_image_id);
        Self {
            base64_data: base64_data.into(),
            mime_type: mime_type.into(),
            dimensions,
            options,
            image_id,
        }
    }

    pub fn image_id(&self) -> u32 {
        self.image_id
    }
}

impl Component for Image {
    fn render(&self, width: usize) -> Vec<String> {
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
        let result = render_image(
            &self.base64_data,
            self.dimensions,
            &ImageRenderOptions {
                max_width_cells: Some(max_width),
                max_height_cells: Some(max_height),
                image_id: Some(self.image_id),
                move_cursor: Some(false),
                ..Default::default()
            },
        );
        let Some(rendered) = result else {
            return vec![image_fallback(
                &self.mime_type,
                Some(self.dimensions),
                self.options.filename.as_deref(),
            )];
        };
        if caps.images == Some(ImageProtocol::Kitty) {
            let mut lines = vec![rendered.sequence];
            lines.resize(rendered.rows as usize, String::new());
            lines
        } else {
            let mut lines = vec![String::new(); rendered.rows.saturating_sub(1) as usize];
            let up = if rendered.rows > 1 {
                format!("\x1b[{}A", rendered.rows - 1)
            } else {
                String::new()
            };
            lines.push(up + &rendered.sequence);
            lines
        }
    }
}
