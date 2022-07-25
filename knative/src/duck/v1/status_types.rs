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

    pub fn mark_true_with_reason(&mut self, condition_type: ConditionType, reason: String, message: Option<String>) {
        self.set_cond(Condition {
            type_: condition_type,
            status: ConditionStatus::True,
            reason: Some(reason),
            message,
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

    pub fn mark_unknown(&mut self, condition_type: ConditionType) {
        self.set_cond(Condition {
            type_: condition_type,
            status: ConditionStatus::Unknown,
            ..Default::default()
        })
    }

    pub fn mark_unknown_with_reason(&mut self, condition_type: ConditionType, reason: String, message: Option<String>) {
        self.set_cond(Condition {
            type_: condition_type,
            status: ConditionStatus::Unknown,
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

impl PartialOrd for Condition {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        use ConditionStatus::*;
        use std::cmp::Ordering;

        let time_ord = match (self.last_transition_time, other.last_transition_time) {
            (Some(left), Some(right)) => left.partial_cmp(&right),
            _ => None
        };

        match (self.status, other.status) {
            (False, False) | (Unknown, Unknown) | (True, True) => match time_ord {
                Some(ord) => Some(ord),
                None => Some(Ordering::Equal)
            },
            (False, _) | (Unknown, True) => Some(Ordering::Greater),
            (Unknown, False) | (True, _) => Some(Ordering::Less),
        }
    }
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
        ConditionSeverity::Error
    }
}

impl Condition {
    fn is_true(&self) -> bool {
        self.status == ConditionStatus::True
    }

    fn is_false(&self) -> bool {
        self.status == ConditionStatus::False
    }

    fn is_unknown(&self) -> bool {
        self.status == ConditionStatus::Unknown
    }
}

struct ConditionManager<'a, const N: usize> {
    set: ConditionSet<N>,
    conditions: &'a mut Conditions,
}

impl<'a, const N: usize> ConditionManager<'a, N> {
    pub fn new(set: ConditionSet<N>, conditions: &'a mut Conditions) -> Self {
        ConditionManager { set, conditions }
    }

    fn get_condition(&self, condition_type: ConditionType) -> Option<&Condition> {
        self.conditions.iter().find(|cond| cond.type_ == condition_type)
    }


    /// Returns the happy condition
    fn get_top_level_condition(&self) -> Option<&Condition> {
        self.get_condition(self.set.happy)
    }

    fn set_condition(&mut self, condition: Condition) {
        self.conditions.set_cond(condition)
    }

    pub fn is_happy(&self) -> bool {
        self.get_top_level_condition()
            .map(|c| c.is_true())
            .unwrap_or(false)
    }

    /// Whether the ConditionType determines happiness
    fn is_terminal(&self, condition_type: ConditionType) -> bool {
        self.set.is_terminal(condition_type)
    }

    fn find_unhappy_dependent(&mut self) -> Option<&mut Condition> {
        self.conditions
            .iter_mut()
            // Filter to non-true, terminal dependents
            .filter(|cond| cond.type_ != self.set.happy && self.set.is_terminal(cond.type_) && !cond.is_true())
            // Return a condition, prioritizing most recent False over most recent Unknown
            .reduce(|unhappy, cond| if cond > unhappy { cond } else { unhappy })
    }

    /// Mark the happy condition to true if all other dependents are also true
    fn recompute_happiness(&mut self, condition_type: ConditionType) {
        let type_ = self.set.happy;
        let severity = self.set.severity(self.set.happy);

        let cond = if let Some(dependent) = self.find_unhappy_dependent() {
            // make unhappy dependent reflect in happy condition
            Some(Condition {
                type_,
                status: dependent.status,
                reason: dependent.reason.clone(),
                message: dependent.message.clone(),
                severity,
                ..Default::default()
            })
        } else if condition_type != self.set.happy {
            // set happy to true
            Some(Condition {
                type_,
                status: ConditionStatus::True,
                severity,
                ..Default::default()
            })
        } else {
            None
        };

        if let Some(cond) = cond {
            self.conditions.set_cond(cond);
        }
    }

    pub fn mark_true(&mut self, condition_type: ConditionType) {
        self.conditions.mark_true(condition_type);
        self.recompute_happiness(condition_type);
    }

    pub fn mark_true_with_reason(&mut self, condition_type: ConditionType, reason: &str, message: Option<String>) {
        self.conditions.mark_true_with_reason(condition_type, reason.to_string(), message);
        self.recompute_happiness(condition_type);
    }

    /// Set the status to unknown and also set the happy condition to unknown if no other dependent
    /// condition is in an error state
    pub fn mark_unknown(&mut self, condition_type: ConditionType, reason: &str, message: Option<String>) {
        self.conditions.mark_unknown_with_reason(condition_type, reason.to_string(), message.clone());

        // set happy condition to false if another dependent is false, otherwise set happy
        // condition to unknown if this condition is a dependent
        if let Some(dependent) = self.find_unhappy_dependent() {
            if dependent.is_false() {
                if let Some(happy) = self.get_top_level_condition() {
                    if !happy.is_false() {
                        self.mark_false(self.set.happy, reason, message);
                   }
                }
            }
        } else if self.set.is_terminal(condition_type) {
           self.conditions.mark_unknown_with_reason(self.set.happy, reason.to_string(), message);
        }
    }

    /// Set the status of the condition type to false, as well as the happy condition if this
    /// condition is a dependent
    pub fn mark_false(&mut self, condition_type: ConditionType, reason: &str, message: Option<String>) {
        self.conditions.mark_false(condition_type, reason.to_string(), message.clone());

        if self.set.dependents.contains(&condition_type) {
            self.conditions.mark_false(self.set.happy, reason.to_string(), message)
        }
    }
}

/// ConditionSet defines how a set of Conditions
/// depend on one another
struct ConditionSet<const N: usize> {
    pub happy: ConditionType,
    pub dependents: [ConditionType; N],
}

impl<const N: usize> ConditionSet<N> {
    fn is_terminal(&self, condition_type: ConditionType) -> bool {
        self.dependents.contains(&condition_type) || self.happy == condition_type
    }

    fn severity(&self, condition_type: ConditionType) -> ConditionSeverity {
        if self.is_terminal(condition_type) {
            ConditionSeverity::Error
        } else {
            ConditionSeverity::Info
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn find_unhappy_dependent_does_not_sort_vec() {
        let dt = chrono::Utc.ymd(2022, 1, 1);
        let mut conditions = Conditions(vec![
            Condition {
                type_: ConditionType::Ready,
                status: ConditionStatus::False,
                last_transition_time: Some(dt.and_hms(0, 0, 0)),
                ..Default::default()
            },
            Condition {
                type_: ConditionType::SinkProvided,
                status: ConditionStatus::False,
                last_transition_time: Some(dt.and_hms(1, 0, 0)),
                ..Default::default()
            },
            Condition {
                type_: ConditionType::SinkProvided,
                status: ConditionStatus::Unknown,
                last_transition_time: Some(dt.and_hms(2, 0, 0)),
                ..Default::default()
            },
            Condition {
                type_: ConditionType::SinkProvided,
                status: ConditionStatus::False,
                last_transition_time: Some(dt.and_hms(3, 0, 0)),
                ..Default::default()
            },
            Condition {
                type_: ConditionType::Succeeded,
                status: ConditionStatus::False,
                last_transition_time: Some(dt.and_hms(2, 0, 0)),
                ..Default::default()
            }
        ]);

        let cond_set = ConditionSet {
            happy: ConditionType::Ready,
            dependents: [ConditionType::SinkProvided]
        };

        let mut manager = ConditionManager::new(cond_set, &mut conditions);
        let unhappy = manager.find_unhappy_dependent().unwrap();
        // Returns most recent False dependent
        assert_eq!(unhappy.type_, ConditionType::SinkProvided);
        assert_eq!(unhappy.status, ConditionStatus::False);
        assert_eq!(unhappy.last_transition_time.unwrap(), dt.and_hms(3, 0, 0));
        // Maintains order
        let mut iter = conditions.iter();
        assert_eq!(iter.next().unwrap().type_, ConditionType::Ready);
        assert_eq!(iter.next().unwrap().type_, ConditionType::SinkProvided);
        assert_eq!(iter.next().unwrap().type_, ConditionType::SinkProvided);
        assert_eq!(iter.next().unwrap().type_, ConditionType::SinkProvided);
        assert_eq!(iter.next().unwrap().type_, ConditionType::Succeeded);
    }
}
