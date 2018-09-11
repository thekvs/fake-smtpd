extern crate clap;
extern crate env_logger;
#[macro_use]
extern crate log;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate lazy_static;
extern crate ctrlc;
extern crate rand;
extern crate regex;
extern crate threadpool;

use std::io::prelude::*;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time;

use clap::{App, Arg, ArgMatches};
use failure::Error;
use threadpool::ThreadPool;

mod proto;

use proto::reply::*;
use proto::*;

fn write_reply<W>(writer: &mut W, reply: &Reply) -> Result<(), Error>
where
    W: Write,
{
    writer.write_all(format!("{}", reply).as_bytes())?;
    writer.flush()?;

    Ok(())
}

fn handle_connection(stream: TcpStream, reject_ratio: f32) {
    let peer_addr = match stream.peer_addr() {
        Ok(peer_addr) => peer_addr,
        Err(err) => {
            error!("{}", err);
            return;
        }
    };

    let mut buffer = String::with_capacity(1024 * 8);
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

            {
                let reply = smtp.process_command(buffer.as_str());
                if let Err(err) = write_reply(&mut writer, &reply) {
                    error!("{}: {}", peer_addr, err);
                    break;
                }
                // if reply.status >= 400 {
                //     break;
                // }
            }

            if smtp.is_data() {
                match smtp.process_data(&mut reader) {
                    Ok(reply) => {
                        if let Err(err) = write_reply(&mut writer, &reply) {
                            error!("{}: {}", peer_addr, err);
                            break;
                        }
                        // if reply.status >= 400 {
                        //     break;
                        // }
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

    let socket = TcpListener::bind(&addr)?;
    let pool = ThreadPool::new(workers);

    // Setup Ctrl-C handling
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    // Accept and process connections in separate thread
    thread::spawn(move || loop {
        let (stream, _addr) = match socket.accept() {
            Ok(result) => result,
            Err(err) => {
                error!("accept failed: {:?}", err);
                break;
            }
        };
        pool.execute(move || handle_connection(stream, reject_ratio));
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

    Ok(())
}

fn main() {
    env_logger::init();

    let args = App::new("Fake SMTP server")
        .author("Konstantin Sorokin <kvs@sigterm.ru>")
        .arg(
            Arg::with_name("address")
                .short("a")
                .long("address")
                .takes_value(true)
                .default_value("127.0.0.1:2500")
                .required(false)
                .help("Address to listen"),
        )
        .arg(
            Arg::with_name("workers")
                .short("w")
                .long("workers")
                .takes_value(true)
                .default_value("800")
                .required(false)
                .help("Number of workers to launch"),
        )
        .arg(
            Arg::with_name("ratio")
                .short("r")
                .long("reject-ratio")
                .takes_value(true)
                .default_value("0")
                .required(false)
                .help("Ratio of emails to reject"),
        )
        .get_matches();

    if let Err(e) = run(&args) {
        error!("{}", e);
        std::process::exit(-1);
    }
}
