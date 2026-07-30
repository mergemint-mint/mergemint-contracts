use soroban_sdk::{Address, Env};
use soroban_sdk::token::TokenClient;

pub fn get_token_client(env: &Env, token_address: &Address) -> TokenClient {
    TokenClient::new(env, token_address)
}
