#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Env, Symbol, Val, vec as scvec};

    #[test]
    fn test_extract_bounty_id_hex_single_value() {
        let env = Env::default();
        let bounty_id = [0x12u8, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0,
                         0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
                         0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00,
                         0xa1, 0xb2, 0xc3, 0xd4, 0xe5, 0xf6, 0x07, 0x18];
        
        let bounty_id_bytes = soroban_sdk::Bytes::from_slice(&env, &bounty_id);
        let scval = ScVal::Bytes(bounty_id_bytes.into());
        
        let result = extract_bounty_id_hex(&scval);
        assert_eq!(
            result,
            Some("123456789abcdef01122334455667788".to_string() + "99aabbccddeeff00a1b2c3d4e5f60718")
        );
    }

    #[test]
    fn test_extract_bounty_id_hex_tuple_event() {
        let env = Env::default();
        let bounty_id = [0xaau8, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11,
                         0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99,
                         0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
                         0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff];
        
        let bounty_id_bytes = soroban_sdk::Bytes::from_slice(&env, &bounty_id);
        let other_value = ScVal::U64(12345);
        
        let tuple_vec = scvec![&env, bounty_id_bytes.to_val(), other_value.to_val()];
        let scval = ScVal::Vec(Some(tuple_vec.into()));
        
        let result = extract_bounty_id_hex(&scval);
        assert_eq!(
            result,
            Some("aabbccddeeff001122334455667788".to_string() + "990011223344556677"
                 + "8899aabbccddeeff")
        );
    }

    #[test]
    fn test_extract_bounty_id_hex_invalid_length() {
        let env = Env::default();
        let short_bytes = soroban_sdk::Bytes::from_slice(&env, &[0x12, 0x34]);
        let scval = ScVal::Bytes(short_bytes.into());
        
        let result = extract_bounty_id_hex(&scval);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_bounty_id_hex_not_bytes() {
        let scval = ScVal::U64(42);
        let result = extract_bounty_id_hex(&scval);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_bounty_id_hex_empty_tuple() {
        let env = Env::default();
        let empty_vec = scvec![&env];
        let scval = ScVal::Vec(Some(empty_vec.into()));
        
        let result = extract_bounty_id_hex(&scval);
        assert_eq!(result, None);
    }
}
