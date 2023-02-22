use anyhow::Result;
use log::{debug, error, info, warn};
// use regex::Regex;
use serde::Deserialize;
use serde_with::serde_as;
use std::collections::HashMap;
// use std::sync::Arc;
use std::net::SocketAddr;
use thiserror::Error;
use std::str::FromStr;
use warp::Filter;
use ractor::{Actor, ActorRef, ActorProcessingErr};

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

#[derive(Debug, Clone)]
pub enum Msg {
    Init,
}

#[async_trait::async_trait]
impl Actor for Http {
    type Msg = Msg;
    type Arguments = ();
    type State = ();

    async fn pre_start(&self, myself: ActorRef<Self>, _: Self::Arguments)
        -> Result<Self::State, ActorProcessingErr>
    {
        myself.cast(Self::Msg::Init)?;
        Ok(())
    }

    async fn handle(&self, myself: ActorRef<Self>, message: Self::Msg, _: &mut Self::State)
    -> Result<Self::State, ActorProcessingErr>
    {
        // Assume the message is Init because it's the only one
        for service in &self.services {
            self.make_service(service).await?;
        }

        Ok(())
    }
}

impl Http {
    async fn make_service(&self, service_cfg: &Service) -> Result<()> {
        let mut svc_name_route = warp::any();
        for service_name in service_cfg.service_names.unwrap_or_default() {
            svc_name_route.or(warp::host::exact(&service_name));
        }

        let mut svc_routes = warp::any();
        for location in service_cfg.locations {
            match location {
                (path, Method::Static(Static::Alias { alias })) =>
                    svc_routes.or(warp::path(path).map(|| format!("Would've opened file {}", alias))),
                (path, Method::Static(Static::Root { root })) =>
                    svc_routes.or(warp::path(path).map(|| format!("Would've served from root {}", root))),
                (path, Method::Proxy { to, add_headers }) =>
                    unimplemented!(),
            }
        }
        // let service_names = service_cfg.service_names
        //     .unwrap_or_default()
        //     .into_iter()
        //     .fold(warp::any(), |svc, service_name| svc.or(warp::host::exact(&service_name)));
        
        // let routes = service_cfg.locations
        //     .into_iter()
        //     .fold(warp::any(), |svc, location| svc.or(
        //         match location {
        //             (path, Method::Static(Static::Alias { alias })) =>
        //                 warp::path(path).map(|| format!("Would've opened file {}", alias)),
        //             (path, Method::Static(Static::Root { root })) =>
        //                 warp::path(path).map(|| format!("Would've served from root {}", root)),
        //             (path, Method::Proxy { to, add_headers }) =>
        //                 unimplemented!(),
        //         }
        //     ));

        for listen in service_cfg.listen {
            warp::serve(warp::get().and(svc_name_route).and(svc_routes))
                .run(SocketAddr::from_str(&listen)?);
        }
    
        Ok(())
    }
}