use std::{
    fs::File,
    io::{prelude::*, BufReader, BufWriter},
    path::Path,
    time::Duration
};

use clap::Parser;
use csv::Writer;
use flate2::bufread::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use lib::{parse_domain, parse_tld};
use regex::Regex;
use suffix::SuffixTable;
use tar::Archive;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// TLD file
    #[clap(short, long)]
    tld: String,

    /// Input file with entries like username@subdomain.domain.tld:password
    #[clap(short, long)]
    input: String,

    /// Input file type: tar.gz or plain
    #[clap(long, default_value = "plain")]
    input_type: String,

    /// Output file
    #[clap(short, long)]
    output: String,

    /// Error file
    #[clap(short, long)]
    error: String,
}

lazy_static! {
    static ref CRED_FIRST_RE: Regex = Regex::new(r"^([a-zA-Z0-9]{1,35}(?:[_\-\.][a-zA-Z0-9]{0,35}){0,10})[:;](.+)@((?:[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.{1,2})+[a-zA-Z0-9][a-zA-Z0-9]{0,61}[a-zA-Z0-9])\.{0,10}$").unwrap();
    static ref CRED_LAST_RE: Regex = Regex::new(r"^([a-zA-Z0-9]{1,35}(?:[_\-\.][a-zA-Z0-9]{0,35}){0,10})@((?:[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.{1,2})+[a-zA-Z0-9][a-zA-Z0-9]{0,61}[a-zA-Z0-9])\.{0,10}[:;](.+)$").unwrap();
}

fn regex_extract(entry: &str) -> Result<(&str, &str, &str), String> {
    // Specifies used format
    // if true: login:password@domain
    // if false: login@domain:password
    let credentials_first = if let Some(n) = entry.find(|c: char| c == '@' || c == ':') {
        entry.as_bytes()[n] == b':'
    } else {
        return Err("Failed to parse entry, separators were not found".to_string());
    };

    let (username, domain, password) = if credentials_first {
        let (username, password, domain) = if let Some(caps) = CRED_FIRST_RE.captures(entry) {
            if caps.len() < 3 {
                return Err("Failed to parse entry".to_string());
            } else {
                (
                    caps.get(1).unwrap().as_str(),
                    caps.get(2).unwrap().as_str(),
                    caps.get(3).unwrap().as_str(),
                )
            }
        } else {
            return Err("Failed to parse entry".to_string());
        };
        (username, domain, password)
    } else {
        let (username, domain, password) = if let Some(caps) = CRED_LAST_RE.captures(entry) {
            if caps.len() < 3 {
                return Err("Failed to parse entry".to_string());
            } else {
                (
                    caps.get(1).unwrap().as_str(),
                    caps.get(2).unwrap().as_str(),
                    caps.get(3).unwrap().as_str(),
                )
            }
        } else {
            return Err("Failed to parse entry".to_string());
        };
        (username, domain, password)
    };

    Ok((username, domain, password))
}

fn parse_entry<'a>(
    entry: &'a str,
    st: &SuffixTable<'static, 'static>,
) -> Result<(&'a str, &'a str, String, String), String> {
    let (username, domain, password) = regex_extract(entry)?;
    if username.len() > 40 {
        return Err("username to long".to_string());
    }

    let domain = domain.trim();

    if domain.is_empty() {
        return Err("domain is empty".to_string());
    }

    let domain = domain.to_lowercase().replace("..", ".");

    let (subdomain, domain) = parse_domain(&domain, st);

    Ok((
        username,
        password,
        subdomain.to_string(),
        domain.to_string(),
    ))
}

struct Indexer {
    st: SuffixTable<'static, 'static>,
    output_writer: Writer<File>,
    error_writer: BufWriter<File>,
    input_type: String,
}

impl Indexer {
    fn new(
        input_type: String,
        output_path: &Path,
        error_path: &Path,
        st: SuffixTable<'static, 'static>,
    ) -> Indexer {
        let output_writer = Writer::from_path(output_path).unwrap();
        let error = File::create(error_path).unwrap();
        let error_writer = BufWriter::new(error);

        Indexer {
            input_type,
            st,
            output_writer,
            error_writer,
        }
    }

    fn entry_reader(&mut self, reader: &mut impl std::io::BufRead) {
        for line in reader.lines() {
            if line.is_err() {
                continue;
            }
            let line = line.unwrap();
            let trimmed = line.trim();
            if let Ok((username, password, subdomain, domain)) = parse_entry(trimmed, &self.st) {
                self.output_writer
                    .write_record(&[&domain, &subdomain, username, password])
                    .unwrap();
            } else {
                self.error_writer
                    .write_all((line + "\n").as_bytes())
                    .unwrap();
            }
        }
    }

    fn process_archive(&mut self, input_reader: &mut impl std::io::BufRead) {
        let tar_gz = GzDecoder::new(input_reader);
        let mut archive = Archive::new(tar_gz);

        for file in archive.entries().unwrap() {
            let file = file.unwrap();
            let path = file.path().unwrap_or_default().into_owned();

            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();

            let mut reader = BufReader::new(file);

            if let Ok(buf) = reader.fill_buf() {
                if let Some(kind) = infer::get(buf) {
                    match kind.matcher_type() {
                        infer::MatcherType::Doc
                        | infer::MatcherType::Image
                        | infer::MatcherType::Text
                        | infer::MatcherType::Archive => continue,
                        _ => {}
                    }
                } else {
                }
            }

            if let Some(ext) = path.extension() {
                if ext == "csv" {
                    continue;
                }
            }

            self.error_writer
                .write_all((format!("//{}\n", name)).as_bytes())
                .unwrap();
            self.entry_reader(&mut reader);
        }
    }

    fn handle_by_type(&mut self, input_reader: &mut impl std::io::BufRead) {
        match self.input_type.as_str() {
            "tar.gz" => self.process_archive(input_reader),
            "plain" => self.entry_reader(input_reader),
            _ => panic!("Unsupported input"),
        }
    }

    fn process(&mut self, input_path: &str) {
        let tick: u64 = 500;
        let (input, pb): (Box<dyn Read>, ProgressBar) = match input_path {
            "-" => {
                let pb = ProgressBar::new_spinner();
                pb.enable_steady_tick(Duration::from_millis(tick));
                (Box::new(std::io::stdin().lock()), pb)
            }
            _ => {
                let input_path = Path::new(input_path);
                let input = File::open(input_path).unwrap();

                let pb = ProgressBar::new(input_path.metadata().unwrap().len());
                pb.enable_steady_tick(Duration::from_millis(tick));
                pb.set_style(ProgressStyle::default_bar().template("{spinner:.green} {wide_bar:.green/black} {bytes:>11.green}/{total_bytes:<11.green} {bytes_per_sec:>13.red} [{elapsed_precise}] eta ({eta:.blue})").unwrap()
            .progress_chars("━╾╴─"));
                (Box::new(input), pb)
            }
        };
        let input_wrap = pb.wrap_read(input);
        let mut reader = BufReader::new(input_wrap);

        self.handle_by_type(&mut reader);
    }
}

fn read_tld(tld_path: &Path) -> String {
    let file = File::open(tld_path).unwrap();
    let mut reader = BufReader::new(file);
    parse_tld(&mut reader)
}

fn main() {
    let args = Args::parse();
    let tld_path = Path::new(&args.tld);
    let input_path = &args.input;
    let output_path = Path::new(&args.output);
    let error_path = Path::new(&args.error);

    env_logger::init();

    let tlds = read_tld(tld_path);
    let st = SuffixTable::new(tlds);

    let mut indexer = Indexer::new(args.input_type, output_path, error_path, st);
    indexer.process(input_path);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gen_test_st() -> SuffixTable<'static, 'static> {
        SuffixTable::new("com net co.uk")
    }

    #[test]
    fn simple() {
        let st = gen_test_st();
        let (username, password, subdomain, domain) =
            parse_entry("wolya@yandex.net:5555", &st).unwrap();
        assert_eq!(username, "wolya");
        assert_eq!(password, "5555");
        assert!(subdomain.is_empty());
        assert_eq!(domain, "yandex.net");
    }

    #[test]
    fn credentials_scary_at() {
        let st = gen_test_st();
        let (username, password, subdomain, domain) =
            parse_entry("username36@yahoo.com:password@", &st).unwrap();
        assert_eq!(username, "username36");
        assert_eq!(password, "password@");
        assert!(subdomain.is_empty());
        assert_eq!(domain, "yahoo.com");
    }

    #[test]
    fn credentials_first() {
        let st = gen_test_st();
        let (username, password, subdomain, domain) =
            parse_entry("wolya:5555@yandex.net", &st).unwrap();
        assert_eq!(username, "wolya");
        assert_eq!(password, "5555");
        assert!(subdomain.is_empty());
        assert_eq!(domain, "yandex.net");
    }

    #[test]
    fn credentials_first_double_at() {
        let st = gen_test_st();
        let (username, password, subdomain, domain) =
            parse_entry("wolya:55@55@yandex.net", &st).unwrap();
        assert_eq!(username, "wolya");
        assert_eq!(password, "55@55");
        assert!(subdomain.is_empty());
        assert_eq!(domain, "yandex.net");
    }

    #[test]
    fn credentials_scary_0() {
        let st = gen_test_st();
        let (username, password, subdomain, domain) =
            parse_entry("wolya@yandex.conm.:5555", &st).unwrap();
        assert_eq!(username, "wolya");
        assert_eq!(password, "5555");
        assert!(subdomain.is_empty());
        assert_eq!(domain, "yandex.conm");
    }

    #[test]
    fn credentials_scary_1() {
        let st = gen_test_st();
        let (username, password, subdomain, domain) =
            parse_entry("wolya@yandex.com..:5555dd", &st).unwrap();
        assert_eq!(username, "wolya");
        assert_eq!(password, "5555dd");
        assert!(subdomain.is_empty());
        assert_eq!(domain, "yandex.com");
    }

    #[test]
    fn credentials_scary_2() {
        let st = gen_test_st();
        let (username, password, subdomain, domain) =
            parse_entry("user.name@wanadoo.fr:Password", &st).unwrap();
        assert_eq!(username, "user.name");
        assert_eq!(password, "Password");
        assert!(subdomain.is_empty());
        assert_eq!(domain, "wanadoo.fr");
    }

    #[test]
    fn credentials_scary_3() {
        let st = gen_test_st();
        let (username, password, subdomain, domain) =
            parse_entry("wolya@gotadsl.co.uk:password!", &st).unwrap();
        assert_eq!(username, "wolya");
        assert_eq!(password, "password!");
        assert!(subdomain.is_empty());
        assert_eq!(domain, "gotadsl.co.uk");
    }

    #[test]
    fn credentials_scary_4() {
        let st = gen_test_st();
        let (username, password, subdomain, domain) =
            parse_entry("user-name@wanadoo.fr:password2password", &st).unwrap();
        assert_eq!(username, "user-name");
        assert_eq!(password, "password2password");
        assert!(subdomain.is_empty());
        assert_eq!(domain, "wanadoo.fr");
    }

    #[test]
    fn no_undescore_domain_name() {
        let st = gen_test_st();
        assert!(parse_entry("user-name@wana_doo.fr:password2password", &st).is_err());
    }

    #[test]
    fn no_undescore_domain_name_2() {
        let st = gen_test_st();
        assert!(parse_entry("user-name:password2password@wana_doo.fr", &st).is_err());
    }

    #[test]
    fn dash_domain_name() {
        let st = gen_test_st();
        let (username, password, subdomain, domain) =
            parse_entry("user-name@wana-doo.fr:password2password", &st).unwrap();
        assert_eq!(username, "user-name");
        assert_eq!(password, "password2password");
        assert!(subdomain.is_empty());
        assert_eq!(domain, "wana-doo.fr");
    }

    #[test]
    fn dash_domain_name_2() {
        let st = gen_test_st();
        let (username, password, subdomain, domain) =
            parse_entry("user-name:password2password@wana-doo.fr", &st).unwrap();
        assert_eq!(username, "user-name");
        assert_eq!(password, "password2password");
        assert!(subdomain.is_empty());
        assert_eq!(domain, "wana-doo.fr");
    }

    #[test]
    fn number_login() {
        let st = gen_test_st();
        let (username, password, subdomain, domain) =
            parse_entry("999999@yahoo.com:112233", &st).unwrap();
        assert_eq!(username, "999999");
        assert_eq!(domain, "yahoo.com");
        assert_eq!(password, "112233");
        assert!(subdomain.is_empty());
    }

    #[test]
    fn domain_case() {
        let st = gen_test_st();
        let (username, password, subdomain, domain) =
            parse_entry("username@AOL.com:password", &st).unwrap();
        assert_eq!(username, "username");
        assert_eq!(password, "password");
        assert!(subdomain.is_empty());
        assert_eq!(domain, "aol.com");
    }

    #[test]
    fn large_username() {
        let st = gen_test_st();
        let (username, password, subdomain, domain) =
            parse_entry("wqwepqowqeiweyyyteyetetqewwqwqw@yahoo.com:parter", &st).unwrap();
        assert_eq!(username, "wqwepqowqeiweyyyteyetetqewwqwqw");
        assert_eq!(password, "parter");
        assert!(subdomain.is_empty());
        assert_eq!(domain, "yahoo.com");
    }

    #[test]
    fn dot_dot() {
        let st = gen_test_st();
        let (username, password, subdomain, domain) =
            parse_entry("username@yahoo..com:parter", &st).unwrap();
        assert_eq!(username, "username");
        assert_eq!(password, "parter");
        assert!(subdomain.is_empty());
        assert_eq!(domain, "yahoo.com");
    }

    #[test]
    fn domain_lowercase() {
        let st = gen_test_st();
        let (username, password, subdomain, domain) =
            parse_entry("username@DOMAIN.COM:parter", &st).unwrap();
        assert_eq!(username, "username");
        assert_eq!(password, "parter");
        assert!(subdomain.is_empty());
        assert_eq!(domain, "domain.com");
    }
}
