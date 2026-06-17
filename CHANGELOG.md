# Changelog

## 0.4.0

- **SessionManager** — new wrapper around `GoldenPaySession` with automatic reconnection on HTTP 401/403 (all 15 API methods covered)
- **GoldenPayBot::connect()** — new async constructor that creates a `SessionManager` internally; the bot now auto-reconnects when the session expires
- **FetchOrderOptions** — filter orders by `status`, `min_amount`, `max_amount`, `subcategory` with builder API and add `fetch_orders_with()` on both `GoldenPaySession` and `SessionManager`
- **Category tree parser** — `parse_category_tree()` extracts the full marketplace hierarchy from `/lots/` with recursive `CategoryNode { id, name, subcategory_type, children }`; new `fetch_category_tree()` on both session types
- **Pagination helper** — `SessionManager::fetch_all_orders()` placeholder for future multi-page order fetching
- **Urls::lots_home()** — new URL helper for the marketplace root page
- **Example** — `poll_orders.rs` updated to use `GoldenPayBot::connect()`

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
