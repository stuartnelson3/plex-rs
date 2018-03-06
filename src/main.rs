extern crate clap;
extern crate futures;
extern crate hyper;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate urlencoding;

use clap::{App, Arg};

use futures::future::Future;
use futures::Stream;

use hyper::header::ContentLength;
use hyper::server::{Http, Request, Response, Service};
use hyper::StatusCode;
use std::process::Command;

struct PlexDownloader<'a> {
    split: &'a str,
    src_server: &'a str,
}

#[derive(Deserialize, Default, Debug)]
struct SftpRequest<'a> {
    link: &'a str,
    destination: &'a str,
}

impl<'a> SftpRequest<'a> {
    fn path(&self, split: &str) -> String {
        let p = urlencoding::decode(&self.link).unwrap();
        p.split(split).last().unwrap().to_owned()
    }

    fn dst(&self) -> String {
        format!("/var/lib/plexmediaserver/{}", self.destination)
    }
}

impl<'a> Service for PlexDownloader<'a> {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Request) -> Self::Future {
        Box::new(req.body().concat2().map(|b| {
            let sftp_req: SftpRequest = if let Ok(j) = serde_json::from_slice(b.as_ref()) {
                j
            } else {
                let bad_request: &[u8] = b"bad request";
                return Response::new()
                    .with_status(StatusCode::BadRequest)
                    .with_header(ContentLength(bad_request.len() as u64))
                    .with_body(bad_request);
            };

            // let path = format!(
            //     "{}:\"{}\"",
            //     // "icarus.whatbox.ca:files/thing",
            //     // ".",
            //     self.src_server,
            //     sftp_req.path(self.split)
            // );
            let (res, status_code) = match Command::new("sftp")
                .args(&["-r", "TODO", &sftp_req.dst()])
                .spawn()
            {
                Ok(_) => (format!("downloading {}", sftp_req.link), StatusCode::Ok),
                Err(err) => (format!("error {}", err), StatusCode::InternalServerError),
            };

            // let res = format!("downloading {}", sftp_req.link);
            // let status_code = StatusCode::Ok;
            Response::new()
                .with_status(status_code)
                .with_header(ContentLength(res.len() as u64))
                .with_body(res)
        }))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_clean_path() {
        use SftpRequest;
        let link = "sftp://example.biz/mnt/mpathm/roy_rogers/files/Blade%20Runner%202049%201080p%20WEB-DL%20H264%20AC3-EVO";
        let destination = "/usr/what";
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

    let src_server = matches.value_of("source_server").unwrap();
    let split = matches.value_of("split").unwrap();
    let port = matches.value_of("port").unwrap();

    let addr = format!("127.0.0.1:{}", port).parse().unwrap();
    let server = Http::new()
        .bind(&addr, move || {
            Ok(PlexDownloader {
                split: "roy_rogers/",
                src_server: "user@minty",
            })
        })
        .unwrap();
    server.run().unwrap();
}
