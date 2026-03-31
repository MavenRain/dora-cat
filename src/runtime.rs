//! Runtime entry points for executing a dataflow graph.
//!
//! [`execute_local`] wires up channels, starts the tarpc-cat server,
//! spawns node threads, and coordinates shutdown.  All routing state
//! is immutable; shutdown is a fold over a done-signal channel.

use std::net::TcpListener;
use std::sync::mpsc;

use comp_cat_rs::effect::io::Io;

use crate::dispatcher::{coordinate, wire};
use crate::error::Error;
use crate::graph::{DataflowGraph, NodeDescriptor, NodeId};
use crate::node::NodeHandle;
use crate::service::RuntimeService;

/// Runtime configuration.
#[derive(Debug, Clone, Copy)]
pub struct RuntimeConfig {
    listen_addr: tarpc_cat::server::ListenAddr,
}

impl RuntimeConfig {
    /// Create a new runtime configuration.
    ///
    /// Use `ListenAddr::new("127.0.0.1:0".parse().unwrap())` to
    /// let the OS pick a random port.
    #[must_use]
    pub fn new(listen_addr: tarpc_cat::server::ListenAddr) -> Self {
        Self { listen_addr }
    }

    /// The address to listen on.
    #[must_use]
    pub fn listen_addr(self) -> tarpc_cat::server::ListenAddr {
        self.listen_addr
    }
}

/// Start the runtime and spawn node logic as local threads.
///
/// 1. Wires channels for the dataflow graph.
/// 2. Starts the tarpc-cat server (for `SendOutput` routing).
/// 3. Spawns the coordinator thread (counts done signals, sends Stop).
/// 4. Spawns a thread per node, calling `spawn_node(node_id, handle)`.
/// 5. Joins all node threads.
///
/// The `spawn_node` closure receives a [`NodeId`] and a
/// [`NodeHandle`].  The handle provides blocking methods for
/// receiving events and sending outputs.  Node logic should run
/// inside [`Io::suspend`].
///
/// # Errors
///
/// Returns [`Error::Io`] on binding or spawning failures, or
/// [`Error::NodePanicked`] if a node thread panics.
///
/// # Examples
///
/// ```rust,ignore
/// execute_local(graph, config, |node_id, handle| {
///     Io::suspend(move || {
///         handle.events().try_for_each(|event| {
///             // process event, send outputs
///             Ok(())
///         })
///     })
/// })
/// ```
#[must_use]
pub fn execute_local<F>(
    graph: DataflowGraph,
    config: RuntimeConfig,
    spawn_node: F,
) -> Io<Error, ()>
where
    F: Fn(NodeId, NodeHandle) -> Io<Error, ()> + Send + Clone + 'static,
{
    Io::suspend(move || {
        // Bind to discover actual address (supports port 0).
        let listener = TcpListener::bind(config.listen_addr().addr())?;
        let actual_addr = listener.local_addr()?;
        drop(listener);

        let node_count = graph.nodes().len();
        let node_ids: Vec<NodeId> = graph.nodes().iter().map(NodeDescriptor::id).collect();

        // Wire channels and build immutable routing table.
        let wiring = wire(&graph);

        // Coordinator: folds done signals, propagates Stop downstream.
        let (done_tx, done_rx) = mpsc::channel();
        let stop_senders = wiring.stop_senders;
        std::thread::Builder::new()
            .name("dora-cat-coordinator".to_owned())
            .spawn(move || coordinate(&done_rx, &graph, node_count, &stop_senders))?;

        // Start tarpc-cat server for SendOutput routing.
        let server_listen = tarpc_cat::server::ListenAddr::new(actual_addr);
        let service = RuntimeService::new(wiring.routing_table, done_tx);
        std::thread::Builder::new()
            .name("dora-cat-server".to_owned())
            .spawn(move || {
                let _: Result<core::convert::Infallible, _> =
                    tarpc_cat::server::serve(server_listen, service).run();
            })?;

        // Brief pause for the server to bind.
        std::thread::sleep(std::time::Duration::from_millis(20));

        let server_addr = tarpc_cat::client::ServerAddr::new(actual_addr);

        // Spawn node threads.  Each gets its own receiver + RPC address.
        let handles: Result<Vec<_>, std::io::Error> = wiring
            .receivers
            .into_iter()
            .zip(node_ids)
            .map(|((node_id, event_rx), _)| {
                let node_fn = spawn_node.clone();
                let handle = NodeHandle::new(node_id, event_rx, server_addr);
                std::thread::Builder::new()
                    .name(format!("node-{}", node_id.value()))
                    .spawn(move || node_fn(node_id, handle).run())
            })
            .collect();

        // Join all node threads.
        handles?.into_iter().try_for_each(|h| {
            h.join()
                .map_err(|_| Error::NodePanicked {
                    node_id: NodeId::new(0),
                })?
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::graph::{
        Connection, DataflowGraphBuilder, InputPort, NodeDescriptor, OutputPort,
    };

    #[test]
    fn linear_pipeline_flows_data() -> Result<(), Error> {
        let graph = DataflowGraphBuilder::new()
            .add_node(NodeDescriptor::new(
                NodeId::new(0),
                "source".into(),
                vec![],
                vec![OutputPort::new("out".into())],
            ))?
            .add_node(NodeDescriptor::new(
                NodeId::new(1),
                "sink".into(),
                vec![InputPort::new("in".into())],
                vec![],
            ))?
            .add_connection(Connection::new(
                NodeId::new(0),
                OutputPort::new("out".into()),
                NodeId::new(1),
                InputPort::new("in".into()),
            ))
            .build()?;

        let config = RuntimeConfig::new(tarpc_cat::server::ListenAddr::new(
            "127.0.0.1:0".parse().map_err(|_| {
                Error::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "bad addr",
                ))
            })?,
        ));

        execute_local(graph, config, |node_id, handle| {
            Io::suspend(move || match node_id.value() {
                0 => {
                    // Source: send 3 messages then done.
                    (0u8..3).try_for_each(|i| {
                        handle.send_output_blocking(OutputPort::new("out".into()), vec![i])
                    })?;
                    handle.done_blocking()
                }
                1 => {
                    // Sink: collect until Stop.
                    let collected: Result<Vec<u8>, Error> =
                        handle.events().try_fold(Vec::new(), |acc, result| {
                            result.map(|event| match event {
                                Event::Input { data, .. } => {
                                    acc.into_iter().chain(data).collect()
                                }
                                Event::Stop => acc,
                            })
                        });
                    assert_eq!(collected?, vec![0, 1, 2]);
                    handle.done_blocking()
                }
                _ => Ok(()),
            })
        })
        .run()
    }
}
