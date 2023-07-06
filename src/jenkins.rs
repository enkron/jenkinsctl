#![warn(clippy::all, clippy::pedantic)]
use base64::{engine, Engine as _};
use hyper::{body::HttpBody as _, Body, Client, Method, Request, Response, StatusCode};
use hyper_tls::HttpsConnector;
use serde::Deserialize;
use serde_json;
use std::str::FromStr;
use tokio::io::{self, AsyncWriteExt as _};
use urlencoding::encode;

use crate::{CopyItem, NodeState, ShutdownState};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct Tree {
    query: String,
}

impl Tree {
    pub fn new(query: String) -> Self {
        Self { query }
    }

    pub fn build_path(self, path: &str) -> Self {
        let mut query = self.query;
        let mut job_path = std::path::Path::new(&path)
            .iter()
            .map(|e| e.to_str().unwrap())
            .collect::<Vec<_>>();
        job_path.reverse();

        for component in job_path {
            query.insert_str(0, format!("job/{component}/").as_str());
        }

        Self { query }
    }
}

impl std::fmt::Display for Tree {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

enum Signal {
    Hup,
    Term,
    Kill,
}

impl FromStr for Signal {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Signal, Self::Err> {
        match s {
            "HUP" | "1" => Ok(Signal::Hup),
            "TERM" | "15" => Ok(Signal::Term),
            "KILL" | "9" => Ok(Signal::Kill),
            _ => Err(s.to_string()),
        }
    }
}

pub struct Jenkins<'x> {
    user: &'x str,
    pswd: &'x str,
    url: hyper::Uri,
}

impl<'x> Jenkins<'x> {
    pub fn new(user: &'x str, pswd: &'x str, jenkins_url: &'x str) -> Self {
        let parsed_url = jenkins_url.parse::<hyper::Uri>().unwrap();
        let url = format!(
            "{}://{}:{}@{}:{}",
            parsed_url.scheme().unwrap(),
            user,
            pswd,
            parsed_url.host().unwrap(),
            parsed_url.port_u16().unwrap_or(443)
        )
        .parse::<hyper::Uri>()
        .unwrap();

        Self { url, user, pswd }
    }

    async fn send_request(
        url: &hyper::Uri,
        user: &str,
        pswd: &str,
        method: Method,
    ) -> Result<Response<Body>> {
        let host = url.host().expect("uri has no host");
        let port = url.port_u16().unwrap();
        let scheme = url.scheme_str().unwrap();

        let req = Request::builder()
            .uri(url)
            .method(method)
            .header(hyper::header::HOST, format!("{host}:{port}"))
            .header(
                hyper::header::AUTHORIZATION,
                format!(
                    "Basic {}",
                    engine::general_purpose::URL_SAFE.encode(format!("{user}:{pswd}"))
                ),
            )
            .body(hyper::body::Body::empty())?;

        let res = if scheme == "http" {
            let client = Client::new();
            client.request(req).await
        } else {
            let stream = HttpsConnector::new();
            let client = Client::builder().build::<_, Body>(stream);
            client.request(req).await
        }?;

        Ok(res)
    }

    pub async fn get_json_data(&self, tree: &Tree) -> Result<io::BufWriter<Vec<u8>>> {
        let url = format!("{}/{}", self.url, tree.query).parse::<hyper::Uri>()?;
        let mut res = Self::send_request(&url, self.user, self.pswd, Method::GET).await?;

        if res.status() == StatusCode::NOT_FOUND {
            return Err(format!("{}", res.status().as_str()).into());
        }

        let buf = Vec::new();
        let mut writer = io::BufWriter::new(buf);

        while let Some(next) = res.data().await {
            let chunk = next?;
            writer.write_all(&chunk).await?;
        }
        writer.flush().await?;

        Ok(writer)
    }

    pub async fn get_console_log(&self, tree: &Tree) -> Option<(io::BufWriter<Vec<u8>>, usize)> {
        let url = format!("{}/{}", self.url, tree.query)
            .parse::<hyper::Uri>()
            .ok()?;
        let mut res = Self::send_request(&url, self.user, self.pswd, Method::GET)
            .await
            .ok()?;

        let offset = res
            .headers()
            .get("x-text-size")
            .unwrap()
            .to_str()
            .ok()?
            .parse::<usize>()
            .ok()?;

        let buf = Vec::new();
        let mut writer = io::BufWriter::new(buf);

        while let Some(next) = res.data().await {
            let chunk = next.ok()?;
            writer.write_all(&chunk).await.ok()?;
        }
        writer.flush().await.ok()?;

        if !res.headers().contains_key("x-more-data") {
            print!("{}", String::from_utf8_lossy(writer.get_ref()));
            return None;
        }

        Some((writer, offset))
    }

    pub async fn shutdown(self, state: ShutdownState) -> Result<Response<Body>> {
        match state {
            ShutdownState::On { reason } => {
                if !reason.is_empty() {
                    let url = format!("{}/quietDown?reason={}", self.url, encode(reason.as_str()))
                        .parse::<hyper::Uri>()?;
                    return Self::send_request(&url, self.user, self.pswd, Method::POST).await;
                }

                let url = format!("{}/quietDown", self.url).parse::<hyper::Uri>()?;
                Self::send_request(&url, self.user, self.pswd, Method::POST).await
            }
            ShutdownState::Off => {
                let url = format!("{}/cancelQuietDown", self.url).parse::<hyper::Uri>()?;
                Self::send_request(&url, self.user, self.pswd, Method::POST).await
            }
        }
    }

    pub async fn restart(self, hard: bool) -> Result<Response<Body>> {
        if hard {
            let url = format!("{}/restart", self.url).parse::<hyper::Uri>()?;
            return Self::send_request(&url, self.user, self.pswd, Method::POST).await;
        }

        let url = format!("{}/safeRestart", self.url).parse::<hyper::Uri>()?;
        Self::send_request(&url, self.user, self.pswd, Method::POST).await
    }

    pub async fn copy(self, service: CopyItem) -> Result<Response<Body>> {
        match service {
            CopyItem::Job { from, to } => {
                if to.contains('/') {
                    eprintln!("error: copy to a directory is not enabled {to}");
                    std::process::exit(1);
                }
                let url = format!(
                    "{}/createItem?from={}&mode=copy&name={}",
                    self.url,
                    encode(from.as_str()),
                    encode(to.as_str())
                )
                .parse::<hyper::Uri>()?;
                Self::send_request(&url, self.user, self.pswd, Method::POST).await
            }
            CopyItem::View { from, to } => {
                let url = format!(
                    "{}/createView?from={}&mode=copy&name={}",
                    self.url,
                    encode(from.as_str()),
                    encode(to.as_str())
                )
                .parse::<hyper::Uri>()?;
                Self::send_request(&url, self.user, self.pswd, Method::POST).await
            }
        }
    }

    pub async fn system<'de, I>(&self, json_data: &'de [u8]) -> Result<I>
    where
        I: Deserialize<'de>,
    {
        let info: I = serde_json::from_slice(json_data)?;

        Ok(info)
    }

    pub async fn build(&self, job_path: &str, params: String) -> Result<Response<Body>> {
        let path_components = std::path::Path::new(job_path)
            .components()
            .map(|e| format!("job/{}/", e.as_os_str().to_str().unwrap()))
            .collect::<String>();

        let url = match params.as_str() {
            "" => {
                format!("{}/{}build?delay=0sec", self.url, path_components).parse::<hyper::Uri>()?
            }
            "-" => format!(
                "{}/{}buildWithParameters?delay=0sec",
                self.url, path_components
            )
            .parse::<hyper::Uri>()?,
            _ => {
                let params = params
                    .split(',')
                    .map(|p| format!("&{}", p))
                    .collect::<String>();
                format!(
                    "{}/{}buildWithParameters?delay=0sec{}",
                    self.url, path_components, params
                )
                .parse::<hyper::Uri>()?
            }
        };
        Self::send_request(&url, self.user, self.pswd, Method::POST).await
    }

    pub async fn remove(self, job_path: &str) -> Result<Response<Body>> {
        let path_components = std::path::Path::new(job_path)
            .components()
            .map(|e| format!("job/{}/", e.as_os_str().to_str().unwrap()))
            .collect::<String>();

        let url = format!("{}/{}", self.url, path_components).parse::<hyper::Uri>()?;

        Self::send_request(&url, self.user, self.pswd, Method::DELETE).await
    }

    pub async fn kill(&self, tree: &Tree, signal: String) -> Result<Response<Body>> {
        if let Err(e) = Signal::from_str(signal.as_str()) {
            return Err(format!("invalid signal: {e}").into());
        }

        let url = match Signal::from_str(signal.as_str())? {
            Signal::Hup => format!("{}/{}/stop", self.url, tree.query),
            Signal::Term => format!("{}/{}/term", self.url, tree.query),
            Signal::Kill => format!("{}/{}/kill", self.url, tree.query),
        }
        .parse::<hyper::Uri>()?;

        Self::send_request(&url, self.user, self.pswd, Method::POST).await
    }

    pub async fn set(&self, tree: &Tree, state: NodeState) -> Result<Response<Body>> {
        let url = match state {
            NodeState::Disconnect { reason } => {
                if !reason.is_empty() {
                    format!(
                        "{}{}/doDisconnect?offlineMessage={}",
                        self.url,
                        tree.query,
                        encode(reason.as_str())
                    )
                } else {
                    format!("{}{}/doDisconnect", self.url, tree.query)
                }
            }
            NodeState::Connect => format!("{}{}/launchSlaveAgent", self.url, tree.query),
            NodeState::Offline { reason } => {
                if !reason.is_empty() {
                    format!(
                        "{}{}/toggleOffline?offlineMessage={}",
                        self.url,
                        tree.query,
                        encode(reason.as_str())
                    )
                } else {
                    format!("{}{}/toggleOffline", self.url, tree.query)
                }
            }
            NodeState::Online => format!("{}{}/toggleOffline", self.url, tree.query),
        }
        .parse::<hyper::Uri>()?;

        Self::send_request(&url, self.user, self.pswd, Method::POST).await
    }
}
