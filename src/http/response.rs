use std::fs;
use std::path::Path;
use std::io::ErrorKind;
use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;
use strum::Display;
use anyhow::Result;
use log::error;

const MIN_HTTP_VERSION: &'static str = "HTTP/1.0";

pub struct Response {
    code: ResponseCode,
    body: Option<Vec<u8>>,
}

#[derive(Debug, Display)]
pub enum ResponseCode {
    #[strum(serialize = "200 OK")]
    Ok,
    #[strum(serialize = "404 Not Found")]
    NotFound,
    #[strum(serialize = "500 Internal Server Error")]
    InternalServerError,
}

impl Response {
    pub fn path(path: String) -> Result<Self> {
        match Path::new(&path).metadata() {
            Ok(v) if v.is_file() => Response::file(path),
            Ok(v) if v.is_dir()  => Response::dir(path),
            Ok(_) => todo!(),
            Err(e) => {
                match e.kind() {
                    ErrorKind::NotFound => Ok(Response::not_found()),
                    _ => Err(e.into()),
                }
            },
        }
    }

    pub fn file(path: String) -> Result<Self> {
        match fs::read_to_string(&path) {
            Ok(f)  => Ok(Self { code: ResponseCode::Ok, body: Some(f.as_bytes().to_owned()) }),
            Err(e) => {
                error!("couldn't read file {}, but it should exist: {}", path, e);
                Ok(Self { code: ResponseCode::InternalServerError, body: None })
            },
        }
    }

    pub fn dir(path: String) -> Result<Self> {
        Self::file(path + "/index.html")
    }

    pub fn not_found() -> Self {
        Self { code: ResponseCode::NotFound, body: None }
    }

    pub async fn write(self, mut socket: TcpStream) -> Result<(), std::io::Error> {
        socket.write(MIN_HTTP_VERSION.as_bytes()).await?;
        socket.write(b" ").await?;
        socket.write(self.code.to_string().as_bytes()).await?;
        socket.write(b"\r\n\r\n").await?;
        match &self.body {
            None    => { },
            Some(v) => {
                socket.write(v).await?;
                socket.write(b"\r\n").await?;
            }
        }

        Ok(())
    }
}

// impl Response {
//     pub fn new() -> Self {
//         Response { code: ResponseCode::Ok, body: None, }
//     }

//     pub fn body(mut self, body: Vec<u8>) -> Self {
//         self.body = Some(body);
//         self
//     }

//     /// Modifies the response's body to be the contents
//     /// of `path`. If `path` can't be read succesfully,
//     /// the response's body is emptied and the response
//     /// code becomes 404 Not Found.
//     pub fn body_file(mut self, path: String) -> Self {
//         match fs::read_to_string(path) {
//             Ok(f)  => self.body = Some(f.as_bytes().to_owned()),
//             Err(e) => {
//                 warn!("couldn't respond with file: {}", e);
//                 self.code = ResponseCode::NotFound;
//                 self.body = None;
//             },
//         };

//         self
//     }

//     /// If `path` points to a file, this function is the same as
//     /// `body_file`, otherwise, `index.html` is appended, and that is
//     /// tried too.
//     pub fn body_path(self, path: String) -> Self {
//         self.body_file(path);
//         if self.body.is_none() {
//             self.body_file(path + "/index.html");
//         }

//         self
//     }

//     pub async fn write(self, mut socket: TcpStream) -> Result<(), std::io::Error> {
//         socket.write(self.code.to_string().as_bytes()).await?;
//         socket.write(b"\r\n\r\n").await?;
//         match &self.body {
//             None    => { },
//             Some(v) => {
//                 socket.write(v).await?;
//                 socket.write(b"\r\n").await?;
//             }
//         }

//         Ok(())
//     }
// }