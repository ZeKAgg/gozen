mod cyclic_dependency;
mod invalid_node_path;
mod invalid_resource_type;
mod missing_class_name;
mod missing_parent_method_contract;
mod missing_parent_node_contract;
mod missing_parent_signal_contract;
mod missing_signal_handler;
mod parent_contract_support;
mod unused_autoload;
mod unused_signal;

pub use cyclic_dependency::CyclicDependency;
pub use invalid_node_path::InvalidNodePath;
pub use invalid_resource_type::InvalidResourceType;
pub use missing_class_name::MissingClassName;
pub use missing_parent_method_contract::MissingParentMethodContract;
pub use missing_parent_node_contract::MissingParentNodeContract;
pub use missing_parent_signal_contract::MissingParentSignalContract;
pub use missing_signal_handler::MissingSignalHandler;
pub use unused_autoload::UnusedAutoload;
pub use unused_signal::UnusedSignal;

use crate::rule::ProjectRule;

pub fn all_project_rules() -> Vec<Box<dyn ProjectRule>> {
    vec![
        Box::new(MissingSignalHandler),
        Box::new(InvalidNodePath),
        Box::new(InvalidResourceType),
        Box::new(MissingClassName),
        Box::new(MissingParentNodeContract),
        Box::new(MissingParentSignalContract),
        Box::new(MissingParentMethodContract),
        Box::new(UnusedAutoload),
        Box::new(UnusedSignal),
        Box::new(CyclicDependency),
    ]
}
