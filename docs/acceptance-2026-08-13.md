# 2026-08-13 feature acceptance

This pass used an isolated temporary Git repository and isolated Studio config/data/cache
directories. The screenshots were captured from the real Linux debug application through
the Studio WebView snapshot path, not from a static mock.

## Results

| Area | Result | Evidence |
| --- | --- | --- |
| Git Review | Passed | [Unified Markdown diff](design/acceptance-2026-08-13/git-review-final.png) shows staged, unstaged, overlapping, and untracked changes with per-file scope badges. Real `Stage file` and `Unstage` requests were also exercised; unstaging preserved the working-tree content. |
| PDF | Passed | [PDF search](design/acceptance-2026-08-13/pdf-reader-final.png) shows the local PDF reader, a 43-match search, and page highlights. Text/region anchor serialization is covered by the PDF comment-provider tests. |
| CSV grid | Passed | [CSV grid](design/acceptance-2026-08-13/csv-grid-final.png) renders the fixture's five rows and three columns without a trailing phantom row. |
| CSV chart | Passed | [CSV chart](design/acceptance-2026-08-13/csv-chart-final.png) renders numeric data locally in the artifact rail. |
| XLSX grid | Passed | [XLSX grid](design/acceptance-2026-08-13/xlsx-grid-final.png) loads the selected worksheet through the locally bundled parser. |
| Project environment | Passed | [Environment profile](design/acceptance-2026-08-13/environment-profile-final.png) shows ordinary variables, redacted Secret names, enabled network policy, allowed hosts, and cache variables after save/reopen. The Unix environment store mode was verified as `0600`. |

## Safety boundaries exercised

- Git commands use literal, root-confined path arguments and never pass a repository path
  through a shell. A real-repository integration test covers spaces, leading dashes,
  staged-plus-unstaged overlap, and an unborn branch.
- Review exposes only stage and unstage mutations. It has no discard/revert operation.
- PDF and spreadsheet parsing remain local and bounded; no CDN or remote document parser is
  used.
- Stored Secret values are not returned to the WebView and were not included in acceptance
  output.

Windows behavior was not cross-compiled or manually accepted in this pass, per the requested
scope. Native Windows release packaging remains a CI/release-machine responsibility.
