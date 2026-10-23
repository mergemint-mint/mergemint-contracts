#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BountyEvent {
    pub name: String,
    pub bounty_id: String,
}

pub fn encode_bounty_event(name: &str, bounty_id: &str) -> String {
    format!("{name}:{bounty_id}")
}

pub fn decode_bounty_event(message: &str) -> BountyEvent {
    match message.split_once(':') {
        Some((name, bounty_id)) => BountyEvent {
            name: name.to_string(),
            bounty_id: bounty_id.to_string(),
        },
        None => BountyEvent {
            name: "bounty_updated".to_string(),
            bounty_id: message.to_string(),
        },
    }
}
