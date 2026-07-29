use zedflow_coding_agent::utils::clipboard_image::extension_for_image_mime_type;

#[test]
fn recognizes_supported_image_mime_types_case_insensitively() {
    assert_eq!(
        extension_for_image_mime_type("IMAGE/JPEG; charset=binary"),
        Some("jpg")
    );
    assert_eq!(extension_for_image_mime_type("image/bmp"), None);
}
