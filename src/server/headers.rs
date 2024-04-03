use axum::extract::Request;
use axum::http::header::HeaderMap;
use axum::http::header::HeaderValue;
use axum::http::header::CACHE_CONTROL;
use axum::http::header::REFERRER_POLICY;
use axum::http::header::X_CONTENT_TYPE_OPTIONS;
use axum::middleware::Next;
use axum::response::Response;

const CROSS_ORIGIN_EMBEDDER_POLICY: &str = "cross-origin-embedder-policy";
const CROSS_ORIGIN_EMBEDDER_POLICY_DEFAULT: &str = "require-corp";
const CROSS_ORIGIN_OPENER_POLICY: &str = "cross-origin-opener-policy";
const CROSS_ORIGIN_RESOURCE_POLICY: &str = "cross-origin-resource-policy";
const HTTP_STRICT_TRANSPORT_SECURITY: &str = "http-strict-transport-security";
const HTTP_STRICT_TRANSPORT_SECURITY_DEFAULT: &str = "max-age=31536000 ; includeSubDomains";
const PERMISSIONS_POLICY: &str = "permissions-policy";
const PERMISSIONS_POLICY_DEFAULT: &str =
    "accelerometer=(),autoplay=(),camera=(),display-capture=(),document-domain=(),encrypted-media=(),fullscreen=(),\
     geolocation=(),gyroscope=(),magnetometer=(),microphone=(),midi=(),payment=(),picture-in-picture=(),\
     publickey-credentials-get=(),screen-wake-lock=(),sync-xhr=(self),usb=(),web-share=(),xr-spatial-tracking=()";
const REFERRER_POLICY_DEFAULT: &str = "no-referrer";
const SAME_ORIGIN: &str = "same-origin";
const X_CONTENT_TYPE_OPTIONS_DEFAULT: &str = "nosniff";
const X_PERMITTED_CROSS_DOMAIN_POLICIES: &str = "x-permitted-cross-domain-policies";
const X_PERMITTED_CROSS_DOMAIN_POLICIES_DEFAULT: &str = "none";


pub fn static_cache_control() -> HeaderMap {
    let mut h = HeaderMap::new();

    h.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );

    h
}

pub async fn add_security_headers(
    req: Request,
    next: Next,
) -> Response {
    let response = next.run(req).await;
    let (mut parts, body) = response.into_parts();
    
    let h = &mut parts.headers;
    
    h.reserve(12);
    h.insert(
        HTTP_STRICT_TRANSPORT_SECURITY,
        HeaderValue::from_static(HTTP_STRICT_TRANSPORT_SECURITY_DEFAULT),
    );
    h.insert(
        X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static(X_CONTENT_TYPE_OPTIONS_DEFAULT),
    );
    h.insert(
        X_PERMITTED_CROSS_DOMAIN_POLICIES,
        HeaderValue::from_static(X_PERMITTED_CROSS_DOMAIN_POLICIES_DEFAULT),
    );
    h.insert(REFERRER_POLICY, HeaderValue::from_static(REFERRER_POLICY_DEFAULT));
    h.insert(
        CROSS_ORIGIN_EMBEDDER_POLICY,
        HeaderValue::from_static(CROSS_ORIGIN_EMBEDDER_POLICY_DEFAULT),
    );
    h.insert(CROSS_ORIGIN_OPENER_POLICY, HeaderValue::from_static(SAME_ORIGIN));
    h.insert(CROSS_ORIGIN_RESOURCE_POLICY, HeaderValue::from_static(SAME_ORIGIN));
    h.insert(PERMISSIONS_POLICY, HeaderValue::from_static(PERMISSIONS_POLICY_DEFAULT));

    Response::from_parts(parts, body)
}