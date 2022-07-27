use knative_derive::ConditionType;
use knative_conditions::ConditionType as _;

#[derive(ConditionType, Copy, Clone, Debug, PartialEq)]
enum MyCondition {
    Ready,
    #[dependent]
    SinkProvided
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
