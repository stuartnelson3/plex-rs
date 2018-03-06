extern crate actix;
extern crate actix_web;
extern crate clap;
extern crate futures;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate urlencoding;

use actix_web::*;

use clap::{App, Arg};

use futures::future::Future;
use futures::Stream;

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

impl<'a> PlexDownloader<'a> {
    fn index_mjsonrust(&self, req: HttpRequest) -> Box<Future<Item = HttpResponse, Error = Error>> {
        req.concat2()
            .from_err()
            .and_then(|b| {
                let sftp_req: SftpRequest = if let Ok(j) = serde_json::from_slice(b.as_ref()) {
                    j
                } else {
                    let bad_request = Body::from_slice(b"{\"err\":\"failed to parse request\"}");
                    return Ok(HttpResponse::build(StatusCode::BAD_REQUEST)
                        .content_type("application/json")
                        .body(bad_request)
                        .unwrap());
                };

                // let path = format!("{}:\"{}\"", self.src_server, sftp_req.path(self.split));
                // let (res, status_code) = match Command::new("sftp")
                //     .args(&["-r", &path, &sftp_req.dst()])
                //     .spawn()
                // {
                //     Ok(_) => (format!("downloading {}", sftp_req.link), StatusCode::OK),
                //     Err(err) => (format!("error {}", err), StatusCode::INTERNAL_SERVER_ERROR),
                // };
                let (res, status_code) = ("asdf".to_owned(), StatusCode::OK);

                let body = Body::from_slice(&res.into_bytes());
                Ok(HttpResponse::build(status_code)
                    .content_type("application/json")
                    .body(body)
                    .unwrap())
            })
            .responder()
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

    let addr = format!("127.0.0.1:{}", port);

    let sys = actix::System::new("json-example");

    println!("Started http server: {}", addr);

    HttpServer::new(|| {
        Application::new().resource("/", |r| {
            // TODO: Find how to make this live long enough that it doesn't have to be moved.
            let downloader = PlexDownloader {
                split: "roy_rogers/",
                src_server: "user@minty",
            };
            r.method(Method::POST)
                .f(move |c| downloader.index_mjsonrust(c))
        })
    }).bind(&addr)
        .unwrap()
        .shutdown_timeout(1)
        .start();

    let _ = sys.run();
}
