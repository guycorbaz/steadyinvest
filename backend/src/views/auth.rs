//! Authentication response DTOs.

use serde::{Deserialize, Serialize};

use crate::models::_entities::users;

/// JSON response returned after a successful login.
#[derive(Debug, Deserialize, Serialize)]
pub struct LoginResponse {
    /// JWT bearer token for subsequent authenticated requests.
    pub token: String,
    /// Public identifier for the user.
    pub pid: String,
    /// User's display name.
    pub name: String,
    /// Whether the user's email has been verified.
    pub is_verified: bool,
}

impl LoginResponse {
    /// Creates a login response from a user model and JWT token.
    #[must_use]
    pub fn new(user: &users::Model, token: &String) -> Self {
        Self {
            token: token.to_string(),
            pid: user.pid.to_string(),
            name: user.name.clone(),
            is_verified: user.email_verified_at.is_some(),
        }
    }
}

/// JSON response for the "current user" profile endpoint.
#[derive(Debug, Deserialize, Serialize)]
pub struct CurrentResponse {
    /// Public identifier for the user.
    pub pid: String,
    /// User's display name.
    pub name: String,
    /// User's email address.
    pub email: String,
}

impl CurrentResponse {
    /// Creates a current-user response from a user model.
    #[must_use]
    pub fn new(user: &users::Model) -> Self {
        Self {
            pid: user.pid.to_string(),
            name: user.name.clone(),
            email: user.email.clone(),
        }
    }
}
