//! Thin Tauri command wrappers. Business logic lives in the domain modules so it
//! can be unit-tested without a running app; see `docs/API.md` for the contract.

pub mod ai;
pub mod app;
pub mod course;
pub mod document;
