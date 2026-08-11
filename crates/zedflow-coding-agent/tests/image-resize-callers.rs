use zedflow_coding_agent::utils::image_resize_core::resize_image_in_process;

#[test]
fn resize_callers_reject_invalid_image_data() {
    assert!(resize_image_in_process(b"not an image", "image/png", None).is_none());
}
