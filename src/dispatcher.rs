//! Immutable routing infrastructure for the dataflow graph.
//!
//! Instead of a mutable state array, routing is handled by:
//!
//! - An immutable [`RoutingTable`] mapping `(source_node, output_port)`
//!   to downstream `(target_input_port, mpsc::Sender<Event>)` entries.
//!   All senders are `Clone`, so the table is `Clone`.
//!
//! - A coordinator that folds over done signals, propagating
//!   [`Event::Stop`] to downstream nodes as their upstream producers
//!   finish.  The fold accumulator is a `HashSet<NodeId>` of completed
//!   nodes -- no mutable arrays.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc;

use crate::event::Event;
use crate::graph::{DataflowGraph, InputPort, NodeId, OutputPort};

/// A single route: data from `(source_node, source_port)` is delivered
/// to `target_sender` as an [`Event::Input`] with `target_port`.
#[derive(Clone)]
pub struct Route {
    source_node: NodeId,
    source_port: OutputPort,
    target_port: InputPort,
    target_sender: mpsc::Sender<Event>,
}

/// Immutable routing table built from the dataflow graph.
///
/// Maps `(source_node, output_port)` to downstream delivery targets.
/// `Clone` because all `mpsc::Sender`s are `Clone`.
#[derive(Clone)]
pub struct RoutingTable {
    routes: Vec<Route>,
}

impl RoutingTable {
    /// Route output data to all downstream consumers.
    ///
    /// Finds all routes matching `(source_node, source_port)` and
    /// sends [`Event::Input`] on each target sender.
    pub fn deliver(
        &self,
        source_node: NodeId,
        source_port: &OutputPort,
        data: &[u8],
    ) {
        self.routes
            .iter()
            .filter(|r| r.source_node == source_node && r.source_port == *source_port)
            .for_each(|r| {
                let _ = r.target_sender.send(Event::Input {
                    port: r.target_port.clone(),
                    data: data.to_vec(),
                });
            });
    }
}

/// Result of wiring a dataflow graph: per-node receivers and routing table.
pub struct Wiring {
    /// Per-node event receivers, indexed by graph vertex order.
    pub receivers: Vec<(NodeId, mpsc::Receiver<Event>)>,
    /// The immutable routing table.
    pub routing_table: RoutingTable,
    /// Per-node stop senders, keyed by node id.
    pub stop_senders: HashMap<NodeId, mpsc::Sender<Event>>,
}

/// Create channels and build the routing table for a dataflow graph.
#[must_use]
pub fn wire(graph: &DataflowGraph) -> Wiring {
    let channels: Vec<(NodeId, mpsc::Sender<Event>, mpsc::Receiver<Event>)> = graph
        .nodes()
        .iter()
        .map(|n| {
            let (tx, rx) = mpsc::channel();
            (n.id(), tx, rx)
        })
        .collect();

    let sender_for: HashMap<NodeId, mpsc::Sender<Event>> = channels
        .iter()
        .map(|(id, tx, _)| (*id, tx.clone()))
        .collect();

    let routes: Vec<Route> = graph
        .connections()
        .iter()
        .filter_map(|conn| {
            sender_for.get(&conn.target_node()).map(|target_tx| Route {
                source_node: conn.source_node(),
                source_port: conn.source_port().clone(),
                target_port: conn.target_port().clone(),
                target_sender: target_tx.clone(),
            })
        })
        .collect();

    let stop_senders: HashMap<NodeId, mpsc::Sender<Event>> = channels
        .iter()
        .map(|(id, tx, _)| (*id, tx.clone()))
        .collect();

    let receivers: Vec<(NodeId, mpsc::Receiver<Event>)> = channels
        .into_iter()
        .map(|(id, _, rx)| (id, rx))
        .collect();

    Wiring {
        receivers,
        routing_table: RoutingTable { routes },
        stop_senders,
    }
}

/// Run the coordinator: fold over done signals, propagating Stop
/// to downstream nodes as their upstream producers finish.
///
/// The fold accumulator is a `HashSet<NodeId>` of completed nodes.
/// On each done signal, all nodes whose upstream producers are fully
/// done receive [`Event::Stop`].  The fold terminates when all
/// nodes have signaled done.
pub fn coordinate<S: ::std::hash::BuildHasher>(
    done_rx: &mpsc::Receiver<NodeId>,
    graph: &DataflowGraph,
    node_count: usize,
    stop_senders: &HashMap<NodeId, mpsc::Sender<Event>, S>,
) {
    done_rx
        .iter()
        .try_fold(HashSet::new(), |done_set, node_id| {
            let done_set: HashSet<NodeId> = done_set
                .into_iter()
                .chain(std::iter::once(node_id))
                .collect();

            // Propagate Stop to nodes whose upstream is fully done.
            graph.nodes().iter().for_each(|node| {
                let upstream = graph.upstream_nodes(node.id());
                let all_upstream_done = !upstream.is_empty()
                    && upstream.iter().all(|u| done_set.contains(u));
                if all_upstream_done && !done_set.contains(&node.id()) {
                    stop_senders.get(&node.id()).map(|tx| {
                        let _ = tx.send(Event::Stop);
                    });
                }
            });

            if done_set.len() >= node_count {
                Err(done_set)
            } else {
                Ok(done_set)
            }
        })
        .ok();
}
