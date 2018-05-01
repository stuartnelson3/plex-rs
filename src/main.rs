extern crate clap;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate urlencoding;

extern crate actix;
extern crate actix_web;
extern crate futures;
#[macro_use]
extern crate prometheus;
use prometheus::Encoder;

use actix_web::{http, middleware, server, HttpRequest, HttpResponse, Json, Result};

extern crate env_logger;
#[macro_use]
extern crate log;

use clap::{App, Arg};
use std::process::Command;

use std::thread;

struct PlexDownloader {
    split: String,
    src_server: String,
    active_downloads_gauge: prometheus::Gauge,
}

fn metrics(_req: HttpRequest<PlexDownloader>) -> HttpResponse {
    let encoder = prometheus::TextEncoder::new();
    let metrics = prometheus::gather();
    let mut buffer = vec![];
    encoder.encode(&metrics, &mut buffer).unwrap();
    HttpResponse::Ok().content_type("plain/text").body(buffer)
}

fn start_sftp(req: HttpRequest<PlexDownloader>, sftp_req: Json<SftpRequest>) -> Result<String> {
    let src_server = req.state().src_server.clone();
    let split = req.state().split.clone();
    let gauge = req.state().active_downloads_gauge.clone();
    let path = format!("{}:\"{}\"", src_server, sftp_req.path(&split));
    let child = Command::new("sftp")
        .args(&["-r", &path, &sftp_req.dst()])
        .spawn()?;

    gauge.inc();

    thread::spawn(move || {
        match child.wait_with_output() {
            Ok(output) => info!("success={}", output.status.success()),
            Err(err) => info!("error={}", err),
        };
        gauge.dec();
    });

    Ok("spawned".to_owned())
}

#[derive(Deserialize, Default, Debug)]
struct SftpRequest {
    destination: String,
    link: String,
}

impl SftpRequest {
    fn path(&self, split: &str) -> String {
        let p = urlencoding::decode(&self.link).unwrap();
        p.split(split).last().unwrap().to_owned()
    }

    fn dst(&self) -> String {
        format!("/var/lib/plexmediaserver/{}", self.destination)
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
    let sys = actix::System::new("plex_downloader");

    ::std::env::set_var("RUST_LOG", "plex_downloader=info");
    env_logger::init();
    let address = format!("0.0.0.0:{}", port);

    info!("Running plex_downloader at {}", address);

    let gauge = register_gauge!(
        "plex_downloader_active_downloads",
        "A gauge of current active sftp downloads."
    ).unwrap();

    server::new(move || {
        actix_web::App::with_state(PlexDownloader {
            split: split.clone(),
            src_server: src_server.clone(),
            active_downloads_gauge: gauge.clone(),
        }).middleware(middleware::Logger::default())
            .resource("/metrics", |r| r.method(http::Method::GET).with(metrics))
            .resource("/", |r| r.method(http::Method::POST).with2(start_sftp))
    }).bind(address)
        .unwrap()
        .start();

    let _ = sys.run();
}
