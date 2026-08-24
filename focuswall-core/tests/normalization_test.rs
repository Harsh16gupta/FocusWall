use focuswall_core::{normalize_domain_input, DomainError};

#[test]
fn test_normalization_various_url_formats() {
    // 1. Bare domain
    let n1 = normalize_domain_input("reddit.com").unwrap();
    assert_eq!(n1.root_domain, "reddit.com");
    assert!(n1.domains.contains(&"reddit.com".to_string()));
    assert!(n1.domains.contains(&"www.reddit.com".to_string()));

    // 2. Full URL with path, query, fragment
    let n2 = normalize_domain_input("https://www.youtube.com/watch?v=123#t=10s").unwrap();
    assert_eq!(n2.root_domain, "youtube.com");

    // 3. Subdomain deep link
    let n3 = normalize_domain_input("https://old.reddit.com/r/all").unwrap();
    assert_eq!(n3.root_domain, "reddit.com");

    // 4. Multi-level TLD
    let n4 = normalize_domain_input("http://news.bbc.co.uk/articles/1").unwrap();
    assert_eq!(n4.root_domain, "bbc.co.uk");
}

#[test]
fn test_normalization_rejections() {
    // Empty
    assert_eq!(normalize_domain_input("   "), Err(DomainError::EmptyInput));

    // Localhost
    assert!(matches!(
        normalize_domain_input("localhost"),
        Err(DomainError::IpOrLocalhostNotAllowed(_))
    ));

    // Direct IP
    assert!(matches!(
        normalize_domain_input("192.168.1.1"),
        Err(DomainError::IpOrLocalhostNotAllowed(_))
    ));
}
