pub fn format_title_with_parent(title: &str, parent: Option<&str>) -> String {
    match parent {
        None => title.to_string(),
        Some(parent) => format!("{}({})", title, parent),
    }
}
