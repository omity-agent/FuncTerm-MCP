use crate::support::{TestGuard, locked_with_env};
use core::time::Duration;
use interprocess::local_socket::{GenericNamespaced, tokio::prelude::*};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
#[derive(Serialize)]
enum ProbeRequest {
    Ping,
}
#[derive(Debug, Deserialize, PartialEq, Eq)]
enum ProbeResponse {
    Ok { payload: ProbePayload },
}
#[derive(Debug, Deserialize, PartialEq, Eq)]
enum ProbePayload {
    Pong,
}
#[tokio::test]
async fn ipc_preserves_fragmented_and_pipelined_frames() {
    let guard = locked_with_env(&[]);
    let mut stream = connect(&guard).await;
    let body = rmp_serde::to_vec(&ProbeRequest::Ping).unwrap();
    let header = u32::try_from(body.len()).unwrap().to_be_bytes();
    for byte in header.iter().chain(body.iter()) {
        stream.write_all(core::slice::from_ref(byte)).await.unwrap();
    }
    for _ in 0_usize..3 {
        stream.write_all(&header).await.unwrap();
        stream.write_all(&body).await.unwrap();
    }
    for _ in 0_usize..4 {
        assert_pong(&mut stream).await;
    }
    drop(guard);
}
#[tokio::test]
async fn incomplete_and_oversized_ipc_frames_do_not_block_other_clients() {
    let guard = locked_with_env(&[]);
    let mut incomplete = connect(&guard).await;
    incomplete.write_all(&[0, 0]).await.unwrap();
    let mut oversized = connect(&guard).await;
    oversized.write_all(&u32::MAX.to_be_bytes()).await.unwrap();
    let mut byte = [0];
    let closed = tokio::time::timeout(Duration::from_secs(5), oversized.read(&mut byte))
        .await
        .unwrap_or_else(|error| panic!("daemon did not reject oversized IPC frame: {error}"));
    assert!(
        matches!(closed, Ok(0) | Err(_)),
        "oversized frame should close its connection"
    );
    let mut healthy = connect(&guard).await;
    let body = rmp_serde::to_vec(&ProbeRequest::Ping).unwrap();
    healthy
        .write_all(&u32::try_from(body.len()).unwrap().to_be_bytes())
        .await
        .unwrap();
    healthy.write_all(&body).await.unwrap();
    assert_pong(&mut healthy).await;
    drop(guard);
}
async fn connect(guard: &TestGuard) -> LocalSocketStream {
    let environment = guard.env();
    let service = &environment
        .iter()
        .find(|pair| pair.0 == "FUNCTERM_DAEMON_SERVICE_NAME")
        .unwrap()
        .1;
    let socket = format!("functerm-ipc-{}", blake3::hash(service.as_bytes()).to_hex());
    let name = socket.as_str().to_ns_name::<GenericNamespaced>().unwrap();
    LocalSocketStream::connect(name).await.unwrap()
}
async fn assert_pong(stream: &mut LocalSocketStream) {
    let reply = async {
        let length = stream.read_u32().await.unwrap();
        let mut body = vec![0; usize::try_from(length).unwrap()];
        stream.read_exact(&mut body).await.unwrap();
        rmp_serde::from_slice::<ProbeResponse>(&body).unwrap()
    };
    let response = tokio::time::timeout(Duration::from_secs(5), reply)
        .await
        .unwrap_or_else(|error| panic!("IPC response timed out: {error}"));
    assert_eq!(
        response,
        ProbeResponse::Ok {
            payload: ProbePayload::Pong
        }
    );
}
