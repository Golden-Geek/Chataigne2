/// Defines a node enum with static dispatch over `golden_core::node::Node`.
///
/// Internal node variants are always included automatically:
/// - `Container($crate::node::Container)`
/// - `Parameter($crate::parameter::Parameter)`
/// - `Manager($crate::node::Manager)`
///
/// Shorthand form for app-specific nodes:
/// `define_node_enum!(pub enum MyNodes { Oscillator, Envelope });`
///
/// Explicit variant/type form (useful for namespaced types):
/// `define_node_enum!(pub enum MyNodes { Audio(my_app::AudioNode) });`
#[macro_export]
macro_rules! define_node_enum {
    ($vis:vis enum $enum_name:ident { $($variant:ident($node_ty:ty)),* $(,)? }) => {
        $vis enum $enum_name {
            Container($crate::node::Container),
            Parameter($crate::parameter::Parameter),
            Manager($crate::node::Manager),
            $($variant($node_ty),)*
        }

        impl $crate::node::Node for $enum_name {
            fn node_data(&self) -> &$crate::node::NodeData {
                match self {
                    Self::Container(node) => node.node_data(),
                    Self::Parameter(node) => node.node_data(),
                    Self::Manager(node) => node.node_data(),
                    $(Self::$variant(node) => node.node_data(),)*
                }
            }

            fn node_data_mut(&mut self) -> &mut $crate::node::NodeData {
                match self {
                    Self::Container(node) => node.node_data_mut(),
                    Self::Parameter(node) => node.node_data_mut(),
                    Self::Manager(node) => node.node_data_mut(),
                    $(Self::$variant(node) => node.node_data_mut(),)*
                }
            }

            fn get_type(&self) -> &str {
                match self {
                    Self::Container(node) => node.get_type(),
                    Self::Parameter(node) => node.get_type(),
                    Self::Manager(node) => node.get_type(),
                    $(Self::$variant(node) => node.get_type(),)*
                }
            }
        }

        impl From<$crate::node::Container> for $enum_name {
            fn from(node: $crate::node::Container) -> Self {
                Self::Container(node)
            }
        }

        impl From<$crate::parameter::Parameter> for $enum_name {
            fn from(node: $crate::parameter::Parameter) -> Self {
                Self::Parameter(node)
            }
        }

        impl From<$crate::node::Manager> for $enum_name {
            fn from(node: $crate::node::Manager) -> Self {
                Self::Manager(node)
            }
        }

        $(
            impl From<$node_ty> for $enum_name {
                fn from(node: $node_ty) -> Self {
                    Self::$variant(node)
                }
            }
        )*
    };

    ($vis:vis enum $enum_name:ident { $($node_ty:ident),* $(,)? }) => {
        $crate::define_node_enum! {
            $vis enum $enum_name {
                $($node_ty($node_ty)),*
            }
        }
    };

    ($enum_name:ident { $($node_ty:ident),* $(,)? }) => {
        $crate::define_node_enum! {
            enum $enum_name {
                $($node_ty),*
            }
        }
    };
}
