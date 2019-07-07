use hexplay::HexViewBuilder;
use log::{trace, debug};
use regex::RegexBuilder;

#[derive(Debug)]
/*
 * A RxFilter struct. 
 * Create a filter with a whitelist and/or blacklist option.
 * 
 * Call filter on a packet via .filter&([u8]). 
 * Returns Some(Nicely Formatted Hex)
 *         None  if filter by the white/blacklist
 */
pub struct RxFilter {
    rx_filter: Option<regex::Regex>,
    rx_blacklist_filter: Option<regex::Regex>,
}

impl RxFilter {
    // Create an RxFilter struct. 
    // Converts two input strings into two regex filters
    pub fn create(whitelist: Option<String>, blacklist: Option<String>) -> Self {
        let rx_filter = whitelist.map(|filter| {
            RegexBuilder::new(&filter)
                .case_insensitive(true)
                .ignore_whitespace(true)
                .multi_line(true)
                .dot_matches_new_line(true)
                .build()
                .expect("Invalid Regex")
        });
        debug!("Whitelist is: {:?}", rx_filter);
        let rx_blacklist_filter = blacklist.map(|filter| {
            RegexBuilder::new(&filter)
                .case_insensitive(true)
                .ignore_whitespace(true)
                .multi_line(true)
                .dot_matches_new_line(true)
                .build()
                .expect("Invalid Regex")
        });
        debug!("Blacklist is: {:?}", rx_blacklist_filter);

        RxFilter {
            rx_filter,
            rx_blacklist_filter,
        }
    }

    // Called with an input packet. Returns nicely formatted 
    // hex string of the packet if it does not get filtered
    pub fn filter(&self, packet: &[u8]) -> Option<String> {
        debug!("Filtering packet");
        trace!("Packet: {:?}", packet);
        if self.rx_filter.is_some() || self.rx_blacklist_filter.is_some() {
            debug!("Filters are enabled");
            // Convert the bytes into a basic hex string so that regex filters
            // are easy to write for it
            let hex_string = hex::encode(packet);

            // Match the whitelist if set
            if let Some(rx_filter) = &self.rx_filter {
                if !rx_filter.is_match(&hex_string.to_string()) {
                    debug!("Killed by whitelist");
                    return None;
                }
            }
            // Do not match the blackist if set
            if let Some(rx_blacklist_filter) = &self.rx_blacklist_filter {
                if rx_blacklist_filter.is_match(&hex_string.to_string()) {
                    debug!("Killed by blacklist");
                    return None;
                }
            }
        }

        // Format the packet into a nice hex format
        Some(
            HexViewBuilder::new(packet)
                .row_width(16)
                .finish()
                .to_string(),
        )
    }
}

#[cfg(test)]
mod filter_test {
    use super::*;

    #[test]
    fn test_white() {
        // Whitelist only filter
        let whitelist = RxFilter::create(Some("001122".to_string()), None);

        assert!(whitelist.filter(&[1, 2, 3, 4, 5]).is_none());
        assert!(whitelist.filter(&[0x00, 0x11, 0x22, 4, 5]).is_some());
        assert!(whitelist.filter(&[0x00, 0x22, 0x11]).is_none());
        assert!(whitelist.filter(&[0x00, 0x11]).is_none());
    }

    #[test]
    fn test_black() {
        // Whitelist only filter
        let blacklist = RxFilter::create(None, Some("123456".to_string()));

        assert!(blacklist.filter(&[0x12, 0x34, 0x56, 0x78]).is_none());
        assert!(blacklist.filter(&[0x56, 0x34, 0x12]).is_some());
        assert!(blacklist.filter(&[0x12, 0x34]).is_some());
        assert!(blacklist
            .filter(&[0x33, 0x12, 0x12, 0x34, 0x56, 0x78])
            .is_none());
    }

    #[test]
    fn test_combo() {
        let both = RxFilter::create(Some("12".to_string()), Some("65".to_string()));
        // Stopped by whitelist
        assert!(both.filter(&[0x34, 0x56]).is_none());
        // Stopped by blacklist
        assert!(both.filter(&[0x12, 0x34, 0x65]).is_none());
        // Stopped by both
        assert!(both.filter(&[0x34, 0x56]).is_none());
        // Allowed by both
        assert!(both.filter(&[0x34, 0x56, 0x12]).is_some());
    }
}
