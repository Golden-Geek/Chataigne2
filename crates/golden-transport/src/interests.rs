use std::collections::{BTreeMap, BTreeSet};

use golden_protocol::{ClientId, ObservationInterest, ScopeId, ViewId};

#[derive(Default)]
pub struct ObservationRegistry {
    views: BTreeMap<(ClientId, ViewId), BTreeSet<ScopeId>>,
}

impl ObservationRegistry {
    pub fn replace(&mut self, interest: ObservationInterest) {
        self.views
            .insert((interest.client, interest.view), interest.scopes.into_iter().collect());
    }

    pub fn clear_view(&mut self, client: &ClientId, view: &ViewId) -> bool {
        self.views.remove(&(client.clone(), view.clone())).is_some()
    }

    pub fn clear_client(&mut self, client: &ClientId) -> usize {
        let before = self.views.len();
        self.views.retain(|(candidate, _), _| candidate != client);
        before - self.views.len()
    }

    pub fn scopes_for(&self, client: &ClientId, view: &ViewId) -> Option<&BTreeSet<ScopeId>> {
        self.views.get(&(client.clone(), view.clone()))
    }

    pub fn interested_views(&self, scope: &ScopeId) -> impl Iterator<Item = (&ClientId, &ViewId)> {
        self.views
            .iter()
            .filter(move |(_, scopes)| scopes.contains(scope))
            .map(|((client, view), _)| (client, view))
    }
}
