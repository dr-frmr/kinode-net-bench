use kinode_process_lib::{println, *};
use serde::{Deserialize, Serialize};

wit_bindgen::generate!({
    path: "wit",
    world: "process",
});

#[derive(Debug, Serialize, Deserialize)]
pub enum UserReq {
    /// Test the connection speed with a given Node ID
    Speedtest(NodeId),
    /// Do a bandwidth test with a given Node ID and given params
    BandwidthTest {
        node_id: NodeId,
        message_bytes: usize,
        message_count: usize,
    },
}

call_init!(init);

fn init(our: Address) {
    println!("{}: start", our.process.process());

    // wait for UserReq
    let Ok(Message::Request { source, body, .. }) = await_message() else {
        println!("invalid UserReq");
        return;
    };

    if source.node != our.node {
        println!("invalid source");
        return;
    }

    let req: UserReq = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            println!("invalid UserReq: {}", e);
            return;
        }
    };

    match handle_user_req(&our, &req) {
        Ok(_) => {}
        Err(e) => {
            println!("handle_user_req failed: {}", e);
        }
    }

    println!("{}: end", our.process.process());
}

fn handle_user_req(our: &Address, req: &UserReq) -> anyhow::Result<()> {
    match req {
        UserReq::Speedtest(node_id) => {
            if node_id == &our.node {
                println!("cannot speedtest with ourselves");
                return Ok(());
            }
            speedtest(our, node_id)?;
        }
        UserReq::BandwidthTest {
            node_id,
            message_bytes,
            message_count,
        } => {
            if node_id == &our.node {
                println!("cannot bandwidth test with ourselves");
                return Ok(());
            }
            bandwidth_test(our, node_id, vec![0; *message_bytes], *message_count)?;
        }
    }
    Ok(())
}

/// Speedtest attempts to measure realistic end-to-end transfer speed
/// between processes running on two separate nodes.
///
/// We first send a single message and expect a response, measuring
/// "ping time". We do this twice to ensure the second ping does not
/// include the time taken to establish a connection.
///
/// Then, we transfer a bundle of messages containing data payloads
/// to calculate bandwidth in megabytes per second.
fn speedtest(our: &Address, node_id: &NodeId) -> anyhow::Result<()> {
    println!("speedtest with {:?}", node_id);

    // ping #1
    let start = std::time::Instant::now();
    let result = Request::to(Address::new(
        node_id,
        ("receiver", our.process.package(), our.process.publisher()),
    ))
    .body(vec![])
    .send_and_await_response(60)
    .unwrap();
    match result {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("ping #1: {:?}", elapsed);
        }
        Err(e) => {
            println!("ping #1 failed: {}", e);
        }
    }

    // ping #2
    let start = std::time::Instant::now();
    let result = Request::to(Address::new(
        node_id,
        ("receiver", our.process.package(), our.process.publisher()),
    ))
    .body(vec![])
    .send_and_await_response(60)
    .unwrap();
    match result {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("ping #2: {:?}", elapsed);
        }
        Err(e) => {
            println!("ping #2 failed: {}", e);
        }
    }

    // Perform bandwidth tests with different message sizes
    bandwidth_test(
        our,
        node_id,
        "this is exactly 24 bytes".as_bytes().to_vec(),
        10_000,
    )?;
    bandwidth_test(our, node_id, vec![0; 1024], 1_000)?;
    bandwidth_test(our, node_id, vec![0; 60_000], 1_000)?;
    bandwidth_test(our, node_id, vec![0; 102_400], 100)?;
    bandwidth_test(our, node_id, vec![0; 1_048_576], 10)?;

    Ok(())
}

/// Perform a bandwidth test with a given message size and count.
fn bandwidth_test(
    our: &Address,
    node_id: &NodeId,
    message: Vec<u8>,
    message_count: usize,
) -> anyhow::Result<()> {
    let message_size = message.len();
    println!(
        "measuring bandwidth ({} bytes per message)...",
        message_size
    );
    let start = std::time::Instant::now();
    for _ in 0..message_count - 1 {
        Request::to(Address::new(
            node_id,
            ("receiver", our.process.package(), our.process.publisher()),
        ))
        .body(vec![])
        .blob_bytes(message.clone())
        .send()
        .unwrap();
    }
    Request::to(Address::new(
        node_id,
        ("receiver", our.process.package(), our.process.publisher()),
    ))
    .body(vec![])
    .blob_bytes(message.clone())
    .expects_response(60)
    .send()
    .unwrap();
    let Ok(Message::Response { .. }) = await_message() else {
        return Err(anyhow::anyhow!("bandwidth test failed"));
    };
    let elapsed = start.elapsed();
    println!("bandwidth test completed in: {:?}", elapsed);
    // calculate bandwidth in MB/s
    let bytes = message_size * message_count;
    let seconds = elapsed.as_secs_f64();
    let mbs = if message_size < 1024 {
        (bytes as f64 / seconds) / 1_000_000.0
    } else {
        (bytes as f64 / seconds) / 1_048_576.0
    };
    println!("bandwidth: {:.2} MB/s", mbs);

    Ok(())
}
