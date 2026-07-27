//! Cross-module integration tests for SwiftPan-owned backend workflows.
//!
//! These tests compose production subsystems with deterministic local
//! adapters. They deliberately exclude Tauri runtime behavior and real S3/R2
//! protocol behavior, which belong to higher-cost test layers.

mod settings_persistence;
mod share_flow;
mod transfer_pipeline;
