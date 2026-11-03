const CENSOR_CHAR: char = '♥';

const PROFANE_STEMS: &[&str] = &[
    "хуй", "хуе", "хуя", "хую", "хуи",
    "пизд", "пезд",
    "бля",
    "еб",
    "заеб", "выеб", "наеб", "доеб", "уеб", "поеб", "съеб", "отъеб", "разъеб", "въеб", "объеб", "изъеб",
    "муда", "мудо",
    "пидор", "пидар", "пидр", "педик", "педер",
    "гандон", "гондон",
    "залуп",
    "дроч",
    "елда",
    "сука", "сучар", "сучк",
];

const PROFANE_ROOTS: &[&str] = &[
    "хуй", "хуе", "пизд", "бляд", "залуп",
    "гандон", "гондон", "мудак", "пидор", "пидар", "долбоеб",
];

pub fn censor(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    for ch in text.chars() {

        if ch.is_alphanumeric() || ch == '_' {
            word.push(ch);
        } else {
            flush_word(&mut out, &word);
            word.clear();
            out.push(ch);
        }
    }
    flush_word(&mut out, &word);
    out
}

fn flush_word(out: &mut String, word: &str) {
    if word.is_empty() {
        return;
    }
    if is_profane(word) {
        for _ in word.chars() {
            out.push(CENSOR_CHAR);
        }
    } else {
        out.push_str(word);
    }
}

fn is_profane(word: &str) -> bool {
    let norm = normalize_word(word);
    if norm.is_empty() {
        return false;
    }
    for stem in PROFANE_STEMS {
        if norm.starts_with(stem) {
            return true;
        }
    }
    for root in PROFANE_ROOTS {
        if norm.contains(root) {
            return true;
        }
    }
    false
}

fn normalize_word(w: &str) -> String {
    let lower = w.to_lowercase().replace('ё', "е");
    lower.chars().filter(|c| c.is_alphabetic()).collect()
}
