/// Creation context for one node instance after attachment, decode, and child reconciliation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeCreationContext {
    /// Fresh node inserted by regular runtime editing or blueprint creation.
    Fresh,
    /// Node recreated while loading a persisted project.
    ProjectLoad,
    /// App-owned structure added after persisted project reconstruction but before cutover.
    ///
    /// This retains loaded-data lifecycle semantics while allowing the engine to omit
    /// transient event and UI projections that the host replaces with a full snapshot.
    ProjectLoadAugmentation,
    /// Node recreated while duplicating/copy-pasting a persisted subtree.
    Duplicate,
    /// Duplicate whose caller supplied explicit initial parameter values.
    ///
    /// App lifecycle code may use this to avoid applying a default duplicate
    /// adjustment over a value the higher-level operation already resolved.
    DuplicateWithInitialParams,
    /// Authored state has already been restored by history. Ready callbacks may
    /// reactivate resources; their immediate authored edits are not replayed.
    HistoryRestore,
}

impl NodeCreationContext {
    /// Returns `true` when this creation path restores existing authored data.
    pub const fn has_loaded_data(self) -> bool {
        !matches!(self, Self::Fresh)
    }

    /// Returns `true` for persisted reconstruction and pre-cutover augmentation.
    pub const fn is_project_load(self) -> bool {
        matches!(self, Self::ProjectLoad | Self::ProjectLoadAugmentation)
    }
}
