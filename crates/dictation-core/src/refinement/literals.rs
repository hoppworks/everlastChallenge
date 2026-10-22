use std::ops::Range;

pub(super) fn protected_atom(atom: &str) -> bool {
    atom.chars().any(|c| {
        c.is_numeric()
            || matches!(
                c,
                '@' | '/' | '\\' | '=' | '_' | '<' | '>' | '`' | '#' | '+' | '-' | '%'
            )
    }) || atom.split('.').filter(|part| !part.is_empty()).count() > 1
        || matches!(
            atom.trim_matches(|c| matches!(c, '(' | ')' | ',' | ';' | ':')),
            "z." | "B."
                | "bzw."
                | "Dr."
                | "Prof."
                | "ca."
                | "u."
                | "a."
                | "Mr."
                | "Mrs."
                | "Ms."
                | "etc."
                | "vs."
        )
}

/// Complete whitespace-delimited literal spans, including signs and separators.
/// Conservative boundaries avoid rewriting only one component of a value.
pub(super) fn ranges(source: &str) -> Vec<Range<usize>> {
    let mut result = Vec::new();
    let mut start = None;
    for (index, c) in source
        .char_indices()
        .chain(std::iter::once((source.len(), ' ')))
    {
        if !c.is_whitespace() {
            start.get_or_insert(index);
        } else if let Some(begin) = start.take()
            && protected_atom(&source[begin..index])
        {
            result.push(begin..index);
        }
    }
    result
}
