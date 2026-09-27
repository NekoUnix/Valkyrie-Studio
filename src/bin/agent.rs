use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    env, fs,
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpStream},
    time::Duration,
};
use valkyrie_studio::app_paths;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--help") {
        println!("Usage: valkyrie-agent --check | --connect | JSON_COMMAND");
        return Ok(());
    }
    let port: u16 = env::var("L2D_API_PORT")
        .unwrap_or_else(|_| "4141".into())
        .parse()?;
    let root = app_paths::data_dir()?;
    let token_file = root.join("tmp").join(if port == 4141 {
        "api-token".into()
    } else {
        format!("api-token-{port}")
    });
    let token = match env::var("L2D_API_TOKEN") {
        Ok(value) if !value.is_empty() => value,
        _ => fs::read_to_string(&token_file)
            .with_context(|| format!("Cannot read session token at {}", token_file.display()))?
            .trim()
            .to_owned(),
    };
    let connect = args.first().is_some_and(|a| a == "--connect");
    let check = args.first().is_some_and(|a| a == "--check");
    let command: Value = if connect || check || args.is_empty() {
        json!({"op":"status"})
    } else {
        serde_json::from_str(&args[0])?
    };
    if !command.is_object() {
        bail!("Command must be a JSON object")
    }
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut socket = TcpStream::connect_timeout(&addr, Duration::from_secs(10))?;
    socket.set_read_timeout(Some(Duration::from_secs(10)))?;
    socket.set_write_timeout(Some(Duration::from_secs(10)))?;
    if connect {
        send(&mut socket, &token, json!({"op":"mode","mode":"agent"}))?;
    }
    let response = send(&mut socket, &token, command)?;
    if connect || check {
        let result = &response["result"];
        let ready = result["agent"]["ready"].as_bool().unwrap_or(false);
        println!(
            "Valkyrie Studio: {} | mode: {} | agent: {} | commands: {}",
            result["model"].as_str().unwrap_or("none"),
            result["mode"].as_str().unwrap_or("unknown"),
            if ready { "READY" } else { "NOT READY" },
            result["agent"]["commands"].as_u64().unwrap_or(0)
        );
        if !ready {
            println!("Load a model and run valkyrie-agent --connect to enable agent control.");
        }
    } else {
        println!("{}", response);
    }
    Ok(())
}

fn send(socket: &mut TcpStream, token: &str, mut command: Value) -> Result<Value> {
    let object = command
        .as_object_mut()
        .context("Command must be an object")?;
    object.insert("token".into(), Value::String(token.into()));
    object.insert(
        "id".into(),
        Value::String(format!("rust-{}", std::process::id())),
    );
    let line = serde_json::to_vec(&command)?;
    socket.write_all(&line)?;
    socket.write_all(b"\n")?;
    let mut response = String::new();
    BufReader::new(socket).read_line(&mut response)?;
    if response.is_empty() {
        bail!("Studio closed the connection without a response")
    }
    let response: Value = serde_json::from_str(&response)?;
    if response["ok"] != true {
        bail!(
            "{}",
            response["error"].as_str().unwrap_or("Agent request failed")
        )
    }
    Ok(response)
}
