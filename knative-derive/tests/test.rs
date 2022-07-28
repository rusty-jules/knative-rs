use knative_derive::ConditionType;
use knative_conditions::ConditionType as _;
use knative_conditions::{ConditionAccessor, Conditions};

#[derive(ConditionType, Copy, Clone, Debug, PartialEq)]
enum MyCondition {
    Ready,
    #[dependent]
    SinkProvided
}

struct MyStatus {
    conditions: Conditions<MyCondition>
}

impl ConditionAccessor<MyCondition> for MyStatus {
    fn conditions(&mut self) -> &mut Conditions<MyCondition> {
        &mut self.conditions
    }
}

#[test]
fn variant_functions_exist() {
    assert_eq!(MyCondition::Ready, MyCondition::happy());
    assert_eq!(MyCondition::SinkProvided, MyCondition::sinkprovided());
}

#[test]
fn has_dependents() {
    assert_eq!([MyCondition::SinkProvided], MyCondition::dependents());
}

#[test]
fn can_be_managed() {
    let mut status = MyStatus { conditions: Conditions::default() };
    status.mark_sinkprovided();
}
