use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fmt;
use thiserror::Error;
use url::Url;

#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Invalid URL format: {0}")]
    InvalidUrl(String),
    #[error("Unsupported URL scheme: {0}")]
    UnsupportedScheme(String),
    #[error("URL cannot be empty")]
    EmptyUrl,
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
        let host = url
            .host_str()
            .ok_or_else(|| ValidationError::InvalidUrl("URL must have a host".to_string()))?;

        // Host cannot be empty
        if host.is_empty() {
            return Err(ValidationError::InvalidUrl(
                "Host cannot be empty".to_string(),
            ));
        }

        // Normalize host to lowercase and check for local addresses
        let host = host.to_lowercase();
        if host == "localhost"
            || host.starts_with("127.")
            || host.starts_with("192.168.")
            || host.starts_with("10.")
        {
            return Err(ValidationError::InvalidUrl(
                "Local addresses not allowed".to_string(),
            ));
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

    let url = Url::parse(url_str).map_err(|_| ValidationError::InvalidUrl(url_str.to_string()))?;
    ValidatedUrl::try_from(url)
}

pub fn normalize_url(url_str: &str) -> Result<String, ValidationError> {
    let validated_url = validate_url(url_str)?;
    Ok(validated_url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_url_valid() {
        let valid_urls = vec![
            "https://example.com",
            "http://example.com",
            "https://example.com/path",
            "https://example.com/path?query=value",
            "https://subdomain.example.com",
            "https://example.com:8080/path",
        ];

        for url in valid_urls {
            assert!(validate_url(url).is_ok(), "URL should be valid: {url}");
        }
    }

    #[test]
    fn test_validate_url_invalid() {
        let invalid_urls = vec![
            ("", ValidationError::EmptyUrl),
            (
                "not-a-url",
                ValidationError::InvalidUrl("not-a-url".to_string()),
            ),
            (
                "ftp://example.com",
                ValidationError::UnsupportedScheme("ftp".to_string()),
            ),
            (
                "javascript:alert('xss')",
                ValidationError::UnsupportedScheme("javascript".to_string()),
            ),
        ];

        for (url, expected_error_type) in invalid_urls {
            let result = validate_url(url);
            assert!(result.is_err(), "URL should be invalid: {url}");

            match (&result.unwrap_err(), &expected_error_type) {
                (ValidationError::EmptyUrl, ValidationError::EmptyUrl) => {}
                (ValidationError::InvalidUrl(_), ValidationError::InvalidUrl(_)) => {}
                (ValidationError::UnsupportedScheme(_), ValidationError::UnsupportedScheme(_)) => {}
                _ => panic!("Unexpected error type for URL: {url}"),
            }
        }
    }

    #[test]
    fn test_normalize_url_fragments() {
        assert_eq!(
            normalize_url("https://example.com/path#fragment").unwrap(),
            "https://example.com/path"
        );
    }

    #[test]
    fn test_normalize_url_trailing_slash() {
        assert_eq!(
            normalize_url("https://example.com/path/").unwrap(),
            "https://example.com/path"
        );

        assert_eq!(
            normalize_url("https://example.com/").unwrap(),
            "https://example.com/"
        );

        assert_eq!(
            normalize_url("https://example.com").unwrap(),
            "https://example.com/"
        );
    }

    #[test]
    fn test_normalize_url_query_params() {
        assert_eq!(
            normalize_url("https://example.com/search?c=3&a=1&b=2").unwrap(),
            "https://example.com/search?a=1&b=2&c=3"
        );
    }

    #[test]
    fn test_normalize_url_case_sensitivity() {
        assert_eq!(
            normalize_url("https://EXAMPLE.COM/Path/To/Resource").unwrap(),
            "https://example.com/Path/To/Resource"
        );
    }

    #[test]
    fn test_normalize_url_default_ports() {
        assert_eq!(
            normalize_url("https://example.com:443/path").unwrap(),
            "https://example.com/path"
        );

        assert_eq!(
            normalize_url("http://example.com:80/path").unwrap(),
            "http://example.com/path"
        );

        assert_eq!(
            normalize_url("https://example.com:8080/path").unwrap(),
            "https://example.com:8080/path"
        );
    }

    #[test]
    fn test_validated_url_type_safety() {
        // Test that ValidatedUrl guarantees valid state
        let validated = validate_url("https://EXAMPLE.COM:443/Path/?c=3&a=1#fragment").unwrap();

        // The type guarantees these properties
        assert_eq!(validated.scheme, Scheme::Https);
        assert_eq!(validated.host, "example.com"); // normalized to lowercase
        assert_eq!(validated.port, None); // default port removed
        assert_eq!(validated.path, "/Path"); // trailing slash removed, fragment gone
        // Query should be stored as structured data, sorted
        let expected_query = {
            let mut map = BTreeMap::new();
            map.insert("a".to_string(), "1".to_string());
            map.insert("c".to_string(), "3".to_string());
            Some(map)
        };
        assert_eq!(validated.query, expected_query);

        // Converting to string gives normalized URL
        assert_eq!(validated.to_string(), "https://example.com/Path?a=1&c=3");
    }

    #[test]
    fn test_try_from_trait() {
        // Test the TryFrom<Url> trait implementation
        let url = Url::parse("https://example.com/test?b=2&a=1").unwrap();
        let validated = ValidatedUrl::try_from(url).unwrap();

        assert_eq!(validated.host, "example.com");
        assert_eq!(validated.path, "/test");

        // Query parameters should be stored as BTreeMap (automatically sorted)
        let mut expected_params = BTreeMap::new();
        expected_params.insert("a".to_string(), "1".to_string());
        expected_params.insert("b".to_string(), "2".to_string());
        assert_eq!(validated.query, Some(expected_params));

        // Display should show sorted parameters
        assert_eq!(validated.to_string(), "https://example.com/test?a=1&b=2");
    }
}
