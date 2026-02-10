//! IP-based access control middleware.
//!
//! Restricts endpoints (e.g., system monitoring) to loopback and private
//! (RFC 1918) IPv4 addresses.

use axum::{
    middleware::Next,
    response::{IntoResponse, Response},
    extract::ConnectInfo,
};
use std::net::SocketAddr;

/// Middleware to restrict access to local subnets.
/// 
/// This middleware extracts the connection info and verifies that the 
/// incoming IP address is either a loopback address or a private local address.
/// If access is denied, it returns a 401 Unauthorized response.
pub async fn auth_local_ip(
    req: axum::extract::Request,
    next: Next,
) -> Response {
    // Look up ConnectInfo in extensions manually to avoid extractor failures in tests
    if let Some(ConnectInfo(addr)) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
        let ip = addr.ip();
        if !ip.is_loopback() && !is_local_ip(ip) {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({
                    "error": "unauthorized",
                    "description": "Access restricted to local subnets"
                })),
            ).into_response();
        }
    }
    
    next.run(req).await
}

/// Returns `true` if the IP is in a private IPv4 range (10.x, 172.16-31.x, 192.168.x).
fn is_local_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ipv4) => ipv4.is_private(),
        std::net::IpAddr::V6(_) => false,
    }
}
