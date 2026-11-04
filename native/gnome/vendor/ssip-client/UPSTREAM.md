# Speech Dispatcher client

The native Linux frontend uses the unmodified `ssip-client` 0.10.2 crate through
Cargo. This directory contains its MIT license notice, not a source fork.

Source: https://gitlab.com/lp-accessibility/ssip-client
Package: https://crates.io/crates/ssip-client/0.10.2
Package SHA-256: `5349534527378e1e24486a9b361127c0a044b0311857f778ec1ec2953c5ffbbf`

The client supplies SSIP commands and reply/notification parsing. Studio wraps
literal, dot-stuffed text, bounded worker admission and connection-scoped Stop.
No speech engine, voice model, C binding generator or Python runtime is bundled.
