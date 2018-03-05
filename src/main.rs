extern crate futures;
extern crate hyper;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use futures::future::Future;
use futures::Stream;

use hyper::header::ContentLength;
use hyper::server::{Http, Request, Response, Service};
use hyper::StatusCode;

struct PlexDownloader;

#[derive(Deserialize, Default, Debug)]
struct SftpRequest {
    link: String,
    destination: String,
}

const PHRASE: &'static str = "Hello, World!";

impl Service for PlexDownloader {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Request) -> Self::Future {
        Box::new(req.body().concat2().map(|b| {
            let sftp_req: SftpRequest = if let Ok(j) = serde_json::from_slice(b.as_ref()) {
                j
            } else {
                return Response::new()
                    .with_header(ContentLength(PHRASE.len() as u64))
                    .with_body(PHRASE);
            };

            println!("executing {} {}", sftp_req.link, sftp_req.destination);

            Response::new()
                .with_header(ContentLength(PHRASE.len() as u64))
                .with_body(PHRASE)
        }))
    }
}

fn main() {
    let addr = "127.0.0.1:3000".parse().unwrap();
    let server = Http::new().bind(&addr, || Ok(PlexDownloader)).unwrap();
    server.run().unwrap();
}
