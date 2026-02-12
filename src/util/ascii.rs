pub fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }

    let hay = haystack.as_bytes();
    let needle = needle.as_bytes();
    if needle.len() > hay.len() {
        return false;
    }

    for start in 0..=(hay.len() - needle.len()) {
        let mut matched = true;
        for i in 0..needle.len() {
            if hay[start + i].to_ascii_lowercase() != needle[i].to_ascii_lowercase() {
                matched = false;
                break;
            }
        }
        if matched {
            return true;
        }
    }

    false
}
