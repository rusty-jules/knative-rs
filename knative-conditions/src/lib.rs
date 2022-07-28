use schemars::JsonSchema;
use serde::{Serialize, Deserialize};
use std::fmt::Debug;

/// Enums that implement [`ConditionType`] can be used to differentiate [`Condition`]
/// and describe the state of the resource.
pub trait ConditionType: Clone + Copy + Default + Debug +  PartialEq
where Self: 'static {
    /// The top-level variant that determines overall readiness of the resource.
    fn happy() -> Self;

    /// Variants that must be true to consider the happy condition true.
    fn dependents() -> &'static [Self];

    /// Whether the [`ConditionType`] determines happiness.
    fn is_terminal(&self) -> bool {
        Self::dependents().contains(self) || *self == Self::happy()
    }

    /// A [`Condition`] severity defaults to whether it determines overall resource readiness or
    /// not.
    fn severity(&self) -> ConditionSeverity {
        if self.is_terminal() {
            ConditionSeverity::Error
        } else {
            ConditionSeverity::Info
        }
    }
}

/// Provides [`ConditionManager`] access to the [`Conditions`],
/// and exposes control of the top-level [`Condition`].
pub trait ConditionAccessor<C: ConditionType> {
    /// Return the conditions of your CR status type.
    fn conditions(&mut self) -> &mut Conditions<C>;

    /// Returns a [`ConditionManager`] for more fine-grained control of [`Conditions`].
    fn manager(&mut self) -> ConditionManager<C> {
        ConditionManager::new(self.conditions())
    }

    /// Returns true if the resource is ready overall.
    fn is_ready(&mut self) -> bool {
        self.manager().is_happy()
    }

    /// Set the status of the top level condition type to false
    fn mark_false(&mut self, reason: &str, message: Option<String>) {
        let t = self.manager().get_top_level_condition().type_;
        self.manager().mark_false(t, reason, message);
    }

    /// Set the status of the top level condition to unknown. Typically used when beginning the
    /// reconciliation of a new generation.
    fn mark_unknown(&mut self) {
        let t = self.manager().get_top_level_condition().type_;
        self.manager().mark_unknown(
            t,
            "NewObservedGenFailure",
            Some("unsuccessfully observed a new generation".into())
        );
    }

    fn mark_unknown_with_message(&mut self, reason: &str, message: Option<String>) {
        let t = self.manager().get_top_level_condition().type_;
        self.manager().mark_unknown(t, reason, message);
    }
}

/// The state of a [`Condition`].
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
/// The importance of a conditions status.
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

impl ConditionSeverity {
    pub fn is_err(&self) -> bool {
        *self == ConditionSeverity::Error
    }
}

/// A custom resource status condition.
#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
pub struct Condition<C: ConditionType> {
    #[serde(rename = "type")]
    pub type_: C,
    pub status: ConditionStatus,
    /// ConditionSeverityError specifies that a failure of a condition type
    /// should be viewed as an error.  As "Error" is the default for conditions
    /// we use the empty string (coupled with omitempty) to avoid confusion in
    /// the case where the condition is in state "True" (aka nothing is wrong).
    // In rust lang we accomplish this with Error as a Default variant
    #[serde(default)]
    #[serde(skip_serializing_if = "ConditionSeverity::is_err")]
    pub severity: ConditionSeverity,
    // TODO: make this a "VolatileTime"
    //#[serde(deserialize_with = "from_ts")]
    pub last_transition_time: Option<chrono::DateTime<chrono::Utc>>,
    pub reason: Option<String>,
    pub message: Option<String>,
}

impl<C: ConditionType> Default for Condition<C> {
    fn default() -> Condition<C> {
        Condition {
            type_: C::default(),
            status: ConditionStatus::default(),
            severity: ConditionSeverity::default(),
            last_transition_time: Some(chrono::Utc::now()),
            reason: None,
            message: None
        }
    }
}

impl<C: ConditionType> PartialOrd for Condition<C> {
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

impl<C: ConditionType> Condition<C> {
    fn new(type_: C) -> Self {
        Condition {
            type_,
            ..Default::default()
        }
    }

    fn with_status(type_: C, status: ConditionStatus) -> Condition<C> {
        Condition {
            status,
            ..Condition::new(type_)
        }
    }

    fn is_true(&self) -> bool {
        self.status == ConditionStatus::True
    }

    fn is_false(&self) -> bool {
        self.status == ConditionStatus::False
    }

    #[allow(dead_code)]
    fn is_unknown(&self) -> bool {
        self.status == ConditionStatus::Unknown
    }
}

/// A `Vec<Condition>` that maintains transition times.
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct Conditions<C>(Vec<Condition<C>>)
    where C: ConditionType;

impl<C> Default for Conditions<C> 
where C: ConditionType {
    fn default() -> Self {
        let iter = [C::happy()]
            .into_iter()
            .chain(C::dependents().iter().cloned())
            .map(Condition::new);
        Conditions(Vec::from_iter(iter))
    }
}

impl<C: ConditionType> Conditions<C> {
    pub fn with_conditions(conditions: Vec<Condition<C>>) -> Conditions<C> {
        assert!(
            conditions.iter().any(|c| c.type_ == C::happy()),
            "Conditions must be initialized with the happy ConditionType"
        );
        assert!(
            conditions.iter().fold(std::collections::HashSet::new(), |mut acc, c| {
                // insert the ConditionType as a string to avoid C: Hashable bound
                acc.insert(format!("{:?}", c.type_));
                acc
            }).len() == conditions.len(),
            "ConditionType must be unique to each Condition"
        );
        Conditions(conditions)
    }

    fn get_cond(&self, type_: &C) -> Option<&Condition<C>> {
        self.0.iter().find(|c| c.type_ == *type_)
    }

    fn get_cond_mut(&mut self, type_: &C) -> Option<&mut Condition<C>> {
        self.0.iter_mut().find(|c| c.type_ == *type_)
    }

    fn set_cond(&mut self, mut condition: Condition<C>) {
        // The go version collects all the conditions of different type != arg
        // into a new array, then checks if only the time has changed
        // on the condition to set. If so it returns, otherwise
        // it updates that single condition, re-sorts the array of conditions
        // by Type (alphabetically?) and sets the new array as the conditions.
        // This may be due to the "accessor" interface that we have skipped here.
        match self.get_cond_mut(&condition.type_) {
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
                self.0.push(condition);
                // TODO: sort the output...alphabetically by type name?
            }
        }
    }

    fn mark_true(&mut self, condition_type: C) {
        self.set_cond(Condition::with_status(condition_type, ConditionStatus::True))
    }

    fn mark_true_with_reason(&mut self, condition_type: C, reason: String, message: Option<String>) {
        self.set_cond(Condition {
            reason: Some(reason),
            message,
            ..Condition::with_status(condition_type, ConditionStatus::True)
        })
    }

    fn mark_false(&mut self, condition_type: C, reason: String, message: Option<String>) {
        self.set_cond(Condition {
            reason: Some(reason),
            message,
            ..Condition::with_status(condition_type, ConditionStatus::False)
        });
    }

    fn mark_unknown(&mut self, condition_type: C, reason: String, message: Option<String>) {
        self.set_cond(Condition {
            reason: Some(reason),
            message,
            ..Condition::with_status(condition_type, ConditionStatus::Unknown)
        });
    }
}

/// Mutates [`Conditions`] in accordance with the condition dependency chain defined by a
/// [`ConditionType`].
pub struct ConditionManager<'a, C>
where C: ConditionType {
    conditions: &'a mut Conditions<C>,
}

impl<'a, C> ConditionManager<'a, C>
where C: ConditionType {
    pub fn new(conditions: &'a mut Conditions<C>) -> Self {
        assert!(
            !C::dependents().contains(&C::happy()),
            "dependents may not contain happy condition"
        );
        ConditionManager { conditions }
    }

    pub fn get_condition(&self, condition_type: C) -> Option<&Condition<C>> {
        self.conditions.get_cond(&condition_type)
    }

    /// Returns the happy [`Condition`].
    ///
    /// # Panic
    /// Panics if the [`Conditions`] have not been properly initialized.
    /// See [`Conditions::default()`].
    pub fn get_top_level_condition(&self) -> &Condition<C> {
        self.get_condition(C::happy())
            .as_ref()
            .expect("top level condition is initialized")
    }

    pub fn is_happy(&self) -> bool {
        self.get_top_level_condition().is_true()
    }

    fn find_unhappy_dependent(&self) -> Option<&Condition<C>> {
        self.conditions.0
            .iter()
            // Filter to non-true, terminal dependents
            .filter(|cond| cond.type_ != C::happy() && cond.type_.is_terminal() && !cond.is_true())
            // Return a condition, prioritizing most recent False over most recent Unknown
            .reduce(|unhappy, cond| if cond > unhappy { cond } else { unhappy })
    }

    /// Mark the happy condition to true if all other dependents are also true.
    fn recompute_happiness(&mut self, condition_type: &C) {
        match self.find_unhappy_dependent() {
            Some(dependent) => {
                let cond = Condition {
                    type_: C::happy(),
                    status: dependent.status,
                    reason: dependent.reason.clone(),
                    message: dependent.message.clone(),
                    severity: C::happy().severity(),
                    ..Default::default()
                };
                self.conditions.set_cond(cond);
            },
            None => if *condition_type != C::happy() {
                // set happy to true
                self.conditions.set_cond(Condition {
                    type_: C::happy(),
                    status: ConditionStatus::True,
                    severity: C::happy().severity(),
                    ..Default::default()
                })
            }
        }
    }

    pub fn mark_true(&mut self, condition_type: C) {
        self.conditions.mark_true(condition_type);
        self.recompute_happiness(&condition_type);
    }

    pub fn mark_true_with_reason(&mut self, condition_type: C, reason: &str, message: Option<String>) {
        self.conditions.mark_true_with_reason(condition_type, reason.to_string(), message);
        self.recompute_happiness(&condition_type);
    }

    /// Set the status of the condition type to false, as well as the happy condition if this
    /// condition is a dependent.
    pub fn mark_false(&mut self, condition_type: C, reason: &str, message: Option<String>) {
        self.conditions.mark_false(condition_type, reason.to_string(), message.clone());

        if C::dependents().contains(&condition_type) {
            self.conditions.mark_false(C::happy(), reason.to_string(), message)
        }
    }

    /// Set the status to unknown and also set the happy condition to unknown if no other dependent
    /// condition is in an error state.
    pub fn mark_unknown(&mut self, condition_type: C, reason: &str, message: Option<String>) {
        self.conditions.mark_unknown(condition_type, reason.to_string(), message.clone());

        // set happy condition to false if another dependent is false, otherwise set happy
        // condition to unknown if this condition is a dependent
        if let Some(dependent) = self.find_unhappy_dependent() {
            if dependent.is_false() {
                if !self.get_top_level_condition().is_false() {
                    self.mark_false(C::happy(), reason, message);
               }
            }
        } else if condition_type.is_terminal() {
           self.conditions.mark_unknown(C::happy(), reason.to_string(), message);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use chrono::TimeZone;

    #[derive(Deserialize, Copy, Clone, Debug, PartialEq)]
    enum TestCondition {
        Ready,
        SinkProvided,
        OtherCondition,
        Unimportant
    }

    impl ConditionType for TestCondition {
        fn happy() -> Self {
            TestCondition::Ready
        }

        fn dependents() -> &'static [Self] {
            &[TestCondition::SinkProvided, TestCondition::OtherCondition]
        }
    }

    impl Default for TestCondition {
        fn default() -> Self {
            TestCondition::Ready
        }
    }

    #[test]
    fn find_unhappy_dependent_does_not_sort_vec() {
        let dt = chrono::Utc.ymd(2022, 1, 1);
        let mut conditions = Conditions::with_conditions(vec![
            Condition {
                type_: TestCondition::Ready,
                status: ConditionStatus::False,
                last_transition_time: Some(dt.and_hms(0, 0, 0)),
                ..Default::default()
            },
            Condition {
                type_: TestCondition::SinkProvided,
                status: ConditionStatus::False,
                last_transition_time: Some(dt.and_hms(3, 0, 0)),
                ..Default::default()
            },
            Condition {
                type_: TestCondition::OtherCondition,
                status: ConditionStatus::False,
                last_transition_time: Some(dt.and_hms(2, 0, 0)),
                ..Default::default()
            },
            Condition {
                type_: TestCondition::Unimportant,
                status: ConditionStatus::False,
                last_transition_time: Some(dt.and_hms(2, 0, 0)),
                ..Default::default()
            },
        ]);

        let manager = ConditionManager::new(&mut conditions);
        let unhappy = manager.find_unhappy_dependent().unwrap();
        // Returns most recent False dependent
        assert_eq!(unhappy.type_, TestCondition::SinkProvided);
        assert_eq!(unhappy.status, ConditionStatus::False);
        assert_eq!(unhappy.last_transition_time.unwrap(), dt.and_hms(3, 0, 0));
        // Maintains order
        let mut iter = conditions.0.iter();
        assert_eq!(iter.next().unwrap().type_, TestCondition::Ready);
        assert_eq!(iter.next().unwrap().type_, TestCondition::SinkProvided);
        assert_eq!(iter.next().unwrap().type_, TestCondition::OtherCondition);
        assert_eq!(iter.next().unwrap().type_, TestCondition::Unimportant);
    }

    #[test]
    fn condition_type_deserializes() {
        let condition_type: TestCondition = serde_json::from_value(serde_json::json!(
            "SinkProvided"
        )).unwrap();
        assert_eq!(condition_type, TestCondition::SinkProvided);
        let condition_type: TestCondition = serde_json::from_value(serde_json::json!(
            "Ready"
        )).unwrap();
        assert_eq!(condition_type, TestCondition::Ready);
        let condition_type: Result<TestCondition, _> = serde_json::from_value(serde_json::json!(
            "Succeeded"
        ));
        assert!(condition_type.is_err());
    }
}
