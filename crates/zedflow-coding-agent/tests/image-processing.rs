use zedflow_coding_agent::utils::image_process::{ResizedImage, format_dimension_note};
#[test]
fn resized_images_describe_dimension_changes() {
    let image = ResizedImage {
        data: String::new(),
        mime_type: "image/png".into(),
        original_width: 100,
        original_height: 50,
        width: 50,
        height: 25,
        was_resized: true,
    };
    assert!(format_dimension_note(&image).unwrap().contains("100x50"));
}
