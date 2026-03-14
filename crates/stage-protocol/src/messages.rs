use serde::{Deserialize, Serialize};

/// Top-level message type tag, used to dispatch incoming messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Message {
    /// Addon → Server: initial handshake (sent unsolicited on connect)
    #[serde(rename = "handshake")]
    Handshake(crate::handshake::Handshake),

    /// Server → Addon: handshake accepted
    #[serde(rename = "handshake_ack")]
    HandshakeAck(crate::handshake::HandshakeAck),

    /// Server → Addon: handshake rejected
    #[serde(rename = "handshake_error")]
    HandshakeError(crate::handshake::HandshakeError),

    /// Server → Addon: query request
    #[serde(rename = "query")]
    Query {
        request_id: String,
        method: String,
        #[serde(default)]
        params: serde_json::Value,
    },

    /// Addon → Server: query response
    #[serde(rename = "response")]
    Response {
        request_id: String,
        data: serde_json::Value,
    },

    /// Addon → Server: query error
    #[serde(rename = "error")]
    Error {
        request_id: String,
        code: String,
        message: String,
    },

    /// Addon → Server: push event (unsolicited)
    #[serde(rename = "event")]
    Event {
        event: String,
        #[serde(flatten)]
        data: serde_json::Value,
    },
}
