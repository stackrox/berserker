use log::debug;

use crate::script::ast::Node;

/// Contains a list of transformation to apply after parsing
/// TODO: Add following rules:
/// - add path if directory is expected
/// - add default worker arguments
pub fn apply_rules(works: Vec<&Node>) -> Vec<&Node> {
    debug!("Applying rules");
    works
}
