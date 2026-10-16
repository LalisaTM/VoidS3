# Proposed improvements

Priorities reflect the large-image-batch feedback. These are proposals, not implemented features or performance promises.

| Priority | Improvement                                               | Why it matters                                                               | Evidence needed                                                             |
| -------- | --------------------------------------------------------- | ---------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| Next     | Bandwidth limits and scheduled low-impact mode            | Keep a large upload from saturating a shared connection                      | Measure sustained rate, bursts and fairness across files                    |
| Next     | Retry failed groups with backoff and error categories     | Recover batches after network outages without clicking each file             | Exercise offline recovery, throttling, expired credentials and cancellation |
| Next     | Persist discovery cursors                                 | Resume unqueued work after restart without selecting the folder again        | Handle renamed/deleted files and changed traversal safely                   |
| Next     | Per-batch ignore rules and saved include/exclude patterns | Avoid thumbnails, sidecars, build outputs or unwanted folders                | Preview which files match, including nested paths                           |
| Later    | Background destination index using paginated listings     | Reduce duplicate checks and rejected body traffic in mostly-existing folders | Bound cache size/age and retain atomic write conditions                     |
| Later    | Live concurrency changes and adaptive limits              | Apply presets without restarting and respond to throttling                   | Measure resources and provider throttling before automatic tuning           |
| Later    | Comparison-based sync with a durable baseline             | Simplify repeat photo-library uploads and changed-file detection             | Define timestamps/checksums, conflicts, deletion and restart guarantees     |
| Ongoing  | Native large-batch profiling and provider matrix          | Find UI, database, disk and network bottlenecks                              | Trace installed builds with 10,000+ real images on S3/R2 and slow disks     |

Version 0.3.0 addresses startup delays, upload queue pressure, bounded queue rendering and beginner-friendly presets first. Current limits: discovery cursors are not persisted; file/request concurrency changes need a restart; presets are validated settings rather than benchmarked speed guarantees. See [transfers.md](transfers.md) and [validation.md](validation.md).
