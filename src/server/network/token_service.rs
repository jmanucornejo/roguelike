use crate::shared::constants::PROTOCOL_ID;
use bevy::log::{info, warn};
use bevy_renet::netcode::ConnectToken;
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    thread,
    time::{Duration, SystemTime},
};

const TOKEN_EXPIRY_SECONDS: u64 = 30;
const CLIENT_TIMEOUT_SECONDS: i32 = 15;
const MAX_HTTP_REQUEST_BYTES: usize = 4096;

/// Starts the internal HTTP service used by the TLS reverse proxy to issue
/// short-lived Renet connect tokens. The private key never leaves the server.
pub fn start(
    bind_addr: SocketAddr,
    public_server_addr: SocketAddr,
    private_key: [u8; 32],
) -> io::Result<()> {
    let listener = TcpListener::bind(bind_addr)?;
    thread::Builder::new()
        .name("netcode-token-service".to_string())
        .spawn(move || {
            info!("Netcode token service listening on http://{bind_addr}");
            for connection in listener.incoming() {
                match connection {
                    Ok(stream) => {
                        if let Err(error) =
                            handle_connection(stream, public_server_addr, &private_key)
                        {
                            warn!("Token service request failed: {error}");
                        }
                    }
                    Err(error) => warn!("Token service could not accept a connection: {error}"),
                }
            }
        })?;
    Ok(())
}

fn handle_connection(
    mut stream: TcpStream,
    public_server_addr: SocketAddr,
    private_key: &[u8; 32],
) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let mut request_line = String::new();
    BufReader::new(&mut stream)
        .take(MAX_HTTP_REQUEST_BYTES as u64)
        .read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();

    match (method, path.split('?').next().unwrap_or_default()) {
        ("GET", "/health") => write_response(&mut stream, "200 OK", "text/plain", b"ok"),
        ("GET", "/token") => {
            let current_time = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_err(io::Error::other)?;
            let client_id = fastrand::u64(1..=u64::MAX);
            let token = ConnectToken::generate(
                current_time,
                PROTOCOL_ID,
                TOKEN_EXPIRY_SECONDS,
                client_id,
                CLIENT_TIMEOUT_SECONDS,
                vec![public_server_addr],
                None,
                private_key,
            )
            .map_err(io::Error::other)?;
            let mut encoded = Vec::new();
            token.write(&mut encoded)?;
            write_response(&mut stream, "200 OK", "application/octet-stream", &encoded)
        }
        _ => write_response(&mut stream, "404 Not Found", "text/plain", b"not found"),
    }
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)
}
