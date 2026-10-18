use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Duration,
};
pub fn serve(directory: &str, port: u16) -> std::io::Result<()> {
    let root = Arc::new(fs::canonicalize(directory)?);
    if !root.join("index.html").is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Build web assets first: ./tools/build-web.sh",
        ));
    }
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    println!("KekLocator: http://127.0.0.1:{port}");
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let root = root.clone();
                std::thread::spawn(move || {
                    let _ = request(stream, &root);
                });
            }
            Err(error) => eprintln!("{error}"),
        }
    }
    Ok(())
}
fn response(
    stream: &mut TcpStream,
    status: &str,
    mime: &str,
    body: &[u8],
    head: bool,
) -> std::io::Result<()> {
    write!(stream,"HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",body.len())?;
    if !head {
        stream.write_all(body)?;
    }
    stream.flush()
}
fn request(mut stream: TcpStream, root: &Path) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut data = [0u8; 8192];
    let count = stream.read(&mut data)?;
    let request = String::from_utf8_lossy(&data[..count]);
    let mut words = request.lines().next().unwrap_or("").split_whitespace();
    let method = words.next().unwrap_or("");
    let raw = words.next().unwrap_or("/").split('?').next().unwrap_or("/");
    let head = method == "HEAD";
    if method != "GET" && !head {
        return response(
            &mut stream,
            "405 Method Not Allowed",
            "text/plain",
            b"Method not allowed",
            head,
        );
    }
    let raw = raw.trim_start_matches('/');
    let name = if raw.is_empty() { "index.html" } else { raw };
    let name = decode(name);
    let path = PathBuf::from(&name);
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return response(
            &mut stream,
            "400 Bad Request",
            "text/plain",
            b"Invalid path",
            head,
        );
    }
    let candidate = match fs::canonicalize(root.join(path)) {
        Ok(path) if path.starts_with(root) => path,
        _ => {
            return response(
                &mut stream,
                "404 Not Found",
                "text/plain",
                b"Not found",
                head,
            )
        }
    };
    let bytes = match fs::read(&candidate) {
        Ok(v) => v,
        Err(_) => {
            return response(
                &mut stream,
                "404 Not Found",
                "text/plain",
                b"Not found",
                head,
            )
        }
    };
    let mime = match candidate.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "json" => "application/json",
        "wav" => "audio/wav",
        _ => "application/octet-stream",
    };
    response(&mut stream, "200 OK", mime, &bytes, head)
}
fn decode(value: &str) -> String {
    fn hex(v: u8) -> Option<u8> {
        match v {
            b'0'..=b'9' => Some(v - b'0'),
            b'a'..=b'f' => Some(v - b'a' + 10),
            b'A'..=b'F' => Some(v - b'A' + 10),
            _ => None,
        }
    }
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(a), Some(b)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(a * 16 + b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
