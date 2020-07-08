extern crate clap;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate urlencoding;

extern crate actix_web;
#[macro_use]
extern crate prometheus;
use prometheus::Encoder;

use actix_web::web;
use actix_web::web::{Data, Json};
use actix_web::{middleware, App, HttpRequest, HttpResponse, HttpServer, Result};

extern crate env_logger;
#[macro_use]
extern crate log;

extern crate ssh2;

use clap::Arg;

use std::thread;

use std::io;

struct PlexDownloader {
    username: String,
    src_server: String,
    active_downloads_gauge: prometheus::Gauge,
}

fn metrics(_req: HttpRequest) -> HttpResponse {
    let encoder = prometheus::TextEncoder::new();
    let metrics = prometheus::gather();
    let mut buffer = vec![];
    encoder.encode(&metrics, &mut buffer).unwrap();
    HttpResponse::Ok().content_type("plain/text").body(buffer)
}

async fn start_sftp(
    sftp_req: Json<SftpRequest>,
    state: Data<PlexDownloader>,
    _req: HttpRequest,
) -> Result<String> {
    let gauge = state.active_downloads_gauge.clone();
    gauge.inc();

    thread::spawn(move || {
        use ssh2::Session;
        use std::fs::File;
        use std::io::prelude::*;
        use std::io::{BufReader, BufWriter};
        use std::net::TcpStream;
        use std::path::Path;

        // Connect to the local SSH server
        let tcp = TcpStream::connect(format!("{}:22", state.src_server)).unwrap();
        let mut sess = Session::new().unwrap();
        sess.set_tcp_stream(tcp);
        sess.handshake().unwrap();

        // Try to authenticate with the first identity in the agent.
        sess.userauth_agent(&state.username).unwrap();

        let sftp = sess.sftp().unwrap();
        let path = sftp_req.path();
        let path = Path::new(&path);
        let stat = sftp.stat(path).unwrap();
        if stat.is_dir() {
            // recursively dl files
        } else {
            // it's a file, just download it
            let mut src = BufReader::new(sftp.open(&path).unwrap());

            // Destination file
            let dst = File::create(format!(
                "{}/{}",
                sftp_req.dst(),
                path.file_name().unwrap().to_str().unwrap()
            ))
            .expect("Unable to create file");

            // Allocate and reuse a 512kb buffer
            // It seems most read calls are 30-180kb
            let mut buffer = [0; 512 * 1024];
            let mut dst = BufWriter::new(dst);

            // Loop over read() calls and write successively to dst
            while let Ok(n) = src.read(&mut buffer[..]) {
                if n == 0 {
                    // EOF
                    break;
                }

                match dst.write(&buffer[..n]) {
                    Ok(_) => (),
                    Err(err) => println!("write error {}", err),
                };
            }

            println!("written");
            dst.flush().unwrap();
        }

        gauge.dec();
    });

    Ok("spawned".to_owned())
}

#[derive(Deserialize, Default, Debug)]
struct SftpRequest {
    destination: String,
    path: String,
}

impl SftpRequest {
    fn path(&self) -> String {
        urlencoding::decode(&self.path).unwrap()
    }

    fn dst(&self) -> String {
        format!("/var/lib/plexmediaserver/{}", self.destination)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_clean_path() {
        use super::SftpRequest;
        let path = "files/Blade%20Runner%202049%201080p%20WEB-DL%20H264%20AC3-EVO".to_owned();
        let destination = "/usr/what".to_owned();
        let sftp_req = SftpRequest {
            path: path,
            destination: destination,
        };

        let expected = "files/Blade Runner 2049 1080p WEB-DL H264 AC3-EVO".to_owned();

        assert_eq!(expected, sftp_req.path());
    }
}

#[actix_rt::main]
async fn main() -> io::Result<()> {
    let matches = clap::App::new("Plex Downloader")
        .version("0.1.0")
        .author("stuart nelson <stuartnelson3@gmail.com>")
        .about("Queues up downloading files from remote server")
        .arg(
            Arg::with_name("server")
                .short("s")
                .long("server")
                .value_name("host")
                .help("Connection info for server")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("user")
                .help("username for ssh connection")
                .short("u")
                .long("username")
                .value_name("username")
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

    let src_server = matches.value_of("server").unwrap().to_owned();
    let username = matches.value_of("user").unwrap().to_owned();
    let port = matches.value_of("port").unwrap();

    ::std::env::set_var("RUST_LOG", "plex_downloader=info");
    env_logger::init();
    let address = format!("0.0.0.0:{}", port);

    info!("Running plex_downloader at {}", address);

    let gauge = register_gauge!(
        "plex_downloader_active_downloads",
        "A gauge of current active sftp downloads."
    )
    .unwrap();

    HttpServer::new(move || {
        let downloader = Data::new(PlexDownloader {
            username: username.clone(),
            src_server: src_server.clone(),
            active_downloads_gauge: gauge.clone(),
        });
        App::new()
            .app_data(downloader)
            .wrap(middleware::Logger::default())
            .service(web::resource("/metrics").route(web::get().to(metrics)))
            .service(web::resource("/").route(web::post().to(start_sftp)))
    })
    .bind(address)?
    .run()
    .await
}
