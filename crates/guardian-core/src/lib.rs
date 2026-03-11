//! guardian-core — Rust-Guardian 코어 엔진
//!
//! Config, Pipeline 오케스트레이션, SecureBytes, Frame, Error 타입을 제공한다.
//! 모든 다른 크레이트의 기반 계층.

pub mod contracts;
pub mod mock;
pub mod config;
pub mod pipeline;
pub mod engine;
pub mod error;
