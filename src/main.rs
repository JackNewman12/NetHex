extern crate env_logger;
extern crate hex;
extern crate hexplay;
extern crate log;
extern crate pbr;
extern crate pnet;

use hex::FromHex;
use log::Level::Info;
use log::{debug, error, info, log_enabled};
use pbr::ProgressBar;
use pnet::datalink::Channel::Ethernet;
use pnet::datalink::{self, Config, NetworkInterface};
use structopt::StructOpt;

use std::io;
use std::thread;
use std::time::{Duration, Instant};

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
    raw(setting = "structopt::clap::AppSettings::ColoredHelp")
)]
/// A small utility for reading / writing directly to a network interface
struct Opt {
    /// Number of packet to receive before exiting
    #[structopt(short = "c", long = "count", default_value = "-1")]
    rx_count: i64,

    /// Time to receive for before exiting
    #[structopt(short = "t", long = "timeout")]
    rx_timeout: Option<u64>,

    /// Number of packet to transmit
    #[structopt(short = "s", long = "send", default_value = "1")]
    tx_send: u64,

    /// Rate to transmit (Packets Per Second)
    #[structopt(short = "r", long = "rate")]
    tx_rate: Option<f64>,

    /// The network interface to use
    #[structopt(name = "interface")]
    interface: Option<String>,

    /// The hex bytes to send over the network
    #[structopt(name = "bytes")]
    bytes: Option<String>,
}

// Invoke as echo <interface name>
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
    let mut datalink_config = Config::default();
    datalink_config.read_timeout = Some(Duration::from_millis(10));

    // Create a new channel, dealing with layer 2 packets
    let (mut tx, mut rx) = match datalink::channel(&interface, datalink_config) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("Unhandled channel type"),
        Err(e) => panic!("Error while creating datalink channel: {:?}", e),
    };

    // Progressing to rx and tx steps. Add a waitgroup for them
    let wg = crossbeam_utils::sync::WaitGroup::new();

    // Decode the hex input if the user specified one
    if let Some(arg) = opt.bytes {
        let bytes = match Vec::from_hex(arg) {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("{}", e);
                std::process::exit(1);
            }
        };
        info!("Input bytes: {:X?}", bytes);

        let rate = opt.tx_rate;
        let count = opt.tx_send;
        let wg = wg.clone();
        thread::spawn(move || {
            // Transmit those bytes

            // Create some tickers for sending bytes + updating the progress bar
            let rate = rate.map(|rate| Duration::from_micros((1e6 / rate) as u64));
            let mut progress_bar = ProgressBar::new(count);
            if log_enabled!(Info) {
                progress_bar.set(0);
                progress_bar.set_max_refresh_rate(Some(Duration::from_millis(500)));
            }

            let mut now = Instant::now();
            for idx in 0..count {
                debug!("Sending Packet!");
                let res = tx.send_to(&bytes, None).unwrap();
                if let Err(error) = res {
                    error!("{:?}", error);
                    std::process::exit(1);
                };

                if log_enabled!(Info) {
                    progress_bar.set(idx);
                }
                if let Some(rate) = rate {
                    if let Some(sleep) = rate.checked_sub(now.elapsed()) {
                        thread::sleep(sleep);
                    }
                    now += rate;
                };
            }
            progress_bar.finish();
            drop(wg);
        });
    }

    {
        let rx_timeout = opt.rx_timeout.map(Duration::from_secs);
        let mut rx_countlimit = opt.rx_count;
        let wg = wg.clone();
        // Now do the Rx part
        thread::spawn(move || {
            let now = Instant::now();
            while rx_countlimit != 0 {
                match rx.next() {
                    Ok(packet) => {
                        use hexplay::HexViewBuilder;
                        let view = HexViewBuilder::new(packet).row_width(16).finish();
                        // Dont think this makes sense to be part of the logger.
                        // If you dont want it to print. Kill the thread via -c 0
                        println!("Recv Packet\n{}", view);
                        rx_countlimit -= 1;
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {
                        // Timeout errors are fine. Ignore.
                    }
                    Err(e) => {
                        // If any other error occurs, we can handle it here
                        panic!("An error occurred while reading: {:?}", e);
                    }
                }
                if let Some(rx_timeout) = rx_timeout {
                    // If there is a timeout enabled. Check it
                    if now.elapsed() > rx_timeout {
                        debug!("Rx time limit reached!");
                        std::process::exit(0);
                    }
                }
            }
            drop(wg);
        });
    }
    wg.wait();
}
