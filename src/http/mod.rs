use std::collections::HashMap;
use serde::Deserialize;
use thiserror::Error;
use anyhow::Result;
use std::sync::Arc;
use tokio::task::JoinSet;
use tokio::net::{TcpListener, TcpStream};
use log::{error, info, warn};
use regex::Regex;
use serde_with::serde_as;

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
    #[serde(rename = "serviceNames")]
    pub service_names: Vec<String>,
    #[serde_as(as = "HashMap<_, _>")]
    #[serde(rename = "location")]
    pub locations: Vec<(String, Method)>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "method")]
pub enum Method {
    #[serde(rename = "static")]
    Static { root: Option<String>, alias: Option<String> },
    #[serde(rename = "proxy")]
    Proxy  {
        to: String,
        #[serde(rename = "addHeaders")]
        add_headers: HashMap<String, String>
    },
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
                async move {
                    t_self.listen_to_address(i).await
                }
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
    
        info!("listening to http://{} for names {:?}", address, self.service_names);
        loop {
            let (socket, remote_addr) = listener.accept().await?;
            info!("accepted request from {:?} -> {}", remote_addr.ip(), address);
            socket.readable().await?;
            let mut raw_req = [0; 4096];
            socket.try_read(&mut raw_req)?;

            let request = match Request::try_from(std::str::from_utf8(&raw_req)?) {
                Ok(v) => v,
                Err(e) => {
                    warn!("dropping request: {}", e);
                    continue;
                }
            };

            match request.method {
                request::Method::Get => {
                    self.get(socket, request.path).await.unwrap_or_else(|e| {
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

    pub async fn get(&self, socket: TcpStream, path: String) -> Result<()> {
        let Some((re, method)) = self.locations
            .iter()
            // TODO: Not this
            // Try to have Serde deserialize the field as a Regex in the first place
            .find(|(regex, _)| { Regex::new(regex).unwrap().is_match(&path) })
        else { error!("no match"); todo!() };

        info!("path {} matched {}", path, re);
        
        match method {
            Method::Static { alias: Some(alias), .. } =>
                Response::file(alias.to_owned())?.write(socket).await?,
            Method::Static { root: Some(root), .. } =>
                Response::path(root.to_owned() + &path)?.write(socket).await?,
            x => {
                info!("method {:?} is unsupported, failing", x);
                return Ok(());
            }
        }

        Ok(())
    }
}