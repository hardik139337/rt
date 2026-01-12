//! Magnet link parser
//!
//! Handles parsing of magnet:// URIs to extract torrent metadata.

use anyhow::{anyhow, Result};
use tracing::{debug, info, warn};
use url::Url;

/// Parsed magnet link information
#[derive(Debug, Clone)]
pub struct MagnetInfo {
    /// SHA1 info hash from the magnet link
    pub info_hash: [u8; 20],
    /// Display name (dn parameter)
    pub display_name: Option<String>,
    /// Tracker URLs (tr parameters)
    pub trackers: Vec<String>,
    /// Web seed URLs (ws parameters)
    pub web_seeds: Vec<String>,
    /// Exact source URLs (xs parameters)
    pub exact_sources: Vec<String>,
    /// Total file size in bytes (xl parameter)
    pub total_size: Option<u64>,
}

/// Parser for magnet links
pub struct MagnetParser;

impl MagnetParser {
    /// Parse a magnet link string
    ///
    /// # Arguments
    /// * `magnet_uri` - The magnet link URI string
    ///
    /// # Example
    /// ```ignore
    /// let magnet = "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&dn=Big+Buck+Bunny";
    /// let info = MagnetParser::parse(magnet)?;
    /// ```
    pub fn parse(magnet_uri: &str) -> Result<MagnetInfo> {
        info!("Parsing magnet link: {}", magnet_uri);

        // Parse the URL
        let url = Url::parse(magnet_uri).map_err(|e| {
            warn!("Invalid magnet URL format: {}", e);
            anyhow!("Invalid magnet URL format: {}", e)
        })?;

        // Verify it's a magnet link
        if url.scheme() != "magnet" {
            warn!("URL is not a magnet link: scheme is '{}'", url.scheme());
            return Err(anyhow!("URL is not a magnet link"));
        }

        debug!("Parsed magnet URL with scheme: magnet");

        // Get query parameters
        let params: Vec<(String, String)> = url.query_pairs().into_owned().collect();
        debug!("Found {} query parameters", params.len());

        let mut info_hash = None;
        let mut display_name = None;
        let mut trackers = Vec::new();
        let mut web_seeds = Vec::new();
        let mut exact_sources = Vec::new();
        let mut total_size = None;

        for (key, value) in params {
            debug!("Processing parameter: {} = {}", key, value);

            match key.as_str() {
                // Exact topic (xt) - contains the info hash
                "xt" => {
                    if let Some(hash) = Self::extract_info_hash(&value)? {
                        info_hash = Some(hash);
                        debug!("Extracted info hash: {}", hex::encode(hash));
                    }
                }
                // Display name (dn)
                "dn" => {
                    display_name = Some(value.clone());
                    debug!("Display name: {}", value);
                }
                // Tracker (tr)
                "tr" => {
                    trackers.push(value.clone());
                    debug!("Added tracker: {}", value);
                }
                // Web seed (ws)
                "ws" => {
                    web_seeds.push(value.clone());
                    debug!("Added web seed: {}", value);
                }
                // Exact source (xs)
                "xs" => {
                    exact_sources.push(value.clone());
                    debug!("Added exact source: {}", value);
                }
                // Exact length (xl) - file size
                "xl" => {
                    if let Ok(size) = value.parse::<u64>() {
                        total_size = Some(size);
                        debug!("Total size: {} bytes", size);
                    } else {
                        warn!("Invalid xl parameter value: {}", value);
                    }
                }
                _ => {
                    debug!("Ignoring unknown parameter: {}", key);
                }
            }
        }

        // Info hash is required
        let info_hash = info_hash.ok_or_else(|| {
            warn!("Magnet link missing required info hash (xt parameter)");
            anyhow!("Magnet link must contain an info hash (xt=urn:btih:<hash>)")
        })?;

        info!(
            "Successfully parsed magnet link: info_hash={}, name={}, trackers={}",
            hex::encode(info_hash),
            display_name.as_deref().unwrap_or("(none)"),
            trackers.len()
        );

        Ok(MagnetInfo {
            info_hash,
            display_name,
            trackers,
            web_seeds,
            exact_sources,
            total_size,
        })
    }

    /// Extract info hash from an xt parameter value
    ///
    /// The xt parameter has the format: urn:btih:<hash>
    /// where <hash> can be either a 40-character hex string or a base32-encoded string
    fn extract_info_hash(xt_value: &str) -> Result<Option<[u8; 20]>> {
        // Check if it's a BitTorrent info hash
        if !xt_value.starts_with("urn:btih:") {
            debug!("xt parameter is not a BitTorrent info hash: {}", xt_value);
            return Ok(None);
        }

        let hash_str = &xt_value[9..]; // Skip "urn:btih:"
        debug!("Extracting hash from: {}", hash_str);

        // Try hex format first (40 characters)
        if hash_str.len() == 40 {
            debug!("Attempting to parse as 40-character hex string");
            match hex::decode(hash_str) {
                Ok(bytes) if bytes.len() == 20 => {
                    let mut hash = [0u8; 20];
                    hash.copy_from_slice(&bytes);
                    debug!("Successfully parsed as hex");
                    return Ok(Some(hash));
                }
                Ok(bytes) => {
                    warn!("Hex decoded to {} bytes, expected 20", bytes.len());
                }
                Err(e) => {
                    debug!("Failed to decode as hex: {}", e);
                }
            }
        }

        // Try base32 format (32 characters)
        if hash_str.len() == 32 {
            debug!("Attempting to parse as 32-character base32 string");
            // Note: base32 decoding would require the base32 crate
            // For now, we'll return an error suggesting hex format
            return Err(anyhow!(
                "Base32-encoded info hashes are not yet supported. Please use a hex-encoded magnet link."
            ));
        }

        warn!(
            "Info hash has invalid length: {} (expected 40 for hex or 32 for base32)",
            hash_str.len()
        );
        Err(anyhow!(
            "Info hash has invalid length: {} (expected 40 for hex)",
            hash_str.len()
        ))
    }

    /// Check if a string looks like a magnet link
    pub fn is_magnet_link(input: &str) -> bool {
        input.trim().starts_with("magnet:?") || input.trim().starts_with("magnet://")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Big Buck Bunny magnet link from the user's example
    const BIG_BUCK_BUNNY_MAGNET: &str = "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&dn=Big+Buck+Bunny&tr=udp%3A%2F%2Fexplodie.org%3A6969&tr=udp%3A%2F%2Ftracker.coppersurfer.tk%3A6969&tr=udp%3A%2F%2Ftracker.empire-js.us%3A1337&tr=udp%3A%2F%2Ftracker.leechers-paradise.org%3A6969&tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337&tr=wss%3A%2F%2Ftracker.btorrent.xyz&tr=wss%3A%2F%2Ftracker.fastcast.nz&tr=wss%3A%2F%2Ftracker.openwebtorrent.com&ws=https%3A%2F%2Fwebtorrent.io%2Ftorrents%2F&xs=https%3A%2F%2Fwebtorrent.io%2Ftorrents%2Fbig-buck-bunny.torrent";

    #[test]
    fn test_parse_big_buck_bunny_magnet() {
        let info = MagnetParser::parse(BIG_BUCK_BUNNY_MAGNET).unwrap();

        // Check info hash
        assert_eq!(
            hex::encode(info.info_hash),
            "dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c"
        );

        // Check display name
        assert_eq!(info.display_name, Some("Big Buck Bunny".to_string()));

        // Check we have multiple trackers
        assert!(!info.trackers.is_empty());
        assert!(info.trackers.len() >= 9);

        // Check we have web seeds
        assert!(!info.web_seeds.is_empty());

        // Check we have exact sources
        assert!(!info.exact_sources.is_empty());
    }

    #[test]
    fn test_parse_simple_magnet() {
        let magnet = "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c";
        let info = MagnetParser::parse(magnet).unwrap();

        assert_eq!(
            hex::encode(info.info_hash),
            "dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c"
        );
        assert!(info.display_name.is_none());
        assert!(info.trackers.is_empty());
    }

    #[test]
    fn test_parse_magnet_with_name() {
        let magnet = "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&dn=Test+Torrent";
        let info = MagnetParser::parse(magnet).unwrap();

        assert_eq!(info.display_name, Some("Test Torrent".to_string()));
    }

    #[test]
    fn test_parse_magnet_with_trackers() {
        let magnet = "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&tr=http://tracker1.com&tr=http://tracker2.com";
        let info = MagnetParser::parse(magnet).unwrap();

        assert_eq!(info.trackers.len(), 2);
        assert!(info.trackers.contains(&"http://tracker1.com".to_string()));
        assert!(info.trackers.contains(&"http://tracker2.com".to_string()));
    }

    #[test]
    fn test_parse_magnet_with_size() {
        let magnet = "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&xl=1234567890";
        let info = MagnetParser::parse(magnet).unwrap();

        assert_eq!(info.total_size, Some(1234567890));
    }

    #[test]
    fn test_parse_magnet_with_web_seeds() {
        let magnet = "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&ws=http://seed1.com&ws=http://seed2.com";
        let info = MagnetParser::parse(magnet).unwrap();

        assert_eq!(info.web_seeds.len(), 2);
    }

    #[test]
    fn test_parse_invalid_magnet_no_info_hash() {
        let magnet = "magnet:?dn=Test+Torrent&tr=http://tracker.com";
        assert!(MagnetParser::parse(magnet).is_err());
    }

    #[test]
    fn test_parse_invalid_url() {
        let magnet = "not-a-magnet-link";
        assert!(MagnetParser::parse(magnet).is_err());
    }

    #[test]
    fn test_parse_invalid_scheme() {
        let magnet = "http:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c";
        assert!(MagnetParser::parse(magnet).is_err());
    }

    #[test]
    fn test_is_magnet_link() {
        assert!(MagnetParser::is_magnet_link("magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c"));
        assert!(MagnetParser::is_magnet_link("magnet://?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c"));
        assert!(MagnetParser::is_magnet_link("  magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c  "));
        assert!(!MagnetParser::is_magnet_link("http://example.com"));
        assert!(!MagnetParser::is_magnet_link("example.torrent"));
    }

    #[test]
    fn test_extract_info_hash_valid() {
        let xt = "urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c";
        let hash = MagnetParser::extract_info_hash(xt).unwrap().unwrap();
        assert_eq!(
            hex::encode(hash),
            "dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c"
        );
    }

    #[test]
    fn test_extract_info_hash_invalid_length() {
        let xt = "urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d";
        assert!(MagnetParser::extract_info_hash(xt).is_err());
    }

    #[test]
    fn test_extract_info_hash_non_bittorrent() {
        let xt = "urn:sha1:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c";
        assert!(MagnetParser::extract_info_hash(xt).unwrap().is_none());
    }
}
