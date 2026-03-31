//! Events and RPC protocol types.
//!
//! Defines the [`Event`] sum type delivered to nodes and the
//! [`RuntimeRequest`] / [`RuntimeResponse`] wire protocol for
//! communication between nodes and the runtime via tarpc-cat.

use serde::{Deserialize, Serialize};

use crate::graph::{InputPort, NodeId, OutputPort};

/// An event delivered to a processing node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    /// Data arrived on an input port.
    Input {
        /// The port that received the data.
        port: InputPort,
        /// The raw data payload.
        data: Vec<u8>,
    },
    /// The runtime is requesting graceful shutdown.
    Stop,
}

/// A request from a node to the runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeRequest {
    /// Register this node with the runtime.
    Register {
        /// The node requesting registration.
        node_id: NodeId,
    },
    /// Poll for the next event (blocks until available).
    NextEvent {
        /// The node polling for events.
        node_id: NodeId,
    },
    /// Send output data on a port.
    SendOutput {
        /// The node sending output.
        node_id: NodeId,
        /// The output port.
        port: OutputPort,
        /// The raw data payload.
        data: Vec<u8>,
    },
    /// Signal that this node has finished processing.
    NodeDone {
        /// The node that is done.
        node_id: NodeId,
    },
}

/// A response from the runtime to a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeResponse {
    /// Registration acknowledged.
    Registered,
    /// The next event for the node.
    Event(Event),
    /// Output was accepted and routed to downstream nodes.
    OutputAccepted,
    /// Node shutdown acknowledged.
    DoneAcknowledged,
}
