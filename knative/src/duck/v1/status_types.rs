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

#[cfg(test)]
mod test {
    use super::*;

    #[derive(knative_derive::ConditionType, Copy, Clone, Debug, PartialEq)]
    enum CustomCondition {
        Succeeded,
        SomethingElse
    }

    struct CustomStatus {
        status: Status<CustomCondition>,
    }

    #[test]
    fn can_manage_custom_status_with_no_dependents() {
        let mut custom_status = CustomStatus {
            status: Status::default()
        };

        let status = &mut custom_status.status;

        // a status with no dependents does not begin as ready
        assert_eq!(status.is_ready(), false);

        // marking any condition as true makes the status ready
        status.mark_somethingelse();
        assert_eq!(status.is_ready(), true);

        // marking non_dependent status as not true does not make status not ready
        status.mark_not_somethingelse("NotSomethingElse", None);
        assert_eq!(status.is_ready(), true);

        // marking status as not ready works
        status.mark_false("ActuallyNotReady", None);
        assert_eq!(status.is_ready(), false);

        // explicity marking status as ready works
        status.manager().mark_true(CustomCondition::Succeeded);
        assert_eq!(status.is_ready(), true);

        // marking unknown works
        status.mark_unknown();
        assert_eq!(status.is_ready(), false);
    }

    #[test]
    fn can_init_with_custom_condition_state() {
        use knative_conditions::{Condition, ConditionStatus};

        let _status = CustomStatus {
            status: Status {
                conditions: Some(Conditions::with_conditions(vec![
                    Condition::with_status(
                        CustomCondition::Succeeded,
                        ConditionStatus::True
                    )
                ])),
                ..Default::default()
            }
        };
    }

    #[test]
    #[should_panic]
    fn fails_to_init_with_improper_custom_condition_state() {
        use knative_conditions::{Condition, ConditionStatus};

        let _status = CustomStatus {
            status: Status {
                conditions: Some(Conditions::with_conditions(vec![
                    Condition::with_status(
                        CustomCondition::SomethingElse,
                        ConditionStatus::True
                    )
                ])),
                ..Default::default()
            }
        };
    }
}
