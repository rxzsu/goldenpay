# Changelog

All notable changes to this project will be documented in this file.

## [1.1.0] - 2026-07-23
### Added
* **Webhook HMAC Verification**: Added `crypto` module with HMAC-SHA256, hex encode/decode, constant-time comparison. `WebhookConfig.secret` field + `X-Signature-256` header validation. Public `compute_signature()` helper for clients.
* **Offer Group Scheduler**: New `scheduler` module — `OfferScheduler`, `OfferGroup`, `ScheduleRule`, `ScheduleAction`, `ScheduleEntry`. Supports multiple daily schedules with group-level activate/deactivate per category node. Integrated into `GoldenPayBot` via `.with_scheduler()`.
* **Auto-Pricing (Undercut)**: `undercut_price` method to automatically undercut competitors.
* **Auto-Raise**: `raise_offers` with interval-based scheduling.
* **Sleep Scheduling**: `BotOptions::sleep_schedule` for automatic nighttime pause.
* **Chat Interactions**: `upload_chat_file`, `reply_to_review`.
* **Withdrawals**: `withdraw()` method for balance payout.
* **Create Offer**: `create_offer` / `create_offer_with` for new listings.
* **Bulk Operations**: `deactivate_all_offers`, `delete_all_offers`.
* **Balance & Stats**: `fetch_balance`, `calculate_statistics`.
* **CI/CD Pipeline**: GitHub Actions with fmt, clippy, build, test.

### Changed
* All dependencies bumped to latest: async-trait 0.1.91, rand 0.10.2, regex 1.13.1, reqwest 0.13.4, serde 1.0.229, serde_json 1.0.151, thiserror 2.0.19, tokio 1.53.1, tokio-util 0.7.19, tracing 0.1.44, rusqlite 0.40.1, chrono 0.4.45, hmac 0.13.0, sha2 0.11.0.
* `actions/checkout` CI action v4 → v7.
* `cargo fmt` applied project-wide.
* Fixed all clippy warnings (field_reassign_with_default, items_after_test_module, collapsible_if).

### Security
* `SecureString` wrapper masks secrets in Debug/Display.
* `validate_golden_key()` checks format (≥8 chars, alphanumeric + `_` + `-`).
* `GoldenPaySession::check_connection()` — lightweight session health check.
* `SessionManager::rotate_key(new_key)` — seamless key rotation with reconnect.

## [1.0.0] - 2026-07-02
### Added
* **Security & Webhook Module**: Added `SecureString`, webhook server, key validation, session health checks, and key rotation (`rotate_key`) (by you).
* **SQLite Storage**: Implemented `SqliteStateStore` for robust state persistence using `rusqlite`.
* **CI/CD Pipeline**: Configured GitHub Actions to automatically run `cargo check`, `cargo fmt`, `clippy`, and `test`.
* **Auto-Pricing (Undercut)**: Added `undercut_price` method to automatically outbid competitors without going below a minimum threshold.
* **Auto-Raise**: Added `raise_offers` and interval-based scheduling to keep offers at the top of the category.
* **Sleep Scheduling**: Introduced `BotOptions::sleep_schedule` to automatically pause bots during nighttime.
* **Proxy Support**: Added proxy usage and connection verification (`validate_proxy`).
* **Chat Interactions**: Added `upload_chat_file`, `send_message`, and `reply_to_review` features.
* **Withdrawals**: Added the ability to automatically request a balance withdrawal (`withdraw()`).
* **New Listing Automation**: Added `create_offer` and `create_offer_with` to create empty or new offers on the fly.
* **Bulk Operations**: Added `deactivate_all_offers` and `delete_all_offers` for sweeping changes across categories.
* **Balance & Stats**: Implemented `fetch_balance` to parse current account balance and `calculate_statistics` to quickly gather revenue and sales statistics.
* **Store Analytics**: Fetch order volume, average check, and unique buyer numbers based on your order history.

### Changed
* Refactored API and documentation for stability and ease of use (by you).
* Updated `Cargo.toml` logic from `include` to `exclude` for cleaner packaging (by you).
* Fixed stochastic test compilation errors (RngExt vs replace) (by you).
* Improved `BotOptions` builder to be more ergonomic and robust.

### Removed
* Internal stubs and outdated experimental paths.

## [0.5.0] - Previous
* Added batch operations, filters, and cleaned up legacy `fetch_all_orders` stub.
