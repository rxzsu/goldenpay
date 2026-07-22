//! Production-oriented Rust SDK for `FunPay` automation.
//!
//! Provides session management, order polling, delivery automation,
//! offer editing, and chat messaging for the `FunPay` marketplace.

#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    clippy::return_self_not_must_use,
    clippy::similar_names,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::items_after_statements,
    clippy::items_after_test_module,
    clippy::float_cmp
)]

pub mod automation;
pub mod bot;
pub mod client;
pub mod config;
pub mod crypto;
pub mod error;
pub mod event;
pub mod models;
pub mod offer;
pub mod scheduler;
pub mod security;
pub mod session;
pub mod storage;
pub mod webhook;

mod parser;
mod urls;
mod utils;

pub use automation::{
    DeliveredOrderRecord, DeliveryError, DeliveryItem, DeliveryItemFormat, DeliveryMatch,
    DeliveryMessageBuilder, DeliveryMessenger, DeliveryResult, DeliveryService, DeliveryStore,
    ExactSubcategoryMatcher, JsonDeliveryStore, MemoryDeliveryStore, ProcessPaidOrderResult,
    ProductInventory, ProductMatcher,
};
pub use bot::{GoldenPayBot, GoldenPayEvent};
pub use client::{GoldenPay, GoldenPaySession};
pub use config::{GoldenPayConfig, GoldenPayConfigBuilder, RetryPolicy};
pub use error::GoldenPayError;
pub use event::{BotOptions, EventStream, MessageFilter};
pub use models::{
    CategoryFilter, CategoryFilterOption, CategoryFilterType, CategoryNode, CategorySubcategory,
    CategorySubcategoryType, ChatMessage, FetchOrderOptions, MarketOffer, Offer, OfferDetails,
    OfferEdit, OfferField, OfferFieldOption, OfferFieldType, OfferSaveResponse, OrderInfo,
    OrderPage, OrderStatus, PriceCalculation, ProfileReview, RaiseOffersResponse, Review,
    RunnerChatMessage, RunnerChatNode, RunnerObject, RunnerOrdersCounters, RunnerResponse,
    RunnerUnknownObject, UserInfo, WithdrawRequest,
};
pub use offer::OfferEditBuilder;
pub use scheduler::{OfferGroup, OfferScheduler, ScheduleAction, ScheduleEntry, ScheduleRule};
pub use security::{SecureString, validate_golden_key};
pub use session::SessionManager;
pub use storage::{JsonStateStore, MemoryStateStore, SqliteStateStore, StateStore};
pub use webhook::{
    WebhookConfig, WebhookEvent, WebhookHandler, WebhookPayload, WebhookServer, compute_signature,
};

#[doc(hidden)]
pub use parser::{parse_price_calculation, parse_runner_objects};
