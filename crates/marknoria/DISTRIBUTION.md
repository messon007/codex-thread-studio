# Public reader package policy

On 2026-11-05 the owner authorized the first-party code in the fixed shared Web
reader delivery allowlist to be distributed under the accompanying MIT license.
The MarkNoria repository remains private. This is not a blanket license or
publication of files outside that allowlist.

The fixed export manifest identifies the precise commit and allowed files.
Third-party code/assets retain their original licenses, notices and provenance;
the first-party MIT notice does not replace them. Native UI/FFI, private documents,
generated fixtures, screenshots, caches, credentials, active working trees and
unreviewed build output are excluded from the public package.

Build exported resources from the same clean source revision and lockfiles.
Do not fetch a floating main/latest revision during ordinary downstream builds.
