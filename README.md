# dora-cat

Dataflow-oriented robotic architecture built on [comp-cat-rs](https://crates.io/crates/comp-cat-rs) and [tarpc-cat](https://crates.io/crates/tarpc-cat).

Reimagines the core abstractions of [dora-rs](https://github.com/dora-rs/dora) using `Io<E, A>` as the effect type, blocking I/O via `Io::suspend`, and thread-based concurrency.  A dataflow graph is a free category; the runtime interprets it by routing data between nodes via tarpc-cat RPC.

## Features

- **Free category graph model** -- `DataflowGraph` implements `comp_cat_rs::collapse::free_category::Graph`.  Nodes are vertices, connections are edges, data flow paths are morphisms.  Use `TracingMorphism` + `interpret` for validation and tracing.
- **Immutable channel routing** -- the `RoutingTable` maps `(source_node, output_port)` to downstream `mpsc::Sender`s.  All senders are `Clone`; no mutable arrays, no `Arc`, no `Mutex`.
- **tarpc-cat RPC** -- output routing goes through a tarpc-cat server.  The `RuntimeService` implements `tarpc_cat::serve::Serve` with an immutable routing table.
- **Coordinator fold** -- shutdown propagation is a `try_fold` over done signals with a `HashSet<NodeId>` accumulator.  When all upstream producers finish, downstream nodes receive `Event::Stop`.
- **NodeHandle** -- each node owns its `mpsc::Receiver<Event>` directly.  Blocking `next_event_blocking`, `send_output_blocking`, and `done_blocking` methods operate at the effectful boundary.
- **No async, no tokio** -- pure `std::net` + `comp-cat-rs` effects + `tarpc-cat` RPC.

## Usage

### Define a dataflow graph

```rust
use dora_cat::graph::*;

let graph = DataflowGraphBuilder::new()
    .add_node(NodeDescriptor::new(
        NodeId::new(0), "source".into(),
        vec![],
        vec![OutputPort::new("out".into())],
    ))?
    .add_node(NodeDescriptor::new(
        NodeId::new(1), "sink".into(),
        vec![InputPort::new("in".into())],
        vec![],
    ))?
    .add_connection(Connection::new(
        NodeId::new(0), OutputPort::new("out".into()),
        NodeId::new(1), InputPort::new("in".into()),
    ))
    .build()?;
```

### Run locally

```rust
use dora_cat::runtime::{RuntimeConfig, execute_local};
use dora_cat::event::Event;
use comp_cat_rs::effect::io::Io;

let config = RuntimeConfig::new(
    tarpc_cat::server::ListenAddr::new("127.0.0.1:0".parse()?),
);

execute_local(graph, config, |node_id, handle| {
    Io::suspend(move || {
        handle.events().try_for_each(|result| {
            result.and_then(|event| {
                // process event, send outputs
                Ok(())
            })
        })
    })
}).run()?;
```

### Trace data flow paths

```rust
use dora_cat::graph::trace_path;
use comp_cat_rs::collapse::free_category::{Edge, Path};

let path = Path::singleton(&graph, Edge::new(0))?;
let description = trace_path(&graph, &path);
// "source:out -> sink:in"
```

## Architecture

```text
graph.rs         DataflowGraph + builder, Graph trait impl, TracingMorphism
event.rs         Event, RuntimeRequest/RuntimeResponse wire types
dispatcher.rs    Immutable RoutingTable, wire(), coordinator fold
service.rs       RuntimeService (tarpc-cat Serve) with immutable routing
node.rs          NodeHandle with blocking event/output/done methods
runtime.rs       execute_local(): wire channels, start server, spawn nodes
error.rs         Error enum with hand-rolled Display/Error/From
```

## Building

```bash
cargo build
```

## Testing

```bash
cargo test
```

## Linting

```bash
RUSTFLAGS="-D warnings" cargo clippy
```

## Documentation

```bash
cargo doc --no-deps --open
```

Docs are auto-published to GitHub Pages on push to `main` via `.github/workflows/docs.yml`.  Enable in repo Settings > Pages > Source > GitHub Actions.

## License

Licensed under either of

- [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
- [MIT License](http://opensource.org/licenses/MIT)

at your option.
