# Domain glossary

- **Definition**: durable editable workflow, identified by an ID and schema version.
- **AI block**: the only supported typed block; has a name, prompt and canvas position.
- **Connection**: directed edge from one block to another, establishing order, not execution.
- **Canvas**: scrollable visual editor with movable blocks and directional connections.
- **Store**: definition persistence API; host filesystem is its injected boundary.
- **Run**: future immutable execution instance; not implemented in issue #1.
- **Session token**: per-server random value required by the local API; never persisted.
