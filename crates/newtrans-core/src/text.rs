//! Text preparation before translation.

/// Joins hard-wrapped lines (PDF, terminal, e-mail) into sentences while
/// keeping paragraph breaks. A line break is kept when the previous line
/// ends a sentence, is empty, or the next line starts a list item.
pub fn join_wrapped_lines(input: &str) -> String {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.lines().map(str::trim_end).collect();
    let mut out = String::with_capacity(normalized.len());
    for (index, line) in lines.iter().enumerate() {
        out.push_str(line);
        let Some(next) = lines.get(index + 1) else {
            break;
        };
        let keep_break = line.trim().is_empty()
            || next.trim().is_empty()
            || ends_sentence(line)
            || starts_list_item(next);
        if keep_break {
            out.push('\n');
        } else if line.ends_with('-') && !line.ends_with("--") {
            // Hyphenated word broken across lines: "transla-\ntion".
            out.pop();
        } else {
            out.push(' ');
        }
    }
    collapse_blank_lines(out.trim())
}

fn ends_sentence(line: &str) -> bool {
    matches!(
        line.trim_end().chars().last(),
        Some('.' | '!' | '?' | ':' | '。' | '！' | '？' | '"' | '”' | ')')
    )
}

fn starts_list_item(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with(['-', '*', '•', '·', '▪', '○'])
        || trimmed.split_once(['.', ')']).is_some_and(|(head, _)| {
            !head.is_empty() && head.len() <= 3 && head.chars().all(|c| c.is_ascii_digit())
        })
}

fn collapse_blank_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut blank_run = 0;
    for line in text.split('\n') {
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.pop();
    out
}

/// Cheap script-based guess used before calling a provider, so a Korean
/// selection goes straight to the fallback language instead of wasting a
/// request. Returns `None` for Latin text where the script is ambiguous.
pub fn guess_language(text: &str) -> Option<&'static str> {
    let (mut hangul, mut kana, mut han, mut cyrillic, mut letters) = (0usize, 0, 0, 0, 0);
    for ch in text.chars() {
        let code = ch as u32;
        if !ch.is_alphabetic() {
            continue;
        }
        letters += 1;
        match code {
            0xAC00..=0xD7A3 | 0x1100..=0x11FF | 0x3130..=0x318F => hangul += 1,
            0x3040..=0x30FF => kana += 1,
            0x4E00..=0x9FFF => han += 1,
            0x0400..=0x04FF => cyrillic += 1,
            _ => {}
        }
    }
    if letters == 0 {
        return None;
    }
    let share = |count: usize| count * 100 / letters;
    if share(hangul) >= 30 {
        Some("ko")
    } else if kana > 0 && share(kana + han) >= 30 {
        Some("ja")
    } else if share(cyrillic) >= 50 {
        Some("ru")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_soft_wrapped_lines() {
        let input = "This is a sentence that was\nwrapped by a PDF viewer.\nNext sentence.";
        assert_eq!(
            join_wrapped_lines(input),
            "This is a sentence that was wrapped by a PDF viewer.\nNext sentence."
        );
    }

    #[test]
    fn keeps_paragraphs_and_lists() {
        let input = "Intro line\n\n\n\n- first\n- second\n1. one\n2. two";
        assert_eq!(
            join_wrapped_lines(input),
            "Intro line\n\n- first\n- second\n1. one\n2. two"
        );
    }

    #[test]
    fn rejoins_hyphenated_words() {
        assert_eq!(
            join_wrapped_lines("transla-\ntion works"),
            "translation works"
        );
    }

    #[test]
    fn handles_crlf() {
        assert_eq!(join_wrapped_lines("a\r\nb"), "a b");
    }

    #[test]
    fn guesses_scripts() {
        assert_eq!(guess_language("안녕하세요 world"), Some("ko"));
        assert_eq!(guess_language("これは日本語です"), Some("ja"));
        assert_eq!(guess_language("Привет мир"), Some("ru"));
        assert_eq!(guess_language("Hello world"), None);
        assert_eq!(guess_language("12345 !!"), None);
    }
}
