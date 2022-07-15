#![allow(dead_code)]
use schemars::JsonSchema;
use serde::{Serialize, Deserialize};
use std::ops::{Deref,DerefMut};

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// ObservedGeneration is the 'Generation' of the Service that
    /// was last processed by the controller.
    pub observed_generation: Option<i64>,
    /// Conditions the latest available observations of a resource's current state.
    pub conditions: Option<Conditions>,
    /// Annotations is additional Status fields for the Resource to save some
    /// additional State as well as convey more information to the user. This is
    /// roughly akin to Annotations on any k8s resource, just the reconciler conveying
    /// richer information outwards.
    pub annotations: Option<std::collections::BTreeMap<String, String>>,
}

impl Default for Status {
    fn default() -> Status {
        Status {
            observed_generation: Some(0i64),
            conditions: Some(Conditions::new()),
            annotations: None
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct Conditions(Vec<Condition>);

impl Deref for Conditions {
    type Target = Vec<Condition>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Conditions {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Conditions {
    pub fn new() -> Conditions {
        Conditions(vec![])
    }

    pub fn get_cond(&mut self, type_: ConditionType) -> Option<&mut Condition> {
        self.iter_mut().find(|c| c.type_ == type_)
    }

    pub fn set_cond(&mut self, mut condition: Condition) {
        // The go version collects all the conditions of different type != arg
        // into a new array, then checks if only the time has changed
        // on the condition to set. If so it returns, otherwise
        // it updates that single condition, re-sorts the array of conditions
        // by Type (alphabetically?) and sets the new array as the conditions.
        // This may be due to the "accessor" interface that we have skipped here.
        match self.get_cond(condition.type_) {
            Some(cond) => {
                // Check if only the time has changed
                let test_cond = Condition {
                    last_transition_time: condition.last_transition_time,
                    // OPTIMIZE: could check strings explicitly with no need for clone
                    ..cond.clone()
                };
                if test_cond == condition {
                    return
                } else {
                    *cond = Condition {
                        last_transition_time: Some(chrono::Utc::now()),
                        ..condition
                    }
                }
            }
            None => {
                condition.last_transition_time = Some(chrono::Utc::now());
                self.push(condition);
                // TODO: sort the output...alphabetically by type name?
            }
        }
    }

    pub fn mark_true(&mut self, condition_type: ConditionType) {
        self.set_cond(Condition {
            type_: condition_type,
            status: ConditionStatus::True,
            ..Default::default()
        })
    }

    pub fn mark_false(&mut self, condition_type: ConditionType, reason: String, message: Option<String>) {
        self.set_cond(Condition {
            type_: condition_type,
            status: ConditionStatus::False,
            reason: Some(reason),
            message,
            ..Default::default()
        });
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, JsonSchema, PartialEq)]
pub struct Condition {
    #[serde(rename = "type")]
    pub type_: ConditionType,
    pub status: ConditionStatus,
    /// ConditionSeverityError specifies that a failure of a condition type
    /// should be viewed as an error.  As "Error" is the default for conditions
    /// we use the empty string (coupled with omitempty) to avoid confusion in
    /// the case where the condition is in state "True" (aka nothing is wrong).
    // In rust lang we accomplish this with Error as a Default variant
    #[serde(default)]
    pub severity: ConditionSeverity,
    // TODO: make this a "VolatileTime"
    //#[serde(deserialize_with = "from_ts")]
    pub last_transition_time: Option<chrono::DateTime<chrono::Utc>>,
    pub reason: Option<String>,
    pub message: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, JsonSchema, PartialEq)]
pub enum ConditionStatus {
    True,
    False,
    Unknown,
}

impl Default for ConditionStatus {
    fn default() -> Self {
        ConditionStatus::Unknown
    }
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, JsonSchema, PartialEq)]
#[non_exhaustive]
pub enum ConditionType {
    /// Specifies that the resource is ready.
    /// For long-running resources.
    Ready,
    /// Specifies that the resource has finished.
    /// For resources which run to completion.
    Succeeded,
    /// Specifies whether the sink has been properly extracted from the resolver.
    SinkProvided,
}

impl Default for ConditionType {
    fn default() -> Self {
        ConditionType::Ready
    }
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, JsonSchema, PartialEq)]
#[non_exhaustive]
pub enum ConditionSeverity {
    Error,
    Warning,
    Info,
}

impl Default for ConditionSeverity {
    fn default() -> Self {
        ConditionSeverity::Info
    }
}

impl Condition {
    fn is_true(&self) -> bool {
        self.status == ConditionStatus::True
    }
}

/// ConditionSet defines how a set of Conditions
/// depend on one another
struct ConditionSet {
    pub happy: ConditionType,
    pub dependents: Vec<ConditionType>,
}

impl ConditionSet {
    /// GetTopLevelCondition
    fn get_top_level_condition(&self) -> ConditionType {
        self.happy
    }

    fn is_happy(&self, conditions: &mut Conditions) -> bool {
        let happy = self.get_top_level_condition();
        let condition = conditions.iter().find(|c| c.type_ == happy);
        match condition {
            Some(condition) => condition.is_true(),
            None => {
                conditions.push(Condition {
                    type_: happy,
                    status: ConditionStatus::Unknown,
                    severity: ConditionSeverity::Info, // FIXME: not sure if this should be the default
                    last_transition_time: Some(chrono::Utc::now()),
                    reason: None,
                    message: None
                });
                // Try again
                self.is_happy(conditions)
            }
        }
    }
}
