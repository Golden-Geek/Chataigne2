#[doc(hidden)]
#[macro_export]
macro_rules! __dispatch_node_enum {
    ($self:expr, $method:ident ; $($variant:ident),* $(,)?) => {
        match $self {
            Self::Folder(node) => node.$method(),
            Self::Parameter(node) => node.$method(),
            $(Self::$variant(node) => node.$method(),)*
        }
    };
    ($self:expr, $method:ident, $arg1:expr ; $($variant:ident),* $(,)?) => {
        match $self {
            Self::Folder(node) => node.$method($arg1),
            Self::Parameter(node) => node.$method($arg1),
            $(Self::$variant(node) => node.$method($arg1),)*
        }
    };
    ($self:expr, $method:ident, $arg1:expr, $arg2:expr ; $($variant:ident),* $(,)?) => {
        match $self {
            Self::Folder(node) => node.$method($arg1, $arg2),
            Self::Parameter(node) => node.$method($arg1, $arg2),
            $(Self::$variant(node) => node.$method($arg1, $arg2),)*
        }
    };
    ($self:expr, $method:ident, $arg1:expr, $arg2:expr, $arg3:expr ; $($variant:ident),* $(,)?) => {
        match $self {
            Self::Folder(node) => node.$method($arg1, $arg2, $arg3),
            Self::Parameter(node) => node.$method($arg1, $arg2, $arg3),
            $(Self::$variant(node) => node.$method($arg1, $arg2, $arg3),)*
        }
    };
    ($self:expr, $method:ident, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr ; $($variant:ident),* $(,)?) => {
        match $self {
            Self::Folder(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::Parameter(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            $(Self::$variant(node) => node.$method($arg1, $arg2, $arg3, $arg4),)*
        }
    };
    ($self:expr, $method:ident, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr ; $($variant:ident),* $(,)?) => {
        match $self {
            Self::Folder(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::Parameter(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
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

/// Expands `Node` trait methods for container item creation while keeping manager implementation manual.
///
/// Intended usage is inside `impl Node for YourManager { ... }`.
///
/// This macro does not define your manager struct or constructor, so you keep full control over
/// custom fields and runtime logic. Each item can include `when: ...` to gate availability per
/// manager instance.
///
/// Example:
/// `define_user_item_factory_methods! {
///     accepts = ["module"];
///     items = [
///         {
///             node_type: "osc_module",
///             item_kind: "module",
///             label: "OSC Module",
///             create: |_: &Self, label: String| OscModule::create(label),
///         },
///         {
///             node_type: "dmx_module",
///             item_kind: "module",
///             label: "DMX Module",
///             when: |this: &Self| this.allow_dmx,
///             create: |_: &Self, label: String| DmxModule::create(label),
///         },
///     ];
/// }`
#[macro_export]
macro_rules! define_user_item_factory_methods {
    (
        accepts = [$($accepted_kind:literal),* $(,)?];
        items = [
            $(
                {
                    node_type: $node_type:literal,
                    item_kind: $item_kind:literal,
                    label: $label:expr,
                    $(when: $when:expr,)?
                    create: $create:expr
                    $(,)?
                }
            ),* $(,)?
        ];
    ) => {
        fn user_container_rules(&self) -> Option<$crate::node::UserContainerRules> {
            Some($crate::node::UserContainerRules::new(&[$($accepted_kind),*]))
        }

        $crate::define_user_item_factory_methods! {
            @shared
            items = [
                $(
                    {
                        node_type: $node_type,
                        item_kind: $item_kind,
                        label: $label,
                        $(when: $when,)?
                        create: $create
                    }
                ),*
            ];
        }
    };

    (
        items = [
            $(
                {
                    node_type: $node_type:literal,
                    item_kind: $item_kind:literal,
                    label: $label:expr,
                    $(when: $when:expr,)?
                    create: $create:expr
                    $(,)?
                }
            ),* $(,)?
        ];
    ) => {
        $crate::define_user_item_factory_methods! {
            @shared
            items = [
                $(
                    {
                        node_type: $node_type,
                        item_kind: $item_kind,
                        label: $label,
                        $(when: $when,)?
                        create: $create
                    }
                ),*
            ];
        }
    };

    (
        @shared
        items = [
            $(
                {
                    node_type: $node_type:literal,
                    item_kind: $item_kind:literal,
                    label: $label:expr,
                    $(when: $when:expr,)?
                    create: $create:expr
                }
            ),* $(,)?
        ];
    ) => {
        fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
            if !self.user_container_rules().is_some_and(|rules| rules.accepts(item_kind)) {
                return false;
            }

            match item_type {
                $(
                    $node_type => {
                        item_kind == $item_kind
                            && $crate::define_user_item_factory_methods!(@cond self $(, $when )?)
                    }
                )*
                _ => false,
            }
        }

        fn user_creatable_items(&self) -> Vec<$crate::node::UserCreatableItem> {
            let mut items = Vec::new();
            $(
                if $crate::define_user_item_factory_methods!(@cond self $(, $when )?) {
                    items.push($crate::node::UserCreatableItem::new($node_type, $item_kind, $label));
                }
            )*
            items
        }

        fn create_user_item(&self, node_type: &str, label: String) -> Option<Box<dyn $crate::node::Node>> {
            match node_type {
                $(
                    $node_type => {
                        if $crate::define_user_item_factory_methods!(@cond self $(, $when )?) {
                            Some(Box::new(($create)(self, label)))
                        } else {
                            None
                        }
                    }
                )*
                _ => None,
            }
        }
    };

    (@cond $self:expr, $when:expr) => {
        ($when)($self)
    };

    (@cond $self:expr) => {
        true
    };
}

/// Defines a node enum with static dispatch over `golden_core::node::Node`.
///
/// Internal node variants are always included automatically:
/// - `Folder($crate::node::Folder)`
/// - `Parameter($crate::parameter::Parameter)`
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
            fn user_item_kind(&self) -> &str {
                $crate::__dispatch_node_enum!(self, user_item_kind; $($variant),*)
            }

            #[inline(always)]
            fn is_declared_user_item(&self) -> bool {
                $crate::__dispatch_node_enum!(self, is_declared_user_item; $($variant),*)
            }

            #[inline(always)]
            fn user_container_rules(&self) -> Option<$crate::node::UserContainerRules> {
                $crate::__dispatch_node_enum!(self, user_container_rules; $($variant),*)
            }

            #[inline(always)]
            fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
                $crate::__dispatch_node_enum!(self, user_container_accepts_item, item_type, item_kind; $($variant),*)
            }

            #[inline(always)]
            fn user_creatable_items(&self) -> Vec<$crate::node::UserCreatableItem> {
                $crate::__dispatch_node_enum!(self, user_creatable_items; $($variant),*)
            }

            #[inline(always)]
            fn create_user_item(&self, node_type: &str, label: String) -> Option<Box<dyn $crate::node::Node>> {
                $crate::__dispatch_node_enum!(self, create_user_item, node_type, label; $($variant),*)
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
            fn engine_prepare_param_value(&self, value: $crate::parameter::ParamValue) -> Result<$crate::parameter::ParamValue, String> {
                $crate::__dispatch_node_enum!(self, engine_prepare_param_value, value; $($variant),*)
            }

            #[inline(always)]
            fn engine_param_snapshot(&self) -> Option<$crate::parameter::ParameterSnapshot> {
                $crate::__dispatch_node_enum!(self, engine_param_snapshot; $($variant),*)
            }

            #[inline(always)]
            fn engine_visit_references_mut(&mut self, visit: &mut dyn FnMut(&mut $crate::node::NodeReference)) {
                $crate::__dispatch_node_enum!(self, engine_visit_references_mut, visit; $($variant),*)
            }

            #[inline(always)]
            fn engine_sync_param_handle_cache(&mut self, param: $crate::node::NodeId, new_value: &$crate::parameter::ParamValue) {
                $crate::__dispatch_node_enum!(self, engine_sync_param_handle_cache, param, new_value; $($variant),*)
            }

            #[inline(always)]
            fn engine_on_attached(&mut self, ctx: &mut $crate::process_ctx::ProcessCtx) {
                $crate::__dispatch_node_enum!(self, engine_on_attached, ctx; $($variant),*)
            }

            #[inline(always)]
            fn engine_sync_bound_param_handles(&mut self, resolve: &mut dyn FnMut($crate::node::NodeId) -> Option<$crate::parameter::ParamValue>) {
                $crate::__dispatch_node_enum!(self, engine_sync_bound_param_handles, resolve; $($variant),*)
            }

            #[inline(always)]
            fn engine_preprocess_inbox(&mut self, ctx: &mut $crate::process_ctx::ProcessCtx) {
                $crate::__dispatch_node_enum!(self, engine_preprocess_inbox, ctx; $($variant),*)
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
            fn engine_child_event_interest_depth(&self, event: &$crate::events::Event) -> u32 {
                $crate::__dispatch_node_enum!(self, engine_child_event_interest_depth, event; $($variant),*)
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
