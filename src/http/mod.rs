use anyhow::Result;
use log::{debug, error, info, warn};
use regex::Regex;
use serde::Deserialize;
use serde_with::serde_as;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;

mod request;
mod response;
use request::Request;
use response::Response;

#[derive(Deserialize, Debug, Clone)]
pub struct Http {
    #[serde(rename = "service")]
    pub services: Vec<Service>,
}

#[serde_as]
#[derive(Deserialize, Debug, Clone)]
pub struct Service {
    pub listen: Vec<String>,
    pub service_names: Option<Vec<String>>,
    #[serde_as(as = "HashMap<_, _>")]
    #[serde(rename = "location")]
    pub locations: Vec<(String, Method)>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "method")]
pub enum Method {
    #[serde(rename = "static")]
    Static(Static),
    #[serde(rename = "proxy")]
    Proxy {
        to: String,
        add_headers: Option<HashMap<String, String>>,
    },
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Static {
    // #[serde(rename = "root")]
    Root { root: String },
    // #[serde(rename = "alias")]
    Alias { alias: String },
}

#[derive(Debug, Error)]
pub enum HttpError {
    #[error("reached end of serve(); this shouldn't happen")]
    ServeReturn,
}

impl Http {
    pub async fn serve(self) -> Result<()> {
        let mut services = JoinSet::new();

        for s in self.services {
            services.spawn(async move { Arc::new(s).service_all().await });
        }

        while let Some(svc) = services.join_next().await {
            svc??;
        }

        Err(HttpError::ServeReturn.into())
    }
}

impl Service {
    pub async fn service_all(self: Arc<Self>) -> Result<()> {
        let mut addresses = JoinSet::new();

        // TODO: I feel this is a hack
        for i in 0..self.listen.len() {
            addresses.spawn({
                // TODO: I feel this is a hack
                let t_self = Arc::clone(&self);
                async move { t_self.listen_to_address(i).await }
            });
        }

        while let Some(addr) = addresses.join_next().await {
            addr??;
        }

        Err(HttpError::ServeReturn.into())
    }

    pub async fn listen_to_address(&self, addr_idx: usize) -> Result<()> {
        let address = &self.listen[addr_idx];
        let listener = TcpListener::bind(&address).await?;

        // TODO: Not print None when no service names are given
        info!(
            "listening to http://{} for names {:?}",
            address, self.service_names
        );
        loop {
            let (socket, remote_addr) = listener.accept().await?;
            let mut raw_req = [0; 4096];

            info!(
                "accepted request from {:?} -> {}",
                remote_addr.ip(),
                address
            );
            socket.readable().await?;
            socket.try_read(&mut raw_req)?;

            let request = match Request::try_from(std::str::from_utf8(&raw_req)?) {
                Ok(v) => v,
                Err(_) => {
                    Response::bad_request().write(socket).await?;
                    continue;
                }
            };

            match request.method {
                request::Method::Get => {
                    self.get(socket, request).await.unwrap_or_else(|e| {
                        warn!("request failed: {}", e);
                    });
                }
                _ => warn!(
                    "dropping request with unimplemented method {:?}",
                    request.method
                ),
            };
        }
    }

    pub async fn get(&self, socket: TcpStream, request: Request) -> Result<()> {
        let Some((re, method)) = self.locations
            .iter()
            // TODO: Not this
            // Try to have Serde deserialize the field as a Regex in the first place
            .find(|(regex, _)| { Regex::new(regex).unwrap().is_match(&request.path) })
        else { unreachable!() };

        debug!("path {} matched {}", request.path, re);

        match method {
            Method::Static(Static::Alias { alias }) => {
                Response::file(alias.to_owned())?.write(socket).await?
            }
            Method::Static(Static::Root { root }) => {
                Response::path(root.to_owned() + &request.path)?
                    .write(socket)
                    .await?
            }
            Method::Proxy { .. } => {
                Response::proxy(request, method.clone())
                    .await?
                    .write(socket)
                    .await?
            }
        }

        Ok(())
    }
}
