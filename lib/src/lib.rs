use std::io::BufRead;

use serde::{Deserialize, Serialize};
use suffix::SuffixTable;

/// Parses domain into the following parts: subdomain, domain, tld
///
/// # Arguments
///
/// * `domain` - A domain like cloud.yandex.net
/// * `st` - SuffixTable populated with subdomain list
///
/// # Example
///
/// ```
/// use lib::parse_domain;
/// use suffix::SuffixTable;
///
/// let st = SuffixTable::new("edu.ru ru");
/// let (subdomain, domain) = parse_domain("cloud.yandex.edu.ru", &st);
///
/// assert_eq!(subdomain, "cloud");
/// assert_eq!(domain, "yandex.edu.ru");
/// ```
pub fn parse_domain<'a>(domain: &'a str, st: &SuffixTable) -> (&'a str, &'a str) {
    let mut parts = domain.match_indices('.').rev().take(3);

    if let (Some(_), Some(elem)) = (parts.next(), parts.next()) {
        let sd = elem.0;
        let sdn = sd + 1;

        let (td, tdn) = if let Some(elem) = parts.next() {
            (elem.0, elem.0 + 1)
        } else {
            (0, 0)
        };

        let tld_part = &domain[sdn..];
        let tld_pos = st.positions(tld_part);

        if !tld_pos.is_empty() {
            (&domain[..td], &domain[tdn..])
        } else {
            (&domain[..sd], &domain[sdn..])
        }
    } else {
        ("", domain)
    }
}

pub fn parse_tld(reader: &mut impl BufRead) -> String {
    let mut res = String::with_capacity(84000);

    for line in reader.lines() {
        let line = line.unwrap();
        let trimmed = line.trim();

        if trimmed.starts_with("// ===BEGIN PRIVATE DOMAINS") {
            break;
        }

        if !trimmed.is_empty() && !trimmed.starts_with("//") {
            res.push_str(trimmed);
            res.push(' ');
        }
    }
    res.pop();
    res
}

#[derive(Serialize, Deserialize)]
pub struct CredentialData {
    pub subdomain: String,
    pub data: Vec<(String, String)>,
}

#[derive(Serialize, Deserialize)]
pub struct LeakData {
    pub domain: String,
    pub credentials: Vec<CredentialData>,
}
