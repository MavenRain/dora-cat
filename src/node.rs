//! Node handle for participating in the dataflow.
//!
//! A [`NodeHandle`] provides blocking methods for receiving events
//! and sending outputs.  Events arrive via a direct `mpsc` channel
//! (no RPC round-trip); outputs are routed via tarpc-cat RPC to the
//! runtime service.
//!
//! All methods are blocking and intended for use inside
//! [`Io::suspend`](comp_cat_rs::effect::io::Io::suspend) at the
//! effectful boundary.

use std::sync::mpsc;

use crate::error::Error;
use crate::event::{Event, RuntimeRequest, RuntimeResponse};
use crate::graph::{NodeId, OutputPort};

/// A handle for a node to participate in the dataflow.
///
/// Owns the node's event receiver (direct channel, no RPC).
/// Sends outputs and done signals via tarpc-cat RPC.
pub struct NodeHandle {
    node_id: NodeId,
    event_rx: mpsc::Receiver<Event>,
    runtime_addr: tarpc_cat::client::ServerAddr,
}

impl NodeHandle {
    /// Create a new node handle.
    #[must_use]
    pub fn new(
        node_id: NodeId,
        event_rx: mpsc::Receiver<Event>,
        runtime_addr: tarpc_cat::client::ServerAddr,
    ) -> Self {
        Self {
            node_id,
            event_rx,
            runtime_addr,
        }
    }

    /// The node's identifier.
    #[must_use]
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Block until the next event arrives.
    ///
    /// Returns [`Event::Input`] for data or [`Event::Stop`] for
    /// shutdown.  Returns [`Error::ChannelClosed`] if the channel
    /// is closed unexpectedly.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ChannelClosed`] if the sender side is dropped.
    pub fn next_event_blocking(&self) -> Result<Event, Error> {
        self.event_rx.recv().map_err(|_| Error::ChannelClosed)
    }

    /// Send output data on a port.
    ///
    /// Routes the data through the runtime service to downstream
    /// consumers via tarpc-cat RPC.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Rpc`] on communication failure.
    pub fn send_output_blocking(&self, port: OutputPort, data: Vec<u8>) -> Result<(), Error> {
        tarpc_cat::client::call::<RuntimeRequest, RuntimeResponse>(
            self.runtime_addr,
            RuntimeRequest::SendOutput {
                node_id: self.node_id,
                port,
                data,
            },
        )
        .run()
        .map(|_| ())
        .map_err(Error::from)
    }

    /// Signal that this node has finished processing.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Rpc`] on communication failure.
    pub fn done_blocking(&self) -> Result<(), Error> {
        tarpc_cat::client::call::<RuntimeRequest, RuntimeResponse>(
            self.runtime_addr,
            RuntimeRequest::NodeDone {
                node_id: self.node_id,
            },
        )
        .run()
        .map(|_| ())
        .map_err(Error::from)
    }

    /// Iterate over events until [`Event::Stop`] or channel close.
    ///
    /// A convenience wrapper that yields [`Event::Input`] events
    /// and stops on [`Event::Stop`] or channel close.
    pub fn events(&self) -> impl Iterator<Item = Result<Event, Error>> + '_ {
        std::iter::from_fn(|| {
            self.event_rx
                .recv()
                .ok()
                .filter(|e| !matches!(e, Event::Stop))
                .map(Ok)
        })
    }
}
