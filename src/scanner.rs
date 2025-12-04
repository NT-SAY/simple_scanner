/// Scanner module for performing asynchronous port scans on a specified host.
/// It supports scanning a range of ports or a predefined list of common ports,
/// with configurable concurrency, timeouts, and retry logic.
/// # Imports
/// - `crate::services::identify_service` - Function to identify service names based on port numbers.
/// - `tokio::net::TcpStream` - Tokio's asynchronous TCP stream for network connections.
/// - `tokio::sync::{mpsc, Semaphore}` - Tokio's multi-producer, single-consumer channel and semaphore for concurrency control.
/// - `std::sync::Arc` - Atomic reference counting for shared ownership of the semaphore.
/// - `std::time::{Duration, Instant}` - Standard library time utilities for handling time  
/// outs and measuring elapsed time.
/// # Structs
/// - `ScanResult` - Struct representing the result of a port scan, including port number, status, service name, response time, and optional banner.
/// # Functions
/// - `identify_service(port: u16) -> String` - Identifies common services based on their port numbers.
/// - `scan_range(host: &str, start_port: u16, end_port: u16, tx: mpsc::Sender<ScanResult>)` - Scans a range of ports on the specified host and sends results through the provided channel.
/// - `scan_top_ports(host: &str, tx: mpsc::Sender<ScanResult>)` - Scans a predefined list of common ports on the specified host and sends results through the provided channel. 
/// # Examples
/// ```no_run
/// use tokio::sync::mpsc;
/// use crate::scanner::scan_range;
/// 
/// #[tokio::main]
/// async fn main() {
///     let (tx, mut rx) = mpsc::channel::<ScanResult>(2048);
///     tokio::spawn(async move {  
///       scan_range("", 1, 1000, tx).await;
///     });
/// }
/// ```

use crate::services::identify_service;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Semaphore};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;

#[derive(Clone, Debug)]
pub struct ScanResult {
    pub port: u16,
    pub status: String,
    pub service: String,
    pub response_ms: u128,
    pub banner: Option<String>,
}

const TOP_PORTS: &[u16] = &[
    21, 22, 23, 25, 53, 80, 110, 143, 443, 445, 3306, 3389, 5432, 5900, 8080, 8443, 9200,
];

async fn scan_port_once(host: &str, port: u16, timeout: Duration) -> ScanResult {
    let addr = format!("{}:{}", host, port);
    let start = Instant::now();
    
    match tokio::time::timeout(timeout, TcpStream::connect(&addr)).await {
        Ok(Ok(mut stream)) => {
            let mut buf = vec![0u8; 1024];
            let _ = stream.set_nodelay(true);
            let read_res = tokio::time::timeout(Duration::from_millis(500), stream.read(&mut buf)).await;
            
            let banner = match read_res {
                Ok(Ok(n)) if n > 0 => Some(String::from_utf8_lossy(&buf[..n]).trim().to_string()),
                _ => None,
            };
            
            let elapsed = start.elapsed().as_millis();
            ScanResult {
                port,
                status: "open".to_string(),
                service: identify_service(port),
                response_ms: elapsed,
                banner,
            }
        }
        Ok(Err(_)) => {
            let elapsed = start.elapsed().as_millis();
            ScanResult {
                port,
                status: "closed".to_string(),
                service: identify_service(port),
                response_ms: elapsed,
                banner: None,
            }
        }
        Err(_) => ScanResult {
            port,
            status: "timeout".to_string(),
            service: identify_service(port),
            response_ms: timeout.as_millis(),
            banner: None,
        },
    }
}

async fn scan_with_retries(host: &str, port: u16, base_timeout: Duration, retries: u8) -> ScanResult {
    let mut backoff = Duration::from_millis(100);
    
    for _ in 0..=retries {
        let res = scan_port_once(host, port, base_timeout).await;
        if res.status == "open" || res.status == "closed" {
            return res;
        }
        tokio::time::sleep(backoff).await;
        backoff *= 2;
    }
    
    scan_port_once(host, port, base_timeout).await
}

pub async fn scan_range(host: &str, start_port: u16, end_port: u16, tx: mpsc::Sender<ScanResult>) {
    let concurrency = 256usize;
    let timeout = Duration::from_secs(3);
    let retries = 1u8;
    let sem = Arc::new(Semaphore::new(concurrency));
    let mut handles = Vec::with_capacity((end_port - start_port + 1) as usize);

    for port in start_port..=end_port {
        let host = host.to_string();
        let tx = tx.clone();
        let sem = sem.clone();
        
        let h = tokio::spawn(async move {
            let permit = match sem.acquire().await {
                Ok(p) => p,
                Err(_) => return,
            };
            
            let res = scan_with_retries(&host, port, timeout, retries).await;
            let _ = tx.send(res).await;
            drop(permit);
        });
        
        handles.push(h);
    }

    for h in handles {
        let _ = h.await;
    }

    let _ = tx
        .send(ScanResult {
            port: 0,
            status: "DONE".to_string(),
            service: "".to_string(),
            response_ms: 0,
            banner: None,
        })
        .await;
}

pub async fn scan_top_ports(host: &str, tx: mpsc::Sender<ScanResult>) {
    let concurrency = 128usize;
    let timeout = Duration::from_secs(2);
    let sem = Arc::new(Semaphore::new(concurrency));
    let mut handles = Vec::with_capacity(TOP_PORTS.len());

    for &port in TOP_PORTS {
        let host = host.to_string();
        let tx = tx.clone();
        let sem = sem.clone();
        
        let h = tokio::spawn(async move {
            let permit = match sem.acquire().await {
                Ok(p) => p,
                Err(_) => return,
            };
            
            let res = scan_with_retries(&host, port, timeout, 0).await;
            let _ = tx.send(res).await;
            drop(permit);
        });
        
        handles.push(h);
    }

    for h in handles {
        let _ = h.await;
    }

    let _ = tx
        .send(ScanResult {
            port: 0,
            status: "DONE".to_string(),
            service: "".to_string(),
            response_ms: 0,
            banner: None,
        })
        .await;
}