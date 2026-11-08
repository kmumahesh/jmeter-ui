//! The JMeter UI application.

// The state shown to tests and the state file is one large `json!` object; its many keys need a deeper limit.
#![recursion_limit = "256"]

pub mod assets;
pub mod combo;
mod aggregate;
mod clipboard;
mod compare_view;
mod curl_import;
mod editing;
mod expression;
mod har_import;
pub mod new_plan;
mod run_bar;
pub mod file_dialog;
mod requests;
mod run_props;
mod think_times;
mod results_view;
mod run_ctl;
mod running;
mod views_run;
mod chart;
pub mod form;
pub mod fields;
pub mod palette_view;
pub mod shell;
pub mod textarea;
pub mod theme;
pub mod ui_util;
mod views;
mod views_content;
mod views_form;
