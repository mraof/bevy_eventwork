use std::{net::SocketAddr, ops::DerefMut, pin::Pin, sync::Arc};

use crate::{error::NetworkError, managers::NetworkProvider, NetworkPacket};
use async_channel::{Receiver, Sender};
use async_std::net::{TcpListener, TcpStream};
use async_trait::async_trait;
use async_tungstenite::tungstenite::protocol::WebSocketConfig;
use bevy::prelude::{debug, error, info, trace, Deref, DerefMut, Resource};
use futures_lite::{AsyncReadExt, AsyncWriteExt, Future, FutureExt, Stream};
use futures_util::lock::Mutex;
use ws_stream_tungstenite::WsStream;

/// A provider for WebSockets
#[derive(Default, Debug)]
pub struct WebSocketProvider;

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl NetworkProvider for WebSocketProvider {
    type NetworkSettings = NetworkSettings;

    type Socket = Arc<Mutex<WsStream<TcpStream>>>;

    type ReadHalf = Arc<Mutex<WsStream<TcpStream>>>;

    type WriteHalf = Arc<Mutex<WsStream<TcpStream>>>;

    type ConnectInfo = url::Url;
    type AcceptInfo = SocketAddr;

    type AcceptStream = OwnedIncoming;

    async fn accept_loop(
        accept_info: Self::AcceptInfo,
        _: Self::NetworkSettings,
    ) -> Result<Self::AcceptStream, NetworkError> {
        let listener = TcpListener::bind(accept_info)
            .await
            .map_err(NetworkError::Listen)?;
        Ok(OwnedIncoming::new(listener))
    }

    async fn connect_task(
        connect_info: Self::ConnectInfo,
        network_settings: Self::NetworkSettings,
    ) -> Result<Self::Socket, NetworkError> {
        info!("Beginning connection");
        let (stream, response) = async_tungstenite::async_std::connect_async_with_config(
            connect_info,
            Some(*network_settings),
        )
        .await?;
        info!("Connected!");
        return Ok(Arc::new(Mutex::new(WsStream::new(stream))));
    }

    async fn recv_loop(
        read_half: Self::ReadHalf,
        messages: Sender<NetworkPacket>,
        settings: Self::NetworkSettings,
    ) {
        let mut buffer = vec![0; settings.max_message_size.unwrap_or(64 << 20)];
        loop {
            info!("Reading message length");

            let mut read_half = read_half.lock().await;
            let read_half = read_half.deref_mut();

            let length = match read_half.read(&mut buffer[..8]).await {
                Ok(0) => {
                    // EOF, meaning the TCP stream has closed.
                    info!("Client disconnected");
                    // TODO: probably want to do more than just quit the receive task.
                    //       to let eventwork know that the peer disconnected.
                    break;
                }
                Ok(8) => {
                    let bytes = &buffer[..8];
                    u64::from_le_bytes(
                        bytes
                            .try_into()
                            .expect("Couldn't read bytes from connection!"),
                    ) as usize
                }
                Ok(n) => {
                    error!(
                        "Could not read enough bytes for header. Expected 8, got {}",
                        n
                    );
                    break;
                }
                Err(err) => {
                    error!("Encountered error while fetching length: {}", err);
                    break;
                }
            };
            info!("Message length: {}", length);

            info!("Reading message into buffer");
            match read_half.read_exact(&mut buffer[..length]).await {
                Ok(()) => (),
                Err(err) => {
                    error!(
                        "Encountered error while fetching stream of length {}: {}",
                        length, err
                    );
                    break;
                }
            }
            info!("Message read");

            let packet: NetworkPacket = match bincode::deserialize(&buffer[..length]) {
                Ok(packet) => packet,
                Err(err) => {
                    error!("Failed to decode network packet from: {}", err);
                    break;
                }
            };

            if messages.send(packet).await.is_err() {
                error!("Failed to send decoded message to eventwork");
                break;
            }
            info!("Message deserialized and sent to eventwork");
        }
    }

    async fn send_loop(
        write_half: Self::WriteHalf,
        messages: Receiver<NetworkPacket>,
        _settings: Self::NetworkSettings,
    ) {
        while let Ok(message) = messages.recv().await {
            let encoded = match bincode::serialize(&message) {
                Ok(encoded) => encoded,
                Err(err) => {
                    error!("Could not encode packet {:?}: {}", message, err);
                    continue;
                }
            };

            let len = encoded.len() as u64;
            debug!("Sending a new message of size: {}", len);

            let mut write_half = write_half.lock().await;
            let write_half = write_half.deref_mut();

            match write_half.write(&len.to_le_bytes()).await {
                Ok(_) => (),
                Err(err) => {
                    error!("Could not send packet length: {:?}: {}", len, err);
                    break;
                }
            }

            trace!("Sending the content of the message!");

            match write_half.write_all(&encoded).await {
                Ok(_) => (),
                Err(err) => {
                    error!("Could not send packet: {:?}: {}", message, err);
                    break;
                }
            }

            trace!("Succesfully written all!");
        }
    }

    fn split(combined: Self::Socket) -> (Self::ReadHalf, Self::WriteHalf) {
        (combined.clone(), combined.clone())
    }
}

#[derive(Clone, Debug, Resource, Default, Deref, DerefMut)]
#[allow(missing_copy_implementations)]
/// Settings to configure the network, both client and server
pub struct NetworkSettings(WebSocketConfig);

/// A special stream for recieving ws connections
pub struct OwnedIncoming {
    inner: TcpListener,
    stream: Option<Pin<Box<dyn Future<Output = Option<Arc<Mutex<WsStream<TcpStream>>>>>>>>,
}

impl OwnedIncoming {
    fn new(listener: TcpListener) -> Self {
        Self {
            inner: listener,
            stream: None,
        }
    }
}

impl Stream for OwnedIncoming {
    type Item = Arc<Mutex<WsStream<TcpStream>>>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let incoming = self.get_mut();
        if incoming.stream.is_none() {
            let listener: *const TcpListener = &incoming.inner;
            incoming.stream = Some(Box::pin(async move {
                let stream = unsafe {
                    listener
                        .as_ref()
                        .expect("Segfault when trying to read listener in OwnedStream")
                }
                .accept()
                .await
                .map(|(s, _)| s)
                .ok();

                let stream: WsStream<TcpStream> = match stream {
                    Some(stream) => {
                        if let Some(stream) = async_tungstenite::accept_async(stream).await.ok() {
                            WsStream::new(stream)
                        } else {
                            return None;
                        }
                    }

                    None => return None,
                };
                Some(Arc::new(Mutex::new(stream)))
            }));
        }
        if let Some(stream) = &mut incoming.stream {
            if let std::task::Poll::Ready(res) = stream.poll(cx) {
                incoming.stream = None;
                return std::task::Poll::Ready(res);
            }
        }
        std::task::Poll::Pending
    }
}

unsafe impl Send for OwnedIncoming {}
