use anyhow::Result;
use log::*;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use strum::{Display, EnumString};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use super::{Method, Request};

const MIN_HTTP_VERSION: &'static str = "HTTP/1.0";

pub struct Response {
    code: ResponseCode,
    body: Option<Vec<u8>>,
}

#[derive(Debug, Display, EnumString)]
pub enum ResponseCode {
    #[strum(serialize = "200 OK")]
    Ok,
    #[strum(serialize = "404 Not Found")]
    NotFound,
    #[strum(serialize = "500 Internal Server Error")]
    InternalServerError,
    #[strum(serialize = "400 Bad Request")]
    BadRequest,
}

impl Response {
    pub fn path(path: String) -> Result<Self> {
        match Path::new(&path).metadata() {
            Ok(v) if v.is_file() => Response::file(path),
            Ok(v) if v.is_dir() => Response::dir(path),
            Ok(_) => todo!(),
            Err(e) => match e.kind() {
                ErrorKind::NotFound => Ok(Self {
                    code: ResponseCode::NotFound,
                    body: Some("404 Not Found".into()),
                }),
                _ => Err(e.into()),
            },
        }
    }

    pub fn file(path: String) -> Result<Self> {
        match fs::read_to_string(&path) {
            Ok(f) => Ok(Self {
                code: ResponseCode::Ok,
                body: Some(f.as_bytes().to_owned()),
            }),
            Err(e) => {
                error!("couldn't read file {}, but it should exist: {}", path, e);
                Ok(Self {
                    code: ResponseCode::InternalServerError,
                    body: None,
                })
            }
        }
    }

    pub fn dir(path: String) -> Result<Self> {
        Self::file(path + "/index.html")
    }

    pub fn bad_request() -> Self {
        Self {
            code: ResponseCode::BadRequest,
            body: Some("400 Bad Request".into()),
        }
    }

    pub async fn proxy(request: Request, method: Method) -> Result<Self> {
        let Method::Proxy { to, add_headers } = method else { unreachable!() };
        let mut response_buf = Vec::new();
        let mut headers = request.headers.clone();
        if add_headers.is_some() {
            headers.extend(add_headers.unwrap());
        }
        headers.insert("Host".to_string(), to.clone());

        let mut stream = TcpStream::connect(to).await?;
        // TODO: is write buffered? or should this be one big write + format!?
        stream.write(MIN_HTTP_VERSION.as_bytes()).await?;
        stream.write(b" ").await?;
        stream.write(request.method.to_string().as_bytes()).await?;
        stream.write(b" ").await?;
        stream.write(request.path.as_bytes()).await?;
        stream.write(b"\r\n").await?;
        stream
            .write(
                headers
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect::<Vec<String>>()
                    .join("\r\n")
                    .as_bytes(),
            )
            .await?;
        stream.write(b"\r\n\r\n").await?;
        stream.readable().await?;
        stream.try_read_buf(&mut response_buf)?;

        let resp_code = ResponseCode::try_from(
            std::str::from_utf8(&response_buf)?
                .split_once(" ")
                .ok_or_else(|| anyhow::Error::msg("couldn't get response code of proxy request"))?
                .0,
        )?;
        Ok(Self {
            code: resp_code,
            body: Some(response_buf),
        })
    }

    pub async fn write(self, mut socket: TcpStream) -> Result<(), std::io::Error> {
        socket.write(MIN_HTTP_VERSION.as_bytes()).await?;
        socket.write(b" ").await?;
        socket.write(self.code.to_string().as_bytes()).await?;
        socket.write(b"\r\n\r\n").await?;
        match &self.body {
            None => {}
            Some(v) => {
                socket.write(v).await?;
                socket.write(b"\r\n").await?;
            }
        }

        Ok(())
    }
}
