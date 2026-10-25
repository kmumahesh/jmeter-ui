//! The test plan as the app sees it: element kinds, what can go where, the tree and its state.

pub mod catalog;
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
