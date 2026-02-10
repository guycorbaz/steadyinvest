//! Request pipeline middleware.
//!
//! Middleware functions intercept incoming requests before they reach
//! controller handlers, enforcing cross-cutting concerns like auth and access control.
//!
//! - [`auth_ip`] — IP-based access allowlisting

pub mod auth_ip;
