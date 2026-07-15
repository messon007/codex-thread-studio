# Codex App Server integration

Codex Thread Studio is a terminal-free App Server client.

Implemented protocol surface:

- initialization owned by the Rust broker;
- `thread/list`, `thread/start`, `thread/read`, `thread/resume`, `thread/fork`, `thread/name/set`, `thread/archive`, and `thread/delete`;
- `turn/start`, `turn/steer`, and `turn/interrupt`;
- structured Turn/Item lifecycle and common delta notifications;
- command, file-change, and permission approval responses;
- Turn/Item-anchored text comments and prompt assembly.

The implementation intentionally uses the stable stdio JSONL transport rather than App Server's experimental WebSocket listener. See [Architecture](architecture.md) and [Product specification](product.md) for boundaries and acceptance criteria.
