//! # dora-cat
//!
//! Dataflow-oriented robotic architecture built on
//! [`comp-cat-rs`](https://crates.io/crates/comp-cat-rs) and
//! [`tarpc-cat`](https://crates.io/crates/tarpc-cat).
//!
//! A dataflow graph is a free category: nodes are vertices, connections
//! are edges, and data flow paths are morphisms.  The runtime interprets
//! the graph by routing data between nodes via tarpc-cat RPC.
//!
//! ## Quick start
//!
//! ```rust,ignore
//! use dora_cat::graph::*;
//! use dora_cat::runtime::{RuntimeConfig, execute_local};
//! use comp_cat_rs::effect::io::Io;
//!
//! let graph = DataflowGraphBuilder::new()
//!     .add_node(NodeDescriptor::new(
//!         NodeId::new(0), "source".into(), vec![],
//!         vec![OutputPort::new("out".into())],
//!     ))?
//!     .add_node(NodeDescriptor::new(
//!         NodeId::new(1), "sink".into(),
//!         vec![InputPort::new("in".into())], vec![],
//!     ))?
//!     .add_connection(Connection::new(
//!         NodeId::new(0), OutputPort::new("out".into()),
//!         NodeId::new(1), InputPort::new("in".into()),
//!     ))
//!     .build()?;
//!
//! let config = RuntimeConfig::new(
//!     tarpc_cat::server::ListenAddr::new("127.0.0.1:0".parse()?),
//!     256,
//! );
//!
//! execute_local(graph, config, |node_id, client| {
//!     // dispatch node logic by id
//!     Io::pure(())
//! }).run()?;
//! ```

pub mod dispatcher;
pub mod error;
pub mod event;
pub mod graph;
pub mod node;
pub mod runtime;
pub mod service;
