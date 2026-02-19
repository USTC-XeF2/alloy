use super::segment::{AT_PLACEHOLDER_PREFIX, HandleRegistry, IMAGE_PLACEHOLDER_PREFIX};
use alloy_core::RichTextSegment;

/// Simple shell-like argument splitting for plain text.
///
/// Handles:
/// - Space-separated arguments
/// - Quoted strings (single and double quotes)
/// - Escape sequences within quotes
pub fn shell_split(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escape_next = false;

    for ch in input.chars() {
        if escape_next {
            current.push(ch);
            escape_next = false;
            continue;
        }

        match ch {
            '\\' if in_double_quote => {
                escape_next = true;
            }
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            ' ' | '\t' if !in_single_quote && !in_double_quote => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        args.push(current);
    }

    args
}

/// Splits rich text segments into shell-like arguments.
///
/// This function processes a sequence of [`RichTextSegment`]s and produces
/// a list of string tokens suitable for clap parsing:
///
/// - **`Text`** segments are split using standard shell rules (whitespace
///   separation, quoted strings). A segment boundary always acts as a word
///   break, so text in separate segments is never concatenated.
/// - **`Image`** and **`At`** segments are replaced by unique placeholder
///   tokens (`\x00IMG_0`, `\x00AT_0`, etc.) that each become a single
///   argument.
///
/// Returns the argument list together with a [`HandleRegistry`] that maps
/// placeholders back to their original values.
pub fn rich_text_shell_split(segments: &[RichTextSegment]) -> (Vec<String>, HandleRegistry) {
    let mut args: Vec<String> = Vec::new();
    let mut registry = HandleRegistry::default();
    let mut img_counter: usize = 0;
    let mut at_counter: usize = 0;

    for seg in segments {
        match seg {
            RichTextSegment::Text(text) => {
                // Shell-split the text content; each resulting token becomes
                // its own argument. Segment boundaries act as whitespace.
                let sub_args = shell_split(text);
                args.extend(sub_args);
            }
            RichTextSegment::Image(reference) => {
                let placeholder = format!("{IMAGE_PLACEHOLDER_PREFIX}{img_counter}");
                img_counter += 1;
                registry
                    .images
                    .insert(placeholder.clone(), reference.clone());
                args.push(placeholder);
            }
            RichTextSegment::At(user_id) => {
                let placeholder = format!("{AT_PLACEHOLDER_PREFIX}{at_counter}");
                at_counter += 1;
                registry.ats.insert(placeholder.clone(), user_id.clone());
                args.push(placeholder);
            }
        }
    }

    (args, registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_split_simple() {
        let args = shell_split("/echo hello world");
        assert_eq!(args, vec!["/echo", "hello", "world"]);
    }

    #[test]
    fn test_shell_split_quoted() {
        let args = shell_split(r#"/echo "hello world" test"#);
        assert_eq!(args, vec!["/echo", "hello world", "test"]);
    }

    #[test]
    fn test_shell_split_single_quoted() {
        let args = shell_split("/echo 'hello world' test");
        assert_eq!(args, vec!["/echo", "hello world", "test"]);
    }

    #[test]
    fn test_shell_split_mixed_quotes() {
        let args = shell_split(r#"/cmd "double's quote" 'single"s quote'"#);
        assert_eq!(args, vec!["/cmd", "double's quote", r#"single"s quote"#]);
    }

    #[test]
    fn test_shell_split_empty() {
        let args = shell_split("");
        assert!(args.is_empty());
    }

    #[test]
    fn test_shell_split_whitespace_only() {
        let args = shell_split("   \t  ");
        assert!(args.is_empty());
    }

    #[test]
    fn test_rich_text_split_text_only() {
        let segments = vec![RichTextSegment::Text("/echo hello world".into())];
        let (args, registry) = rich_text_shell_split(&segments);
        assert_eq!(args, vec!["/echo", "hello", "world"]);
        assert!(registry.images.is_empty());
        assert!(registry.ats.is_empty());
    }

    #[test]
    fn test_rich_text_split_with_image() {
        let segments = vec![
            RichTextSegment::Text("/send ".into()),
            RichTextSegment::Image("abc.jpg".into()),
        ];
        let (args, registry) = rich_text_shell_split(&segments);
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "/send");
        assert!(args[1].starts_with(IMAGE_PLACEHOLDER_PREFIX));
        assert_eq!(registry.images.get(&args[1]).unwrap(), "abc.jpg");
    }

    #[test]
    fn test_rich_text_split_with_at() {
        let segments = vec![
            RichTextSegment::Text("/kick ".into()),
            RichTextSegment::At("12345".into()),
            RichTextSegment::Text(" reason".into()),
        ];
        let (args, registry) = rich_text_shell_split(&segments);
        assert_eq!(args.len(), 3);
        assert_eq!(args[0], "/kick");
        assert!(args[1].starts_with(AT_PLACEHOLDER_PREFIX));
        assert_eq!(args[2], "reason");
        assert_eq!(registry.ats.get(&args[1]).unwrap(), "12345");
    }

    #[test]
    fn test_rich_text_split_mixed() {
        let segments = vec![
            RichTextSegment::Text("/cmd arg1 ".into()),
            RichTextSegment::At("user1".into()),
            RichTextSegment::Text(" ".into()),
            RichTextSegment::Image("pic.png".into()),
            RichTextSegment::Text(" arg2".into()),
        ];
        let (args, registry) = rich_text_shell_split(&segments);
        assert_eq!(args.len(), 5);
        assert_eq!(args[0], "/cmd");
        assert_eq!(args[1], "arg1");
        assert!(args[2].starts_with(AT_PLACEHOLDER_PREFIX));
        assert!(args[3].starts_with(IMAGE_PLACEHOLDER_PREFIX));
        assert_eq!(args[4], "arg2");
        assert_eq!(registry.images.len(), 1);
        assert_eq!(registry.ats.len(), 1);
    }

    #[test]
    fn test_rich_text_split_segment_boundary_breaks() {
        // Two text segments with no whitespace between them should still
        // act as separate tokens because segment boundaries break words.
        let segments = vec![
            RichTextSegment::Text("/echo".into()),
            RichTextSegment::Text("hello".into()),
        ];
        let (args, _) = rich_text_shell_split(&segments);
        assert_eq!(args, vec!["/echo", "hello"]);
    }
}
