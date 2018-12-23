extern crate clap;
extern crate env_logger;
#[macro_use]
extern crate log;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate lazy_static;
extern crate ctrlc;
extern crate net2;
extern crate rand;
extern crate regex;
extern crate threadpool;

use net2::TcpStreamExt;
use std::io::prelude::*;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Write;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time;

use clap::{crate_authors, crate_version, App, Arg, ArgMatches};
use failure::Error;
use net2::TcpBuilder;
use threadpool::ThreadPool;

mod proto;

use crate::proto::reply::*;
use crate::proto::state::*;
use crate::proto::*;

static READ_TIMEOUT_MS: u32 = 1000 * 30;
static LISTEN_BACKLOG: i32 = 256;
static IO_BUFFER_CAPACITY: usize = 1024 * 8;

struct Stat {
    accepted: AtomicUsize,
    rejected: AtomicUsize,
}

impl Stat {
    fn new() -> Self {
        Stat {
            accepted: AtomicUsize::new(0),
            rejected: AtomicUsize::new(0),
        }
    }
}

fn write_reply<W>(writer: &mut W, reply: &Reply) -> Result<(), Error>
where
    W: Write,
{
    writer.write_all(format!("{}", reply).as_bytes())?;
    writer.flush()?;

    Ok(())
}

fn handle_connection(stream: TcpStream, reject_ratio: f32, stat: Arc<Stat>) {
    let peer_addr = match stream.peer_addr() {
        Ok(peer_addr) => peer_addr,
        Err(err) => {
            error!("{}", err);
            return;
        }
    };

    if let Err(err) = stream.set_read_timeout_ms(Some(READ_TIMEOUT_MS)) {
        error!("{}", err);
        return;
    }

    let mut buffer = String::with_capacity(IO_BUFFER_CAPACITY);
    let mut reader = BufReader::new(&stream);
    let mut writer = BufWriter::new(&stream);
    let mut smtp = Protocol::new();

    smtp.set_reject_ratio(reject_ratio);

    {
        let reply = smtp.start();

        if let Err(err) = write_reply(&mut writer, &reply) {
            error!("{}: {}", peer_addr, err);
            return;
        }
    }

    loop {
        if let Ok(bytes_read) = reader.read_line(&mut buffer) {
            if bytes_read == 0 {
                break;
            }

            let mut status: u16;

            {
                let reply = match smtp.process_command(buffer.as_str()) {
                    Ok(reply) => reply,
                    Err(err) => {
                        error!("Error: {}, data: {}", err, buffer);
                        Reply::unknown_command()
                    }
                };

                if let Err(err) = write_reply(&mut writer, &reply) {
                    error!("{}: {}", peer_addr, err);
                    break;
                }

                status = reply.status;
            }

            if smtp.state == State::Rcpt && status > 500 {
                stat.rejected.fetch_add(1, Ordering::SeqCst);
            }

            if smtp.is_data() {
                match smtp.process_data(&mut reader) {
                    Ok(reply) => {
                        if let Err(err) = write_reply(&mut writer, &reply) {
                            error!("{}: {}", peer_addr, err);
                            break;
                        }
                        if reply.status >= 400 {
                            stat.rejected.fetch_add(1, Ordering::SeqCst);
                        } else {
                            stat.accepted.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    Err(err) => {
                        error!("{}: {}", peer_addr, err);
                        break;
                    }
                };
            }

            if smtp.is_done() {
                break;
            }

            buffer.clear();
        } else {
            info!("{} closed connection", peer_addr);
            break;
        }
    }
}

fn run(matches: &ArgMatches) -> Result<(), Error> {
    let workers = matches.value_of("workers").unwrap().parse::<usize>()?;
    if workers < 1 {
        bail!("number of workers can't be zero");
    }

    let addr = matches
        .value_of("address")
        .unwrap()
        .parse::<std::net::SocketAddr>()?;

    let reject_ratio = matches.value_of("ratio").unwrap().parse::<f32>()?;
    if reject_ratio < 0f32 || reject_ratio > 1f32 {
        bail!("reject ratio coefficient must be between 0 and 1");
    }

    let stat = Arc::new(Stat::new());

    let tcp = if addr.is_ipv4() {
        TcpBuilder::new_v4()?
    } else {
        TcpBuilder::new_v6()?
    };

    let listener = tcp
        .reuse_address(true)?
        .bind(&addr)?
        .listen(LISTEN_BACKLOG)?;

    let pool = ThreadPool::new(workers);

    // Setup Ctrl-C handling
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })?;

    // Accept and process connections in separate thread
    let s = stat.clone();
    thread::spawn(move || loop {
        let (stream, _addr) = match listener.accept() {
            Ok(result) => result,
            Err(err) => {
                error!("accept failed: {:?}", err);
                break;
            }
        };
        let s = s.clone();
        pool.execute(move || handle_connection(stream, reject_ratio, s));
    });

    // Monitor if Ctrl-C was pressed
    let sleep_interval = time::Duration::from_millis(10);
    loop {
        if !running.load(Ordering::SeqCst) {
            info!("Caught Ctrl-C, exiting");
            break;
        }
        thread::sleep(sleep_interval);
    }

    println!();
    println!("Accepted emails: {}", stat.accepted.load(Ordering::SeqCst));
    println!("Rejected emails: {}", stat.rejected.load(Ordering::SeqCst));
    println!();

    Ok(())
}

fn main() {
    env_logger::init();

    let args = App::new(env!("CARGO_PKG_NAME"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .author(crate_authors!())
        .version(crate_version!())
        .arg(
            Arg::with_name("address")
                .short("a")
                .long("address")
                .takes_value(true)
                .default_value("127.0.0.1:2500")
                .value_name("addr")
                .required(false)
                .help("Address to listen"),
        )
        .arg(
            Arg::with_name("workers")
                .short("w")
                .long("workers")
                .takes_value(true)
                .default_value("800")
                .value_name("num")
                .required(false)
                .help("Number of workers to launch"),
        )
        .arg(
            Arg::with_name("ratio")
                .short("r")
                .long("reject-ratio")
                .takes_value(true)
                .default_value("0")
                .value_name("num")
                .required(false)
                .help("Ratio of emails to reject. Must be between 0 and 1"),
        )
        .get_matches();

    if let Err(e) = run(&args) {
        error!("{}", e);
        std::process::exit(-1);
    }
}
