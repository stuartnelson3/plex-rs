extern crate clap;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate urlencoding;

use clap::{App, Arg};
use std::process::Command;

extern crate futures;
extern crate hyper;

extern crate env_logger;
#[macro_use]
extern crate log;

use hyper::{Chunk, StatusCode};
use hyper::Method::Post;
use hyper::server::{Request, Response, Service};

use futures::future::{Future, FutureResult};
use futures::{Sink, Stream};
use futures::sync::mpsc::{channel, Sender};

use std::io;
use std::io::BufRead;
use std::thread;

struct PlexDownloader {
    worker: Sender<Vec<String>>,
    split: String,
    src_server: String,
}

impl Service for PlexDownloader {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, request: Request) -> Self::Future {
        match (request.method(), request.path()) {
            (&Post, "/") => {
                let split = self.split.clone();
                let src_server = self.src_server.clone();
                let future = request
                    .body()
                    .concat2()
                    .and_then(parse_body)
                    .and_then(|sftp_request| start_sftp(src_server, split, sftp_request))
                    .then(make_post_response);
                Box::new(future)
            }
            _ => Box::new(futures::future::ok(
                Response::new().with_status(StatusCode::NotFound),
            )),
        }
    }
}

fn parse_body(body: Chunk) -> FutureResult<SftpRequest, hyper::Error> {
    match serde_json::from_slice(body.as_ref()) {
        Ok(j) => {
            info!("parsed request {:?}", j);
            futures::future::ok(j)
        }
        Err(err) => {
            info!("parsing failed err={}", err);
            futures::future::err(hyper::Error::from(io::Error::new(
                io::ErrorKind::InvalidInput,
                "failed to parse body",
            )))
        }
    }
}

fn start_sftp(
    src_server: String,
    split: String,
    sftp_request: SftpRequest,
) -> FutureResult<SftpRequest, hyper::Error> {
    let path = format!("{}:\"{}\"", src_server, sftp_request.path(&split));
    let result = Command::new("sftp")
        .args(&["-r", &path, &sftp_request.dst()])
        .output();
    match result {
        Ok(output) => {
            info!("success: {:?}", output);
            futures::future::ok(sftp_request)
        }
        Err(err) => {
            info!("failure: {:?}", err);
            futures::future::err(hyper::Error::from(err))
        }
    }
    // self.worker.send(command);
}

fn make_post_response(
    result: Result<SftpRequest, hyper::Error>,
) -> FutureResult<hyper::Response, hyper::Error> {
    futures::future::ok(Response::new().with_status(StatusCode::NotFound))
}

fn spawn_worker() -> Sender<Vec<String>> {
    let (tx, rx) = channel(1);
    thread::spawn(move || {
        rx.for_each(|vec| {
            info!("{:?}", vec);
            // command.output();
            Ok(())
        }).map(|()| {
                info!("The worker has stopped!");
            })
            .wait()
            .unwrap();
    });
    tx
}

#[derive(Deserialize, Default, Debug)]
struct SftpRequest {
    link: String,
    destination: String,
}

impl SftpRequest {
    fn path(&self, split: &str) -> String {
        let p = urlencoding::decode(&self.link).unwrap();
        p.split(split).last().unwrap().to_owned()
    }

    fn dst(&self) -> String {
        format!("/Users/stuartn/workspace/plex-rs/{}", self.destination)
        // format!("/var/lib/plexmediaserver/{}", self.destination)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_clean_path() {
        use SftpRequest;
        let link = "sftp://example.biz/mnt/mpathm/roy_rogers/files/Blade%20Runner%202049%201080p%20WEB-DL%20H264%20AC3-EVO".to_owned();
        let destination = "/usr/what".to_owned();
        let split = "roy_rogers/".to_owned();
        let sftp_req = SftpRequest {
            link: link,
            destination: destination,
        };

        let expected = "files/Blade Runner 2049 1080p WEB-DL H264 AC3-EVO".to_owned();

        assert_eq!(expected, sftp_req.path(&split));
    }
}

fn main() {
    let matches = App::new("Plex Downloader")
        .version("0.1.0")
        .author("stuart nelson <stuartnelson3@gmail.com>")
        .about("Queues up downloading files from remote server")
        .arg(
            Arg::with_name("source_server")
                .short("s")
                .long("src.server")
                .value_name("[user@]host")
                .help("Connection info for server")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("split")
                .help("split incoming link on this value")
                .short("c")
                .long("split")
                .value_name("SPLIT")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("port")
                .help("port to listen on")
                .short("p")
                .long("port")
                .value_name("PORT")
                .default_value("3000")
                .takes_value(true),
        )
        .get_matches();

    let src_server = matches.value_of("source_server").unwrap().to_owned();
    let split = matches.value_of("split").unwrap().to_owned();
    let port = matches.value_of("port").unwrap();

    env_logger::init();
    let worker = spawn_worker();
    let address = format!("127.0.0.1:{}", port).parse().unwrap();
    let server = hyper::server::Http::new()
        .bind(&address, move || {
            Ok(PlexDownloader {
                worker: worker.clone(),
                split: split.clone(),
                src_server: src_server.clone(),
            })
        })
        .unwrap();
    info!("Running microservice at {}", address);
    server.run().unwrap();
}
