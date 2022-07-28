#![allow(dead_code)]
use knative_conditions::{Conditions, ConditionAccessor, ConditionType};
use schemars::JsonSchema;
use serde::{Serialize, Deserialize};


#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Status<C: ConditionType> {
    /// ObservedGeneration is the 'Generation' of the Service that
    /// was last processed by the controller.
    pub observed_generation: Option<i64>,
    /// Conditions the latest available observations of a resource's current state.
    pub conditions: Option<Conditions<C>>,
    /// Annotations is additional Status fields for the Resource to save some
    /// additional State as well as convey more information to the user. This is
    /// roughly akin to Annotations on any k8s resource, just the reconciler conveying
    /// richer information outwards.
    pub annotations: Option<std::collections::BTreeMap<String, String>>,
}

impl<C: ConditionType> Default for Status<C> {
    fn default() -> Status<C> {
        Status {
            observed_generation: Some(0i64),
            conditions: Some(Conditions::default()),
            annotations: None
        }
    }
}

impl<C: ConditionType> ConditionAccessor<C> for Status<C> {
    fn conditions(&mut self) -> &mut Conditions<C> {
        self.conditions.get_or_insert(Conditions::default())
    }
}
