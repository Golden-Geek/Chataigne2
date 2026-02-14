#[doc(hidden)]
#[macro_export]
macro_rules! __dispatch_node_enum {
    ($self:expr, $method:ident ; $($variant:ident),* $(,)?) => {
        match $self {
            Self::Folder(node) => node.$method(),
            Self::Parameter(node) => node.$method(),
            Self::Manager(node) => node.$method(),
            $(Self::$variant(node) => node.$method(),)*
        }
    };
    ($self:expr, $method:ident, $arg1:expr ; $($variant:ident),* $(,)?) => {
        match $self {
            Self::Folder(node) => node.$method($arg1),
            Self::Parameter(node) => node.$method($arg1),
            Self::Manager(node) => node.$method($arg1),
            $(Self::$variant(node) => node.$method($arg1),)*
        }
    };
    ($self:expr, $method:ident, $arg1:expr, $arg2:expr ; $($variant:ident),* $(,)?) => {
        match $self {
            Self::Folder(node) => node.$method($arg1, $arg2),
            Self::Parameter(node) => node.$method($arg1, $arg2),
            Self::Manager(node) => node.$method($arg1, $arg2),
            $(Self::$variant(node) => node.$method($arg1, $arg2),)*
        }
    };
    ($self:expr, $method:ident, $arg1:expr, $arg2:expr, $arg3:expr ; $($variant:ident),* $(,)?) => {
        match $self {
            Self::Folder(node) => node.$method($arg1, $arg2, $arg3),
            Self::Parameter(node) => node.$method($arg1, $arg2, $arg3),
            Self::Manager(node) => node.$method($arg1, $arg2, $arg3),
            $(Self::$variant(node) => node.$method($arg1, $arg2, $arg3),)*
        }
    };
    ($self:expr, $method:ident, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr ; $($variant:ident),* $(,)?) => {
        match $self {
            Self::Folder(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::Parameter(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::Manager(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            $(Self::$variant(node) => node.$method($arg1, $arg2, $arg3, $arg4),)*
        }
    };
    ($self:expr, $method:ident, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr ; $($variant:ident),* $(,)?) => {
        match $self {
            Self::Folder(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::Parameter(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::Manager(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            $(Self::$variant(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),)*
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __downcast_node_enum_variant {
    ($any:ident, $variant:ident, $node_ty:ty) => {
        let $any = match $any.downcast::<$node_ty>() {
            Ok(node) => return Some(Self::$variant(*node)),
            Err(any) => any,
        };
    };
}

/// Defines a node enum with static dispatch over `golden_core::node::Node`.
///
/// Internal node variants are always included automatically:
/// - `Folder($crate::node::Folder)`
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
            Folder($crate::node::Folder),
            Parameter($crate::parameter::Parameter),
            Manager($crate::node::Manager),
            $($variant($node_ty),)*
        }

        impl $crate::node::Node for $enum_name {
            #[inline(always)]
            fn node_data(&self) -> &$crate::node::NodeData {
                $crate::__dispatch_node_enum!(self, node_data; $($variant),*)
            }

            #[inline(always)]
            fn node_data_mut(&mut self) -> &mut $crate::node::NodeData {
                $crate::__dispatch_node_enum!(self, node_data_mut; $($variant),*)
            }

            #[inline(always)]
            fn get_type(&self) -> &str {
                $crate::__dispatch_node_enum!(self, get_type; $($variant),*)
            }

            #[inline(always)]
            fn execution_rule(&self) -> $crate::engine::NodeExecutionRule {
                $crate::__dispatch_node_enum!(self, execution_rule; $($variant),*)
            }

            #[inline(always)]
            fn engine_set_param_value(&mut self, value: $crate::parameter::ParamValue) -> Option<$crate::parameter::ParamValue> {
                $crate::__dispatch_node_enum!(self, engine_set_param_value, value; $($variant),*)
            }

            #[inline(always)]
            fn engine_visit_references_mut(&mut self, visit: &mut dyn FnMut(&mut $crate::node::NodeReference)) {
                $crate::__dispatch_node_enum!(self, engine_visit_references_mut, visit; $($variant),*)
            }

            #[inline(always)]
            fn init(&mut self, ctx: &mut $crate::process_ctx::ProcessCtx) {
                $crate::__dispatch_node_enum!(self, init, ctx; $($variant),*)
            }

            #[inline(always)]
            fn update(&mut self, ctx: &mut $crate::process_ctx::ProcessCtx) {
                $crate::__dispatch_node_enum!(self, update, ctx; $($variant),*)
            }

            #[inline(always)]
            fn destroy(&mut self, ctx: &mut $crate::process_ctx::ProcessCtx) {
                $crate::__dispatch_node_enum!(self, destroy, ctx; $($variant),*)
            }

            #[inline(always)]
            fn child_event_interest_depth(&self, event: &$crate::events::Event) -> u32 {
                $crate::__dispatch_node_enum!(self, child_event_interest_depth, event; $($variant),*)
            }

            #[inline(always)]
            fn bubble_event_depth(&self, event: &$crate::events::Event) -> u32 {
                $crate::__dispatch_node_enum!(self, bubble_event_depth, event; $($variant),*)
            }

            #[inline(always)]
            fn event_propagation(&self, event: &$crate::events::Event, depth: u32) -> $crate::node::EventPropagation {
                $crate::__dispatch_node_enum!(self, event_propagation, event, depth; $($variant),*)
            }

            #[inline(always)]
            fn on_inbox(&mut self, ctx: &mut $crate::process_ctx::ProcessCtx) {
                $crate::__dispatch_node_enum!(self, on_inbox, ctx; $($variant),*)
            }

            #[inline(always)]
            fn on_param_change(&mut self, ctx: &mut $crate::process_ctx::ProcessCtx, param: $crate::node::NodeId, old_value: $crate::parameter::ParamValue) {
                $crate::__dispatch_node_enum!(self, on_param_change, ctx, param, old_value; $($variant),*)
            }

            #[inline(always)]
            fn on_child_added(&mut self, ctx: &mut $crate::process_ctx::ProcessCtx, parent: $crate::node::NodeId, child: $crate::node::NodeId) {
                $crate::__dispatch_node_enum!(self, on_child_added, ctx, parent, child; $($variant),*)
            }

            #[inline(always)]
            fn on_child_added_decl(
                &mut self,
                ctx: &mut $crate::process_ctx::ProcessCtx,
                parent: $crate::node::NodeId,
                child: $crate::node::NodeId,
                decl_id: &$crate::node::DeclId,
            ) {
                $crate::__dispatch_node_enum!(self, on_child_added_decl, ctx, parent, child, decl_id; $($variant),*)
            }

            #[inline(always)]
            fn on_child_removed(&mut self, ctx: &mut $crate::process_ctx::ProcessCtx, parent: $crate::node::NodeId, child: $crate::node::NodeId) {
                $crate::__dispatch_node_enum!(self, on_child_removed, ctx, parent, child; $($variant),*)
            }

            #[inline(always)]
            fn on_child_replaced(&mut self, ctx: &mut $crate::process_ctx::ProcessCtx, parent: $crate::node::NodeId, old: $crate::node::NodeId, new: $crate::node::NodeId) {
                $crate::__dispatch_node_enum!(self, on_child_replaced, ctx, parent, old, new; $($variant),*)
            }

            #[inline(always)]
            fn on_child_replaced_decl(
                &mut self,
                ctx: &mut $crate::process_ctx::ProcessCtx,
                parent: $crate::node::NodeId,
                old: $crate::node::NodeId,
                new: $crate::node::NodeId,
                decl_id: &$crate::node::DeclId,
            ) {
                $crate::__dispatch_node_enum!(self, on_child_replaced_decl, ctx, parent, old, new, decl_id; $($variant),*)
            }

            #[inline(always)]
            fn on_child_moved(&mut self, ctx: &mut $crate::process_ctx::ProcessCtx, child: $crate::node::NodeId, old_parent: $crate::node::NodeId, new_parent: $crate::node::NodeId) {
                $crate::__dispatch_node_enum!(self, on_child_moved, ctx, child, old_parent, new_parent; $($variant),*)
            }

            #[inline(always)]
            fn on_child_reordered(&mut self, ctx: &mut $crate::process_ctx::ProcessCtx, parent: $crate::node::NodeId, child: $crate::node::NodeId) {
                $crate::__dispatch_node_enum!(self, on_child_reordered, ctx, parent, child; $($variant),*)
            }

            #[inline(always)]
            fn on_node_created(&mut self, ctx: &mut $crate::process_ctx::ProcessCtx, node_id: $crate::node::NodeId) {
                $crate::__dispatch_node_enum!(self, on_node_created, ctx, node_id; $($variant),*)
            }

            #[inline(always)]
            fn on_node_deleted(&mut self, ctx: &mut $crate::process_ctx::ProcessCtx, node_id: $crate::node::NodeId) {
                $crate::__dispatch_node_enum!(self, on_node_deleted, ctx, node_id; $($variant),*)
            }

            #[inline(always)]
            fn on_meta_changed(&mut self, ctx: &mut $crate::process_ctx::ProcessCtx, node_id: $crate::node::NodeId, patch: $crate::node::NodeMetaPatch) {
                $crate::__dispatch_node_enum!(self, on_meta_changed, ctx, node_id, patch; $($variant),*)
            }

            #[inline(always)]
            fn on_custom_event(&mut self, ctx: &mut $crate::process_ctx::ProcessCtx, event: $crate::events::CustomEvent) {
                $crate::__dispatch_node_enum!(self, on_custom_event, ctx, event; $($variant),*)
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

                $crate::__downcast_node_enum_variant!(any, Folder, $crate::node::Folder);
                $crate::__downcast_node_enum_variant!(any, Parameter, $crate::parameter::Parameter);
                $crate::__downcast_node_enum_variant!(any, Manager, $crate::node::Manager);

                $(
                    $crate::__downcast_node_enum_variant!(any, $variant, $node_ty);
                )*

                let _ = any;
                None
            }
        }

        impl From<$crate::node::Folder> for $enum_name {
            fn from(node: $crate::node::Folder) -> Self {
                Self::Folder(node)
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

/// Marker macro consumed by `#[golden_core::node]` on `impl Node for ...` blocks.
///
/// This fallback exists so accidental standalone usage produces a clear error.
#[macro_export]
macro_rules! params {
    ($($tt:tt)*) => {
        compile_error!("`params!{...}` is consumed by #[golden_core::node] on `impl Node for ...` blocks and cannot be used standalone");
    };
}
