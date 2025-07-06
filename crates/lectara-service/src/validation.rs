use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fmt;
use thiserror::Error;
use url::Url;

#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("URL cannot be empty")]
    EmptyUrl,
    #[error("Malformed URL: {0}")]
    MalformedUrl(String),
    #[error("URL must have a host")]
    MissingHost,
    #[error("Local addresses not allowed: {0}")]
    LocalAddress(String),
    #[error("Unsupported URL scheme: {0}")]
    UnsupportedScheme(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Scheme {
    Http,
    Https,
}

impl fmt::Display for Scheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Scheme::Http => write!(f, "http"),
            Scheme::Https => write!(f, "https"),
        }
    }
}

/// A URL that has been validated for internet content access
/// Guarantees: non-empty host, HTTP/HTTPS scheme, no local addresses
#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedUrl {
    pub scheme: Scheme,

    /// guaranteed non-empty and non-local
    pub host: String,

    /// only non-default ports
    pub port: Option<u16>,

    /// normalized (no trailing slash except root)
    pub path: String,

    /// sorted parameters as structured data
    pub query: Option<BTreeMap<String, String>>,
}

impl fmt::Display for ValidatedUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}://{}", self.scheme, self.host)?;

        if let Some(port) = self.port {
            write!(f, ":{port}")?;
        }

        write!(f, "{}", self.path)?;

        if let Some(ref query_params) = self.query {
            if !query_params.is_empty() {
                write!(f, "?")?;
                let query_string = query_params
                    .iter()
                    .map(|(k, v)| {
                        if v.is_empty() {
                            k.clone()
                        } else {
                            format!("{k}={v}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("&");
                write!(f, "{query_string}")?;
            }
        }

        Ok(())
    }
}

impl TryFrom<Url> for ValidatedUrl {
    type Error = ValidationError;

    fn try_from(url: Url) -> Result<Self, Self::Error> {
        // Parse and validate scheme
        let scheme = match url.scheme() {
            "http" => Scheme::Http,
            "https" => Scheme::Https,
            scheme => return Err(ValidationError::UnsupportedScheme(scheme.to_string())),
        };

        // Must have a host for internet content
        let host = url.host_str().ok_or(ValidationError::MissingHost)?;

        // Host cannot be empty
        if host.is_empty() {
            return Err(ValidationError::MissingHost);
        }

        // Normalize host to lowercase and check for local addresses
        let host = host.to_lowercase();
        if host == "localhost"
            || host.starts_with("127.")
            || host.starts_with("192.168.")
            || host.starts_with("10.")
        {
            return Err(ValidationError::LocalAddress(host));
        }

        // Normalize port (remove default ports)
        let port = url.port().filter(|&p| {
            let default_port = match scheme {
                Scheme::Http => 80,
                Scheme::Https => 443,
            };
            p != default_port
        });

        // Normalize path
        let path = url.path();
        let path = if path.is_empty() || path == "/" {
            "/".to_string()
        } else if let Some(stripped) = path.strip_suffix('/') {
            stripped.to_string()
        } else {
            path.to_string()
        };

        // Sort query parameters as structured data
        let query = if url.query().is_some() {
            let mut params: BTreeMap<String, String> = BTreeMap::new();
            for (key, value) in url.query_pairs() {
                params.insert(key.to_string(), value.to_string());
            }

            if params.is_empty() {
                None
            } else {
                Some(params)
            }
        } else {
            None
        };

        Ok(ValidatedUrl {
            scheme,
            host,
            port,
            path,
            query,
        })
    }
}

pub fn validate_url(url_str: &str) -> Result<ValidatedUrl, ValidationError> {
    if url_str.is_empty() {
        return Err(ValidationError::EmptyUrl);
    }

    let url =
        Url::parse(url_str).map_err(|_| ValidationError::MalformedUrl(url_str.to_string()))?;
    ValidatedUrl::try_from(url)
}

pub fn normalize_url(url_str: &str) -> Result<String, ValidationError> {
    let validated_url = validate_url(url_str)?;
    Ok(validated_url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Valid URL tests
    #[test]
    fn test_validate_https_url() {
        assert!(validate_url("https://example.com").is_ok());
    }

    #[test]
    fn test_validate_http_url() {
        assert!(validate_url("http://example.com").is_ok());
    }

    #[test]
    fn test_validate_url_with_path() {
        assert!(validate_url("https://example.com/path").is_ok());
    }

    #[test]
    fn test_validate_url_with_query() {
        assert!(validate_url("https://example.com/path?query=value").is_ok());
    }

    #[test]
    fn test_validate_subdomain_url() {
        assert!(validate_url("https://subdomain.example.com").is_ok());
    }

    #[test]
    fn test_validate_url_with_non_default_port() {
        assert!(validate_url("https://example.com:8080/path").is_ok());
    }

    // Invalid URL tests with specific error type checking
    #[test]
    fn test_empty_url_returns_empty_error() {
        assert!(matches!(validate_url(""), Err(ValidationError::EmptyUrl)));
    }

    #[test]
    fn test_malformed_url_returns_malformed_error() {
        assert!(matches!(
            validate_url("not-a-url"),
            Err(ValidationError::MalformedUrl(_))
        ));
    }

    #[test]
    fn test_ftp_scheme_returns_unsupported_error() {
        assert!(matches!(
            validate_url("ftp://example.com"),
            Err(ValidationError::UnsupportedScheme(_))
        ));
    }

    #[test]
    fn test_javascript_scheme_returns_unsupported_error() {
        assert!(matches!(
            validate_url("javascript:alert('xss')"),
            Err(ValidationError::UnsupportedScheme(_))
        ));
    }

    #[test]
    fn test_data_scheme_returns_unsupported_error() {
        assert!(matches!(
            validate_url("data:text/html,<script>alert('xss')</script>"),
            Err(ValidationError::UnsupportedScheme(_))
        ));
    }

    #[test]
    fn test_https_without_host_returns_malformed_error() {
        assert!(matches!(
            validate_url("https://"),
            Err(ValidationError::MalformedUrl(_))
        ));
    }

    #[test]
    fn test_http_without_host_returns_malformed_error() {
        assert!(matches!(
            validate_url("http://"),
            Err(ValidationError::MalformedUrl(_))
        ));
    }

    #[test]
    fn test_url_without_scheme_returns_malformed_error() {
        assert!(matches!(
            validate_url("://example.com"),
            Err(ValidationError::MalformedUrl(_))
        ));
    }

    #[test]
    fn test_localhost_returns_local_address_error() {
        assert!(matches!(
            validate_url("https://localhost/path"),
            Err(ValidationError::LocalAddress(_))
        ));
    }

    #[test]
    fn test_loopback_ip_returns_local_address_error() {
        assert!(matches!(
            validate_url("http://127.0.0.1/test"),
            Err(ValidationError::LocalAddress(_))
        ));
    }

    #[test]
    fn test_private_ip_192_returns_local_address_error() {
        assert!(matches!(
            validate_url("https://192.168.1.1/local"),
            Err(ValidationError::LocalAddress(_))
        ));
    }

    #[test]
    fn test_private_ip_10_returns_local_address_error() {
        assert!(matches!(
            validate_url("http://10.0.0.1/internal"),
            Err(ValidationError::LocalAddress(_))
        ));
    }

    // Fragment normalization tests
    #[test]
    fn test_normalize_url_removes_fragment() {
        assert_eq!(
            normalize_url("https://example.com/path#fragment").unwrap(),
            "https://example.com/path"
        );
    }

    #[test]
    fn test_normalize_url_removes_fragment_with_trailing_slash() {
        assert_eq!(
            normalize_url("https://example.com/path/#fragment").unwrap(),
            "https://example.com/path"
        );
    }

    // Trailing slash normalization tests
    #[test]
    fn test_normalize_url_removes_trailing_slash_from_path() {
        assert_eq!(
            normalize_url("https://example.com/path/").unwrap(),
            "https://example.com/path"
        );
    }

    #[test]
    fn test_normalize_url_preserves_root_slash() {
        assert_eq!(
            normalize_url("https://example.com/").unwrap(),
            "https://example.com/"
        );
    }

    #[test]
    fn test_normalize_url_adds_root_slash_when_missing() {
        assert_eq!(
            normalize_url("https://example.com").unwrap(),
            "https://example.com/"
        );
    }

    // Query parameter normalization tests
    #[test]
    fn test_normalize_url_sorts_query_params() {
        assert_eq!(
            normalize_url("https://example.com/search?c=3&a=1&b=2").unwrap(),
            "https://example.com/search?a=1&b=2&c=3"
        );
    }

    #[test]
    fn test_normalize_url_preserves_empty_query_params() {
        assert_eq!(
            normalize_url("https://example.com/search?a=1&b=&c=3").unwrap(),
            "https://example.com/search?a=1&b&c=3"
        );
    }

    // Case sensitivity normalization tests
    #[test]
    fn test_normalize_url_lowercases_domain_preserves_path_case() {
        assert_eq!(
            normalize_url("https://EXAMPLE.COM/Path/To/Resource").unwrap(),
            "https://example.com/Path/To/Resource"
        );
    }

    // Port normalization tests
    #[test]
    fn test_normalize_url_removes_default_https_port() {
        assert_eq!(
            normalize_url("https://example.com:443/path").unwrap(),
            "https://example.com/path"
        );
    }

    #[test]
    fn test_normalize_url_removes_default_http_port() {
        assert_eq!(
            normalize_url("http://example.com:80/path").unwrap(),
            "http://example.com/path"
        );
    }

    #[test]
    fn test_normalize_url_preserves_non_default_port() {
        assert_eq!(
            normalize_url("https://example.com:8080/path").unwrap(),
            "https://example.com:8080/path"
        );
    }

    // Percent encoding normalization tests
    #[test]
    fn test_normalize_url_preserves_percent_encoding_in_path() {
        assert_eq!(
            normalize_url("https://example.com/path%20with%20spaces").unwrap(),
            "https://example.com/path%20with%20spaces"
        );
    }

    #[test]
    fn test_normalize_url_decodes_query_parameters() {
        assert_eq!(
            normalize_url("https://example.com/search?q=hello%20world").unwrap(),
            "https://example.com/search?q=hello world"
        );
    }

    // Complex normalization test
    #[test]
    fn test_complex_url_normalization() {
        let complex_url = "HTTPS://EXAMPLE.COM:443/Path/To/Resource/?c=3&a=1&b=2#fragment";
        let expected = "https://example.com/Path/To/Resource?a=1&b=2&c=3";
        assert_eq!(normalize_url(complex_url).unwrap(), expected);
    }

    // ValidatedUrl type safety tests
    #[test]
    fn test_validated_url_type_safety() {
        let validated = validate_url("https://EXAMPLE.COM:443/Path/?c=3&a=1#fragment").unwrap();

        assert_eq!(validated.scheme, Scheme::Https);
        assert_eq!(validated.host, "example.com");
        assert_eq!(validated.port, None);
        assert_eq!(validated.path, "/Path");

        let expected_query = {
            let mut map = BTreeMap::new();
            map.insert("a".to_string(), "1".to_string());
            map.insert("c".to_string(), "3".to_string());
            Some(map)
        };
        assert_eq!(validated.query, expected_query);
        assert_eq!(validated.to_string(), "https://example.com/Path?a=1&c=3");
    }

    #[test]
    fn test_try_from_trait() {
        let url = Url::parse("https://example.com/test?b=2&a=1").unwrap();
        let validated = ValidatedUrl::try_from(url).unwrap();

        assert_eq!(validated.host, "example.com");
        assert_eq!(validated.path, "/test");

        let mut expected_params = BTreeMap::new();
        expected_params.insert("a".to_string(), "1".to_string());
        expected_params.insert("b".to_string(), "2".to_string());
        assert_eq!(validated.query, Some(expected_params));
        assert_eq!(validated.to_string(), "https://example.com/test?a=1&b=2");
    }
}
