use lectara_service::validation::{normalize_url, validate_url};

#[test]
fn test_valid_urls() {
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
fn test_invalid_urls() {
    let invalid_urls = vec![
        "",
        "not-a-url",
        "ftp://example.com",
        "javascript:alert('xss')",
        "data:text/html,<script>alert('xss')</script>",
        "https://",
        "http://",
        "://example.com",
        "https://localhost/path",
        "http://127.0.0.1/test",
        "https://192.168.1.1/local",
        "http://10.0.0.1/internal",
    ];

    for url in invalid_urls {
        assert!(validate_url(url).is_err(), "URL should be invalid: {url}");
    }
}

#[test]
fn test_url_normalization_fragments() {
    // Fragments should be removed
    assert_eq!(
        normalize_url("https://example.com/path#fragment").unwrap(),
        "https://example.com/path"
    );

    assert_eq!(
        normalize_url("https://example.com/path/#fragment").unwrap(),
        "https://example.com/path"
    );
}

#[test]
fn test_url_normalization_trailing_slash() {
    // Trailing slashes should be removed from paths (but not root)
    assert_eq!(
        normalize_url("https://example.com/path/").unwrap(),
        "https://example.com/path"
    );

    // Root slash should be preserved
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
fn test_url_normalization_query_params() {
    // Query parameters should be sorted
    assert_eq!(
        normalize_url("https://example.com/search?c=3&a=1&b=2").unwrap(),
        "https://example.com/search?a=1&b=2&c=3"
    );

    // Empty query parameters should be preserved if present
    assert_eq!(
        normalize_url("https://example.com/search?a=1&b=&c=3").unwrap(),
        "https://example.com/search?a=1&b&c=3"
    );
}

#[test]
fn test_url_normalization_case_sensitivity() {
    // Domain should be lowercase, path should preserve case
    assert_eq!(
        normalize_url("https://EXAMPLE.COM/Path/To/Resource").unwrap(),
        "https://example.com/Path/To/Resource"
    );
}

#[test]
fn test_url_normalization_port() {
    // Default ports should be removed
    assert_eq!(
        normalize_url("https://example.com:443/path").unwrap(),
        "https://example.com/path"
    );

    assert_eq!(
        normalize_url("http://example.com:80/path").unwrap(),
        "http://example.com/path"
    );

    // Non-default ports should be preserved
    assert_eq!(
        normalize_url("https://example.com:8080/path").unwrap(),
        "https://example.com:8080/path"
    );
}

#[test]
fn test_url_normalization_percent_encoding() {
    // Common percent-encoded characters should be normalized
    assert_eq!(
        normalize_url("https://example.com/path%20with%20spaces").unwrap(),
        "https://example.com/path%20with%20spaces"
    );

    // Percent-encoded query parameters get normalized (decoded then re-encoded as needed)
    assert_eq!(
        normalize_url("https://example.com/search?q=hello%20world").unwrap(),
        "https://example.com/search?q=hello world"
    );
}

#[test]
fn test_complex_url_normalization() {
    // Test multiple normalization rules applied together
    let complex_url = "HTTPS://EXAMPLE.COM:443/Path/To/Resource/?c=3&a=1&b=2#fragment";
    let expected = "https://example.com/Path/To/Resource?a=1&b=2&c=3";

    assert_eq!(normalize_url(complex_url).unwrap(), expected);
}
