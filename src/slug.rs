//! The rule that turns a title into a file name.

pub fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        let mapped: String = match c {
            'İ' | 'I' => "i".into(),
            _ => c.to_lowercase().collect(),
        };
        for m in mapped.chars() {
            let m = match m {
                'ı' => 'i',
                'ş' => 's',
                'ğ' => 'g',
                'ç' => 'c',
                'ö' => 'o',
                'ü' => 'u',
                '\u{307}' => continue, // combining dot left over from lowercasing
                other => other,
            };
            if m.is_alphanumeric() {
                out.push(m);
            } else if !out.ends_with('-') {
                out.push('-');
            }
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::slug;

    #[test]
    fn examples() {
        assert_eq!(slug("Entry protocol"), "entry-protocol");
        assert_eq!(slug("Tick sırası"), "tick-sirasi");
        assert_eq!(slug("  İstanbul -- Çağ!  "), "istanbul-cag");
    }
}
