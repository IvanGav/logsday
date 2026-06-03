pub fn slug_from(name: &str) -> String {
    let mut s = "".to_string();
    for c in name.chars() {
        match c {
            'a'..='z' | '0'..='9' | '-' | '_' => { s.push(c); },
            'A'..='Z' => { s.push(c.to_ascii_lowercase()); }
            ' ' | '\t' => { s.push('_'); }
            _ => { }
        }
    }
    if s == "" { return "-".to_string(); }
    return s;
}

pub fn slug_valid(slug: &str) -> bool {
    for c in slug.chars() {
        match c {
            'a'..='z' | '0'..='9' | '_' | '-' => {},
            c=> { return false; }
        }
    }
    return true;
}