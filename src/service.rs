//! Runtime service implementing [`tarpc_cat::serve::Serve`].
//!
//! The [`RuntimeService`] holds an immutable [`RoutingTable`] and a
//! clone of the coordinator's done-signal sender.  No mutable state.

use std::sync::mpsc;

use comp_cat_rs::effect::io::Io;
use tarpc_cat::serve::Serve;

use crate::dispatcher::RoutingTable;
use crate::event::{RuntimeRequest, RuntimeResponse};
use crate::graph::NodeId;

/// The tarpc-cat service for the dora-cat runtime.
///
/// All fields are `Clone`:
/// - [`RoutingTable`] contains `mpsc::Sender`s (Clone).
/// - `done_tx` is an `mpsc::Sender` (Clone).
#[derive(Clone)]
pub struct RuntimeService {
    routing_table: RoutingTable,
    done_tx: mpsc::Sender<NodeId>,
}

impl RuntimeService {
    /// Create a new service.
    #[must_use]
    pub fn new(routing_table: RoutingTable, done_tx: mpsc::Sender<NodeId>) -> Self {
        Self {
            routing_table,
            done_tx,
        }
    }
}

impl Serve for RuntimeService {
    type Request = RuntimeRequest;
    type Response = RuntimeResponse;

    fn handle(&self, request: RuntimeRequest) -> Io<tarpc_cat::error::Error, RuntimeResponse> {
        let routing_table = self.routing_table.clone();
        let done_tx = self.done_tx.clone();
        Io::suspend(move || match request {
            RuntimeRequest::Register { .. } => Ok(RuntimeResponse::Registered),
            RuntimeRequest::NextEvent { .. } => Err(tarpc_cat::error::Error::Server {
                message: "NextEvent not supported via RPC; use NodeHandle directly".to_owned(),
            }),
            RuntimeRequest::SendOutput {
                node_id,
                port,
                data,
            } => {
                routing_table.deliver(node_id, &port, &data);
                Ok(RuntimeResponse::OutputAccepted)
            }
            RuntimeRequest::NodeDone { node_id } => {
                done_tx
                    .send(node_id)
                    .map_err(|_| tarpc_cat::error::Error::Server {
                        message: "coordinator channel closed".to_owned(),
                    })?;
                Ok(RuntimeResponse::DoneAcknowledged)
            }
        })
    }
}
