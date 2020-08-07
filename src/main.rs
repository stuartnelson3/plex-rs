extern crate clap;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate urlencoding;

extern crate actix_web;

use actix_web::web;
use actix_web::web::{Data, Json};
use actix_web::{middleware, App, HttpRequest, HttpServer, Result};

extern crate env_logger;
#[macro_use]
extern crate log;
use log::{debug, info};

extern crate ssh2;

use clap::Arg;

use std::thread;

use std::io;

use ssh2::{FileStat, Session};
use std::fs::{create_dir_all, File};
use std::io::prelude::*;
use std::io::{BufReader, BufWriter};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

extern crate crossbeam;
extern crate num_cpus;
use crossbeam::queue::SegQueue;

struct PlexDownloader {
    username: String,
    server: String,
    split: String,
    jobs_queue: SegQueue<SftpRequest>,
    max_threads: usize,
    active_threads: AtomicUsize,
}

async fn start_sftp(
    sftp_req: Json<SftpRequest>,
    state: Data<PlexDownloader>,
    _req: HttpRequest,
) -> Result<String> {
    state.jobs_queue.push(sftp_req.into_inner());

    let active_threads = state.active_threads.load(Ordering::Relaxed);

    if active_threads < state.max_threads {
        // Increment current active threads count so we don't spawn too many threads.
        state
            .active_threads
            .store(active_threads + 1, Ordering::Relaxed);

        thread::spawn(move || {
            debug!("spawned thread {}", active_threads + 1);
            // Connect to the local SSH server

            let tcp = TcpStream::connect(format!("{}:22", state.server)).unwrap();
            let mut sess = Session::new().unwrap();
            sess.set_tcp_stream(tcp);
            sess.handshake().unwrap();

            // Try to authenticate with the first identity in the agent.
            sess.userauth_agent(&state.username).unwrap();

            let sftp = sess.sftp().unwrap();

            while let Ok(req) = state.jobs_queue.pop() {
                let path = req.path(&state.split);
                let path = PathBuf::from(&path);
                // TODO: Handle the file not existing gracefully.
                // https://docs.rs/libc/0.2.72/libc/fn.sendfile.html
                // https://stackoverflow.com/questions/20235843/how-to-receive-a-file-using-sendfile
                let stat = sftp.stat(&path).unwrap();
                let dst = req.dst();
                match download(&sftp, (&path, stat), Path::new(&dst)) {
                    Err(err) => error!("download error {}", err),
                    Ok(_) => info!("downloaded {}", path.to_str().unwrap()),
                }
            }
            debug!("exiting thread {}", active_threads + 1);
            let active_threads = state.active_threads.load(Ordering::Relaxed);
            // No more jobs in the queue.
            // Decrement current active threads count and let the thread exit.
            state
                .active_threads
                .store(active_threads - 1, Ordering::Relaxed);
        });
    }

    Ok("spawned".to_owned())
}

fn download(
    sftp: &ssh2::Sftp,
    (src_path, stat): (&Path, FileStat),
    dst_path: &Path,
) -> Result<usize, Box<dyn std::error::Error>> {
    // destination write path on local disk
    let dst_path = dst_path.join(src_path.file_name().unwrap());
    let mut total = 0;

    if stat.is_dir() {
        // make sure the local dir we want to write into exists
        create_dir_all(&dst_path).unwrap();
        for (path, stat) in sftp.readdir(&src_path)?.into_iter() {
            total += download(sftp, (&path, stat), &dst_path)?;
            info!("downloaded {}", path.to_str().unwrap());
        }
    } else {
        // it's a file, just download it
        let mut src = BufReader::new(sftp.open(&src_path)?);

        // Destination file
        let dst = File::create(dst_path)?;

        // Allocate and reuse a 512kb buffer
        // It seems most read calls are 30-180kb
        let mut buffer = [0; 512 * 1024];
        let mut dst = BufWriter::new(dst);

        // TODO: can we use sendfile?
        // Loop over read() calls and write successively to dst
        while let Ok(n) = src.read(&mut buffer[..]) {
            if n == 0 {
                // EOF
                break;
            }

            total += dst.write(&buffer[..n])?;
        }
        dst.flush()?;
    }

    Ok(total)
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
        use super::SftpRequest;
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

    let (username, server) = {
        let input: Vec<&str> = matches.value_of("server").unwrap().split("@").collect();
        if input.len() == 2 {
            (input[0].to_owned(), input[1].to_owned())
        } else {
            // TODO: Grab the first user from ssh-agent
            // https://docs.rs/ssh2/0.8.2/ssh2/struct.Agent.html
            let username = env!("USER");
            if username == "" {
                panic!("no username! pass USER or set it on the front of the server.")
            }
            (username.to_owned(), input[1].to_owned())
        }
    };
    let split = matches.value_of("split").unwrap().to_owned();
    let port = matches.value_of("port").unwrap();

    env_logger::init();
    info!("username={} server={} split={}", username, server, split);

    let address = format!("0.0.0.0:{}", port);

    info!("running plex_downloader at {}", address);

    HttpServer::new(move || {
        let downloader = Data::new(PlexDownloader {
            username: username.clone(),
            split: split.clone(),
            server: server.clone(),
            jobs_queue: SegQueue::new(),
            max_threads: num_cpus::get(),
            active_threads: AtomicUsize::new(0),
        });
        App::new()
            .app_data(downloader)
            .wrap(middleware::Logger::default())
            .service(web::resource("/").route(web::post().to(start_sftp)))
    })
    .workers(1)
    .bind(address)?
    .run()
    .await
}
