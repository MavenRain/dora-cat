//! Project-wide error type.

use crate::graph::{NodeId, OutputPort};

/// All errors in dora-cat.
#[derive(Debug)]
pub enum Error {
    /// Network or file I/O error.
    Io(std::io::Error),
    /// RPC communication error with the runtime.
    Rpc(tarpc_cat::error::Error),
    /// Free category operation error (graph structure).
    FreeCategory(comp_cat_rs::collapse::free_category::FreeCategoryError),
    /// A node with this identifier already exists in the graph.
    DuplicateNode {
        /// The duplicate node identifier.
        node_id: NodeId,
    },
    /// No node with this identifier exists in the graph.
    NodeNotFound {
        /// The missing node identifier.
        node_id: NodeId,
    },
    /// No output port with this name exists on the node.
    PortNotFound {
        /// The node that was searched.
        node_id: NodeId,
        /// The missing port.
        port: OutputPort,
    },
    /// An internal channel was closed unexpectedly.
    ChannelClosed,
    /// The runtime is shutting down.
    Shutdown,
    /// The node's event queue is full (backpressure).
    QueueFull {
        /// The node whose queue is full.
        node_id: NodeId,
    },
    /// A node thread panicked.
    NodePanicked {
        /// The node that panicked.
        node_id: NodeId,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Rpc(e) => write!(f, "RPC error: {e}"),
            Self::FreeCategory(e) => write!(f, "graph error: {e}"),
            Self::DuplicateNode { node_id } => write!(f, "duplicate node: {node_id}"),
            Self::NodeNotFound { node_id } => write!(f, "node not found: {node_id}"),
            Self::PortNotFound { node_id, port } => {
                write!(f, "port {port} not found on node {node_id}")
            }
            Self::ChannelClosed => write!(f, "channel closed"),
            Self::Shutdown => write!(f, "runtime shutdown"),
            Self::QueueFull { node_id } => write!(f, "queue full for node {node_id}"),
            Self::NodePanicked { node_id } => write!(f, "node {node_id} panicked"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Rpc(e) => Some(e),
            Self::FreeCategory(e) => Some(e),
            Self::DuplicateNode { .. }
            | Self::NodeNotFound { .. }
            | Self::PortNotFound { .. }
            | Self::ChannelClosed
            | Self::Shutdown
            | Self::QueueFull { .. }
            | Self::NodePanicked { .. } => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<tarpc_cat::error::Error> for Error {
    fn from(e: tarpc_cat::error::Error) -> Self {
        Self::Rpc(e)
    }
}

impl From<comp_cat_rs::collapse::free_category::FreeCategoryError> for Error {
    fn from(e: comp_cat_rs::collapse::free_category::FreeCategoryError) -> Self {
        Self::FreeCategory(e)
    }
}

/// Convert a dora-cat error into a tarpc-cat server error for RPC responses.
impl From<Error> for tarpc_cat::error::Error {
    fn from(e: Error) -> Self {
        tarpc_cat::error::Error::Server {
            message: e.to_string(),
        }
    }
}
