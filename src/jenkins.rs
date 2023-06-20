#![warn(clippy::all, clippy::pedantic)]
use base64::{engine, Engine as _};
use hyper::{body::HttpBody as _, Body, Client, Method, Request, Response};
use hyper_tls::HttpsConnector;
use serde_json;
use tokio::io::{self, AsyncWriteExt as _};
use urlencoding::encode;

use crate::job::JobInfo;
use crate::node::NodesInfo;
use crate::{CopyItem, ShutdownState};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

async fn send_request(
    url: &hyper::Uri,
    user: &str,
    pswd: &str,
    method: Method,
) -> Result<Response<Body>> {
    let host = url.host().expect("uri has no host");
    let port = url.port_u16().unwrap_or(443);
    let stream = HttpsConnector::new();

    let client = Client::builder().build::<_, Body>(stream);

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

    let res = client.request(req).await?;

    Ok(res)
}

async fn get_json_data(url: &hyper::Uri, user: &str, pswd: &str) -> Result<io::BufWriter<Vec<u8>>> {
    let mut res = send_request(&url, user, pswd, Method::GET).await?;

    let buf = Vec::new();
    let mut writer = io::BufWriter::new(buf);

    while let Some(next) = res.data().await {
        let chunk = next?;
        writer.write_all(&chunk).await?;
    }
    writer.flush().await?;

    Ok(writer)
}

pub struct Tree<'t> {
    pub query: &'t str,
}

impl<'t> Tree<'t> {
    pub fn new(query: &'t str) -> Self {
        Self { query }
    }
}

impl<'t> std::fmt::Display for Tree<'t> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self)
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
            "{}://{}:{}@{}",
            parsed_url.scheme().unwrap(),
            user,
            pswd,
            parsed_url.host().unwrap()
        )
        .parse::<hyper::Uri>()
        .unwrap();
        Self { url, user, pswd }
    }

    pub async fn shutdown(self, state: ShutdownState) -> Result<Response<Body>> {
        match state {
            ShutdownState::On { reason } => {
                if !reason.is_empty() {
                    let url = format!("{}/quietDown?reason={}", self.url, encode(reason.as_str()))
                        .parse::<hyper::Uri>()?;
                    return send_request(&url, self.user, self.pswd, Method::POST).await;
                }

                let url = format!("{}/quietDown", self.url).parse::<hyper::Uri>()?;
                send_request(&url, self.user, self.pswd, Method::POST).await
            }
            ShutdownState::Off => {
                let url = format!("{}/cancelQuietDown", self.url).parse::<hyper::Uri>()?;
                send_request(&url, self.user, self.pswd, Method::POST).await
            }
        }
    }

    pub async fn restart(self, hard: bool) -> Result<Response<Body>> {
        if hard {
            println!("hard restart is activated");
            let url = format!("{}/restart", self.url).parse::<hyper::Uri>()?;
            return send_request(&url, self.user, self.pswd, Method::POST).await;
        }

        println!("safe restart is activated");
        let url = format!("{}/safeRestart", self.url).parse::<hyper::Uri>()?;
        send_request(&url, self.user, self.pswd, Method::POST).await
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
                send_request(&url, self.user, self.pswd, Method::POST).await
            }
            CopyItem::View { from, to } => {
                let url = format!(
                    "{}/createView?from={}&mode=copy&name={}",
                    self.url,
                    encode(from.as_str()),
                    encode(to.as_str())
                )
                .parse::<hyper::Uri>()?;
                send_request(&url, self.user, self.pswd, Method::POST).await
            }
        }
    }

    pub async fn node(&self) -> Result<NodesInfo> {
        let url = format!("{}/computer/api/json", self.url).parse::<hyper::Uri>()?;

        let json_data = get_json_data(&url, self.user, self.pswd).await?;
        let node: NodesInfo = serde_json::from_slice(json_data.into_inner().as_slice())?;

        Ok(node)
    }

    pub async fn job<'t>(&self, tree: Tree<'t>) -> Result<JobInfo> {
        let url = format!("{}/api/json?tree={}", self.url, tree.query).parse::<hyper::Uri>()?;

        let json_data = get_json_data(&url, self.user, self.pswd).await?;
        let job: JobInfo = serde_json::from_slice(json_data.into_inner().as_slice())?;

        Ok(job)
    }
}
