//! Fuzzy matching and filtering.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FuzzyMatch {
    pub matches: bool,
    pub score: f64,
}

pub fn fuzzy_match(query: &str, text: &str) -> FuzzyMatch {
    fn match_query(query: &str, text: &str) -> FuzzyMatch {
        if query.is_empty() {
            return FuzzyMatch {
                matches: true,
                score: 0.0,
            };
        }
        let q: Vec<char> = query.to_lowercase().chars().collect();
        let t: Vec<char> = text.to_lowercase().chars().collect();
        if q.len() > t.len() {
            return FuzzyMatch {
                matches: false,
                score: 0.0,
            };
        }
        let mut qi = 0;
        let mut score = 0.0;
        let mut last: Option<usize> = None;
        let mut consecutive = 0;
        for (i, c) in t.iter().enumerate() {
            if qi < q.len() && *c == q[qi] {
                let boundary =
                    i == 0 || matches!(t[i - 1], ' ' | '\t' | '\n' | '-' | '_' | '.' | '/' | ':');
                if last == i.checked_sub(1) {
                    consecutive += 1;
                    score -= consecutive as f64 * 5.0;
                } else {
                    consecutive = 0;
                    if let Some(previous) = last {
                        score += (i - previous - 1) as f64 * 2.0;
                    }
                }
                if boundary {
                    score -= 10.0;
                }
                score += i as f64 * 0.1;
                last = Some(i);
                qi += 1;
            }
        }
        if qi != q.len() {
            return FuzzyMatch {
                matches: false,
                score: 0.0,
            };
        }
        if q == t {
            score -= 100.0;
        }
        FuzzyMatch {
            matches: true,
            score,
        }
    }
    let primary = match_query(query, text);
    if primary.matches {
        return primary;
    }
    let chars: Vec<char> = query.to_lowercase().chars().collect();
    let split = chars.iter().position(|c| c.is_ascii_digit());
    let swapped = if let Some(i) = split {
        if i > 0 && chars[i..].iter().all(char::is_ascii_digit) {
            Some(chars[i..].iter().chain(&chars[..i]).collect::<String>())
        } else {
            None
        }
    } else {
        let i = chars.iter().position(|c| c.is_ascii_alphabetic());
        i.filter(|&i| {
            i > 0
                && chars[..i].iter().all(char::is_ascii_digit)
                && chars[i..].iter().all(|c| c.is_ascii_alphabetic())
        })
        .map(|i| chars[i..].iter().chain(&chars[..i]).collect())
    };
    swapped.map_or(primary, |q| {
        let m = match_query(&q, text);
        if m.matches {
            FuzzyMatch {
                matches: true,
                score: m.score + 5.0,
            }
        } else {
            primary
        }
    })
}

pub fn fuzzy_filter<T, F>(items: &[T], query: &str, get_text: F) -> Vec<T>
where
    T: Clone,
    F: Fn(&T) -> &str,
{
    if query.trim().is_empty() {
        return items.to_vec();
    }
    let tokens: Vec<&str> = query
        .trim()
        .split(|c: char| c.is_whitespace() || c == '/')
        .filter(|s| !s.is_empty())
        .collect();
    let mut results: Vec<(T, f64)> = items
        .iter()
        .filter_map(|item| {
            let mut total = 0.0;
            for token in &tokens {
                let m = fuzzy_match(token, get_text(item));
                if !m.matches {
                    return None;
                }
                total += m.score;
            }
            Some((item.clone(), total))
        })
        .collect();
    results.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());
    results.into_iter().map(|(item, _)| item).collect()
}
