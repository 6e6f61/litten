use serde::Deserialize;

use crate::http;

#[derive(Deserialize)]
pub struct Configuration {
    pub http: Option<http::Http>,
}
