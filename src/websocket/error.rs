//! Errors of the QFEX WebSocket clients.

use nautilus_network::error::SendError;
use thiserror::Error;

/// A WebSocket client error.
#[derive(Debug, Clone, Error)]
pub enum QfexWsClientError {
    #[error("network error: {0}")]
    Network(String),
    #[error("transport error: {0}")]
    Transport(#[from] SendError),
    #[error("client error: {0}")]
    Client(String),
}
