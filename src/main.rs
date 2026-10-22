mod filter;
use filter::RxFilter;

use clap::Parser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use log::{debug, info};
use pnet_datalink::{self as datalink, Channel::Ethernet};
use pretty_hex::{config_hex, HexConfig};

use std::io::{self, BufRead};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

// For user-facing errors. A panic would abort with a core dump in release builds
fn die(msg: impl std::fmt::Display) -> ! {
    eprintln!("Error: {}", msg);
    std::process::exit(1);
}

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

// Whitespace inside the hex is ignored. A blank line gives None
fn parse_line(line: &str) -> Result<Option<Vec<u8>>, hex::FromHexError> {
    let line: String = line.split_whitespace().collect();
    if line.is_empty() {
        return Ok(None);
    }
    hex::decode(line).map(Some)
}

#[derive(Parser, Debug)]
#[command(name = "NetHex", version, allow_negative_numbers = true)]
/// A small utility for reading / writing directly to a network interface
struct Opt {
    /// Number of packet to receive before exiting
    #[arg(short = 'c', long = "count", default_value = "-1")]
    rx_count: i64,

    /// Time to receive for before exiting
    #[arg(short = 't', long = "timeout")]
    rx_timeout: Option<u64>,

    /// Only print Rx packets that match this regex filter
    #[arg(short = 'f', long = "filter")]
    rx_filter: Option<String>,

    /// Only print Rx packets that do NOT match this regex filter
    #[arg(short = 'b', long = "blacklist")]
    rx_blacklist_filter: Option<String>,

    /// Number of packet to transmit
    #[arg(short = 's', long = "send", default_value = "1")]
    tx_send: u64,

    /// Rate to transmit (Packets Per Second)
    #[arg(short = 'r', long = "rate", value_parser = clap::value_parser!(u64).range(1..))]
    tx_rate: Option<u64>,

    /// Raw Hex Printing
    #[arg(short = 'R', long = "raw")]
    rawprint: bool,

    /// Inject Hex Packets from File
    #[arg(short = 'F', long = "file", conflicts_with_all = ["tx_stdin", "bytes"])]
    tx_file: Option<PathBuf>,

    /// Inject Hex Packets from stdin
    #[arg(short = 'S', long = "stdin", conflicts_with = "bytes")]
    tx_stdin: bool,

    /// The network interface to use
    #[arg(value_name = "interface")]
    interface: Option<String>,

    /// The hex bytes to send over the network
    #[arg(value_name = "bytes")]
    bytes: Option<String>,
}

fn main() {
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::new().filter_or("LOG", "INFO"));
    builder.target(env_logger::fmt::Target::Stdout);
    builder.init();

    let opt = Opt::parse();
    debug!("\n{:#?}", opt);

    // If the user did not specify any interface. List some to be helpful
    let Some(name) = opt.interface else {
        print_interfaces();
        return;
    };

    // Find the network interface with the provided name
    let interface = datalink::interfaces()
        .into_iter()
        .find(|iface| iface.name == name)
        .unwrap_or_else(|| die(format!("Could not find the network interface {}", name)));
    debug!("{:#?}", interface);

    let rx_filter = RxFilter::create(opt.rx_filter, opt.rx_blacklist_filter);

    // Set the timeout of the socket read to 10ms
    let datalink_config = datalink::Config {
        read_timeout: Some(Duration::from_millis(10)),
        ..Default::default()
    };

    // Create a new channel, dealing with layer 2 packets
    let (mut tx, mut rx) = match datalink::channel(&interface, datalink_config) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => die("Unhandled channel type"),
        Err(e) => die(format!(
            "Could not open {}: {} (needs root or CAP_NET_RAW)",
            name, e
        )),
    };

    // Start of the Rx scope. First convert the users settings
    let rx_thread = {
        let rx_timeout = opt.rx_timeout.map(Duration::from_secs);
        let rawprint = opt.rawprint;
        let mut rx_countlimit = opt.rx_count;
        let hex_config = HexConfig {
            title: false,
            width: 16,
            group: 0,
            ..Default::default()
        };

        // Now spawn the thread for performing the Rx'ing
        thread::spawn(move || {
            // If you dont want it to print Rx packets.
            // Kill the thread via -c 0
            let now = Instant::now();
            while rx_countlimit != 0 {
                match rx.next() {
                    Ok(packet) => {
                        debug!("Rx'd a packet!");

                        if rx_filter.matches(packet) {
                            if rawprint {
                                println!("{}", hex::encode(packet));
                            } else {
                                info!("\n{}", config_hex(&packet, hex_config));
                            }
                            rx_countlimit -= 1;
                        };
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {
                        // Timeout errors are fine
                        // Pass
                    }
                    Err(e) => die(format!("An error occurred while reading: {}", e)),
                }

                // Only stop receiving. Any Tx still in progress finishes
                if rx_timeout.is_some_and(|t| now.elapsed() > t) {
                    debug!("Rx time limit reached!");
                    break;
                }
            }
        })
    };

    // The Transmit Worker Section
    let tx_thread = {
        let rate = opt.tx_rate.map(|r| Duration::from_nanos(1_000_000_000 / r));
        let count = opt.tx_send;
        let tx_file = opt.tx_file;
        let tx_stdin = opt.tx_stdin;
        let cmdbytes = opt.bytes;
        // Bars only make sense when there is something to send
        let show_filebar = tx_file.is_some() || tx_stdin;
        let sending = show_filebar || cmdbytes.is_some();

        let style = ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {wide_bar:.cyan} {pos:>7}/{len:7} {percent:>3}% {per_sec:6} {eta_precise}")
            .unwrap();

        let (s, r) = crossbeam_channel::bounded::<Vec<u8>>(256);

        // A thread for processing whatever input we have
        thread::spawn(move || {
            let reader: Box<dyn BufRead> = if let Some(path) = tx_file {
                let file = std::fs::File::open(&path).unwrap_or_else(|e| {
                    die(format!("Could not open file {}: {}", path.display(), e))
                });
                Box::new(io::BufReader::new(file))
            } else if tx_stdin {
                Box::new(io::stdin().lock())
            } else if let Some(data) = cmdbytes {
                Box::new(io::Cursor::new(data))
            } else {
                debug!("No Tx settings detected");
                return;
            };
            for (idx, line) in reader.lines().enumerate() {
                let line =
                    line.unwrap_or_else(|e| die(format!("Could not read line {}: {}", idx + 1, e)));
                match parse_line(&line) {
                    Ok(Some(data)) => s.send(data).expect("crossbeam send failed!"),
                    Ok(None) => debug!("Skipping blank line {}", idx + 1),
                    Err(e) => die(format!("Line {} is not a valid hex string: {}", idx + 1, e)),
                }
            }
        });

        // A thread for transmitting those bytes
        thread::spawn(move || {
            let mut now = Instant::now();

            // Stacks the file and loop bars on separate lines instead of both drawing over one line
            let bars = MultiProgress::new();
            let filebar = if show_filebar {
                bars.add(ProgressBar::new(r.len() as u64).with_style(style.clone()))
            } else {
                ProgressBar::hidden()
            };
            let loopbar = if sending && count != 1 {
                bars.add(ProgressBar::new(count).with_style(style))
            } else {
                ProgressBar::hidden()
            };
            for bytes in r.iter() {
                filebar.inc(1);
                let nextlength = filebar.position() + r.len() as u64;
                if (nextlength - filebar.length().unwrap_or(nextlength)) > 10 {
                    // Having file or stdin inputs can give us wonkey ETAs. Reset on large jumps
                    filebar.reset_eta();
                }
                filebar.set_length(nextlength);

                debug!("Sending Packet!");
                loopbar.reset();

                for idx in 0..count {
                    debug!("Tx Loop Count {}", idx);

                    if let Some(Err(error)) = tx.send_to(&bytes, None) {
                        die(format!("Could not send packet: {}", error));
                    };
                    loopbar.inc(1);

                    if let Some(rate) = rate {
                        if let Some(sleep) = rate.checked_sub(now.elapsed()) {
                            thread::sleep(sleep);
                        }
                        now += rate;
                    };
                }
            }
            filebar.finish();
            loopbar.finish();
        })
    };

    rx_thread.join().expect("Rx thread panicked");
    tx_thread.join().expect("Tx thread panicked");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_line() {
        assert_eq!(parse_line("").unwrap(), None);
        assert_eq!(parse_line("  \t").unwrap(), None);
        assert_eq!(
            parse_line("AA bb\tCC").unwrap(),
            Some(vec![0xAA, 0xBB, 0xCC])
        );
        assert!(parse_line("zz").is_err());
        assert!(parse_line("ABC").is_err());
    }
}
