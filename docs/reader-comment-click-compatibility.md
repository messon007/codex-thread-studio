# Retired comment-click compatibility asset

The temporary MarkNoria #39 derivation was retired when Studio adopted the
fixed SDK `18d122e5ea37cfc754b2d79b58d91334176c87e3`. At adoption, both global and authorized
document asset routes served the unmodified public SDK. The generator, helper,
fixed-hash derivation tests and patch file were removed together.

See [the adoption report](marknoria-source-passage-adoption.md) for the retained
behavioral regressions and validation. Historical provenance remains in Git.

The later user-requested [direct-edit policy](reader-comment-direct-edit.md) is
a separate, scoped adapter tracked in #88 / MarkNoria #42; it does not reintroduce
the retired click-fix derivation.
