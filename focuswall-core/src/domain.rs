//! Domain normalization and list definitions.

use thiserror::Error;
use url::Url;

/// Canonical list of YouTube domains that must be blocked.
pub const YOUTUBE_DOMAINS: &[&str] = &[
    "youtube.com",
    "www.youtube.com",
    "m.youtube.com",
    "music.youtube.com",
    "youtu.be",
    "youtube-nocookie.com",
    "ytimg.com",
    "googlevideo.com",
];

#[derive(Error, Debug, PartialEq, Eq)]
pub enum DomainError {
    #[error("Input domain or URL cannot be empty")]
    EmptyInput,
    #[error("Invalid URL format: {0}")]
    InvalidUrl(String),
    #[error("Could not extract a valid hostname from '{0}'")]
    MissingHost(String),
    #[error("Invalid or unresolvable domain '{0}'")]
    InvalidDomain(String),
    #[error("IP addresses and localhost cannot be added as domain rules: '{0}'")]
    IpOrLocalhostNotAllowed(String),
}

/// Normalized domain representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedRule {
    /// The registrable root domain (e.g., "reddit.com", "example.co.uk")
    pub root_domain: String,
    /// List of exact and wildcard domain patterns to block
    pub domains: Vec<String>,
}

/// Normalizes raw user input (e.g. `reddit.com/r/rust`, `https://www.reddit.com`, `sub.example.co.uk`)
/// into a canonical registrable domain using the Public Suffix List.
pub fn normalize_domain_input(raw_input: &str) -> Result<NormalizedRule, DomainError> {
    let trimmed = raw_input.trim();
    if trimmed.is_empty() {
        return Err(DomainError::EmptyInput);
    }

    // Step 1: Prepend https:// if no scheme is provided
    let url_str = if !trimmed.contains("://") {
        format!("https://{}", trimmed)
    } else {
        trimmed.to_string()
    };

    // Step 2: Parse with the url crate
    let parsed_url = Url::parse(&url_str)
        .map_err(|e| DomainError::InvalidUrl(format!("{}: {}", trimmed, e)))?;

    // Step 3: Extract host
    let host = parsed_url
        .host_str()
        .ok_or_else(|| DomainError::MissingHost(trimmed.to_string()))?
        .to_lowercase();

    // Check for localhost or direct IP
    if host == "localhost" || host.parse::<std::net::IpAddr>().is_ok() {
        return Err(DomainError::IpOrLocalhostNotAllowed(host));
    }

    if !is_valid_hostname(&host) {
        return Err(DomainError::InvalidDomain(trimmed.to_string()));
    }

    // Strip leading www. if present
    let stripped_host = host.strip_prefix("www.").unwrap_or(&host);

    // Step 4: Extract registrable domain using Public Suffix List
    let root_domain = match psl::domain_str(stripped_host) {
        Some(d) => d.to_string(),
        None => stripped_host.to_string(),
    };

    if root_domain.is_empty() || !root_domain.contains('.') || !is_valid_hostname(&root_domain) {
        return Err(DomainError::InvalidDomain(trimmed.to_string()));
    }

    // Step 5: Generate domain list for dnsmasq/nftables (root + www representation)
    let domains = vec![
        root_domain.clone(),
        format!("www.{}", root_domain),
    ];

    Ok(NormalizedRule {
        root_domain,
        domains,
    })
}

/// Validates that a domain string strictly conforms to RFC hostname standards
/// and does not contain any control characters, newlines, slashes, or injection characters.
pub fn is_valid_hostname(domain: &str) -> bool {
    if domain.is_empty() || domain.len() > 253 {
        return false;
    }
    for label in domain.split('.') {
        if label.is_empty() || label.len() > 63 {
            return false;
        }
        if label.starts_with('-') || label.ends_with('-') {
            return false;
        }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_raw_domains() {
        // Plain domain
        let res1 = normalize_domain_input("reddit.com").unwrap();
        assert_eq!(res1.root_domain, "reddit.com");

        // Domain with path and params
        let res2 = normalize_domain_input("reddit.com/r/programming?sort=new").unwrap();
        assert_eq!(res2.root_domain, "reddit.com");

        // Domain with full URL and https://www.
        let res3 = normalize_domain_input("https://www.reddit.com/r/rust").unwrap();
        assert_eq!(res3.root_domain, "reddit.com");

        // Subdomain (e.g. old.reddit.com -> root is reddit.com)
        let res4 = normalize_domain_input("https://old.reddit.com/").unwrap();
        assert_eq!(res4.root_domain, "reddit.com");

        // Multi-level TLD (co.uk)
        let res5 = normalize_domain_input("https://www.bbc.co.uk/news").unwrap();
        assert_eq!(res5.root_domain, "bbc.co.uk");
    }

    #[test]
    fn test_normalize_invalid_inputs() {
        assert_eq!(normalize_domain_input(""), Err(DomainError::EmptyInput));
        assert_eq!(
            normalize_domain_input("localhost"),
            Err(DomainError::IpOrLocalhostNotAllowed("localhost".to_string()))
        );
        assert_eq!(
            normalize_domain_input("127.0.0.1"),
            Err(DomainError::IpOrLocalhostNotAllowed("127.0.0.1".to_string()))
        );
        assert_eq!(
            normalize_domain_input("nodotdomain"),
            Err(DomainError::InvalidDomain("nodotdomain".to_string()))
        );
    }

    #[test]
    fn test_hostname_validation_and_injection_prevention() {
        assert!(is_valid_hostname("reddit.com"));
        assert!(is_valid_hostname("sub.domain.co.uk"));
        assert!(is_valid_hostname("youtube.com"));

        // Injections & invalid characters
        assert!(!is_valid_hostname("reddit.com\nserver=/evil.com/1.1.1.1"));
        assert!(!is_valid_hostname("reddit.com/path"));
        assert!(!is_valid_hostname("reddit; rm -rf /"));
        assert!(!is_valid_hostname("-reddit.com"));
        assert!(!is_valid_hostname("reddit-.com"));
        assert!(!is_valid_hostname(""));
    }
}
