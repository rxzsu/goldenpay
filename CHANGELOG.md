# Changelog

## 0.5.0

### Breaking changes
- **remove `SessionManager::fetch_all_orders`** — the method was a misleading alias for `fetch_orders` (FunPay's trade page does not support traditional pagination). Call `fetch_orders` (and apply `FetchOrderOptions` filters) directly.

### Bug fixes
- remove duplicate doc comment on `GoldenPayError`
- add missing `#[must_use]` to `FetchOrderOptions::subcategory`

### Features
- `FetchOrderOptions` — new `buyer` and `description` text filters (case-insensitive contains match) with builder methods `.buyer()` / `.description()`
- **Batch operations** — `GoldenPaySession::send_messages()` and `GoldenPaySession::fetch_orders_batch()` send messages and fetch order pages concurrently via `JoinSet`
- `SessionManager::send_messages()` and `SessionManager::fetch_orders_batch()` — same batch APIs with auto-reconnect on auth error
- **Example** — `examples/batch_send.rs` shows how to combine `FetchOrderOptions` filters with concurrent `send_messages` and `fetch_orders_batch`

### Refactoring
- `GoldenPayBot::poll_once` now uses `EventStream::should_emit_order` instead of inlining `seen_orders.insert`

### Tests
- 8 new unit tests for `FetchOrderOptions` (filters, case-insensitivity, combined filters)
- 4 new unit tests for `parse_user` (Unauthorized detection paths, `phpsessid` extraction)
- 5 new unit tests for `GoldenPayConfig` / `RetryPolicy` (defaults, debug redaction, chaining)

## 0.4.0

### Performance
- move 10 regexes from per-call `Regex::new` to `std::sync::LazyLock` statics for significant CPU savings on hot paths
- replace sequential chat-message fetching in `bootstrap()` / `poll_once()` with `JoinSet` + `Semaphore` (configurable concurrency, default 5)
- update `scraper` from 0.26.0 to 0.27.0

### Bot reliability
- **SessionManager** — new wrapper around `GoldenPaySession` with automatic reconnection on HTTP 401/403; all 15 API methods retry once after reconnecting
- **GoldenPayBot::connect()** — new async constructor that creates a `SessionManager` internally; the bot survives session expiry transparently
- **Graceful shutdown** — add `CancellationToken` support (`.cancel()`, `.with_cancellation_token()`), `listen_for_shutdown()` spawns a Ctrl+C listener
- **Throttling** — add `.with_concurrency_limit(n)` builder (default 5) to prevent API abuse
- **Tracing** — add `tracing::info!`/`debug!`/`warn!` logs in `load_state`, `bootstrap`, `poll_once`, `run`, `connect`, `request_with_retry`, and session reconnect
- **Session expired detection** — `connect()` returns `Unauthorized` when the golden key is invalid, fixing silent failures

### Security
- **Debug redact** — manual `fmt::Debug` on `GoldenPayConfig` masks `golden_key`, `UserInfo` masks `csrf_token` and `phpsessid`; credentials no longer leak into logs

### API improvements
- **FetchOrderOptions** — filter orders by `status`, `min_amount`, `max_amount`, `subcategory` with builder API; `fetch_orders_with()` on both `GoldenPaySession` and `SessionManager`
- **Category tree parser** — `parse_category_tree()` extracts the full marketplace hierarchy from `/lots/` with recursive `CategoryNode { id, name, subcategory_type, children }`; new `fetch_category_tree()` on both session types
- **Pagination helper** — `SessionManager::fetch_all_orders()` placeholder for future multi-page order fetching
- **`#[non_exhaustive]`** — added to `GoldenPayEvent`, `OrderStatus`, `OfferFieldType` for forward compatibility
- **Urls::lots_home()** — new URL helper for the marketplace root page
- **Doc comments** — added crate-level docs and doc-comments on all public types/methods/variants

### Clippy
- fixed all 112 clippy pedantic warnings (auto-fixes + crate-level `#[allow]` for opinionated lints); zero warnings on `cargo clippy -- -W clippy::pedantic`

### Example
- `poll_orders.rs` updated to use `GoldenPayBot::connect()`

### Dependencies
- bump tokio features: `rt`, `macros`, `signal`
- add `tokio-util` with `rt` feature
- add `tracing` 0.1
- bump `scraper` 0.26.0 → 0.27.0
- bump `goldenpay` 0.3.0 → 0.4.0

## 0.3.0

- switch delivery flow to a safer reserve -> send -> commit sequence with pending delivery records
- add pending/delivered delivery record states to reduce duplicate sends after partial failures
- add internal mutex protection to JSON state and delivery stores to avoid parallel write races
- keep JSON persistence atomic via temporary-file writes and rename
- fix `calc_price()` so decimal prices are not truncated before request submission
- add typed `GoldenPayError::Delivery(...)` errors instead of collapsing delivery failures into generic state errors
- surface runner and offer-save error messages through typed response structs
- refactor `client.rs` with reusable HTML GET and form POST helpers to reduce request boilerplate
- add `GoldenPaySession::fetch_paid_orders()`
- add `GoldenPaySession::fetch_category_metadata()`
- add `DeliveryService::remaining_items()`

## 0.2.0

- add typed `PriceCalculation` parsing with extracted seller, buyer, and commission fields
- add typed runner objects for chat nodes and order counters
- add delivery automation layer with inventory matching, message building, and paid-order processing
- add `DeliveryStore` abstractions with memory and JSON implementations
- add `DeliveryMessenger` abstraction for testable high-level delivery flows
- add `process_paid_order` example
- improve public docs for runner and automation APIs
- switch `reqwest` TLS configuration to `native-tls` for more stable Windows builds

## 0.1.1

- add builder-based configuration API
- improve release metadata and documentation
- add parser fixtures and tests
- add publish checklist and examples
- add git repository metadata for the public GitHub repo

## 0.1.0

- initial `goldenpay` release on crates.io
- session-based FunPay client with authenticated session flow
- polling bot with persistent state storage
- proxy and retry configuration
- chat messaging support
- order page parsing
- offer read and edit support
- category and market offer parsing
- examples, fixtures, parser tests, and publish metadata
