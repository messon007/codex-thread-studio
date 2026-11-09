# Performance and size

## Measurement method

Record the exact source revision, build flags, tool versions, input shape and
cache conditions. Measure worker indexing time/peak RSS, process launch-to-content,
JavaScript first useful content, transport/serialization, DOM/layout and input
heartbeat separately. A bounded response or JS heap figure is not total process
memory; PSS and summed RSS are different quantities. Warm OS/platform/font/media
caches do not represent cold startup. Keep raw inputs, samples and screenshots
in private ignored artifacts, not one report file per run.

Native parser/highlighter benchmarks and whole-string conversions do different
work from indexed, source-aware document windows. Use equivalent valid output,
retained highlighting/escaping/source bytes, alternating order and repeated
samples; a plain-text fallback must not count as a faster highlighter. The
repository's isolated benchmark tools document their comparison protocol.

## Full and reduced grammar sample

Source `c8947e7eb3a5471780dbdadf9089252c95eaedc8`, Linux x86_64 Ubuntu 26.04.1,
rustc/Cargo 1.97.0. Identical locked release flags select only `web,syntax-full`
or `web,syntax-reduced`; a final-binary-only `-Cstrip=none` retains audit symbols.
Copies are stripped identically and packed as one executable in normalized
USTAR/gzip-9. These are MarkNoria server ELF sizes, not Studio or installer sizes.

| Bytes | Full | Reduced | Difference |
| --- | ---: | ---: | ---: |
| Unstripped ELF |222,590,648|168,439,272|54,151,376|
| Stripped ELF |212,575,512|158,505,032|54,070,480|
| Single-server tar.gz |35,518,410|31,867,075|3,651,335|
| Primary parse tables |120,846,560|78,157,346|42,689,214|

Actual `ts_parse_table` bytes were identified against compiled parser objects by
size/SHA-256: 110 full, 97 reduced, exactly the specified 13 removed. All 97 retained
fingerprints match, including Lean/Verilog. This is actual linked-table evidence,
not dependency download or cache size. See [Syntax profiles](highlighting.md#build-time-syntax-profiles)
for selection/conflict guards and the reusable binary-audit/timing commands.

The production-highlighter probe used 12 alternating fresh processes per profile,
with warm OS caches and no concurrent builds. Median milliseconds:

| Probe phase | Full | Reduced |
| --- | ---: | ---: |
| Process to ready, including stdin |1.2293|1.1482|
| First Rust highlight |29.9794|29.8383|
| Mean of 32 warm calls |0.0466|0.0467|

These timings measure the standalone highlighter probe, not server/desktop or
cold-disk startup. First/warm differences are too small to establish a reliable
speedup. macOS/Windows need their own compile/link measurements; target graphs
alone do not establish executable behavior.

## Unresolved first-use latency observations

A separate Ubuntu 26.04.1 native Wayland WebKit experiment with a fresh private
platform cache took 1,952 ms to reader content and recorded a 1,839 ms heartbeat gap.
An instrumented consumer-window comparison measured 3,753 ms launch-to-content
normally versus 1,936 ms with GStreamer plugins disabled only in the QA subprocess;
reader first-content/heartbeat remained about 98 ms. The changed startup cost was
outside that reader timing window. Cache seeding/prewarming and diagnostic plugin
switches were not shipped as fixes; the exact blocking call was not established.

Another bounded experiment opened English Notes, drained paint/timers and then
switched locale. The first Chinese update still produced roughly 398–433 ms maximum
heartbeat gaps, while the language-setting call itself was about 12 ms. Fontconfig
work appeared under forced layout, but suppressing redundant theme requests did
not resolve the gap. Later cumulative maxima do not establish repeat-switch
latency. These are recorded risks with their original scope, not current release
thresholds, a general regression verdict or proof of a production fix.

## Release packaging

An earlier consumer AppImage sample was 88,017,400 bytes and included portable
WebKitGTK/JavaScriptCore/ICU libraries. Recompressing its extracted filesystem
with XZ estimated 80,120,312 bytes with the same launcher, about 9% smaller; no
replacement artifact was released/tested. Individual uncompressed files and raw
ELF sizes are not additive compressed contributions. Required runtime libraries
cannot simply be omitted to make an installer look smaller.

Duplicate embedded JS assets offered a measured 10,118,809-byte uncompressed
opportunity in that consumer sample; compressed savings were unmeasured and D2
wrappers differed. Resource deduplication, symbol changes and compression remained
proposal-only. Those observations do not promise a 50 MB portable package and do
not authorize changes to a consumer's release configuration. Consumer integration
and packaging measurements belong to that consumer, separate from reader SDK
correctness and the explicit syntax profile implemented here.
