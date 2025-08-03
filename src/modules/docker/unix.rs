// src/modules/docker/unix.rs

use std::error::Error;
use http::uri::Authority;
use hyper::{body::Bytes, client::conn::http1, Request};
use tokio::net::UnixStream;
use http_body_util::{BodyExt, Empty};
use hyper_util::rt::TokioIo;

const DOCKER_SOCKET_PATH: &str = "/var/run/docker.sock";

pub async fn request(path: &str) -> Result<Bytes, Box<dyn Error + Send + Sync>> {
    let stream = UnixStream::connect(DOCKER_SOCKET_PATH).await?;
    let io = TokioIo::new(stream);

    let (mut sender, conn) = http1::handshake(io).await?;
    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            eprintln!("Connection failed: {:?}", err);
        }
    });

    let authority: Authority = "localhost".parse()?;
    let req = Request::builder()
        .uri(path)
        .header(hyper::header::HOST, authority.as_ref())
        .body(Empty::<Bytes>::new())?;

    let res = sender.send_request(req).await?;
    let body = res.collect().await?.to_bytes();

    Ok(body)
}