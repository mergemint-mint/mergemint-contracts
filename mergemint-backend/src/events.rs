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

// ---------------------------------------------------------------------------
// XDR-aware event helpers (#1259)
// ---------------------------------------------------------------------------

use stellar_xdr::{Limits, ReadXdr, ScVal};

/// Maximum byte length for a single XDR event payload, mirroring the bound in
/// `indexer.rs`.  Kept here so `events.rs` is self-contained when validating
/// payloads before re-encoding.
const XDR_PAYLOAD_MAX_BYTES: usize = 8 * 1024;

/// Expected length, in bytes, of a `BountyId` (`BytesN<32>`) inside an
/// `ScVal::Bytes`.
pub const BOUNTY_ID_LEN: usize = 32;

/// Decode a base64-encoded Soroban XDR `ScVal` from an event payload, using
/// bounded [`Limits`] so a malicious or corrupt payload cannot cause
/// unbounded allocation.
///
/// Returns `Err` with a human-readable description on any decode failure so
/// callers can emit a structured warning.
pub fn decode_scval(value_base64: &str) -> Result<ScVal, String> {
    ScVal::from_xdr_base64(value_base64, Limits::len(XDR_PAYLOAD_MAX_BYTES))
        .map_err(|error| format!("XDR decode failed: {error}"))
}

/// Extract a 32-byte bounty id from a single-value `ScVal::Bytes` payload.
///
/// Returns `Err` if the `ScVal` is not `Bytes` or the byte length is not
/// exactly [`BOUNTY_ID_LEN`].
pub fn extract_bounty_id_single(scval: &ScVal) -> Result<String, String> {
    let ScVal::Bytes(bytes) = scval else {
        return Err(format!(
            "expected ScVal::Bytes for single-value bounty event, got ScVal::{}",
            scval.name()
        ));
    };
    let bytes: &[u8] = bytes.as_ref();
    if bytes.len() != BOUNTY_ID_LEN {
        return Err(format!(
            "bounty id must be {BOUNTY_ID_LEN} bytes, got {} bytes",
            bytes.len()
        ));
    }
    Ok(hex::encode(bytes))
}

/// Extract a 32-byte bounty id from the first element of an `ScVal::Vec`
/// tuple payload.
///
/// Returns `Err` if the `ScVal` is not a `Vec`, the vec is empty, the first
/// element is not `Bytes`, or the byte length is not exactly
/// [`BOUNTY_ID_LEN`].
pub fn extract_bounty_id_tuple(scval: &ScVal) -> Result<String, String> {
    let ScVal::Vec(Some(vec)) = scval else {
        return Err(format!(
            "expected ScVal::Vec for tuple bounty event, got ScVal::{}",
            scval.name()
        ));
    };
    let Some(first) = vec.0.first() else {
        return Err("bounty event tuple is empty".to_string());
    };
    let ScVal::Bytes(bytes) = first else {
        return Err(format!(
            "first tuple element must be ScVal::Bytes, got ScVal::{}",
            first.name()
        ));
    };
    let bytes: &[u8] = bytes.as_ref();
    if bytes.len() != BOUNTY_ID_LEN {
        return Err(format!(
            "bounty id must be {BOUNTY_ID_LEN} bytes, got {} bytes",
            bytes.len()
        ));
    }
    Ok(hex::encode(bytes))
}

/// Decode and extract a bounty id from a base64 XDR payload, choosing the
/// single-value or tuple shape based on `is_tuple`.
///
/// This is the high-level helper callers use when they know the event shape.
/// It combines [`decode_scval`] with [`extract_bounty_id_single`] /
/// [`extract_bounty_id_tuple`] and returns a single `Result`.
pub fn extract_bounty_id_from_xdr(value_base64: &str, is_tuple: bool) -> Result<String, String> {
    let scval = decode_scval(value_base64)?;
    if is_tuple {
        extract_bounty_id_tuple(&scval)
    } else {
        extract_bounty_id_single(&scval)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build base64 XDR for a single `ScVal::Bytes` of `n` zero bytes.
    fn bytes_xdr(n: usize) -> String {
        use stellar_xdr::WriteXdr;
        let scval = ScVal::Bytes(stellar_xdr::ScBytes(vec![0u8; n].try_into().unwrap()));
        let raw = scval.to_xdr(Limits::none()).unwrap();
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, raw)
    }

    /// Build base64 XDR for an `ScVal::Vec` whose first element is
    /// `ScVal::Bytes(n)` and second is `ScVal::U32(42)`.
    fn tuple_xdr(n: usize) -> String {
        use stellar_xdr::{ScVal, ScVec, WriteXdr};
        let scval = ScVal::Vec(Some(ScVec(
            vec![
                ScVal::Bytes(stellar_xdr::ScBytes(vec![0u8; n].try_into().unwrap())),
                ScVal::U32(42),
            ]
            .try_into()
            .unwrap(),
        )));
        let raw = scval.to_xdr(Limits::none()).unwrap();
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, raw)
    }

    #[test]
    fn test_extract_bounty_id_single_valid_32_bytes() {
        let xdr = bytes_xdr(BOUNTY_ID_LEN);
        let id = extract_bounty_id_from_xdr(&xdr, false).unwrap();
        assert_eq!(id.len(), BOUNTY_ID_LEN * 2); // hex doubles the length
    }

    #[test]
    fn test_extract_bounty_id_single_wrong_length() {
        let xdr = bytes_xdr(31);
        let err = extract_bounty_id_from_xdr(&xdr, false).unwrap_err();
        assert!(err.contains("32 bytes"), "unexpected error: {err}");
    }

    #[test]
    fn test_extract_bounty_id_single_wrong_shape() {
        // Pass a tuple-shaped payload where a single-value is expected.
        let xdr = tuple_xdr(BOUNTY_ID_LEN);
        let err = extract_bounty_id_from_xdr(&xdr, false).unwrap_err();
        assert!(err.contains("Bytes"), "unexpected error: {err}");
    }

    #[test]
    fn test_extract_bounty_id_tuple_valid() {
        let xdr = tuple_xdr(BOUNTY_ID_LEN);
        let id = extract_bounty_id_from_xdr(&xdr, true).unwrap();
        assert_eq!(id.len(), BOUNTY_ID_LEN * 2);
    }

    #[test]
    fn test_extract_bounty_id_tuple_wrong_first_element_length() {
        let xdr = tuple_xdr(16);
        let err = extract_bounty_id_from_xdr(&xdr, true).unwrap_err();
        assert!(err.contains("32 bytes"), "unexpected error: {err}");
    }

    #[test]
    fn test_extract_bounty_id_tuple_wrong_shape() {
        // Pass a single-value payload where a tuple is expected.
        let xdr = bytes_xdr(BOUNTY_ID_LEN);
        let err = extract_bounty_id_from_xdr(&xdr, true).unwrap_err();
        assert!(err.contains("Vec"), "unexpected error: {err}");
    }

    #[test]
    fn test_decode_scval_malformed_base64() {
        let err = decode_scval("!!!not-base64!!!").unwrap_err();
        assert!(err.contains("XDR decode failed"), "unexpected error: {err}");
    }

    #[test]
    fn test_decode_scval_truncated_xdr() {
        // A single zero byte is not valid XDR for any ScVal.
        let err = decode_scval("AA==").unwrap_err();
        assert!(err.contains("XDR decode failed"), "unexpected error: {err}");
    }
}
