use log::{trace, debug};
use regex::{Regex, RegexBuilder};

#[derive(Debug)]
/*
 * A RxFilter struct.
 * Create a filter with a whitelist and/or blacklist option.
 *
 * Call matches on a packet via .matches(&[u8]).
 * Returns true  if the packet passes the white/blacklist
 *         false if it is filtered out
 */
pub struct RxFilter {
    rx_filter: Option<Regex>,
    rx_blacklist_filter: Option<Regex>,
}

impl RxFilter {
    // Create an RxFilter struct.
    // Converts two input strings into two regex filters
    pub fn create(whitelist: Option<String>, blacklist: Option<String>) -> Self {
        let build = |filter: String| {
            RegexBuilder::new(&filter)
                .case_insensitive(true)
                .ignore_whitespace(true)
                .multi_line(true)
                .dot_matches_new_line(true)
                .build()
                .unwrap_or_else(|e| crate::die(format!("Invalid regex filter:\n{}", e)))
        };
        let rx_filter = whitelist.map(build);
        debug!("Whitelist is: {:?}", rx_filter);
        let rx_blacklist_filter = blacklist.map(build);
        debug!("Blacklist is: {:?}", rx_blacklist_filter);

        RxFilter {
            rx_filter,
            rx_blacklist_filter,
        }
    }

    // Called with an input packet. Returns true if it does not get filtered
    pub fn matches(&self, packet: &[u8]) -> bool {
        debug!("Filtering packet");
        trace!("Packet: {:?}", packet);
        if self.rx_filter.is_none() && self.rx_blacklist_filter.is_none() {
            return true;
        }
        debug!("Filters are enabled");
        // Convert the bytes into a basic hex string so that regex filters
        // are easy to write for it
        let hex_string = hex::encode(packet);

        // Match the whitelist if set
        if let Some(rx_filter) = &self.rx_filter {
            if !rx_filter.is_match(&hex_string) {
                debug!("Killed by whitelist");
                return false;
            }
        }
        // Do not match the blackist if set
        if let Some(rx_blacklist_filter) = &self.rx_blacklist_filter {
            if rx_blacklist_filter.is_match(&hex_string) {
                debug!("Killed by blacklist");
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod filter_test {
    use super::*;

    #[test]
    fn test_white() {
        // Whitelist only filter
        let whitelist = RxFilter::create(Some("001122".to_string()), None);

        assert!(!whitelist.matches(&[1, 2, 3, 4, 5]));
        assert!(whitelist.matches(&[0x00, 0x11, 0x22, 4, 5]));
        assert!(!whitelist.matches(&[0x00, 0x22, 0x11]));
        assert!(!whitelist.matches(&[0x00, 0x11]));
    }

    #[test]
    fn test_black() {
        // Blacklist only filter
        let blacklist = RxFilter::create(None, Some("123456".to_string()));

        assert!(!blacklist.matches(&[0x12, 0x34, 0x56, 0x78]));
        assert!(blacklist.matches(&[0x56, 0x34, 0x12]));
        assert!(blacklist.matches(&[0x12, 0x34]));
        assert!(!blacklist.matches(&[0x33, 0x12, 0x12, 0x34, 0x56, 0x78]));
    }

    #[test]
    fn test_combo() {
        let both = RxFilter::create(Some("12".to_string()), Some("65".to_string()));
        // Stopped by whitelist
        assert!(!both.matches(&[0x34, 0x56]));
        // Stopped by blacklist
        assert!(!both.matches(&[0x12, 0x34, 0x65]));
        // Passes whitelist, stopped by blacklist in the same byte pair
        assert!(!both.matches(&[0x12, 0x65]));
        // Allowed by both
        assert!(both.matches(&[0x34, 0x56, 0x12]));
    }
}
