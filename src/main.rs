use clap::Parser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use log::{debug, info};
use pnet_datalink::{self as datalink, Channel::Ethernet};
use pretty_hex::{config_hex, HexConfig};
use regex::{Regex, RegexBuilder};

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

// A packet's bytes plus the offsets that get a fresh random byte on every send
type Packet = (Vec<u8>, Vec<usize>);

// Whitespace inside the hex and anything after a '#' is ignored. A blank line gives None.
// "??" in place of a byte means random
fn parse_line(line: &str) -> Result<Option<Packet>, hex::FromHexError> {
    let line = line.split('#').next().unwrap_or_default();
    let line: String = line.split_whitespace().collect();
    if line.is_empty() {
        return Ok(None);
    }
    let (mut bytes, mut random) = (vec![], vec![]);
    for (i, pair) in line.as_bytes().chunks(2).enumerate() {
        if pair == b"??" {
            random.push(i);
            bytes.push(0);
        } else {
            bytes.extend(hex::decode(pair)?);
        }
    }
    Ok(Some((bytes, random)))
}

// Filters match against the packet as a hex string, ignoring case and whitespace
fn build_filter(filter: String) -> Regex {
    RegexBuilder::new(&filter)
        .case_insensitive(true)
        .ignore_whitespace(true)
        .build()
        .unwrap_or_else(|e| die(format!("Invalid regex filter:\n{}", e)))
}

// True if the packet matches the whitelist (when set) and not the blacklist (when set)
fn passes(whitelist: &Option<Regex>, blacklist: &Option<Regex>, packet: &[u8]) -> bool {
    if whitelist.is_none() && blacklist.is_none() {
        return true;
    }
    let hex = hex::encode(packet);
    whitelist.as_ref().is_none_or(|r| r.is_match(&hex))
        && !blacklist.as_ref().is_some_and(|r| r.is_match(&hex))
}

#[derive(Parser, Debug)]
#[command(name = "NetHex", version)]
/// A small utility for reading / writing directly to a network interface
struct Opt {
    /// Packets to receive before exiting [default: no limit]
    #[arg(short = 'c', long = "count")]
    rx_count: Option<u64>,

    /// Seconds to receive for. Fails if --count is not reached
    #[arg(short = 't', long = "timeout")]
    rx_timeout: Option<f64>,

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

    /// Raw Hex Printing with timestamps
    #[arg(short = 'T', long = "time")]
    timestamp: bool,

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

    let whitelist = opt.rx_filter.map(build_filter);
    let blacklist = opt.rx_blacklist_filter.map(build_filter);
    let rx_timeout = opt.rx_timeout.map(|t| {
        Duration::try_from_secs_f64(t).unwrap_or_else(|e| die(format!("Bad timeout: {}", e)))
    });

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
        let rx_count = opt.rx_count;
        let rawprint = opt.rawprint;
        let timestamp = opt.timestamp;
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
            let mut received: u64 = 0;
            while rx_count.is_none_or(|c| received < c) {
                match rx.next() {
                    Ok(packet) => {
                        debug!("Rx'd a packet!");

                        if passes(&whitelist, &blacklist, packet) {
                            if timestamp {
                                let secs = now.elapsed().as_secs_f64();
                                println!("{:.6} {}", secs, hex::encode(packet));
                            } else if rawprint {
                                println!("{}", hex::encode(packet));
                            } else {
                                info!("\n{}", config_hex(&packet, hex_config));
                            }
                            received += 1;
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
            received
        })
    };

    // The Transmit Worker Section
    let tx_thread = {
        let rate = opt.tx_rate.map(|r| Duration::from_nanos(1_000_000_000 / r));
        let count = opt.tx_send;
        let tx_file = opt.tx_file;
        let tx_stdin = opt.tx_stdin;
        let cmdbytes = opt.bytes;
        let show_filebar = tx_file.is_some() || tx_stdin;

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
                return 0;
            };

            let mut now = Instant::now();
            let mut sent: u64 = 0;

            // Stacks the file and loop bars on separate lines instead of both drawing over one line
            let bars = MultiProgress::new();
            // The file length is unknown up front, so this bar only counts
            let filebar = if show_filebar {
                let style =
                    ProgressStyle::with_template("[{elapsed_precise}] {pos:>7} lines {per_sec:6}");
                bars.add(ProgressBar::no_length().with_style(style.unwrap()))
            } else {
                ProgressBar::hidden()
            };
            let loopbar = if count != 1 {
                let style = ProgressStyle::with_template("[{elapsed_precise}] {wide_bar:.cyan} {pos:>7}/{len:7} {percent:>3}% {per_sec:6} {eta_precise}");
                bars.add(ProgressBar::new(count).with_style(style.unwrap()))
            } else {
                ProgressBar::hidden()
            };

            for (idx, line) in reader.lines().enumerate() {
                let line =
                    line.unwrap_or_else(|e| die(format!("Could not read line {}: {}", idx + 1, e)));
                let (mut bytes, random) = match parse_line(&line) {
                    Ok(Some(packet)) => packet,
                    Ok(None) => {
                        debug!("Skipping blank line {}", idx + 1);
                        continue;
                    }
                    Err(e) => die(format!("Line {} is not a valid hex string: {}", idx + 1, e)),
                };
                filebar.inc(1);

                debug!("Sending Packet!");
                loopbar.reset();

                for idx in 0..count {
                    debug!("Tx Loop Count {}", idx);
                    for &i in &random {
                        bytes[i] = fastrand::u8(..);
                    }

                    if let Some(Err(error)) = tx.send_to(&bytes, None) {
                        die(format!("Could not send packet: {}", error));
                    };
                    loopbar.inc(1);
                    sent += 1;

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
            sent
        })
    };

    let received = rx_thread.join().expect("Rx thread panicked");
    let sent = tx_thread.join().expect("Tx thread panicked");
    eprintln!("Sent {} packets, received {}", sent, received);
    if let Some(count) = opt.rx_count.filter(|&c| received < c) {
        die(format!(
            "Timed out after receiving {} of {} packets",
            received, count
        ));
    }
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
            Some((vec![0xAA, 0xBB, 0xCC], vec![]))
        );
        assert!(parse_line("zz").is_err());
        assert!(parse_line("ABC").is_err());
        // Comments
        assert_eq!(parse_line("# only a comment").unwrap(), None);
        assert_eq!(
            parse_line("aabb # arp").unwrap(),
            Some((vec![0xAA, 0xBB], vec![]))
        );
        // Random bytes only count on a byte boundary, so "a??b" is not one
        assert_eq!(
            parse_line("aa ?? bb ??").unwrap(),
            Some((vec![0xAA, 0, 0xBB, 0], vec![1, 3]))
        );
        assert!(parse_line("a??b").is_err());
        assert!(parse_line("a?").is_err());
    }

    #[test]
    fn test_filters() {
        let f = |s: &str| Some(build_filter(s.to_string()));

        // Whitelist only. Whitespace and case in the filter are ignored
        let white = f("00 11 2A");
        assert!(passes(&white, &None, &[0x00, 0x11, 0x2a, 4, 5]));
        assert!(!passes(&white, &None, &[1, 2, 3, 4, 5]));
        assert!(!passes(&white, &None, &[0x00, 0x11]));

        // Blacklist only
        let black = f("123456");
        assert!(!passes(&None, &black, &[0x33, 0x12, 0x34, 0x56, 0x78]));
        assert!(passes(&None, &black, &[0x56, 0x34, 0x12]));

        // Both
        let (white, black) = (f("12"), f("65"));
        assert!(!passes(&white, &black, &[0x34, 0x56]));
        assert!(!passes(&white, &black, &[0x12, 0x65]));
        assert!(passes(&white, &black, &[0x34, 0x56, 0x12]));

        // No filters
        assert!(passes(&None, &None, &[1, 2, 3]));
    }
}
