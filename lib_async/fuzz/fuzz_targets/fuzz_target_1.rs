#![no_main]
#[macro_use] extern crate libfuzzer_sys;
extern crate lib_async;

fuzz_target!(|data: &[u8]| {
    let mut connection = lib_async::connection::Connection::new(
        lib_async::ConfigBuilder::default().with_max_size(1024*1024).build(),
        Box::new(Handler)
    );

    let mut bytes = bytes::BytesMut::new();
    for data in data.split(|b| b == &b'\n') {
        bytes.extend_from_slice(data);

        let _result = connection.data_received(
            &mut bytes,
            true
        );
    }
});

struct Handler;

#[async_trait::async_trait]
impl lib_async::Handler for Handler {
    async fn validate_address(&self, _: &str) -> bool { true } 
    async fn save_email<'a>(&self, _: &lib_async::Email<'a>) -> Result<(), String> { Ok(()) } 
    fn clone_box(&self) -> Box<dyn lib_async::Handler> { Box::new(Handler) }
}

