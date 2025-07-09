use std::{
    fmt,
    pin::Pin,
    task::{Context, Poll},
};

use actix_http::ws::{CloseReason, Item, Message};
use actix_web::web::Bytes;
use bytestring::ByteString;
use futures_sink::Sink;
use futures_util::SinkExt;
use tokio_util::sync::PollSender;

/// A handle into the websocket session.
///
/// This type can be used to send messages into the WebSocket.
#[derive(Clone)]
pub struct Session(PollSender<Message>);

/// The error representing a closed websocket session
#[derive(Debug)]
pub struct Closed;

impl fmt::Display for Closed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Session is closed")
    }
}

impl std::error::Error for Closed {}

impl Session {
    pub(super) fn new(inner: PollSender<Message>) -> Self {
        Session(inner)
    }

    /// Sends text into the WebSocket.
    ///
    /// ```no_run
    /// # use actix_ws::Session;
    /// # async fn test(mut session: Session) {
    /// if session.text("Some text").await.is_err() {
    ///     // session closed
    /// }
    /// # }
    /// ```
    pub async fn text(&mut self, msg: impl Into<ByteString>) -> Result<(), Closed> {
        self.send(Message::Text(msg.into()))
            .await
            .map_err(|_| Closed)
    }

    /// Sends raw bytes into the WebSocket.
    ///
    /// ```no_run
    /// # use actix_ws::Session;
    /// # async fn test(mut session: Session) {
    /// if session.binary(&b"some bytes"[..]).await.is_err() {
    ///     // session closed
    /// }
    /// # }
    /// ```
    pub async fn binary(&mut self, msg: impl Into<Bytes>) -> Result<(), Closed> {
        self.send(Message::Binary(msg.into()))
            .await
            .map_err(|_| Closed)
    }

    /// Pings the client.
    ///
    /// For many applications, it will be important to send regular pings to keep track of if the
    /// client has disconnected
    ///
    /// ```no_run
    /// # use actix_ws::Session;
    /// # async fn test(mut session: Session) {
    /// if session.ping(b"").await.is_err() {
    ///     // session is closed
    /// }
    /// # }
    /// ```
    pub async fn ping(&mut self, msg: &[u8]) -> Result<(), Closed> {
        self.send(Message::Ping(Bytes::copy_from_slice(msg)))
            .await
            .map_err(|_| Closed)
    }

    /// Pongs the client.
    ///
    /// ```no_run
    /// # use actix_ws::{Message, Session};
    /// # async fn test(mut session: Session, msg: Message) {
    /// match msg {
    ///     Message::Ping(bytes) => {
    ///         let _ = session.pong(&bytes).await;
    ///     }
    ///     _ => (),
    /// }
    /// # }
    pub async fn pong(&mut self, msg: &[u8]) -> Result<(), Closed> {
        self.send(Message::Pong(Bytes::copy_from_slice(msg)))
            .await
            .map_err(|_| Closed)
    }

    /// Manually controls sending continuations.
    ///
    /// Be wary of this method. Continuations represent multiple frames that, when combined, are
    /// presented as a single message. They are useful when the entire contents of a message are
    /// not available all at once. However, continuations MUST NOT be interrupted by other Text or
    /// Binary messages. Control messages such as Ping, Pong, or Close are allowed to interrupt a
    /// continuation.
    ///
    /// Continuations must be initialized with a First variant, and must be terminated by a Last
    /// variant, with only Continue variants sent in between.
    ///
    /// ```no_run
    /// # use actix_ws::{Item, Session};
    /// # async fn test(mut session: Session) -> Result<(), Box<dyn std::error::Error>> {
    /// session.continuation(Item::FirstText("Hello".into())).await?;
    /// session.continuation(Item::Continue(b", World"[..].into())).await?;
    /// session.continuation(Item::Last(b"!"[..].into())).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn continuation(&mut self, msg: Item) -> Result<(), Closed> {
        self.send(Message::Continuation(msg))
            .await
            .map_err(|_| Closed)
    }

    /// Sends a close message.
    ///
    /// This does not close the session itself. You MUST ensure yourself
    /// that no more messages are sent afterwards.
    ///
    /// ```no_run
    /// # use actix_ws::{Closed, Session};
    /// # async fn test(mut session: Session) -> Result<(), Closed> {
    /// session.close(None).await
    /// # }
    /// ```
    pub async fn close(&mut self, reason: Option<CloseReason>) -> Result<(), Closed> {
        self.send(Message::Close(reason)).await.map_err(|_| Closed)
    }
}

impl Sink<Message> for Session {
    type Error = Closed;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready_unpin(cx).map_err(|_| Closed)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        self.0.start_send_unpin(item).map_err(|_| Closed)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_flush_unpin(cx).map_err(|_| Closed)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_close_unpin(cx).map_err(|_| Closed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::channel;

    #[actix_web::test]
    async fn send_text_via_sink_message_is_transmitted() {
        let (tx, mut rx) = channel(32);

        let mut session = Session::new(PollSender::new(tx));

        let content = ByteString::from_static("test message");

        session.send(Message::Text(content.clone())).await.unwrap();

        assert_eq!(Some(Message::Text(content)), rx.recv().await);
    }

    #[actix_web::test]
    async fn send_text_directly_message_is_transmitted() {
        let (tx, mut rx) = channel(32);

        let mut session = Session::new(PollSender::new(tx));

        let content = ByteString::from_static("test message");

        session.text(content.clone()).await.unwrap();

        assert_eq!(Some(Message::Text(content)), rx.recv().await);
    }
}
