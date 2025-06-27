use nym_sdk::mixnet::Recipient;
use url::Url;

const UPSTREAM: &str = "UPSTREAM:";

/// Adds the upstream address to the payload data
/// Format: "UPSTREAMhost:port\n" followed by actual data
pub fn add_upstream_header(upstream_address: &str, bytes: &[u8]) -> Vec<u8> {
    let upstream_header: String = format!("{}{}\n", UPSTREAM, upstream_address);
    let mut payload_with_upstream: Vec<u8> = upstream_header.into_bytes();
    payload_with_upstream.extend_from_slice(bytes);
    payload_with_upstream
}

/// Extract the upstream address from the payload data
/// Expected format: "UPSTREAMhost:port\n" followed by actual data
pub fn extract_upstream_header(data: &[u8]) -> (String, Vec<u8>) {
    // check if the first bytes are UPSTREAM
    // if so slice the data after it
    // return the upstream address and the payload
    if data.starts_with(UPSTREAM.as_bytes()) {
        let upstream_header_pos = data.iter().position(|&b| b == b'\n');

        if let Some(upstream_header_pos) = upstream_header_pos {
            let upstream_address =
                String::from_utf8(data[UPSTREAM.len()..upstream_header_pos].to_vec())
                    .unwrap_or_default();
            let payload = data[upstream_header_pos + 1..].to_vec();
            (upstream_address, payload)
        } else {
            (String::new(), data.to_vec())
        }
    } else {
        (String::new(), data.to_vec())
    }
}

#[derive(Debug, Clone)]
pub struct RpcProviderUrl {
    pub full_host: String,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub scheme: String,
}

/// extracts the host, port, path, and scheme from the RPC provider URL
impl RpcProviderUrl {
    pub fn new(rpc_provider: &str) -> Self {
        let rpc_url = Url::parse(&rpc_provider).unwrap();
        let rpc_host = rpc_url.host_str().unwrap();
        let rpc_scheme = rpc_url.scheme();
        let rpc_default_port = if rpc_scheme == "https" { 443 } else { 80 };
        let rpc_port = rpc_url.port().unwrap_or(rpc_default_port);
        let rpc_path = rpc_url.path();
        let rpc_full_host = format!("{}:{}", rpc_host, rpc_port);
        Self {
            full_host: rpc_full_host,
            host: rpc_host.to_string(),
            port: rpc_port,
            path: rpc_path.to_string(),
            scheme: rpc_scheme.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_upstream_header() {
        let data = b"Hello, World!";
        let upstream_address = "192.168.1.100:8080";
        let result = add_upstream_header(upstream_address, data);
        assert_eq!(result, b"UPSTREAM:192.168.1.100:8080\nHello, World!");
    }

    #[test]
    fn test_extract_upstream_header() {
        // Test with upstream header
        let data_with_header: &[u8] = "UPSTREAM:192.168.1.100:8080\n".as_bytes();
        let (upstream_address, payload) = extract_upstream_header(data_with_header);
        assert_eq!(upstream_address, "192.168.1.100:8080");
        assert_eq!(payload, b"");

        // Test with upstream header and binary data
        let mut data_with_header_binary = "UPSTREAM:localhost:3000\n".as_bytes().to_vec();
        data_with_header_binary.extend_from_slice(&[0x00, 0x01, 0x02, 0xFF]);
        let (upstream_address, payload) = extract_upstream_header(&data_with_header_binary);
        assert_eq!(upstream_address, "localhost:3000");
        assert_eq!(payload, &[0x00, 0x01, 0x02, 0xFF]);

        // Test without upstream header (should return original data)
        let data_without_header = "This is normal data".as_bytes();
        let (_, result) = extract_upstream_header(data_without_header);
        assert_eq!(result, data_without_header);

        // Test with malformed upstream header (no newline)
        let malformed_data = "UPSTREAM:localhost:3000 no newline".as_bytes();
        let (_, result) = extract_upstream_header(malformed_data);
        assert_eq!(result, malformed_data); // Should return original data

        // Test with empty data
        let empty_data = &[];
        let (_, result) = extract_upstream_header(empty_data);
        assert_eq!(result, empty_data);
    }
}

// list of known exit nodes
pub const KNOWN_EXIT_NODE_API_URLS: &[&str] = &[
    "178.156.187.131:8000",
    "138.199.156.168:8000",
    "49.12.199.110:8000",
    "95.216.196.110:8000",
];

/// get the Nym address from the exit node API
pub async fn get_nym_address_from_api(exit_node_api_url: &str) -> Recipient {
    let url = format!("http://{}/api/v1/nym-address", exit_node_api_url);
    let response = reqwest::get(&url)
        .await
        .expect("Failed to get response from exit node API");
    let json: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse JSON response");
    let nym_address = json["nym_address"]
        .as_str()
        .expect("No nym_address field in response")
        .to_string();
    Recipient::try_from_base58_string(nym_address).unwrap()
}
