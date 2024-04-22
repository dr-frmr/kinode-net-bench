use kinode_process_lib::{println, *};

wit_bindgen::generate!({
    path: "wit",
    world: "process",
});

call_init!(init);

fn init(our: Address) {
    println!("{}: start", our.process.process());

    // all that receiver does is handle incoming messages
    // by silently accepting them if they do not expect a response,
    // or responding with an empty response if they do.
    loop {
        let Ok(Message::Request {
            expects_response, ..
        }) = await_message()
        else {
            continue;
        };
        if expects_response.is_some() {
            Response::new().body(vec![]).send().unwrap();
        }
    }
}
