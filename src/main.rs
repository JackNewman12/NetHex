extern crate env_logger;
extern crate hex;
extern crate hexplay;
extern crate log;
extern crate pnet;
extern crate regex;

mod filter;
use filter::RxFilter;

use hex::FromHex;
use io::stdin;
use log::{debug, error, info};
use pnet::datalink::Channel::Ethernet;
use pnet::datalink::{self, NetworkInterface};
use structopt::StructOpt;

use indicatif::ProgressStyle;

use std::time::{Duration, Instant};
use std::{io, path::PathBuf};
use std::{io::BufRead, thread};

fn print_interfaces() {
    println!("Detected Network Interfaces:");
    let list_of_interfaces = datalink::interfaces();
    for interface in list_of_interfaces {
        println!("{}", interface.name);
        for ipaddr in interface.ips {
            println!("  IP: {}", ipaddr);
        }
    }
}

#[derive(StructOpt, Debug)]
#[structopt(
    name = "NetHex",
    setting = structopt::clap::AppSettings::ColoredHelp
)]
/// A small utility for reading / writing directly to a network interface
struct Opt {
    /// Number of packet to receive before exiting
    #[structopt(short = "c", long = "count", default_value = "-1")]
    rx_count: i64,

    /// Time to receive for before exiting
    #[structopt(short = "t", long = "timeout")]
    rx_timeout: Option<u64>,

    /// Only print Rx packets that match this regex filter
    #[structopt(short = "f", long = "filter")]
    rx_filter: Option<String>,

    /// Only print Rx packets that do NOT match this regex filter
    #[structopt(short = "b", long = "blacklist")]
    rx_blacklist_filter: Option<String>,

    /// Number of packet to transmit
    #[structopt(short = "s", long = "send", default_value = "1")]
    tx_send: u64,

    /// Rate to transmit (Packets Per Second)
    #[structopt(short = "r", long = "rate")]
    tx_rate: Option<f64>,

    /// Raw Hex Printing
    #[structopt(short = "R", long = "raw")]
    rawprint: bool,

    /// Inject Hex Packets from File
    #[structopt(short = "F", long = "file")]
    tx_file: Option<PathBuf>,

    /// Inject Hex Packets from File
    #[structopt(short = "S", long = "stdin")]
    tx_stdin: bool,

    /// The network interface to use
    #[structopt(name = "interface")]
    interface: Option<String>,

    /// The hex bytes to send over the network
    #[structopt(name = "bytes")]
    bytes: Option<String>,
}

fn main() {
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::new().filter_or("LOG", "INFO"));
    builder.target(env_logger::fmt::Target::Stdout);
    builder.init();

    let opt = Opt::from_args();
    debug!("\n{:#?}", opt);

    // If the user did not specify any interface. List some to be helpful
    if opt.interface.is_none() {
        print_interfaces();
        std::process::exit(0);
    };

    // Find the network interface with the provided name
    let interfaces = datalink::interfaces();
    let interface = interfaces
        .into_iter()
        // Safe to unwrap since print_interfaces will exit above
        .find(|iface: &NetworkInterface| iface.name == *opt.interface.as_ref().unwrap())
        .expect("Could not find the network interface");
    debug!("{:#?}", interface);

    // Set the timeout of the socket read to 10ms
    let datalink_config = datalink::Config {
        read_timeout: Some(Duration::from_millis(10)),
        ..Default::default()
    };

    // Create a new channel, dealing with layer 2 packets
    let (mut tx, mut rx) = match datalink::channel(&interface, datalink_config) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("Unhandled channel type"),
        Err(e) => panic!("Error while creating datalink channel: {:?}", e),
    };

    // Progressing to rx and tx steps. Add a waitgroup for them
    let wg = crossbeam::sync::WaitGroup::new();

    // Start of the Rx scope. First convert the users settings
    {
        let rx_timeout = opt.rx_timeout.map(Duration::from_secs);
        let rawprint = opt.rawprint;
        let mut rx_countlimit = opt.rx_count;
        let wg = wg.clone();

        let rx_filter = RxFilter::create(opt.rx_filter, opt.rx_blacklist_filter);

        // Now spawn the thread for performing the Rx'ing
        thread::spawn(move || {
            // If you dont want it to print Rx packets.
            // Kill the thread via -c 0
            let now = Instant::now();
            while rx_countlimit != 0 {
                match rx.next() {
                    Ok(packet) => {
                        debug!("Rx'd a packet!");

                        // Filter the packet
                        if let Some(output) = rx_filter.filter(packet) {
                            if rawprint {
                                println!("{}", hex::encode(packet));
                            } else {
                                info!("\n{}", output);
                            }
                            rx_countlimit -= 1;
                        };
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {
                        // Timeout errors are fine
                        // Pass
                    }
                    Err(e) => {
                        // If any other error occurs, we can handle it here
                        panic!("An error occurred while reading: {:?}", e);
                    }
                }

                // If there is a timeout enabled. Check it
                // TDB if this moves to it's own thread so it works for both Tx and Rx
                if let Some(rx_timeout) = rx_timeout {
                    if now.elapsed() > rx_timeout {
                        debug!("Rx time limit reached!");
                        std::process::exit(0);
                    }
                }
            }
            drop(wg);
        });
    }

    // The Transmit Worker Section
    {
        let rate = opt.tx_rate;
        let count = opt.tx_send;
        let tx_file = opt.tx_file;
        let tx_stdin = opt.tx_stdin;
        let cmdbytes = opt.bytes;
        let iscmdbytes: bool = cmdbytes.is_some();
        let wg = wg.clone();

        let (s, r) = crossbeam::channel::bounded::<Vec<u8>>(256);

        // A thread for processing whatever input we have
        thread::spawn(move || {
            if let Some(path) = tx_file {
                // Get the file data
                let file = std::fs::File::open(path).expect("Could not open file");
                let reader = io::BufReader::new(file);
                for line in reader.lines() {
                    let line = line.expect("Could not decode line in file");
                    let data = Vec::from_hex(line).expect("Could parse line as hex string");
                    s.send(data).expect("crossbeam send failed!");
                }
            } else if tx_stdin {
                // Print from stdin
                let stdin = stdin();
                let stdinbuf = stdin.lock().lines();
                for line in stdinbuf {
                    let line = line.expect("Failed to read stdin");
                    let data = Vec::from_hex(line).expect("Could not decode stdin as hex string");
                    s.send(data).expect("crossbeam send failed!");
                }
            } else if let Some(data) = cmdbytes {
                // Grab the bytes from the command line
                let data = Vec::from_hex(data).expect("Could not decode string to hex data");
                s.send(data).expect("crossbeam send failed!");
            } else {
                debug!("No Tx settings detected");
            }
        });

        // A thread for transmitting those bytes
        thread::spawn(move || {
            // Create some tickers for sending bytes + updating the progress bar
            let rate: Option<Duration> =
                rate.map(|rate| Duration::from_micros((1e6 / rate) as u64));
            let mut now = Instant::now();

            let filebar = indicatif::ProgressBar::new(r.len() as u64).with_style(
                ProgressStyle::default_bar()
                    .template("[{elapsed_precise}] {wide_bar:.cyan} {pos:>7}/{len:7} {percent:>3}% {per_sec:6} {eta_precise}"));
            if iscmdbytes {
                // Dont load this progress bar if we are doing a command-line input
                filebar.finish_and_clear();
            }
            for bytes in r.iter() {
                filebar.inc(1);
                let nextlength = filebar.position() + r.len() as u64;
                if (nextlength - filebar.length()) > 10 {
                    // Having file or stdin inputs can give us wonkey ETAs. Reset on large jumps
                    filebar.reset_eta();
                }
                filebar.set_length(filebar.position() + r.len() as u64);

                debug!("Sending Packet!");

                let loopbar = indicatif::ProgressBar::new(count).with_style(
                    ProgressStyle::default_bar()
                        .template("[{elapsed_precise}] {wide_bar:.green} {pos:>7}/{len:7} {percent:>3}% {per_sec:6} {eta_precise}"));
                if count == 1 {
                    loopbar.finish_and_clear();
                }

                for idx in 0..count {
                    debug!("Tx Loop Count {}", idx);
                    loopbar.set_position(idx);

                    let res = tx.send_to(&bytes, None).unwrap();
                    if let Err(error) = res {
                        error!("{:?}", error);
                        std::process::exit(1);
                    };

                    if let Some(rate) = rate {
                        if let Some(sleep) = rate.checked_sub(now.elapsed()) {
                            thread::sleep(sleep);
                        }
                        now += rate;
                    };
                }
            }
            drop(wg);
        });
    }
    wg.wait();
}
