///this function identifies common services based on their port numbers.
/// # Arguments
/// * `port` - A u16 integer representing the port number. 
/// # Returns
/// A String representing the identified service name. 
/// # Examples
/// ```
/// let service = identify_service(80);
/// assert_eq!(service, "http");
///     
/// ```
pub fn identify_service(port: u16) -> String {
    match port {
        21 => "ftp",
        22 => "ssh",
        23 => "telnet",
        25 => "smtp",
        53 => "dns",
        80 => "http",
        110 => "pop3",
        143 => "imap",
        443 => "https",
        445 => "smb",
        3306 => "mysql",
        3389 => "rdp",
        5432 => "postgres",
        5900 => "vnc",
        8080 => "http-alt",
        8443 => "https-alt",
        9200 => "elasticsearch",
        _ => "unknown",
    }
    .to_string()
}