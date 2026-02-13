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

            fn from_boxed_node(node: Box<dyn $crate::node::Node>) -> Option<Self>
            where
                Self: Sized,
            {
                let any: Box<dyn std::any::Any> = node;

                let any = match any.downcast::<Self>() {
                    Ok(node) => return Some(*node),
                    Err(any) => any,
                };

                let any = match any.downcast::<$crate::node::Container>() {
                    Ok(node) => return Some(Self::Container(*node)),
                    Err(any) => any,
                };

                let any = match any.downcast::<$crate::parameter::Parameter>() {
                    Ok(node) => return Some(Self::Parameter(*node)),
                    Err(any) => any,
                };

                let any = match any.downcast::<$crate::node::Manager>() {
                    Ok(node) => return Some(Self::Manager(*node)),
                    Err(any) => any,
                };

                $(
                    let any = match any.downcast::<$node_ty>() {
                        Ok(node) => return Some(Self::$variant(*node)),
                        Err(any) => any,
                    };
                )*

                let _ = any;
                None
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

/// Defines a single app node type with the common `NodeData` boilerplate.
///
/// Example:
/// `define_node_type!(struct DummyNode { dummy_prop: String } type_name: "dummy");`
///
/// Delegate node data to a nested node (no extra `node_data` field):
/// `define_node_type!(
///     struct VeryDummy { dummy_node: DummyNode, verydummy_prop: String }
///     type_name: "verydummy",
///     via: dummy_node,
///     constructor(label, dummy_prop: String, verydummy_prop: String) {
///         Self {
///             dummy_node: DummyNode::new(label, dummy_prop),
///             verydummy_prop,
///         }
///     }
/// );`
///
/// Overriding node lifecycle methods:
/// `define_node_type!(
///     struct DummyNode { dummy_prop: String }
///     type_name: "dummy",
///     node_impl {
///         fn init(&mut self, _ctx: &mut golden_core::process_ctx::ProcessCtx) {}
///         fn destroy(&mut self, _ctx: &mut golden_core::process_ctx::ProcessCtx) {}
///     }
/// );`
#[macro_export]
macro_rules! define_node_type {
    (
        $(#[$meta:meta])*
        $vis:vis struct $node_name:ident {
            $($field_vis:vis $field:ident : $field_ty:ty),* $(,)?
        }
        type_name: $type_name:literal,
        via: $($via:ident).+,
        constructor($label:ident $(, $ctor_arg:ident : $ctor_ty:ty)* $(,)?) $ctor_body:block
        $(,
            node_impl {
                $($node_impl:item)*
            }
        )?
        $(,)?
    ) => {
        $(#[$meta])*
        $vis struct $node_name {
            $($field_vis $field: $field_ty),*
        }

        impl $node_name {
            pub fn new(label: impl Into<String> $(, $ctor_arg: $ctor_ty)*) -> Self {
                let $label = label.into();
                $ctor_body
            }
        }

        impl $crate::node::Node for $node_name {
            fn node_data(&self) -> &$crate::node::NodeData {
                $crate::node::Node::node_data(&self.$($via).+)
            }

            fn node_data_mut(&mut self) -> &mut $crate::node::NodeData {
                $crate::node::Node::node_data_mut(&mut self.$($via).+)
            }

            fn get_type(&self) -> &str {
                $type_name
            }

            $($($node_impl)*)?
        }
    };

    (
        $(#[$meta:meta])*
        $vis:vis struct $node_name:ident {
            $($field_vis:vis $field:ident : $field_ty:ty),* $(,)?
        }
        type_name: $type_name:literal,
        via: $($via:ident).+
        $(,
            node_impl {
                $($node_impl:item)*
            }
        )?
        $(,)?
    ) => {
        $(#[$meta])*
        $vis struct $node_name {
            $($field_vis $field: $field_ty),*
        }

        impl $node_name {
            pub fn new(_label: impl Into<String> $(, $field: $field_ty)*) -> Self {
                Self {
                    $($field),*
                }
            }
        }

        impl $crate::node::Node for $node_name {
            fn node_data(&self) -> &$crate::node::NodeData {
                $crate::node::Node::node_data(&self.$($via).+)
            }

            fn node_data_mut(&mut self) -> &mut $crate::node::NodeData {
                $crate::node::Node::node_data_mut(&mut self.$($via).+)
            }

            fn get_type(&self) -> &str {
                $type_name
            }

            $($($node_impl)*)?
        }
    };

    (
        $(#[$meta:meta])*
        $vis:vis struct $node_name:ident {
            $($field_vis:vis $field:ident : $field_ty:ty),* $(,)?
        }
        type_name: $type_name:literal
        $(,
            node_impl {
                $($node_impl:item)*
            }
        )?
        $(,)?
    ) => {
        $(#[$meta])*
        $vis struct $node_name {
            node_data: $crate::node::NodeData,
            $($field_vis $field: $field_ty),*
        }

        impl $node_name {
            pub fn new(label: impl Into<String> $(, $field: $field_ty)*) -> Self {
                Self {
                    node_data: $crate::node::NodeData::new(label.into()),
                    $($field),*
                }
            }
        }

        impl $crate::node::Node for $node_name {
            fn node_data(&self) -> &$crate::node::NodeData {
                &self.node_data
            }

            fn node_data_mut(&mut self) -> &mut $crate::node::NodeData {
                &mut self.node_data
            }

            fn get_type(&self) -> &str {
                $type_name
            }

            $($($node_impl)*)?
        }
    };
}

/// Defines app node structs and registers them in a node enum in one place.
///
/// Example:
/// `define_app_nodes!(pub enum ChataigneNode { DummyNode => "dummy" { value: i32 } });`
#[macro_export]
macro_rules! define_app_nodes {
    (
        $vis:vis enum $enum_name:ident {
            $(
                $(#[$meta:meta])*
                $node_vis:vis $node_name:ident => $type_name:literal {
                    $($field_vis:vis $field:ident : $field_ty:ty),* $(,)?
                }
            ),* $(,)?
        }
    ) => {
        $(
            $crate::define_node_type! {
                $(#[$meta])*
                $node_vis struct $node_name {
                    $($field_vis $field: $field_ty),*
                }
                type_name: $type_name
            }
        )*

        $crate::define_node_enum! {
            $vis enum $enum_name {
                $($node_name),*
            }
        }
    };
}
