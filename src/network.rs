//! Bounded, loopback-first JSON-lines transport. The render thread owns model
//! state; network workers only enqueue commands and coalesce tracking frames.
use crate::tracking::{Values, parse};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    net::{IpAddr, SocketAddr, TcpListener, TcpStream, UdpSocket},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub struct Command {
    pub request: Value,
    pub reply: SyncSender<Value>,
}
pub struct Inbox {
    sender: SyncSender<Command>,
    receiver: Receiver<Command>,
    tracking: Arc<Mutex<BTreeMap<String, Values>>>,
}
impl Default for Inbox {
    fn default() -> Self {
        Self::new()
    }
}
impl Inbox {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::sync_channel(128);
        Self {
            sender,
            receiver,
            tracking: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
    pub fn sender(&self) -> SyncSender<Command> {
        self.sender.clone()
    }
    pub fn tracking(&self) -> Arc<Mutex<BTreeMap<String, Values>>> {
        Arc::clone(&self.tracking)
    }
    pub fn push_tracking(&self, source: &str, values: Values) {
        if let Ok(mut tracking) = self.tracking.lock() {
            tracking.insert(source.into(), values);
        }
    }
    pub fn drain(&self) -> (BTreeMap<String, Values>, Vec<Command>) {
        let frames = self
            .tracking
            .lock()
            .map(|mut t| std::mem::take(&mut *t))
            .unwrap_or_default();
        let commands = (0..128)
            .map_while(|_| self.receiver.try_recv().ok())
            .collect();
        (frames, commands)
    }
}

#[derive(Clone)]
pub struct NetworkConfig {
    pub api: SocketAddr,
    pub tracking_bind: IpAddr,
    pub tracking_ports: Vec<u16>,
    pub max_clients: usize,
    pub max_packet_bytes: usize,
}
impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            api: SocketAddr::from(([127, 0, 0, 1], 4141)),
            tracking_bind: IpAddr::from([127, 0, 0, 1]),
            tracking_ports: vec![15483, 8001, 49983],
            max_clients: 8,
            max_packet_bytes: 65_536,
        }
    }
}

pub fn new_token() -> Result<String> {
    let mut bytes = [0u8; 24];
    getrandom::getrandom(&mut bytes)
        .map_err(|error| anyhow::anyhow!("OS randomness unavailable: {error}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn equal_token(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let difference = a.bytes().zip(b.bytes()).fold(0u8, |v, (x, y)| v | (x ^ y));
    difference == 0
}

pub struct Network {
    running: Arc<AtomicBool>,
    handles: Vec<JoinHandle<()>>,
    pub api_address: SocketAddr,
    pub tracking_addresses: Vec<SocketAddr>,
}
impl Network {
    pub fn start(config: NetworkConfig, inbox: &Inbox, token: String) -> Result<Self> {
        ensure!(
            config.max_clients > 0 && config.max_clients <= 128,
            "Invalid max_clients"
        );
        ensure!(
            (1024..=1_048_576).contains(&config.max_packet_bytes),
            "Invalid packet limit"
        );
        let listener = TcpListener::bind(config.api).context("Cannot bind agent API")?;
        listener.set_nonblocking(true)?;
        let api_address = listener.local_addr()?;
        let udp: Vec<UdpSocket> = config
            .tracking_ports
            .iter()
            .map(|port| {
                let socket = UdpSocket::bind(SocketAddr::new(config.tracking_bind, *port))?;
                socket.set_read_timeout(Some(Duration::from_millis(200)))?;
                Ok(socket)
            })
            .collect::<Result<_>>()?;
        let tracking_addresses = udp
            .iter()
            .map(UdpSocket::local_addr)
            .collect::<std::io::Result<_>>()?;
        let running = Arc::new(AtomicBool::new(true));
        let clients = Arc::new(AtomicUsize::new(0));
        let sender = inbox.sender();
        let mut handles = Vec::new();
        {
            let running = Arc::clone(&running);
            let clients = Arc::clone(&clients);
            let limit = config.max_clients;
            handles.push(thread::spawn(move || {
                while running.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((socket, _)) => {
                            if clients.fetch_add(1, Ordering::AcqRel) >= limit {
                                clients.fetch_sub(1, Ordering::AcqRel);
                                continue;
                            }
                            let sender = sender.clone();
                            let token = token.clone();
                            let clients = Arc::clone(&clients);
                            let running = Arc::clone(&running);
                            thread::spawn(move || {
                                let _ = serve_client(
                                    socket,
                                    &token,
                                    config.max_packet_bytes,
                                    sender,
                                    &running,
                                );
                                clients.fetch_sub(1, Ordering::AcqRel);
                            });
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10))
                        }
                        Err(_) => break,
                    }
                }
            }));
        }
        for socket in udp {
            let running = Arc::clone(&running);
            let tracking = inbox.tracking();
            handles.push(thread::spawn(move || {
                let mut buf = [0u8; 65_537];
                while running.load(Ordering::Relaxed) {
                    if let Ok((size, peer)) = socket.recv_from(&mut buf) {
                        let source = if peer.ip().is_loopback()
                            && serde_json::from_slice::<Value>(&buf[..size])
                                .ok()
                                .is_some_and(|v| v["source"] == "webcam")
                        {
                            "webcam"
                        } else {
                            "phone"
                        };
                        if let Ok(values) = parse(&buf[..size])
                            && !values.is_empty()
                            && let Ok(mut latest) = tracking.lock()
                        {
                            latest.insert(source.into(), values);
                        }
                    }
                }
            }));
        }
        Ok(Self {
            running,
            handles,
            api_address,
            tracking_addresses,
        })
    }
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        for handle in self.handles.drain(..) {
            let _ = handle.join();
        }
    }
}
impl Drop for Network {
    fn drop(&mut self) {
        self.stop();
    }
}

fn serve_client(
    mut socket: TcpStream,
    token: &str,
    max_bytes: usize,
    sender: SyncSender<Command>,
    running: &AtomicBool,
) -> Result<()> {
    socket.set_read_timeout(Some(Duration::from_millis(200)))?;
    socket.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut reader = BufReader::new(socket.try_clone()?);
    let mut input = Vec::new();
    let mut last_seen = Instant::now();
    while running.load(Ordering::Relaxed) && last_seen.elapsed() < Duration::from_secs(120) {
        input.clear();
        match read_bounded_line(&mut reader, &mut input, max_bytes, running, last_seen) {
            Ok(0) => break,
            Ok(_) => {
                last_seen = Instant::now();
                let response = match serde_json::from_slice::<Value>(&input) {
                    Ok(request) => match authenticate(&request, token) {
                        Ok(()) => {
                            let id = request.get("id").cloned();
                            let (reply_tx, reply_rx) = mpsc::sync_channel(1);
                            match sender.try_send(Command {
                                request,
                                reply: reply_tx,
                            }) {
                                Ok(()) => match reply_rx.recv_timeout(Duration::from_secs(10)) {
                                    Ok(mut reply) => {
                                        if let Some(id) = id
                                            && let Some(object) = reply.as_object_mut()
                                        {
                                            object.insert("id".into(), id);
                                        }
                                        reply
                                    }
                                    Err(_) => {
                                        json!({"ok":false,"error":"Studio response timed out"})
                                    }
                                },
                                Err(_) => {
                                    json!({"ok":false,"error":"Command queue full; retry later"})
                                }
                            }
                        }
                        Err(error) => json!({"ok":false,"error":error.to_string()}),
                    },
                    Err(_) => json!({"ok":false,"error":"Invalid JSON request"}),
                };
                let line = serde_json::to_vec(&response)?;
                socket.write_all(&line)?;
                socket.write_all(b"\n")?;
            }
            Err(ref e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(_) => break,
        }
    }
    Ok(())
}

fn read_bounded_line<R: BufRead>(
    reader: &mut R,
    input: &mut Vec<u8>,
    limit: usize,
    running: &AtomicBool,
    since: Instant,
) -> std::io::Result<usize> {
    while running.load(Ordering::Relaxed) && since.elapsed() < Duration::from_secs(120) {
        let available = match reader.fill_buf() {
            Ok(bytes) => bytes,
            Err(ref error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                continue;
            }
            Err(error) => return Err(error),
        };
        if available.is_empty() {
            return Ok(0);
        }
        let take = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |n| n + 1);
        if input.len().saturating_add(take) > limit {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "API request exceeds packet limit",
            ));
        }
        input.extend_from_slice(&available[..take]);
        reader.consume(take);
        if input.last() == Some(&b'\n') {
            return Ok(input.len());
        }
    }
    Ok(0)
}

fn authenticate(request: &Value, token: &str) -> Result<()> {
    let object = request
        .as_object()
        .context("Request must be a JSON object")?;
    if !equal_token(
        object.get("token").and_then(Value::as_str).unwrap_or(""),
        token,
    ) {
        bail!("Invalid API token")
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coalesces_tracking_and_bounds_commands() {
        let inbox = Inbox::new();
        inbox.push_tracking("phone", Values::from([("yaw".into(), 1.0)]));
        inbox.push_tracking("phone", Values::from([("yaw".into(), 2.0)]));
        let (frames, commands) = inbox.drain();
        assert_eq!(frames["phone"]["yaw"], 2.0);
        assert!(commands.is_empty());
    }
    #[test]
    fn token_auth_is_exact() {
        let token = new_token().unwrap();
        assert_eq!(token.len(), 48);
        assert!(authenticate(&json!({"token":token}), &token).is_ok());
        assert!(authenticate(&json!({"token":"wrong"}), &token).is_err());
    }
    #[test]
    fn tcp_ack_follows_dispatch() {
        let inbox = Inbox::new();
        let config = NetworkConfig {
            api: SocketAddr::from(([127, 0, 0, 1], 0)),
            tracking_ports: vec![],
            ..Default::default()
        };
        let mut network = Network::start(config, &inbox, "secret".into()).unwrap();
        let mut client = TcpStream::connect(network.api_address).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        client
            .write_all(b"{\"token\":\"secret\",\"id\":7,\"op\":\"status\"}\n")
            .unwrap();
        let command = loop {
            if let Some(command) = inbox.drain().1.pop() {
                break command;
            }
            thread::sleep(Duration::from_millis(2));
        };
        assert_eq!(command.request["op"], "status");
        command
            .reply
            .send(json!({"ok":true,"result":"ready"}))
            .unwrap();
        let mut line = String::new();
        BufReader::new(client).read_line(&mut line).unwrap();
        let result: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(result["id"], 7);
        assert_eq!(result["result"], "ready");
        network.stop();
    }
    #[test]
    fn oversized_request_is_stopped_before_allocation() {
        let data = vec![b'a'; 10_000];
        let mut reader = BufReader::new(data.as_slice());
        let mut input = Vec::new();
        let running = AtomicBool::new(true);
        let error =
            read_bounded_line(&mut reader, &mut input, 1024, &running, Instant::now()).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(input.len() <= 1024);
    }
}
