use std::fmt::Display;

use async_tungstenite::tungstenite::{error::ProtocolError, http::Response};

use crate::ConnectionId;

/// Internal errors used by Eventwork
#[derive(Debug)]
pub enum NetworkError {
    /// Error occured when accepting a new connection.
    Accept(std::io::Error),

    /// Connection couldn't be found.
    ConnectionNotFound(ConnectionId),

    /// Failed to send across channel because it was closed.
    ChannelClosed(ConnectionId),

    /// An error occured when trying to listen for connections.
    Listen(std::io::Error),

    /// An error occured when trying to connect.
    Connection(std::io::Error),

    /// Attempted to send data over a closed internal channel.
    SendError,

    /// Serialization error
    Serialization,

    ///An error in HTTP
    Http(Response<Option<Vec<u8>>>),

    /// An error occured in the Io of the connection
    Io(std::io::Error),

    /// An error in the connections protocol
    Protocol(ProtocolError),
}

impl Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Accept(e) => f.write_fmt(format_args!(
                "An error occured when accepting a new connnection: {0}",
                e
            )),
            Self::ConnectionNotFound(id) => {
                f.write_fmt(format_args!("Could not find connection with id: {0}", id))
            }
            Self::ChannelClosed(id) => {
                f.write_fmt(format_args!("Connection closed with id: {0}", id))
            }
            Self::Listen(e) => f.write_fmt(format_args!(
                "An error occured when trying to start listening for new connections: {0}",
                e
            )),
            Self::Connection(e) => f.write_fmt(format_args!(
                "An error occured when trying to connect: {0}",
                e
            )),
            Self::SendError => {
                f.write_fmt(format_args!("Attempted to send data over closed channel"))
            }
            Self::Serialization => f.write_fmt(format_args!("Failed to serialize")),
            Self::Http(err) => {
                let body = serde_json::to_string(&err.body().clone().unwrap()).unwrap();
                f.write_fmt(format_args!("{:?}: Body: {:?}", err, body))
            }
            Self::Io(err) => f.write_fmt(format_args!("{}", err)),
            Self::Protocol(err) => f.write_fmt(format_args!("Protocol Error: {}", err)),
        }
    }
}

impl From<async_tungstenite::tungstenite::Error> for NetworkError {
    fn from(value: async_tungstenite::tungstenite::Error) -> Self {
        match value {
            async_tungstenite::tungstenite::Error::ConnectionClosed => todo!(),
            async_tungstenite::tungstenite::Error::AlreadyClosed => todo!(),
            async_tungstenite::tungstenite::Error::Io(err) => Self::Io(err),
            async_tungstenite::tungstenite::Error::Tls(_) => todo!(),
            async_tungstenite::tungstenite::Error::Capacity(_) => todo!(),
            async_tungstenite::tungstenite::Error::Protocol(err) => Self::Protocol(err),
            async_tungstenite::tungstenite::Error::WriteBufferFull(_) => todo!(),
            async_tungstenite::tungstenite::Error::Utf8 => todo!(),
            async_tungstenite::tungstenite::Error::AttackAttempt => todo!(),
            async_tungstenite::tungstenite::Error::Url(url) => todo!(),
            async_tungstenite::tungstenite::Error::Http(response) => Self::Http(response),
            async_tungstenite::tungstenite::Error::HttpFormat(_) => todo!(),
        }
    }
}
