use stellar_xdr::curr::{ScVal, ScVec};
use your_crate_name::scval::decode_bounty;

#[test]
fn test_decode_bounty_malformed_assignee_tuple_single_element() {
    let malformed_assignee = ScVal::Vec(Some(ScVec(vec![
        ScVal::Symbol("alice".try_into().unwrap()),
    ].try_into().unwrap())));

    let bounty = ScVal::Vec(Some(ScVec(vec![
        ScVal::U64(1),
        ScVal::String("Test Bounty".try_into().unwrap()),
        ScVal::U64(1000),
        malformed_assignee,
        ScVal::Bool(false),
    ].try_into().unwrap())));

    let result = decode_bounty(&bounty);
    assert!(result.is_err());
}

#[test]
fn test_decode_bounty_malformed_assignee_tuple_three_elements() {
    let malformed_assignee = ScVal::Vec(Some(ScVec(vec![
        ScVal::Symbol("alice".try_into().unwrap()),
        ScVal::U64(100),
        ScVal::Bool(true),
    ].try_into().unwrap())));

    let bounty = ScVal::Vec(Some(ScVec(vec![
        ScVal::U64(1),
        ScVal::String("Test Bounty".try_into().unwrap()),
        ScVal::U64(1000),
        malformed_assignee,
        ScVal::Bool(false),
    ].try_into().unwrap())));

    let result = decode_bounty(&bounty);
    assert!(result.is_err());
}
