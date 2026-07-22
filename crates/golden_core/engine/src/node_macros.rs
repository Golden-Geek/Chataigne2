#[doc(hidden)]
#[macro_export]
macro_rules! __dispatch_node_enum {
    ($self:expr, $method:ident ; $($variant:ident),* $(,)?) => {
        match $self {
            Self::Folder(node) => node.$method(),
            Self::UserContext(node) => node.$method(),
            Self::UserContextFolder(node) => node.$method(),
            Self::UserContextMultiplex(node) => node.$method(),
            Self::UserContextMultiplexList(node) => node.$method(),
            Self::Parameter(node) => node.$method(),
            Self::Dashboard(node) => node.$method(),
            Self::DashboardPage(node) => node.$method(),
            Self::DashboardWidgetContainer(node) => node.$method(),
            Self::DashboardNodeWidget(node) => node.$method(),
            Self::DashboardGenericWidget(node) => node.$method(),
            Self::DashboardNodeWidgetInspectorOptions(node) => node.$method(),
            Self::DashboardNodeWidgetParameterEditorOptions(node) => node.$method(),
            Self::DashboardNodeWidgetNumberSliderOptions(node) => node.$method(),
            Self::DashboardNodeWidgetNumberRotaryOptions(node) => node.$method(),
            Self::DashboardNodeWidgetVec2PadOptions(node) => node.$method(),
            Self::DashboardNodeWidgetVec2EditorOptions(node) => node.$method(),
            Self::DashboardNodeWidgetVec3EditorOptions(node) => node.$method(),
            Self::DashboardNodeWidgetColorEditorOptions(node) => node.$method(),
            Self::ParameterAnimationControl(node) => node.$method(),
            Self::Curve(node) => node.$method(),
            Self::CurveRange(node) => node.$method(),
            Self::CurveKey(node) => node.$method(),
            Self::CurveEasing(node) => node.$method(),
            Self::Gradient(node) => node.$method(),
            Self::GradientStop(node) => node.$method(),
            Self::Script(node) => node.$method(),
            $(Self::$variant(node) => node.$method(),)*
        }
    };
    ($self:expr, $method:ident, $arg1:expr ; $($variant:ident),* $(,)?) => {
        match $self {
            Self::Folder(node) => node.$method($arg1),
            Self::UserContext(node) => node.$method($arg1),
            Self::UserContextFolder(node) => node.$method($arg1),
            Self::UserContextMultiplex(node) => node.$method($arg1),
            Self::UserContextMultiplexList(node) => node.$method($arg1),
            Self::Parameter(node) => node.$method($arg1),
            Self::Dashboard(node) => node.$method($arg1),
            Self::DashboardPage(node) => node.$method($arg1),
            Self::DashboardWidgetContainer(node) => node.$method($arg1),
            Self::DashboardNodeWidget(node) => node.$method($arg1),
            Self::DashboardGenericWidget(node) => node.$method($arg1),
            Self::DashboardNodeWidgetInspectorOptions(node) => node.$method($arg1),
            Self::DashboardNodeWidgetParameterEditorOptions(node) => node.$method($arg1),
            Self::DashboardNodeWidgetNumberSliderOptions(node) => node.$method($arg1),
            Self::DashboardNodeWidgetNumberRotaryOptions(node) => node.$method($arg1),
            Self::DashboardNodeWidgetVec2PadOptions(node) => node.$method($arg1),
            Self::DashboardNodeWidgetVec2EditorOptions(node) => node.$method($arg1),
            Self::DashboardNodeWidgetVec3EditorOptions(node) => node.$method($arg1),
            Self::DashboardNodeWidgetColorEditorOptions(node) => node.$method($arg1),
            Self::ParameterAnimationControl(node) => node.$method($arg1),
            Self::Curve(node) => node.$method($arg1),
            Self::CurveRange(node) => node.$method($arg1),
            Self::CurveKey(node) => node.$method($arg1),
            Self::CurveEasing(node) => node.$method($arg1),
            Self::Gradient(node) => node.$method($arg1),
            Self::GradientStop(node) => node.$method($arg1),
            Self::Script(node) => node.$method($arg1),
            $(Self::$variant(node) => node.$method($arg1),)*
        }
    };
    ($self:expr, $method:ident, $arg1:expr, $arg2:expr ; $($variant:ident),* $(,)?) => {
        match $self {
            Self::Folder(node) => node.$method($arg1, $arg2),
            Self::UserContext(node) => node.$method($arg1, $arg2),
            Self::UserContextFolder(node) => node.$method($arg1, $arg2),
            Self::UserContextMultiplex(node) => node.$method($arg1, $arg2),
            Self::UserContextMultiplexList(node) => node.$method($arg1, $arg2),
            Self::Parameter(node) => node.$method($arg1, $arg2),
            Self::Dashboard(node) => node.$method($arg1, $arg2),
            Self::DashboardPage(node) => node.$method($arg1, $arg2),
            Self::DashboardWidgetContainer(node) => node.$method($arg1, $arg2),
            Self::DashboardNodeWidget(node) => node.$method($arg1, $arg2),
            Self::DashboardGenericWidget(node) => node.$method($arg1, $arg2),
            Self::DashboardNodeWidgetInspectorOptions(node) => node.$method($arg1, $arg2),
            Self::DashboardNodeWidgetParameterEditorOptions(node) => node.$method($arg1, $arg2),
            Self::DashboardNodeWidgetNumberSliderOptions(node) => node.$method($arg1, $arg2),
            Self::DashboardNodeWidgetNumberRotaryOptions(node) => node.$method($arg1, $arg2),
            Self::DashboardNodeWidgetVec2PadOptions(node) => node.$method($arg1, $arg2),
            Self::DashboardNodeWidgetVec2EditorOptions(node) => node.$method($arg1, $arg2),
            Self::DashboardNodeWidgetVec3EditorOptions(node) => node.$method($arg1, $arg2),
            Self::DashboardNodeWidgetColorEditorOptions(node) => node.$method($arg1, $arg2),
            Self::ParameterAnimationControl(node) => node.$method($arg1, $arg2),
            Self::Curve(node) => node.$method($arg1, $arg2),
            Self::CurveRange(node) => node.$method($arg1, $arg2),
            Self::CurveKey(node) => node.$method($arg1, $arg2),
            Self::CurveEasing(node) => node.$method($arg1, $arg2),
            Self::Gradient(node) => node.$method($arg1, $arg2),
            Self::GradientStop(node) => node.$method($arg1, $arg2),
            Self::Script(node) => node.$method($arg1, $arg2),
            $(Self::$variant(node) => node.$method($arg1, $arg2),)*
        }
    };
    ($self:expr, $method:ident, $arg1:expr, $arg2:expr, $arg3:expr ; $($variant:ident),* $(,)?) => {
        match $self {
            Self::Folder(node) => node.$method($arg1, $arg2, $arg3),
            Self::UserContext(node) => node.$method($arg1, $arg2, $arg3),
            Self::UserContextFolder(node) => node.$method($arg1, $arg2, $arg3),
            Self::UserContextMultiplex(node) => node.$method($arg1, $arg2, $arg3),
            Self::UserContextMultiplexList(node) => node.$method($arg1, $arg2, $arg3),
            Self::Parameter(node) => node.$method($arg1, $arg2, $arg3),
            Self::Dashboard(node) => node.$method($arg1, $arg2, $arg3),
            Self::DashboardPage(node) => node.$method($arg1, $arg2, $arg3),
            Self::DashboardWidgetContainer(node) => node.$method($arg1, $arg2, $arg3),
            Self::DashboardNodeWidget(node) => node.$method($arg1, $arg2, $arg3),
            Self::DashboardGenericWidget(node) => node.$method($arg1, $arg2, $arg3),
            Self::DashboardNodeWidgetInspectorOptions(node) => node.$method($arg1, $arg2, $arg3),
            Self::DashboardNodeWidgetParameterEditorOptions(node) => node.$method($arg1, $arg2, $arg3),
            Self::DashboardNodeWidgetNumberSliderOptions(node) => node.$method($arg1, $arg2, $arg3),
            Self::DashboardNodeWidgetNumberRotaryOptions(node) => node.$method($arg1, $arg2, $arg3),
            Self::DashboardNodeWidgetVec2PadOptions(node) => node.$method($arg1, $arg2, $arg3),
            Self::DashboardNodeWidgetVec2EditorOptions(node) => node.$method($arg1, $arg2, $arg3),
            Self::DashboardNodeWidgetVec3EditorOptions(node) => node.$method($arg1, $arg2, $arg3),
            Self::DashboardNodeWidgetColorEditorOptions(node) => node.$method($arg1, $arg2, $arg3),
            Self::ParameterAnimationControl(node) => node.$method($arg1, $arg2, $arg3),
            Self::Curve(node) => node.$method($arg1, $arg2, $arg3),
            Self::CurveRange(node) => node.$method($arg1, $arg2, $arg3),
            Self::CurveKey(node) => node.$method($arg1, $arg2, $arg3),
            Self::CurveEasing(node) => node.$method($arg1, $arg2, $arg3),
            Self::Gradient(node) => node.$method($arg1, $arg2, $arg3),
            Self::GradientStop(node) => node.$method($arg1, $arg2, $arg3),
            Self::Script(node) => node.$method($arg1, $arg2, $arg3),
            $(Self::$variant(node) => node.$method($arg1, $arg2, $arg3),)*
        }
    };
    ($self:expr, $method:ident, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr ; $($variant:ident),* $(,)?) => {
        match $self {
            Self::Folder(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::UserContext(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::UserContextFolder(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::UserContextMultiplex(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::UserContextMultiplexList(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::Parameter(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::Dashboard(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::DashboardPage(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::DashboardWidgetContainer(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::DashboardNodeWidget(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::DashboardGenericWidget(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::DashboardNodeWidgetInspectorOptions(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::DashboardNodeWidgetParameterEditorOptions(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::DashboardNodeWidgetNumberSliderOptions(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::DashboardNodeWidgetNumberRotaryOptions(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::DashboardNodeWidgetVec2PadOptions(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::DashboardNodeWidgetVec2EditorOptions(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::DashboardNodeWidgetVec3EditorOptions(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::DashboardNodeWidgetColorEditorOptions(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::ParameterAnimationControl(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::Curve(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::CurveRange(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::CurveKey(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::CurveEasing(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::Gradient(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::GradientStop(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            Self::Script(node) => node.$method($arg1, $arg2, $arg3, $arg4),
            $(Self::$variant(node) => node.$method($arg1, $arg2, $arg3, $arg4),)*
        }
    };
    ($self:expr, $method:ident, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr ; $($variant:ident),* $(,)?) => {
        match $self {
            Self::Folder(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::UserContext(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::UserContextFolder(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::UserContextMultiplex(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::UserContextMultiplexList(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::Parameter(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::Dashboard(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::DashboardPage(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::DashboardWidgetContainer(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::DashboardNodeWidget(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::DashboardGenericWidget(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::DashboardNodeWidgetInspectorOptions(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::DashboardNodeWidgetParameterEditorOptions(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::DashboardNodeWidgetNumberSliderOptions(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::DashboardNodeWidgetNumberRotaryOptions(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::DashboardNodeWidgetVec2PadOptions(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::DashboardNodeWidgetVec2EditorOptions(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::DashboardNodeWidgetVec3EditorOptions(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::DashboardNodeWidgetColorEditorOptions(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::ParameterAnimationControl(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::Curve(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::CurveRange(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::CurveKey(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::CurveEasing(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::Gradient(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::GradientStop(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            Self::Script(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),
            $(Self::$variant(node) => node.$method($arg1, $arg2, $arg3, $arg4, $arg5),)*
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __downcast_node_enum_variant {
    ($any:ident, $variant:ident, $node_ty:ty) => {
        let $any = match $any.downcast::<$node_ty>() {
            Ok(node) => return Some(Self::$variant(node)),
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
///             type: OscModule,
///         },
///         {
///             type: DmxModule,
///             when: |this: &Self| this.allow_dmx,
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
                    type: $node_ty:ty
                    $(, node_type: $typed_node_type:expr)?
                    $(, item_kind: $typed_item_kind:expr)?
                    $(, label: $typed_label:expr)?
                    $(, menu_path: [$($typed_menu_path:expr),* $(,)?])?
                    $(, select_when_created: $typed_select_when_created:expr)?
                    $(, when: $typed_when:expr)?
                    $(, create: $typed_create:expr)?
                    $(,)?
                }
            ),* $(,)?
        ];
    ) => {
        fn user_container_rules(&self) -> Option<$crate::node::UserContainerRules> {
            Some($crate::node::UserContainerRules::new(&[$($accepted_kind),*]))
        }

        $crate::define_user_item_factory_methods! {
            @shared_typed
            items = [
                $(
                    {
                        type: $node_ty
                        $(, node_type: $typed_node_type)?
                        $(, item_kind: $typed_item_kind)?
                        $(, label: $typed_label)?
                        $(, menu_path: [$($typed_menu_path),*])?
                        $(, select_when_created: $typed_select_when_created)?
                        $(, when: $typed_when)?
                        $(, create: $typed_create)?
                    }
                ),*
            ];
        }
    };

    (
        items = [
            $(
                {
                    type: $node_ty:ty
                    $(, node_type: $typed_node_type:expr)?
                    $(, item_kind: $typed_item_kind:expr)?
                    $(, label: $typed_label:expr)?
                    $(, menu_path: [$($typed_menu_path:expr),* $(,)?])?
                    $(, select_when_created: $typed_select_when_created:expr)?
                    $(, when: $typed_when:expr)?
                    $(, create: $typed_create:expr)?
                    $(,)?
                }
            ),* $(,)?
        ];
    ) => {
        $crate::define_user_item_factory_methods! {
            @shared_typed
            items = [
                $(
                    {
                        type: $node_ty
                        $(, node_type: $typed_node_type)?
                        $(, item_kind: $typed_item_kind)?
                        $(, label: $typed_label)?
                        $(, menu_path: [$($typed_menu_path),*])?
                        $(, select_when_created: $typed_select_when_created)?
                        $(, when: $typed_when)?
                        $(, create: $typed_create)?
                    }
                ),*
            ];
        }
    };

    (
        accepts = [$($accepted_kind:literal),* $(,)?];
        items = [
            $(
                {
                    node_type: $node_type:literal,
                    item_kind: $item_kind:literal,
                    label: $label:expr,
                    $(menu_path: [$($menu_path:expr),* $(,)?],)?
                    $(select_when_created: $select_when_created:expr,)?
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
                        $(menu_path: [$($menu_path),*],)?
                        $(select_when_created: $select_when_created,)?
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
                    $(menu_path: [$($menu_path:expr),* $(,)?],)?
                    $(select_when_created: $select_when_created:expr,)?
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
                        $(menu_path: [$($menu_path),*],)?
                        $(select_when_created: $select_when_created,)?
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
                    $(menu_path: [$($menu_path:expr),* $(,)?],)?
                    $(select_when_created: $select_when_created:expr,)?
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
                    let item = $crate::node::UserCreatableItem::new($node_type, $item_kind, $label)
                        .with_select_when_created(
                            $crate::define_user_item_factory_methods!(@select_when_created $( $select_when_created )?),
                        );
                    let item = $crate::define_user_item_factory_methods!(@with_menu_path item $(, [$($menu_path),*])?);
                    items.push(item);
                }
            )*
            items
        }

        fn create_user_item(&self, node_type: &str) -> Option<Box<dyn $crate::node::Node>> {
            match node_type {
                $(
                    $node_type => {
                        if $crate::define_user_item_factory_methods!(@cond self $(, $when )?) {
                            let mut node = ($create)(self);
                            $crate::node::Node::node_data_mut(&mut node).meta.label = ($label).to_string();
                            Some(Box::new(node))
                        } else {
                            None
                        }
                    }
                )*
                _ => None,
            }
        }
    };

    (
        @shared_typed
        items = [
            $(
                {
                    type: $node_ty:ty
                    $(, node_type: $typed_node_type:expr)?
                    $(, item_kind: $typed_item_kind:expr)?
                    $(, label: $typed_label:expr)?
                    $(, menu_path: [$($typed_menu_path:expr),* $(,)?])?
                    $(, select_when_created: $typed_select_when_created:expr)?
                    $(, when: $typed_when:expr)?
                    $(, create: $typed_create:expr)?
                }
            ),* $(,)?
        ];
    ) => {
        fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
            if !self.user_container_rules().is_some_and(|rules| rules.accepts(item_kind)) {
                return false;
            }

            $(
                if item_type == $crate::define_user_item_factory_methods!(@typed_node_type $node_ty $(, $typed_node_type)?) {
                    return item_kind == $crate::define_user_item_factory_methods!(@typed_item_kind $node_ty $(, $typed_item_kind)?)
                        && $crate::define_user_item_factory_methods!(@cond self $(, $typed_when )?);
                }
            )*

            false
        }

        fn user_creatable_items(&self) -> Vec<$crate::node::UserCreatableItem> {
            let mut items = Vec::new();

            $(
                if $crate::define_user_item_factory_methods!(@cond self $(, $typed_when )?) {
                    let item = $crate::node::UserCreatableItem::new(
                        $crate::define_user_item_factory_methods!(@typed_node_type $node_ty $(, $typed_node_type)?),
                        $crate::define_user_item_factory_methods!(@typed_item_kind $node_ty $(, $typed_item_kind)?),
                        $crate::define_user_item_factory_methods!(@typed_label $node_ty $(, $typed_label)?),
                    )
                    .with_select_when_created(
                        $crate::define_user_item_factory_methods!(@select_when_created $( $typed_select_when_created )?),
                    );
                    let item = $crate::define_user_item_factory_methods!(
                        @with_typed_menu_path
                        item,
                        $node_ty
                        $(, [$($typed_menu_path),*])?
                        ;
                        $(node_type $typed_node_type)?
                        $(item_kind $typed_item_kind)?
                        $(label $typed_label)?
                        $(create $typed_create)?
                    );
                    items.push(item);
                }
            )*
            items
        }

        fn create_user_item(&self, node_type: &str) -> Option<Box<dyn $crate::node::Node>> {
            $(
                if node_type == $crate::define_user_item_factory_methods!(@typed_node_type $node_ty $(, $typed_node_type)?) {
                    if $crate::define_user_item_factory_methods!(@cond self $(, $typed_when )?) {
                        let __golden_label = $crate::define_user_item_factory_methods!(@typed_label $node_ty $(, $typed_label)?);
                        let mut node = $crate::define_user_item_factory_methods!(@typed_create self, $node_ty $(, $typed_create)?);
                        if $crate::node::Node::node_data(&node).meta.label != __golden_label {
                            $crate::node::Node::node_data_mut(&mut node).meta.label = __golden_label;
                        }
                        return Some(Box::new(node));
                    }
                    return None;
                }
            )*

            None
        }
    };

    (@typed_node_type $node_ty:ty, $node_type:expr) => {
        $node_type
    };

    (@typed_node_type $node_ty:ty) => {
        <$node_ty as $crate::node::DeclaredUserItemNode>::ITEM_NODE_TYPE
    };

    (@typed_item_kind $node_ty:ty, $item_kind:expr) => {
        $item_kind
    };

    (@typed_item_kind $node_ty:ty) => {
        <$node_ty as $crate::node::DeclaredUserItemNode>::ITEM_KIND
    };

    (@typed_label $node_ty:ty, $label:expr) => {
        {
            let __golden_label: ::std::string::String = ($label).into();
            __golden_label
        }
    };

    (@typed_label $node_ty:ty) => {
        <$node_ty as $crate::node::DeclaredUserItemNode>::item_default_label()
    };

    (@typed_create $self:expr, $node_ty:ty, $create:expr) => {
        ($create)($self)
    };

    (@typed_create $self:expr, $node_ty:ty) => {
        <$node_ty as $crate::node::DeclaredUserItemNode>::create_item()
    };

    (@cond $self:expr, $when:expr) => {
        ($when)($self)
    };

    (@cond $self:expr) => {
        true
    };

    (@select_when_created $select_when_created:expr) => {
        $select_when_created
    };

    (@select_when_created) => {
        true
    };

    (@with_menu_path $item:expr, [$($menu_path:expr),*]) => {
        $item.with_menu_path([$($menu_path),*])
    };

    (@with_menu_path $item:expr) => {
        $item
    };

    (@with_typed_menu_path $item:expr, $node_ty:ty, [$($menu_path:expr),*] ; $($custom:tt)*) => {
        $item.with_menu_path([$($menu_path),*])
    };

    (@with_typed_menu_path $item:expr, $node_ty:ty ;) => {{
        let __golden_menu_path = <$node_ty as $crate::node::DeclaredUserItemNode>::item_menu_path();
        if __golden_menu_path.is_empty() {
            $item
        } else {
            $item.with_menu_path(__golden_menu_path)
        }
    }};

    (@with_typed_menu_path $item:expr, $node_ty:ty ; $($custom:tt)+) => {
        $item
    };
}

/// Defines a node enum with static dispatch over `golden_core::node::Node`.
///
/// Internal node variants are always included automatically:
/// - `Folder($crate::node::Folder)`
/// - `UserContext($crate::node::UserContextNode)`
/// - `UserContextFolder($crate::node::UserContextFolder)`
/// - `Parameter($crate::parameter::Parameter)`
/// - `Dashboard($crate::node::DashboardNode)`
/// - `DashboardPage($crate::node::DashboardPageNode)`
/// - `DashboardWidgetContainer($crate::node::DashboardWidgetContainerNode)`
/// - `DashboardNodeWidget($crate::node::DashboardNodeWidgetNode)`
/// - `DashboardGenericWidget($crate::node::DashboardGenericWidgetNode)`
/// - `DashboardNodeWidgetInspectorOptions($crate::node::DashboardNodeWidgetInspectorOptionsNode)`
/// - `DashboardNodeWidgetParameterEditorOptions($crate::node::DashboardNodeWidgetParameterEditorOptionsNode)`
/// - `DashboardNodeWidgetNumberSliderOptions($crate::node::DashboardNodeWidgetNumberSliderOptionsNode)`
/// - `DashboardNodeWidgetNumberRotaryOptions($crate::node::DashboardNodeWidgetNumberRotaryOptionsNode)`
/// - `DashboardNodeWidgetVec2PadOptions($crate::node::DashboardNodeWidgetVec2PadOptionsNode)`
/// - `DashboardNodeWidgetVec2EditorOptions($crate::node::DashboardNodeWidgetVec2EditorOptionsNode)`
/// - `DashboardNodeWidgetVec3EditorOptions($crate::node::DashboardNodeWidgetVec3EditorOptionsNode)`
/// - `DashboardNodeWidgetColorEditorOptions($crate::node::DashboardNodeWidgetColorEditorOptionsNode)`
/// - `ParameterAnimationControl($crate::parameter::ParameterAnimationControlNode)`
/// - `Curve($crate::node::CurveNode)`
/// - `CurveRange($crate::node::CurveRangeNode)`
/// - `CurveKey($crate::node::CurveKeyNode)`
/// - `CurveEasing($crate::node::CurveEasingNode)`
/// - `Script($crate::script::ScriptNode)`
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
            Folder(Box<$crate::node::Folder>),
            UserContext(Box<$crate::node::UserContextNode>),
            UserContextFolder(Box<$crate::node::UserContextFolder>),
            UserContextMultiplex(Box<$crate::node::UserContextMultiplexNode>),
            UserContextMultiplexList(Box<$crate::node::UserContextMultiplexListNode>),
            Parameter(Box<$crate::parameter::Parameter>),
            Dashboard(Box<$crate::node::DashboardNode>),
            DashboardPage(Box<$crate::node::DashboardPageNode>),
            DashboardWidgetContainer(Box<$crate::node::DashboardWidgetContainerNode>),
            DashboardNodeWidget(Box<$crate::node::DashboardNodeWidgetNode>),
            DashboardGenericWidget(Box<$crate::node::DashboardGenericWidgetNode>),
            DashboardNodeWidgetInspectorOptions(Box<$crate::node::DashboardNodeWidgetInspectorOptionsNode>),
            DashboardNodeWidgetParameterEditorOptions(Box<$crate::node::DashboardNodeWidgetParameterEditorOptionsNode>),
            DashboardNodeWidgetNumberSliderOptions(Box<$crate::node::DashboardNodeWidgetNumberSliderOptionsNode>),
            DashboardNodeWidgetNumberRotaryOptions(Box<$crate::node::DashboardNodeWidgetNumberRotaryOptionsNode>),
            DashboardNodeWidgetVec2PadOptions(Box<$crate::node::DashboardNodeWidgetVec2PadOptionsNode>),
            DashboardNodeWidgetVec2EditorOptions(Box<$crate::node::DashboardNodeWidgetVec2EditorOptionsNode>),
            DashboardNodeWidgetVec3EditorOptions(Box<$crate::node::DashboardNodeWidgetVec3EditorOptionsNode>),
            DashboardNodeWidgetColorEditorOptions(Box<$crate::node::DashboardNodeWidgetColorEditorOptionsNode>),
            ParameterAnimationControl(Box<$crate::parameter::ParameterAnimationControlNode>),
            Curve(Box<$crate::node::CurveNode>),
            CurveRange(Box<$crate::node::CurveRangeNode>),
            CurveKey(Box<$crate::node::CurveKeyNode>),
            CurveEasing(Box<$crate::node::CurveEasingNode>),
            Gradient(Box<$crate::node::GradientNode>),
            GradientStop(Box<$crate::node::GradientStopNode>),
            Script(Box<$crate::script::ScriptNode>),
            $($variant(Box<$node_ty>),)*
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
            fn as_any(&self) -> &dyn std::any::Any {
                match self {
                    Self::Folder(node) => node.as_ref(),
                    Self::UserContext(node) => node.as_ref(),
                    Self::UserContextFolder(node) => node.as_ref(),
                    Self::UserContextMultiplex(node) => node.as_ref(),
                    Self::UserContextMultiplexList(node) => node.as_ref(),
                    Self::Parameter(node) => node.as_ref(),
                    Self::Dashboard(node) => node.as_ref(),
                    Self::DashboardPage(node) => node.as_ref(),
                    Self::DashboardWidgetContainer(node) => node.as_ref(),
                    Self::DashboardNodeWidget(node) => node.as_ref(),
                    Self::DashboardGenericWidget(node) => node.as_ref(),
                    Self::DashboardNodeWidgetInspectorOptions(node) => node.as_ref(),
                    Self::DashboardNodeWidgetParameterEditorOptions(node) => node.as_ref(),
                    Self::DashboardNodeWidgetNumberSliderOptions(node) => node.as_ref(),
                    Self::DashboardNodeWidgetNumberRotaryOptions(node) => node.as_ref(),
                    Self::DashboardNodeWidgetVec2PadOptions(node) => node.as_ref(),
                    Self::DashboardNodeWidgetVec2EditorOptions(node) => node.as_ref(),
                    Self::DashboardNodeWidgetVec3EditorOptions(node) => node.as_ref(),
                    Self::DashboardNodeWidgetColorEditorOptions(node) => node.as_ref(),
                    Self::ParameterAnimationControl(node) => node.as_ref(),
                    Self::Curve(node) => node.as_ref(),
                    Self::CurveRange(node) => node.as_ref(),
                    Self::CurveKey(node) => node.as_ref(),
                    Self::CurveEasing(node) => node.as_ref(),
                    Self::Gradient(node) => node.as_ref(),
                    Self::GradientStop(node) => node.as_ref(),
                    Self::Script(node) => node.as_ref(),
                    $(Self::$variant(node) => node.as_ref(),)*
                }
            }

            #[inline(always)]
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                match self {
                    Self::Folder(node) => node.as_mut(),
                    Self::UserContext(node) => node.as_mut(),
                    Self::UserContextFolder(node) => node.as_mut(),
                    Self::UserContextMultiplex(node) => node.as_mut(),
                    Self::UserContextMultiplexList(node) => node.as_mut(),
                    Self::Parameter(node) => node.as_mut(),
                    Self::Dashboard(node) => node.as_mut(),
                    Self::DashboardPage(node) => node.as_mut(),
                    Self::DashboardWidgetContainer(node) => node.as_mut(),
                    Self::DashboardNodeWidget(node) => node.as_mut(),
                    Self::DashboardGenericWidget(node) => node.as_mut(),
                    Self::DashboardNodeWidgetInspectorOptions(node) => node.as_mut(),
                    Self::DashboardNodeWidgetParameterEditorOptions(node) => node.as_mut(),
                    Self::DashboardNodeWidgetNumberSliderOptions(node) => node.as_mut(),
                    Self::DashboardNodeWidgetNumberRotaryOptions(node) => node.as_mut(),
                    Self::DashboardNodeWidgetVec2PadOptions(node) => node.as_mut(),
                    Self::DashboardNodeWidgetVec2EditorOptions(node) => node.as_mut(),
                    Self::DashboardNodeWidgetVec3EditorOptions(node) => node.as_mut(),
                    Self::DashboardNodeWidgetColorEditorOptions(node) => node.as_mut(),
                    Self::ParameterAnimationControl(node) => node.as_mut(),
                    Self::Curve(node) => node.as_mut(),
                    Self::CurveRange(node) => node.as_mut(),
                    Self::CurveKey(node) => node.as_mut(),
                    Self::CurveEasing(node) => node.as_mut(),
                    Self::Gradient(node) => node.as_mut(),
                    Self::GradientStop(node) => node.as_mut(),
                    Self::Script(node) => node.as_mut(),
                    $(Self::$variant(node) => node.as_mut(),)*
                }
            }

            #[inline(always)]
            fn user_item_kind(&self) -> &str {
                $crate::__dispatch_node_enum!(self, user_item_kind; $($variant),*)
            }

            #[inline(always)]
            fn type_description(&self) -> Option<&str> {
                $crate::__dispatch_node_enum!(self, type_description; $($variant),*)
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
            fn script_host_policy(&self) -> Option<$crate::script::ScriptHostPolicy> {
                $crate::__dispatch_node_enum!(self, script_host_policy; $($variant),*)
            }

            #[inline(always)]
            fn user_context_host_policy(&self) -> Option<$crate::node::UserContextHostPolicy> {
                $crate::__dispatch_node_enum!(self, user_context_host_policy; $($variant),*)
            }

            #[inline(always)]
            fn engine_script_state(&self) -> Option<$crate::script::ScriptUiState> {
                $crate::__dispatch_node_enum!(self, engine_script_state; $($variant),*)
            }

            #[inline(always)]
            fn engine_set_script_config(&mut self, config: $crate::script::ScriptNodeConfig, force_reload: bool) -> Result<(), String> {
                $crate::__dispatch_node_enum!(self, engine_set_script_config, config, force_reload; $($variant),*)
            }

            #[inline(always)]
            fn engine_request_script_reload(&mut self) -> Result<(), String> {
                $crate::__dispatch_node_enum!(self, engine_request_script_reload; $($variant),*)
            }

            #[inline(always)]
            fn project_encode_data(&self) -> Result<serde_json::Value, String> {
                $crate::__dispatch_node_enum!(self, project_encode_data; $($variant),*)
            }

            #[inline(always)]
            fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
                $crate::__dispatch_node_enum!(self, project_decode_data, data; $($variant),*)
            }

            #[inline(always)]
            fn project_create(node_type: &str) -> Option<Self>
            where
                Self: Sized,
            {
                <Self as $crate::app::ProjectNode>::project_create_node(node_type)
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
            fn user_creatable_items_with_context(
                &self,
                snapshot: &$crate::process_ctx::ProcessTreeSnapshot,
                parent: $crate::node::NodeId,
                child_catalog: &dyn Fn($crate::node::NodeId) -> Vec<$crate::node::UserCreatableItem>,
            ) -> Vec<$crate::node::UserCreatableItem> {
                $crate::__dispatch_node_enum!(
                    self,
                    user_creatable_items_with_context,
                    snapshot,
                    parent,
                    child_catalog;
                    $($variant),*
                )
            }

            #[inline(always)]
            fn create_user_item(&self, node_type: &str) -> Option<Box<dyn $crate::node::Node>> {
                $crate::__dispatch_node_enum!(self, create_user_item, node_type; $($variant),*)
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
            fn engine_dashboard_widget_target_descriptor(&self) -> $crate::node::DashboardWidgetTargetDescriptor {
                $crate::__dispatch_node_enum!(self, engine_dashboard_widget_target_descriptor; $($variant),*)
            }

            #[inline(always)]
            fn engine_param_control_state(&self) -> Option<$crate::parameter::ParameterControlState> {
                $crate::__dispatch_node_enum!(self, engine_param_control_state; $($variant),*)
            }

            #[inline(always)]
            fn engine_set_param_control_state(&mut self, state: $crate::parameter::ParameterControlState) -> Result<(), String> {
                $crate::__dispatch_node_enum!(self, engine_set_param_control_state, state; $($variant),*)
            }

            #[inline(always)]
            fn engine_set_param_constraints(&mut self, constraints: $crate::parameter::ParameterConstraints) -> Result<(), String> {
                $crate::__dispatch_node_enum!(self, engine_set_param_constraints, constraints; $($variant),*)
            }

            #[inline(always)]
            fn engine_restore_param_state(
                &mut self,
                value: $crate::parameter::ParamValue,
                constraints: $crate::parameter::ParameterConstraints,
            ) -> Result<(), String> {
                $crate::__dispatch_node_enum!(self, engine_restore_param_state, value, constraints; $($variant),*)
            }

            #[inline(always)]
            fn engine_script_descriptor(&self) -> $crate::node::NodeScriptDescriptor {
                $crate::__dispatch_node_enum!(self, engine_script_descriptor; $($variant),*)
            }

            #[inline(always)]
            fn engine_set_script_property(
                &mut self,
                ctx: &mut $crate::process_ctx::ProcessCtx,
                property: &str,
                value: $crate::parameter::ParamValue,
            ) -> Result<bool, String> {
                $crate::__dispatch_node_enum!(self, engine_set_script_property, ctx, property, value; $($variant),*)
            }

            #[inline(always)]
            fn engine_call_script_method(
                &mut self,
                ctx: &mut $crate::process_ctx::ProcessCtx,
                method: &str,
                args: &[$crate::parameter::ParamValue],
            ) -> Result<bool, String> {
                $crate::__dispatch_node_enum!(self, engine_call_script_method, ctx, method, args; $($variant),*)
            }

            #[inline(always)]
            fn engine_visit_references(&self, visit: &mut dyn FnMut(&$crate::node::NodeReference)) {
                $crate::__dispatch_node_enum!(self, engine_visit_references, visit; $($variant),*)
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
            fn engine_materialize_declared_inbox(&mut self, ctx: &mut $crate::process_ctx::ProcessCtx) {
                $crate::__dispatch_node_enum!(self, engine_materialize_declared_inbox, ctx; $($variant),*)
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
            fn on_node_ready(
                &mut self,
                ctx: &mut $crate::process_ctx::ProcessCtx,
                context: $crate::node::NodeCreationContext,
            ) {
                $crate::__dispatch_node_enum!(self, on_node_ready, ctx, context; $($variant),*)
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
            fn needs_update(&self) -> bool {
                $crate::__dispatch_node_enum!(self, needs_update; $($variant),*)
            }

            #[inline(always)]
            fn update_requires_tree_snapshot(&self) -> bool {
                $crate::__dispatch_node_enum!(self, update_requires_tree_snapshot; $($variant),*)
            }

            #[inline(always)]
            fn inbox_requires_tree_snapshot(&self, events: &$crate::events::EventFrame) -> bool {
                $crate::__dispatch_node_enum!(self, inbox_requires_tree_snapshot, events; $($variant),*)
            }

            #[inline(always)]
            fn lifecycle_requires_tree_snapshot(&self) -> bool {
                $crate::__dispatch_node_enum!(self, lifecycle_requires_tree_snapshot; $($variant),*)
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
            fn on_effective_enabled_changed(&mut self, ctx: &mut $crate::process_ctx::ProcessCtx, enabled: bool) {
                $crate::__dispatch_node_enum!(self, on_effective_enabled_changed, ctx, enabled; $($variant),*)
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
                $crate::__downcast_node_enum_variant!(any, UserContext, $crate::node::UserContextNode);
                $crate::__downcast_node_enum_variant!(any, UserContextFolder, $crate::node::UserContextFolder);
                $crate::__downcast_node_enum_variant!(any, UserContextMultiplex, $crate::node::UserContextMultiplexNode);
                $crate::__downcast_node_enum_variant!(any, UserContextMultiplexList, $crate::node::UserContextMultiplexListNode);
                $crate::__downcast_node_enum_variant!(any, Parameter, $crate::parameter::Parameter);
                $crate::__downcast_node_enum_variant!(any, Dashboard, $crate::node::DashboardNode);
                $crate::__downcast_node_enum_variant!(any, DashboardPage, $crate::node::DashboardPageNode);
                $crate::__downcast_node_enum_variant!(any, DashboardWidgetContainer, $crate::node::DashboardWidgetContainerNode);
                $crate::__downcast_node_enum_variant!(any, DashboardNodeWidget, $crate::node::DashboardNodeWidgetNode);
                $crate::__downcast_node_enum_variant!(any, DashboardGenericWidget, $crate::node::DashboardGenericWidgetNode);
                $crate::__downcast_node_enum_variant!(any, DashboardNodeWidgetInspectorOptions, $crate::node::DashboardNodeWidgetInspectorOptionsNode);
                $crate::__downcast_node_enum_variant!(any, DashboardNodeWidgetParameterEditorOptions, $crate::node::DashboardNodeWidgetParameterEditorOptionsNode);
                $crate::__downcast_node_enum_variant!(any, DashboardNodeWidgetNumberSliderOptions, $crate::node::DashboardNodeWidgetNumberSliderOptionsNode);
                $crate::__downcast_node_enum_variant!(any, DashboardNodeWidgetNumberRotaryOptions, $crate::node::DashboardNodeWidgetNumberRotaryOptionsNode);
                $crate::__downcast_node_enum_variant!(any, DashboardNodeWidgetVec2PadOptions, $crate::node::DashboardNodeWidgetVec2PadOptionsNode);
                $crate::__downcast_node_enum_variant!(any, DashboardNodeWidgetVec2EditorOptions, $crate::node::DashboardNodeWidgetVec2EditorOptionsNode);
                $crate::__downcast_node_enum_variant!(any, DashboardNodeWidgetVec3EditorOptions, $crate::node::DashboardNodeWidgetVec3EditorOptionsNode);
                $crate::__downcast_node_enum_variant!(any, DashboardNodeWidgetColorEditorOptions, $crate::node::DashboardNodeWidgetColorEditorOptionsNode);
                $crate::__downcast_node_enum_variant!(any, ParameterAnimationControl, $crate::parameter::ParameterAnimationControlNode);
                $crate::__downcast_node_enum_variant!(any, Curve, $crate::node::CurveNode);
                $crate::__downcast_node_enum_variant!(any, CurveRange, $crate::node::CurveRangeNode);
                $crate::__downcast_node_enum_variant!(any, CurveKey, $crate::node::CurveKeyNode);
                $crate::__downcast_node_enum_variant!(any, CurveEasing, $crate::node::CurveEasingNode);
                $crate::__downcast_node_enum_variant!(any, Gradient, $crate::node::GradientNode);
                $crate::__downcast_node_enum_variant!(any, GradientStop, $crate::node::GradientStopNode);
                $crate::__downcast_node_enum_variant!(any, Script, $crate::script::ScriptNode);

                $(
                    $crate::__downcast_node_enum_variant!(any, $variant, $node_ty);
                )*

                let _ = any;
                None
            }
        }

        impl $crate::app::ProjectNode for $enum_name {
            fn project_create_node(node_type: &str) -> Option<Self> {
                if let Some(node) = <$crate::node::Folder as $crate::node::Node>::project_create(node_type) {
                    return Some(Self::Folder(Box::new(node)));
                }
                if let Some(node) = <$crate::node::UserContextNode as $crate::node::Node>::project_create(node_type) {
                    return Some(Self::UserContext(Box::new(node)));
                }
                if let Some(node) = <$crate::node::UserContextFolder as $crate::node::Node>::project_create(node_type) {
                    return Some(Self::UserContextFolder(Box::new(node)));
                }
                if let Some(node) = <$crate::node::UserContextMultiplexNode as $crate::node::Node>::project_create(node_type) {
                    return Some(Self::UserContextMultiplex(Box::new(node)));
                }
                if let Some(node) = <$crate::node::UserContextMultiplexListNode as $crate::node::Node>::project_create(node_type) {
                    return Some(Self::UserContextMultiplexList(Box::new(node)));
                }
                if let Some(node) = <$crate::parameter::Parameter as $crate::node::Node>::project_create(node_type) {
                    return Some(Self::Parameter(Box::new(node)));
                }
                if node_type == $crate::node::DASHBOARD_NODE_TYPE {
                    return Some(Self::Dashboard(Box::new($crate::node::DashboardNode::new())));
                }
                if node_type == $crate::node::DASHBOARD_PAGE_NODE_TYPE {
                    return Some(Self::DashboardPage(Box::new($crate::node::DashboardPageNode::new())));
                }
                if node_type == $crate::node::DASHBOARD_WIDGET_CONTAINER_NODE_TYPE {
                    return Some(Self::DashboardWidgetContainer(Box::new($crate::node::DashboardWidgetContainerNode::new())));
                }
                if node_type == $crate::node::DASHBOARD_NODE_WIDGET_NODE_TYPE {
                    return Some(Self::DashboardNodeWidget(Box::new($crate::node::DashboardNodeWidgetNode::new())));
                }
                if node_type == $crate::node::DASHBOARD_GENERIC_WIDGET_NODE_TYPE {
                    return Some(Self::DashboardGenericWidget(Box::new($crate::node::DashboardGenericWidgetNode::new())));
                }
                if node_type == $crate::node::DASHBOARD_NODE_WIDGET_INSPECTOR_OPTIONS_NODE_TYPE {
                    return Some(Self::DashboardNodeWidgetInspectorOptions(
                        Box::new($crate::node::DashboardNodeWidgetInspectorOptionsNode::new()),
                    ));
                }
                if node_type == $crate::node::DASHBOARD_NODE_WIDGET_PARAMETER_EDITOR_OPTIONS_NODE_TYPE {
                    return Some(Self::DashboardNodeWidgetParameterEditorOptions(
                        Box::new($crate::node::DashboardNodeWidgetParameterEditorOptionsNode::new()),
                    ));
                }
                if node_type == $crate::node::DASHBOARD_NODE_WIDGET_NUMBER_SLIDER_OPTIONS_NODE_TYPE {
                    return Some(Self::DashboardNodeWidgetNumberSliderOptions(
                        Box::new($crate::node::DashboardNodeWidgetNumberSliderOptionsNode::new()),
                    ));
                }
                if node_type == $crate::node::DASHBOARD_NODE_WIDGET_NUMBER_ROTARY_OPTIONS_NODE_TYPE {
                    return Some(Self::DashboardNodeWidgetNumberRotaryOptions(
                        Box::new($crate::node::DashboardNodeWidgetNumberRotaryOptionsNode::new()),
                    ));
                }
                if node_type == $crate::node::DASHBOARD_NODE_WIDGET_VEC2_PAD_OPTIONS_NODE_TYPE {
                    return Some(Self::DashboardNodeWidgetVec2PadOptions(
                        Box::new($crate::node::DashboardNodeWidgetVec2PadOptionsNode::new()),
                    ));
                }
                if node_type == $crate::node::DASHBOARD_NODE_WIDGET_VEC2_EDITOR_OPTIONS_NODE_TYPE {
                    return Some(Self::DashboardNodeWidgetVec2EditorOptions(
                        Box::new($crate::node::DashboardNodeWidgetVec2EditorOptionsNode::new()),
                    ));
                }
                if node_type == $crate::node::DASHBOARD_NODE_WIDGET_VEC3_EDITOR_OPTIONS_NODE_TYPE {
                    return Some(Self::DashboardNodeWidgetVec3EditorOptions(
                        Box::new($crate::node::DashboardNodeWidgetVec3EditorOptionsNode::new()),
                    ));
                }
                if node_type == $crate::node::DASHBOARD_NODE_WIDGET_COLOR_EDITOR_OPTIONS_NODE_TYPE {
                    return Some(Self::DashboardNodeWidgetColorEditorOptions(
                        Box::new($crate::node::DashboardNodeWidgetColorEditorOptionsNode::new()),
                    ));
                }
                if node_type == $crate::node::PARAMETER_ANIMATION_CONTROL_NODE_TYPE {
                    return Some(Self::ParameterAnimationControl(
                        Box::new($crate::parameter::ParameterAnimationControlNode::new("Animation")),
                    ));
                }
                if node_type == $crate::node::PARAMETER_ANIMATION_CURVE_NODE_TYPE {
                    return Some(Self::Curve(Box::new($crate::node::CurveNode::new_with_label("Curve"))));
                }
                if node_type == $crate::node::PARAMETER_ANIMATION_RANGE_NODE_TYPE {
                    return Some(Self::CurveRange(Box::new($crate::node::CurveRangeNode::new(None, true))));
                }
                if node_type == $crate::node::PARAMETER_ANIMATION_KEY_NODE_TYPE {
                    return Some(Self::CurveKey(Box::new($crate::node::CurveKeyNode::new_with_label("Key"))));
                }
                if node_type == $crate::node::PARAMETER_ANIMATION_EASING_NODE_TYPE {
                    return Some(Self::CurveEasing(Box::new($crate::node::CurveEasingNode::new())));
                }
                if node_type == $crate::node::GRADIENT_NODE_TYPE {
                    return Some(Self::Gradient(Box::new($crate::node::GradientNode::new_with_label("Gradient"))));
                }
                if node_type == $crate::node::GRADIENT_STOP_NODE_TYPE {
                    return Some(Self::GradientStop(Box::new($crate::node::GradientStopNode::new_with_label("Stop"))));
                }
                if let Some(node) = <$crate::script::ScriptNode as $crate::node::Node>::project_create(node_type) {
                    return Some(Self::Script(Box::new(node)));
                }

                $(
                    if let Some(node) = <$node_ty as $crate::node::Node>::project_create(node_type) {
                        return Some(Self::$variant(Box::new(node)));
                    }
                )*

                None
            }

            fn project_decode_node(node_type: &str, data: &serde_json::Value, meta: &$crate::node::NodeMeta) -> Result<Self, String> {
                if let Some(mut node) = <$crate::node::Folder as $crate::node::Node>::project_create(node_type) {
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::Folder(Box::new(node)));
                }
                if let Some(mut node) = <$crate::node::UserContextNode as $crate::node::Node>::project_create(node_type) {
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::UserContext(Box::new(node)));
                }
                if let Some(mut node) = <$crate::node::UserContextFolder as $crate::node::Node>::project_create(node_type) {
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::UserContextFolder(Box::new(node)));
                }
                if let Some(mut node) = <$crate::node::UserContextMultiplexNode as $crate::node::Node>::project_create(node_type) {
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::UserContextMultiplex(Box::new(node)));
                }
                if let Some(mut node) = <$crate::node::UserContextMultiplexListNode as $crate::node::Node>::project_create(node_type) {
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::UserContextMultiplexList(Box::new(node)));
                }
                if let Some(mut node) = <$crate::parameter::Parameter as $crate::node::Node>::project_create(node_type) {
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::Parameter(Box::new(node)));
                }
                if node_type == $crate::node::DASHBOARD_NODE_TYPE {
                    let mut node = $crate::node::DashboardNode::new();
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::Dashboard(Box::new(node)));
                }
                if node_type == $crate::node::DASHBOARD_PAGE_NODE_TYPE {
                    let mut node = $crate::node::DashboardPageNode::new();
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::DashboardPage(Box::new(node)));
                }
                if node_type == $crate::node::DASHBOARD_WIDGET_CONTAINER_NODE_TYPE {
                    let mut node = $crate::node::DashboardWidgetContainerNode::new();
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::DashboardWidgetContainer(Box::new(node)));
                }
                if node_type == $crate::node::DASHBOARD_NODE_WIDGET_NODE_TYPE {
                    let mut node = $crate::node::DashboardNodeWidgetNode::new();
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::DashboardNodeWidget(Box::new(node)));
                }
                if node_type == $crate::node::DASHBOARD_GENERIC_WIDGET_NODE_TYPE {
                    let mut node = $crate::node::DashboardGenericWidgetNode::new();
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::DashboardGenericWidget(Box::new(node)));
                }
                if node_type == $crate::node::DASHBOARD_NODE_WIDGET_INSPECTOR_OPTIONS_NODE_TYPE {
                    let mut node = $crate::node::DashboardNodeWidgetInspectorOptionsNode::new();
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::DashboardNodeWidgetInspectorOptions(Box::new(node)));
                }
                if node_type == $crate::node::DASHBOARD_NODE_WIDGET_PARAMETER_EDITOR_OPTIONS_NODE_TYPE {
                    let mut node = $crate::node::DashboardNodeWidgetParameterEditorOptionsNode::new();
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::DashboardNodeWidgetParameterEditorOptions(Box::new(node)));
                }
                if node_type == $crate::node::DASHBOARD_NODE_WIDGET_NUMBER_SLIDER_OPTIONS_NODE_TYPE {
                    let mut node = $crate::node::DashboardNodeWidgetNumberSliderOptionsNode::new();
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::DashboardNodeWidgetNumberSliderOptions(Box::new(node)));
                }
                if node_type == $crate::node::DASHBOARD_NODE_WIDGET_NUMBER_ROTARY_OPTIONS_NODE_TYPE {
                    let mut node = $crate::node::DashboardNodeWidgetNumberRotaryOptionsNode::new();
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::DashboardNodeWidgetNumberRotaryOptions(Box::new(node)));
                }
                if node_type == $crate::node::DASHBOARD_NODE_WIDGET_VEC2_PAD_OPTIONS_NODE_TYPE {
                    let mut node = $crate::node::DashboardNodeWidgetVec2PadOptionsNode::new();
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::DashboardNodeWidgetVec2PadOptions(Box::new(node)));
                }
                if node_type == $crate::node::DASHBOARD_NODE_WIDGET_VEC2_EDITOR_OPTIONS_NODE_TYPE {
                    let mut node = $crate::node::DashboardNodeWidgetVec2EditorOptionsNode::new();
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::DashboardNodeWidgetVec2EditorOptions(Box::new(node)));
                }
                if node_type == $crate::node::DASHBOARD_NODE_WIDGET_VEC3_EDITOR_OPTIONS_NODE_TYPE {
                    let mut node = $crate::node::DashboardNodeWidgetVec3EditorOptionsNode::new();
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::DashboardNodeWidgetVec3EditorOptions(Box::new(node)));
                }
                if node_type == $crate::node::DASHBOARD_NODE_WIDGET_COLOR_EDITOR_OPTIONS_NODE_TYPE {
                    let mut node = $crate::node::DashboardNodeWidgetColorEditorOptionsNode::new();
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::DashboardNodeWidgetColorEditorOptions(Box::new(node)));
                }
                if node_type == $crate::node::PARAMETER_ANIMATION_CONTROL_NODE_TYPE {
                    let mut node = $crate::parameter::ParameterAnimationControlNode::new(meta.label.clone());
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::ParameterAnimationControl(Box::new(node)));
                }
                if node_type == $crate::node::PARAMETER_ANIMATION_CURVE_NODE_TYPE {
                    let mut node = $crate::node::CurveNode::new_with_label(meta.label.clone());
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::Curve(Box::new(node)));
                }
                if node_type == $crate::node::PARAMETER_ANIMATION_RANGE_NODE_TYPE {
                    let mut node = $crate::node::CurveRangeNode::new(None, true);
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::CurveRange(Box::new(node)));
                }
                if node_type == $crate::node::PARAMETER_ANIMATION_KEY_NODE_TYPE {
                    let mut node = $crate::node::CurveKeyNode::new_with_label(meta.label.clone());
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::CurveKey(Box::new(node)));
                }
                if node_type == $crate::node::PARAMETER_ANIMATION_EASING_NODE_TYPE {
                    let mut node = $crate::node::CurveEasingNode::new();
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::CurveEasing(Box::new(node)));
                }
                if node_type == $crate::node::GRADIENT_NODE_TYPE {
                    let mut node = $crate::node::GradientNode::new_with_label(meta.label.clone());
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::Gradient(Box::new(node)));
                }
                if node_type == $crate::node::GRADIENT_STOP_NODE_TYPE {
                    let mut node = $crate::node::GradientStopNode::new_with_label(meta.label.clone());
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::GradientStop(Box::new(node)));
                }
                if let Some(mut node) = <$crate::script::ScriptNode as $crate::node::Node>::project_create(node_type) {
                    $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                    $crate::node::Node::project_decode_data(&mut node, data)?;
                    return Ok(Self::Script(Box::new(node)));
                }

                $(
                    if let Some(mut node) = <$node_ty as $crate::node::Node>::project_create(node_type) {
                        $crate::node::Node::node_data_mut(&mut node).meta.label = meta.label.clone();
                        $crate::node::Node::project_decode_data(&mut node, data)?;
                        return Ok(Self::$variant(Box::new(node)));
                    }
                )*

                Err(format!("unsupported node type '{node_type}'"))
            }
        }

        impl From<$crate::node::Folder> for $enum_name {
            fn from(node: $crate::node::Folder) -> Self {
                Self::Folder(Box::new(node))
            }
        }

        impl From<$crate::node::UserContextNode> for $enum_name {
            fn from(node: $crate::node::UserContextNode) -> Self {
                Self::UserContext(Box::new(node))
            }
        }

        impl From<$crate::node::UserContextFolder> for $enum_name {
            fn from(node: $crate::node::UserContextFolder) -> Self {
                Self::UserContextFolder(Box::new(node))
            }
        }

        impl From<$crate::node::UserContextMultiplexNode> for $enum_name {
            fn from(node: $crate::node::UserContextMultiplexNode) -> Self {
                Self::UserContextMultiplex(Box::new(node))
            }
        }

        impl From<$crate::node::UserContextMultiplexListNode> for $enum_name {
            fn from(node: $crate::node::UserContextMultiplexListNode) -> Self {
                Self::UserContextMultiplexList(Box::new(node))
            }
        }

        impl From<$crate::parameter::Parameter> for $enum_name {
            fn from(node: $crate::parameter::Parameter) -> Self {
                Self::Parameter(Box::new(node))
            }
        }

        impl From<$crate::node::DashboardNode> for $enum_name {
            fn from(node: $crate::node::DashboardNode) -> Self {
                Self::Dashboard(Box::new(node))
            }
        }

        impl From<$crate::node::DashboardPageNode> for $enum_name {
            fn from(node: $crate::node::DashboardPageNode) -> Self {
                Self::DashboardPage(Box::new(node))
            }
        }

        impl From<$crate::node::DashboardWidgetContainerNode> for $enum_name {
            fn from(node: $crate::node::DashboardWidgetContainerNode) -> Self {
                Self::DashboardWidgetContainer(Box::new(node))
            }
        }

        impl From<$crate::node::DashboardNodeWidgetNode> for $enum_name {
            fn from(node: $crate::node::DashboardNodeWidgetNode) -> Self {
                Self::DashboardNodeWidget(Box::new(node))
            }
        }

        impl From<$crate::node::DashboardGenericWidgetNode> for $enum_name {
            fn from(node: $crate::node::DashboardGenericWidgetNode) -> Self {
                Self::DashboardGenericWidget(Box::new(node))
            }
        }

        impl From<$crate::node::DashboardNodeWidgetInspectorOptionsNode> for $enum_name {
            fn from(node: $crate::node::DashboardNodeWidgetInspectorOptionsNode) -> Self {
                Self::DashboardNodeWidgetInspectorOptions(Box::new(node))
            }
        }

        impl From<$crate::node::DashboardNodeWidgetParameterEditorOptionsNode> for $enum_name {
            fn from(node: $crate::node::DashboardNodeWidgetParameterEditorOptionsNode) -> Self {
                Self::DashboardNodeWidgetParameterEditorOptions(Box::new(node))
            }
        }

        impl From<$crate::node::DashboardNodeWidgetNumberSliderOptionsNode> for $enum_name {
            fn from(node: $crate::node::DashboardNodeWidgetNumberSliderOptionsNode) -> Self {
                Self::DashboardNodeWidgetNumberSliderOptions(Box::new(node))
            }
        }

        impl From<$crate::node::DashboardNodeWidgetNumberRotaryOptionsNode> for $enum_name {
            fn from(node: $crate::node::DashboardNodeWidgetNumberRotaryOptionsNode) -> Self {
                Self::DashboardNodeWidgetNumberRotaryOptions(Box::new(node))
            }
        }

        impl From<$crate::node::DashboardNodeWidgetVec2PadOptionsNode> for $enum_name {
            fn from(node: $crate::node::DashboardNodeWidgetVec2PadOptionsNode) -> Self {
                Self::DashboardNodeWidgetVec2PadOptions(Box::new(node))
            }
        }

        impl From<$crate::node::DashboardNodeWidgetVec2EditorOptionsNode> for $enum_name {
            fn from(node: $crate::node::DashboardNodeWidgetVec2EditorOptionsNode) -> Self {
                Self::DashboardNodeWidgetVec2EditorOptions(Box::new(node))
            }
        }

        impl From<$crate::node::DashboardNodeWidgetVec3EditorOptionsNode> for $enum_name {
            fn from(node: $crate::node::DashboardNodeWidgetVec3EditorOptionsNode) -> Self {
                Self::DashboardNodeWidgetVec3EditorOptions(Box::new(node))
            }
        }

        impl From<$crate::node::DashboardNodeWidgetColorEditorOptionsNode> for $enum_name {
            fn from(node: $crate::node::DashboardNodeWidgetColorEditorOptionsNode) -> Self {
                Self::DashboardNodeWidgetColorEditorOptions(Box::new(node))
            }
        }

        impl From<$crate::parameter::ParameterAnimationControlNode> for $enum_name {
            fn from(node: $crate::parameter::ParameterAnimationControlNode) -> Self {
                Self::ParameterAnimationControl(Box::new(node))
            }
        }

        impl From<$crate::node::CurveNode> for $enum_name {
            fn from(node: $crate::node::CurveNode) -> Self {
                Self::Curve(Box::new(node))
            }
        }

        impl From<$crate::node::CurveRangeNode> for $enum_name {
            fn from(node: $crate::node::CurveRangeNode) -> Self {
                Self::CurveRange(Box::new(node))
            }
        }

        impl From<$crate::node::CurveKeyNode> for $enum_name {
            fn from(node: $crate::node::CurveKeyNode) -> Self {
                Self::CurveKey(Box::new(node))
            }
        }

        impl From<$crate::node::CurveEasingNode> for $enum_name {
            fn from(node: $crate::node::CurveEasingNode) -> Self {
                Self::CurveEasing(Box::new(node))
            }
        }

        impl From<$crate::node::GradientNode> for $enum_name {
            fn from(node: $crate::node::GradientNode) -> Self {
                Self::Gradient(Box::new(node))
            }
        }

        impl From<$crate::node::GradientStopNode> for $enum_name {
            fn from(node: $crate::node::GradientStopNode) -> Self {
                Self::GradientStop(Box::new(node))
            }
        }

        impl From<$crate::script::ScriptNode> for $enum_name {
            fn from(node: $crate::script::ScriptNode) -> Self {
                Self::Script(Box::new(node))
            }
        }


        $(
            impl From<$node_ty> for $enum_name {
                fn from(node: $node_ty) -> Self {
                    Self::$variant(Box::new(node))
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
