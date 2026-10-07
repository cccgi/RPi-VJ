pub fn send(message: &str) {
    #[cfg(target_os = "linux")]
    {
        use std::os::{
            linux::net::SocketAddrExt,
            unix::{
                ffi::OsStrExt,
                net::{SocketAddr, UnixDatagram},
            },
        };
        if let Some(path) = std::env::var_os("NOTIFY_SOCKET") {
            let bytes = path.as_bytes();
            let address = if bytes.first() == Some(&b'@') {
                SocketAddr::from_abstract_name(&bytes[1..])
            } else {
                SocketAddr::from_pathname(path)
            };
            if let (Ok(socket), Ok(address)) = (UnixDatagram::unbound(), address) {
                if socket.set_nonblocking(true).is_ok() {
                    let _ = socket.send_to_addr(message.as_bytes(), &address);
                }
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = message;
}
