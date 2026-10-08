use std::io::Read;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() == 1 && matches!(args[0].as_str(), "--help" | "-h" | "help") {
        println!(
            "Usage: kajitsu [ipc call <target> [arguments...]]\n\nNo arguments starts the Mango shell."
        );
        return Ok(());
    }
    if args.len() < 3 || args[0] != "ipc" || args[1] != "call" {
        return Err("expected: kajitsu ipc call <target> [arguments...]; see --help".into());
    }
    if std::env::var_os("XDG_RUNTIME_DIR").is_none() {
        return Err("XDG_RUNTIME_DIR is not set; run IPC in the shell's Wayland session".into());
    }
    let reply = call(&amane::ipc_socket(), &args[2], &args[3..])?;
    println!("{reply}");
    Ok(())
}

fn call(socket: &Path, target: &str, args: &[String]) -> Result<String, String> {
    if target.is_empty()
        || target.contains(['\n', '\r'])
        || args.iter().any(|arg| arg.contains(['\n', '\r']))
    {
        return Err("IPC target and arguments must each fit on one line".into());
    }
    let send = || -> std::io::Result<String> {
        let mut stream = UnixStream::connect(socket)?;
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;
        stream.set_write_timeout(Some(Duration::from_secs(3)))?;
        amane::IpcCall::new(target, args).write(&mut stream)?;
        stream.shutdown(Shutdown::Write)?;
        let mut reply = String::new();
        stream.read_to_string(&mut reply)?;
        Ok(reply)
    };
    send().map_err(|error| format!("IPC at {} failed: {error}", socket.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    #[test]
    fn closes_writer_and_reads_reply_with_exact_argument_framing() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("kajitsu-ipc-{}-{nonce}.sock", std::process::id()));
        let listener = UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut received = String::new();
            stream.read_to_string(&mut received).unwrap();
            assert_eq!(received, "launcher\nopen\na session name\n");
            stream.write_all(b"opened").unwrap();
        });
        assert_eq!(
            call(&path, "launcher", &["open".into(), "a session name".into()]).unwrap(),
            "opened"
        );
        server.join().unwrap();
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn rejects_unknown_commands_and_missing_sockets() {
        assert!(run(&["unknown".into()]).is_err());
        assert!(call(Path::new("/nonexistent/kajitsu.sock"), "launcher", &[]).is_err());
    }
}
