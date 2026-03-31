//! Dataflow graph: nodes, ports, connections, and categorical structure.
//!
//! A [`DataflowGraph`] is a directed graph where vertices are processing
//! nodes and edges are data connections between output and input ports.
//! It implements [`comp_cat_rs::collapse::free_category::Graph`], making
//! the dataflow a free category that can be interpreted via
//! [`GraphMorphism`] and [`interpret`].
//!
//! [`GraphMorphism`]: comp_cat_rs::collapse::free_category::GraphMorphism
//! [`interpret`]: comp_cat_rs::collapse::free_category::interpret

use std::collections::HashMap;

use comp_cat_rs::collapse::free_category::{
    Edge, FreeCategoryError, Graph, GraphMorphism, Path, Vertex, interpret,
};
use serde::{Deserialize, Serialize};

use crate::error::Error;

// ---------------------------------------------------------------------------
// Newtypes
// ---------------------------------------------------------------------------

/// Unique identifier for a processing node in the dataflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(u64);

impl NodeId {
    /// Create a new node identifier.
    #[must_use]
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// The underlying numeric value.
    #[must_use]
    pub fn value(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeId({})", self.0)
    }
}

/// A named output port on a node.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OutputPort(String);

impl OutputPort {
    /// Create a new output port.
    #[must_use]
    pub fn new(name: String) -> Self {
        Self(name)
    }

    /// The port name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for OutputPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A named input port on a node.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InputPort(String);

impl InputPort {
    /// Create a new input port.
    #[must_use]
    pub fn new(name: String) -> Self {
        Self(name)
    }

    /// The port name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for InputPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// Descriptors
// ---------------------------------------------------------------------------

/// Describes a processing node in the dataflow graph.
pub struct NodeDescriptor {
    id: NodeId,
    name: String,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
}

impl NodeDescriptor {
    /// Create a new node descriptor.
    #[must_use]
    pub fn new(
        id: NodeId,
        name: String,
        inputs: Vec<InputPort>,
        outputs: Vec<OutputPort>,
    ) -> Self {
        Self {
            id,
            name,
            inputs,
            outputs,
        }
    }

    /// The node's identifier.
    #[must_use]
    pub fn id(&self) -> NodeId {
        self.id
    }

    /// The node's human-readable name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The node's declared input ports.
    #[must_use]
    pub fn inputs(&self) -> &[InputPort] {
        &self.inputs
    }

    /// The node's declared output ports.
    #[must_use]
    pub fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }
}

/// A connection from an output port on one node to an input port on another.
pub struct Connection {
    source_node: NodeId,
    source_port: OutputPort,
    target_node: NodeId,
    target_port: InputPort,
}

impl Connection {
    /// Create a new connection.
    #[must_use]
    pub fn new(
        source_node: NodeId,
        source_port: OutputPort,
        target_node: NodeId,
        target_port: InputPort,
    ) -> Self {
        Self {
            source_node,
            source_port,
            target_node,
            target_port,
        }
    }

    /// The source node.
    #[must_use]
    pub fn source_node(&self) -> NodeId {
        self.source_node
    }

    /// The source output port.
    #[must_use]
    pub fn source_port(&self) -> &OutputPort {
        &self.source_port
    }

    /// The target node.
    #[must_use]
    pub fn target_node(&self) -> NodeId {
        self.target_node
    }

    /// The target input port.
    #[must_use]
    pub fn target_port(&self) -> &InputPort {
        &self.target_port
    }
}

// ---------------------------------------------------------------------------
// DataflowGraph
// ---------------------------------------------------------------------------

/// An immutable dataflow graph.
///
/// Constructed via [`DataflowGraphBuilder`].  Implements the
/// [`Graph`] trait from `comp-cat-rs`, making it a free category
/// where vertices are nodes and edges are connections.
pub struct DataflowGraph {
    nodes: Vec<NodeDescriptor>,
    connections: Vec<Connection>,
    node_index: HashMap<NodeId, usize>,
}

impl DataflowGraph {
    /// The nodes in this graph.
    #[must_use]
    pub fn nodes(&self) -> &[NodeDescriptor] {
        &self.nodes
    }

    /// The connections in this graph.
    #[must_use]
    pub fn connections(&self) -> &[Connection] {
        &self.connections
    }

    /// Look up the vertex index for a [`NodeId`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::NodeNotFound`] if no node with that id exists.
    pub fn vertex_for(&self, node_id: NodeId) -> Result<Vertex, Error> {
        self.node_index
            .get(&node_id)
            .map(|&i| Vertex::new(i))
            .ok_or(Error::NodeNotFound { node_id })
    }

    /// Look up the [`NodeDescriptor`] for a [`NodeId`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::NodeNotFound`] if no node with that id exists.
    pub fn node_for(&self, node_id: NodeId) -> Result<&NodeDescriptor, Error> {
        self.node_index
            .get(&node_id)
            .and_then(|&i| self.nodes.get(i))
            .ok_or(Error::NodeNotFound { node_id })
    }

    /// All connections originating from `(node_id, port)`.
    #[must_use]
    pub fn outgoing(&self, node_id: NodeId, port: &OutputPort) -> Vec<&Connection> {
        self.connections
            .iter()
            .filter(|c| c.source_node == node_id && c.source_port == *port)
            .collect()
    }

    /// All distinct upstream producer node ids for a given node.
    ///
    /// A producer is any node that has a connection targeting this node.
    #[must_use]
    pub fn upstream_nodes(&self, node_id: NodeId) -> Vec<NodeId> {
        self.connections
            .iter()
            .filter(|c| c.target_node == node_id)
            .map(|c| c.source_node)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }
}

impl Graph for DataflowGraph {
    fn vertex_count(&self) -> usize {
        self.nodes.len()
    }

    fn edge_count(&self) -> usize {
        self.connections.len()
    }

    fn source(&self, edge: Edge) -> Result<Vertex, FreeCategoryError> {
        self.connections
            .get(edge.index())
            .and_then(|c| self.node_index.get(&c.source_node))
            .map(|&i| Vertex::new(i))
            .ok_or(FreeCategoryError::EdgeOutOfBounds {
                edge,
                count: self.connections.len(),
            })
    }

    fn target(&self, edge: Edge) -> Result<Vertex, FreeCategoryError> {
        self.connections
            .get(edge.index())
            .and_then(|c| self.node_index.get(&c.target_node))
            .map(|&i| Vertex::new(i))
            .ok_or(FreeCategoryError::EdgeOutOfBounds {
                edge,
                count: self.connections.len(),
            })
    }
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Builder for constructing a [`DataflowGraph`].
///
/// Consumes `self` on each method call (no mutation), and validates
/// on [`build`](Self::build).
pub struct DataflowGraphBuilder {
    nodes: Vec<NodeDescriptor>,
    connections: Vec<Connection>,
}

impl DataflowGraphBuilder {
    /// Create an empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            connections: Vec::new(),
        }
    }

    /// Add a node to the graph.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DuplicateNode`] if a node with the same id
    /// already exists.
    pub fn add_node(self, descriptor: NodeDescriptor) -> Result<Self, Error> {
        let id = descriptor.id();
        if self.nodes.iter().any(|n| n.id() == id) {
            Err(Error::DuplicateNode { node_id: id })
        } else {
            let nodes = self
                .nodes
                .into_iter()
                .chain(std::iter::once(descriptor))
                .collect();
            Ok(Self {
                nodes,
                connections: self.connections,
            })
        }
    }

    /// Add a connection to the graph.
    #[must_use]
    pub fn add_connection(self, connection: Connection) -> Self {
        let connections = self
            .connections
            .into_iter()
            .chain(std::iter::once(connection))
            .collect();
        Self {
            nodes: self.nodes,
            connections,
        }
    }

    /// Validate and build the immutable [`DataflowGraph`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::NodeNotFound`] if a connection references
    /// a node not in the graph.
    pub fn build(self) -> Result<DataflowGraph, Error> {
        let node_index: HashMap<NodeId, usize> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id(), i))
            .collect();

        // Validate all connections reference existing nodes
        self.connections.iter().try_for_each(|c| {
            node_index
                .get(&c.source_node)
                .ok_or(Error::NodeNotFound {
                    node_id: c.source_node,
                })
                .and_then(|_| {
                    node_index
                        .get(&c.target_node)
                        .ok_or(Error::NodeNotFound {
                            node_id: c.target_node,
                        })
                })
                .map(|_| ())
        })?;

        Ok(DataflowGraph {
            nodes: self.nodes,
            connections: self.connections,
            node_index,
        })
    }
}

impl Default for DataflowGraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tracing morphism
// ---------------------------------------------------------------------------

/// A [`GraphMorphism`] that produces human-readable descriptions.
///
/// Maps vertices to node names and edges to connection descriptions.
/// Useful for validation, logging, and debugging via [`interpret`].
pub struct TracingMorphism<'a> {
    graph: &'a DataflowGraph,
}

impl<'a> TracingMorphism<'a> {
    /// Create a tracing morphism for the given graph.
    #[must_use]
    pub fn new(graph: &'a DataflowGraph) -> Self {
        Self { graph }
    }
}

impl GraphMorphism<DataflowGraph> for TracingMorphism<'_> {
    type Object = String;
    type Morphism = String;

    fn map_vertex(&self, v: Vertex) -> String {
        self.graph
            .nodes
            .get(v.index())
            .map_or_else(|| format!("unknown({v:?})"), |n| format!("{}({})", n.name(), n.id()))
    }

    fn map_edge(&self, e: Edge) -> String {
        self.graph
            .connections
            .get(e.index())
            .map_or_else(
                || format!("unknown({e:?})"),
                |c| {
                    let src_name = self.graph.node_for(c.source_node).map_or("?", NodeDescriptor::name);
                    let tgt_name = self.graph.node_for(c.target_node).map_or("?", NodeDescriptor::name);
                    format!("{src_name}:{} -> {tgt_name}:{}", c.source_port, c.target_port)
                },
            )
    }
}

/// Trace a data flow path through the graph, returning a human-readable
/// description of each hop.
#[must_use]
pub fn trace_path(graph: &DataflowGraph, path: &Path) -> String {
    let morphism = TracingMorphism::new(graph);
    interpret::<DataflowGraph, _>(
        &morphism,
        path,
        |obj| format!("[identity at {obj}]"),
        |a, b| format!("{a} >> {b}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_graph() -> Result<DataflowGraph, Error> {
        DataflowGraphBuilder::new()
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
            .build()
    }

    #[test]
    fn graph_has_correct_counts() -> Result<(), Error> {
        let graph = simple_graph()?;
        assert_eq!(graph.vertex_count(), 2);
        assert_eq!(graph.edge_count(), 1);
        Ok(())
    }

    #[test]
    fn graph_source_target_correct() -> Result<(), Error> {
        let graph = simple_graph()?;
        let src = graph.source(Edge::new(0))?;
        let tgt = graph.target(Edge::new(0))?;
        assert_eq!(src, Vertex::new(0));
        assert_eq!(tgt, Vertex::new(1));
        Ok(())
    }

    #[test]
    fn duplicate_node_rejected() {
        let result = DataflowGraphBuilder::new()
            .add_node(NodeDescriptor::new(
                NodeId::new(0),
                "a".into(),
                vec![],
                vec![],
            ))
            .and_then(|b| {
                b.add_node(NodeDescriptor::new(
                    NodeId::new(0),
                    "b".into(),
                    vec![],
                    vec![],
                ))
            });
        assert!(matches!(result, Err(Error::DuplicateNode { .. })));
    }

    #[test]
    fn trace_path_describes_flow() -> Result<(), Error> {
        let graph = simple_graph()?;
        let path = Path::singleton(&graph, Edge::new(0))?;
        let description = trace_path(&graph, &path);
        assert!(description.contains("source"));
        assert!(description.contains("sink"));
        Ok(())
    }
}
