use lib::parse_domain;
use suffix::SuffixTable;

#[test]
fn one_subdomain() {
    let tlds = "ru edu.ru com net";
    let st = SuffixTable::new(tlds);
    let domain = "test.yandex.ru";
    let (subdomain, domain) = parse_domain(domain, &st);
    assert_eq!(subdomain, "test");
    assert_eq!(domain, "yandex.ru");
}

#[test]
fn two_subdomains() {
    let tlds = "ru edu.ru com net";
    let st = SuffixTable::new(tlds);
    let domain = "test2.test.yandex.ru";
    let (subdomain, domain) = parse_domain(domain, &st);
    assert_eq!(subdomain, "test2.test");
    assert_eq!(domain, "yandex.ru");
}

#[test]
fn three_subdomains() {
    let tlds = "ru edu.ru com net";
    let st = SuffixTable::new(tlds);
    let domain = "test3.test2.test.yandex.edu.ru";
    let (subdomain, domain) = parse_domain(domain, &st);
    assert_eq!(subdomain, "test3.test2.test");
    assert_eq!(domain, "yandex.edu.ru");
}

#[test]
fn many_subdomains() {
    let tlds = "ru edu.ru com net";
    let st = SuffixTable::new(tlds);
    let domain = "test5.test4.test3.test2.test.yandex.edu.ru";
    let (subdomain, domain) = parse_domain(domain, &st);
    assert_eq!(subdomain, "test5.test4.test3.test2.test");
    assert_eq!(domain, "yandex.edu.ru");
}

#[test]
fn domain() {
    let tlds = "ru edu.ru com net";
    let st = SuffixTable::new(tlds);
    let domain = "yandex.net";
    let (subdomain, domain) = parse_domain(domain, &st);
    assert!(subdomain.is_empty());
    assert_eq!(domain, "yandex.net");
}

#[test]
fn without_domain() {
    let tlds = "ru edu.ru com net";
    let st = SuffixTable::new(tlds);
    let domain = "yandexnet";
    let (subdomain, domain) = parse_domain(domain, &st);
    assert!(subdomain.is_empty());
    assert_eq!(domain, "yandexnet");
}

#[test]
fn utf8_str() {
    let tlds = "ru edu.ru com net co.kr";
    let st = SuffixTable::new(tlds);
    let domain = "Р»вЂћВ¤Р»Сњ РјвЂўв„ў.co.kr";

    let (subdomain, domain) = parse_domain(domain, &st);

    assert!(subdomain.is_empty());
    assert_eq!(domain, "Р»вЂћВ¤Р»Сњ РјвЂўв„ў.co.kr");
}
