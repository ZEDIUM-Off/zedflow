use zedflow_tui::word_navigation::{find_word_backward, find_word_forward};

#[test]
fn navigates_words_and_punctuation_segments() {
    assert_eq!(find_word_backward("hello world", 11), 6);
    assert_eq!(find_word_backward("foo.bar", 7), 4);
    assert_eq!(find_word_forward("hello world", 0), 5);
    assert_eq!(find_word_forward("foo.bar", 3), 4);
    assert_eq!(find_word_forward("hello", 5), 5);
}

#[test]
fn handles_unicode_and_boundaries() {
    let text = "你好世界 test";
    assert!(find_word_backward(text, text.len()) < text.len());
    assert_eq!(find_word_backward("hello", 0), 0);
    let mut pos = 0;
    while pos < text.len() {
        let next = find_word_forward(text, pos);
        assert!(next > pos);
        pos = next;
    }
    assert_eq!(pos, text.len());
}
