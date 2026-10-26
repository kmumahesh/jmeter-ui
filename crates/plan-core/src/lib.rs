//! The test plan as the app sees it: element kinds, what can go where, the tree and its state.

pub mod catalog;

/// The size of plan, in elements, that opens in under five seconds: measured at about 80,000 elements in a test build on the
/// development machine (50,000 take 2.3 seconds), kept as 60,000. Bigger plans open, but take longer, and the app says so.
pub const COMFORTABLE_PLAN_ELEMENTS: usize = 60_000;
pub mod curl;
pub mod har;
pub mod derive;
pub mod edit;
pub mod files;
pub mod forms;
pub mod fuzzy;
pub mod model;
pub mod problems;
pub mod rules;
pub mod scripts;
pub mod scope;
pub mod search;
pub mod starters;
pub mod templates;
pub mod tree;
pub mod variables;

pub use catalog::Category;
pub use edit::{EditablePlan, Outcome, Place};
pub use model::{Node, NodeId, Plan};
pub use tree::{Move, Row, TreeState};
