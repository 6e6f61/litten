//! Functions to manage interpreting HTTP requests.
use std::collections::HashMap;
use std::convert::TryFrom;
use thiserror::Error;

/// Nginx' default.
const MAX_HEADER_SIZE: usize = 4096;

#[derive(Debug)]
pub struct Request {
    pub method: Method,
    pub path: String,
    pub headers: HashMap<String, String>,
}

#[derive(Debug)]
pub enum Method {
    Get,
    Head,
    Post,
    Put,
    Delete,
    Connect,
    Options,
    Trace,
    Patch,
}

#[derive(Debug, Error)]
pub enum RequestError {
    #[error("encountered header line without a colon")]
    NoColon,
    #[error("total header length is > {}", MAX_HEADER_SIZE)]
    TooLong,
    #[error("no request line")]
    NoRequestLine,
    #[error("request line HTTP version wasn't 1.0 or 1.1")]
    InvalidHttpVersion,
    #[error("request line lacked neccesary information")]
    BadRequestLine,
    #[error("unsupported or invalid HTTP method")]
    BadMethod,
    #[error("request doesn't contain \\r\\n\\r\\n terminator")]
    NoEnd,
}

impl TryFrom<&str> for Request {
    type Error = RequestError;

    // TODO: Error handling isn't too pretty here
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let (relevant, _) = s.split_once("\r\n\r\n").ok_or(Self::Error::NoEnd)?;
        if relevant.len() > MAX_HEADER_SIZE {
            return Err(Self::Error::TooLong);
        }

        let mut h = HashMap::new();
        let (req_line, headers) = relevant
            .split_once("\r\n")
            .ok_or(Self::Error::NoRequestLine)?;

        // Request line
        let mut req_line_parts = req_line.split(' ');
        let method = Method::try_from(req_line_parts.next().ok_or(Self::Error::BadRequestLine)?)?;
        let path =
            match urlencoding::decode(req_line_parts.next().ok_or(Self::Error::BadRequestLine)?) {
                Ok(v) => Ok(v),
                Err(_) => Err(Self::Error::BadRequestLine),
            }?;
        // Validate but then ignore the requester's HTTP version
        match req_line_parts.next().ok_or(Self::Error::BadRequestLine)? {
            "HTTP/1.0" | "HTTP/1.1" => {}
            _ => return Err(Self::Error::InvalidHttpVersion)?,
        }

        // Headers
        for header in headers.split("\r\n") {
            match header.split_once(": ") {
                Some((k, v)) => {
                    h.insert(k.to_string(), v.to_string());
                }
                None => return Err(Self::Error::NoColon),
            }
        }

        Ok(Self {
            headers: h,
            method: method,
            path: path.into_owned(),
        })
    }
}

impl TryFrom<&str> for Method {
    type Error = RequestError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        use Method::*;

        match s {
            "GET" => Ok(Get),
            "HEAD" => Ok(Head),
            "POST" => Ok(Post),
            "PUT" => Ok(Put),
            "DELETE" => Ok(Delete),
            "CONNECT" => Ok(Connect),
            "OPTIONS" => Ok(Options),
            "TRACE" => Ok(Trace),
            "PATCH" => Ok(Patch),
            _ => Err(Self::Error::BadMethod),
        }
    }
}
