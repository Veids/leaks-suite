use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::ops::AddAssign;
use std::path::Path;

use clap::Parser;
use csv::ByteRecord;
use dotenv::dotenv;
use indicatif::{ProgressBar, ProgressStyle};
use lib::{CredentialData, LeakData};
use serde::Deserialize;

static MAX_JSON_SIZE: usize = 16777216;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Input CSV file
    #[clap(short, long)]
    input: String,

    /// Output file
    #[clap(short, long)]
    output: String,
}

#[derive(Debug, Deserialize)]
struct Leak<'a> {
    domain: &'a [u8],
    subdomain: &'a [u8],
    username: &'a [u8],
    password: &'a [u8],
}

// Function get called very rarely, so i don't think we should
// spend our time optimizing it
fn split(leak_data: LeakData, n: usize) -> Vec<LeakData> {
    let mut splits: Vec<LeakData> = (0..n)
        .map(|_| LeakData {
            domain: leak_data.domain.clone(),
            credentials: Vec::new(),
        })
        .collect();

    let total: usize = leak_data.credentials.iter().map(|x| x.data.len()).sum();
    let neach = total / n;
    let mut left: Vec<usize> = vec![neach; n];
    left.last_mut().unwrap().add_assign(total - neach);

    for mut x in leak_data.credentials.into_iter() {
        for (i, l) in left.iter_mut().enumerate() {
            if *l == 0 {
                continue;
            };

            let x_len = x.data.len();
            if x_len <= *l {
                splits[i].credentials.push(x);
                *l -= x_len;
                break;
            } else {
                let point = x_len - *l;
                let cd = CredentialData {
                    subdomain: x.subdomain.clone(),
                    data: x.data.split_off(point),
                };
                splits[i].credentials.push(cd);
                *l = 0;
            }
        }
    }
    splits
}

fn fflush_object_buffer(
    domain: String,
    credential_datas: HashMap<String, CredentialData>,
    writer: &mut BufWriter<File>,
    pb: &ProgressBar,
) {
    if !credential_datas.is_empty() {
        let leak_data = LeakData {
            domain,
            credentials: credential_datas.into_iter().map(|(_, data)| data).collect(),
        };
        let leak_str = serde_json::to_string(&leak_data).unwrap() + "\n";
        let leak_str_size = leak_str.as_bytes().len();
        if leak_str_size > MAX_JSON_SIZE {
            drop(leak_str);

            pb.println(format!(
                "{} is oversized - {} mb, splitting...",
                &leak_data.domain,
                leak_str_size / 1024 / 1024
            ));

            let n = (leak_str_size + MAX_JSON_SIZE - 1) / MAX_JSON_SIZE;
            for x in split(leak_data, n) {
                let leak_str = serde_json::to_string(&x).unwrap() + "\n";
                writer.write_all(leak_str.as_bytes()).unwrap();
            }
        } else {
            writer.write_all(leak_str.as_bytes()).unwrap();
        }
    }
}

fn parse(csv: &Path, out: &Path) -> Result<(), Box<dyn Error>> {
    let file = File::open(csv)?;
    let pb = ProgressBar::new(file.metadata()?.len());
    pb.enable_steady_tick(500);
    pb.set_style(ProgressStyle::default_bar().template("{spinner:.green} {wide_bar:40.green/black} {bytes:>11.green}/{total_bytes:<11.green} {bytes_per_sec:>13.red} [{elapsed_precise}] eta ({eta:.blue})")
        .progress_chars("━╾╴─"));
    let input_wrap = pb.wrap_read(file);

    let buf_reader = BufReader::new(input_wrap);
    let mut rdr = csv::Reader::from_reader(buf_reader);
    let headers = ByteRecord::from(vec!["domain", "subdomain", "username", "password"]);

    let out_file = File::create(out)?;
    let mut writer = BufWriter::new(out_file);

    let mut credential_datas: HashMap<String, CredentialData> = HashMap::new();

    let mut raw_record = csv::ByteRecord::new();
    let mut last_domain = Vec::new();

    while rdr.read_byte_record(&mut raw_record)? {
        let record: Leak = raw_record.deserialize(Some(&headers))?;

        let username = std::str::from_utf8(record.username)?.to_string();
        let password = std::str::from_utf8(record.password)?.to_string();
        let subdomain = std::str::from_utf8(record.subdomain)?;
        if record.domain == last_domain {
            let entry = if let Some(entry) = credential_datas.get_mut(subdomain) {
                entry
            } else {
                let subdomain = subdomain.to_string();
                let entry = credential_datas
                    .entry(subdomain.clone())
                    .or_insert(CredentialData {
                        subdomain,
                        data: Vec::new(),
                    });
                entry
            };
            entry.data.push((username, password));
        } else {
            let domain_s = std::str::from_utf8(&last_domain)?.to_string();
            fflush_object_buffer(domain_s, credential_datas, &mut writer, &pb);
            credential_datas = HashMap::new();

            let subdomain = subdomain.to_string();
            credential_datas.insert(
                subdomain.clone(),
                CredentialData {
                    subdomain,
                    data: vec![(username, password)],
                },
            );

            // Well i believe you can optimize that fragment
            last_domain = record.domain.to_vec();
        }
    }
    let domain_s = std::str::from_utf8(&last_domain)?.to_string();
    fflush_object_buffer(domain_s, credential_datas, &mut writer, &pb);
    pb.finish();

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    env_logger::init();

    let args = Args::parse();
    let csv = Path::new(&args.input);
    let output = Path::new(&args.output);

    assert!(csv.exists());
    assert!(!output.exists());
    parse(csv, output)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_test_data() -> (usize, LeakData) {
        let arrange: Vec<usize> = vec![100, 200, 300];
        let total_expected: usize = arrange.iter().sum();

        let test_data = LeakData {
            domain: "".to_string(),
            credentials: arrange
                .iter()
                .map(|n| CredentialData {
                    subdomain: "".to_string(),
                    data: vec![("kek".to_string(), "kek".to_string()); *n],
                })
                .collect(),
        };
        (total_expected, test_data)
    }

    #[test]
    fn split_2() {
        let n = 2;
        let (total_expected, test_data) = get_test_data();
        let total: usize = split(test_data, n)
            .iter()
            .map(|x| -> usize { x.credentials.iter().map(|y| y.data.len()).sum() })
            .sum();
        assert_eq!(total, total_expected);
    }

    #[test]
    fn split_3() {
        let n = 3;
        let (total_expected, test_data) = get_test_data();
        let total: usize = split(test_data, n)
            .iter()
            .map(|x| -> usize { x.credentials.iter().map(|y| y.data.len()).sum() })
            .sum();
        assert_eq!(total, total_expected);
    }

    #[test]
    fn split_4() {
        let n = 4;
        let (total_expected, test_data) = get_test_data();
        let total: usize = split(test_data, n)
            .iter()
            .map(|x| -> usize { x.credentials.iter().map(|y| y.data.len()).sum() })
            .sum();
        assert_eq!(total, total_expected);
    }
}
