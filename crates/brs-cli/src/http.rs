//! Native HTTP host. Requests are values, never shell commands.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::io::Read;
pub fn request(text: &str) -> String {
    let result = (|| -> Result<Value, String> {
        let input: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let url = input["url"].as_str().ok_or("Missing URL")?;
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("Unsupported URL".into());
        }
        let timeout = std::time::Duration::from_millis(
            input["timeout"].as_u64().unwrap_or(30000).clamp(1, 60000),
        );
        let agent = ureq::AgentBuilder::new()
            .timeout(timeout)
            .redirects(10)
            .build();
        let mut request = agent.request(input["method"].as_str().unwrap_or("GET"), url);
        if let Some(headers) = input["headers"].as_object() {
            for (name, value) in headers {
                if let Some(value) = value.as_str() {
                    request = request.set(name, value);
                }
            }
        }
        let body = STANDARD
            .decode(input["body"].as_str().unwrap_or(""))
            .map_err(|e| e.to_string())?;
        let response = match request.send_bytes(&body) {
            Ok(response) => response,
            Err(ureq::Error::Status(_, response)) => response,
            Err(error) => return Err(error.to_string()),
        };
        let status = response.status();
        let mut headers = serde_json::Map::new();
        let mut header_array = Vec::new();
        for name in response.headers_names() {
            for value in response.all(&name) {
                headers.insert(name.clone(), json!(value));
                header_array.push(json!({name.clone():value}));
            }
        }
        let mut data = Vec::new();
        response
            .into_reader()
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        if data.len() > 8 * 1024 * 1024 {
            return Err("HTTP response exceeds 8 MiB".into());
        }
        Ok(
            json!({"status":status,"headers":headers,"headersArray":header_array,"body":STANDARD.encode(data),"failure":""}),
        )
    })();
    result
        .unwrap_or_else(|error| json!({"status":-7,"failure":error}))
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn native_host_preserves_http_error_body_and_headers() {
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = server.local_addr().unwrap().port();
        let response = std::thread::spawn(move || {
            let (mut socket, _) = server.accept().unwrap();
            let mut data = [0; 4096];
            let _ = socket.read(&mut data).unwrap();
            socket.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 7\r\nX-Test: yes\r\nConnection: close\r\n\r\nmissing").unwrap();
        });
        let value: Value = serde_json::from_str(&request(
            &json!({"url":format!("http://127.0.0.1:{port}/"),"method":"GET"}).to_string(),
        ))
        .unwrap();
        assert_eq!(value["status"], 404);
        assert_eq!(value["headers"]["x-test"], "yes");
        assert_eq!(
            STANDARD.decode(value["body"].as_str().unwrap()).unwrap(),
            b"missing"
        );
        response.join().unwrap();
    }
}
