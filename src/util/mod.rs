use std::future::Future;
use tokio::sync::mpsc;

pub trait MpscSenderExt<T> {

    fn send_or_expect(&mut self, message: T) -> impl Future<Output = ()>;

}

impl <T> MpscSenderExt<T> for mpsc::Sender<T> {
    async fn send_or_expect(&mut self, message: T) {
        self.send(message).await.expect("Failed to send message");
    }
}