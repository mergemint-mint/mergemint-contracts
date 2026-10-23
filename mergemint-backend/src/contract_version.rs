use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use stellar_xdr::{
    ContractId, Hash, HostFunction, InvokeContractArgs, InvokeHostFunctionOp, Limits, MuxedAccount,
    Operation, OperationBody, ReadXdr, ScAddress, ScSymbol, SequenceNumber, SorobanTransactionData,
    Transaction, TransactionEnvelope, TransactionExt, TransactionV1Envelope, Uint256, VecM,
    WriteXdr,
};

#[derive(Debug, Deserialize)]
struct RpcResponse {
    result: Option<SimulateResult>,
    error: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct SimulateResult {
    results: Vec<SimulateOperationResult>,
}

#[derive(Debug, Deserialize)]
struct SimulateOperationResult {
    xdr: String,
}

pub async fn check_contract_version(
    rpc_url: &str,
    contract_id: &str,
    source_account: &str,
) -> Result<String> {
    let contract = stellar_strkey::Contract::from_string(contract_id)
        .context("CONTRACT_ID is not a valid Stellar contract address")?;
    let source = stellar_strkey::ed25519::PublicKey::from_string(source_account)
        .context("CONTRACT_VERSION_SOURCE_ACCOUNT is not a valid Stellar account address")?;
    let function_name = ScSymbol::try_from(b"version".to_vec())?;
    let invoke = InvokeContractArgs {
        contract_address: ScAddress::Contract(ContractId(Hash(contract.0))),
        function_name,
        args: VecM::default(),
    };
    let operation = Operation {
        source_account: None,
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            host_function: HostFunction::InvokeContract(invoke),
            auth: VecM::default(),
        }),
    };
    let transaction = Transaction {
        source_account: MuxedAccount::Ed25519(Uint256(source.0)),
        fee: 100_000,
        seq_num: SequenceNumber(0),
        operations: vec![operation]
            .try_into()
            .map_err(|_| anyhow!("could not construct version query transaction"))?,
        ext: TransactionExt::V1(SorobanTransactionData::default()),
        ..Default::default()
    };
    let envelope = TransactionEnvelope::Tx(TransactionV1Envelope {
        tx: transaction,
        signatures: VecM::default(),
    });
    let transaction_xdr = envelope.to_xdr_base64(Limits::none())?;

    let response: RpcResponse = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?
        .post(rpc_url)
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "simulateTransaction",
            "params": { "transaction": transaction_xdr },
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    if let Some(error) = response.error {
        return Err(anyhow!("Soroban RPC simulateTransaction failed: {error}"));
    }
    let result = response
        .result
        .context("Soroban RPC simulateTransaction response omitted result")?;
    let encoded = result
        .results
        .first()
        .context("contract version query returned no result")?;
    let value = stellar_xdr::ScVal::from_xdr_base64(&encoded.xdr, Limits::none())?;
    let stellar_xdr::ScVal::Symbol(symbol) = value else {
        return Err(anyhow!("contract version() did not return a symbol"));
    };
    String::from_utf8(symbol.into()).context("contract version symbol was not valid UTF-8")
}

pub fn is_supported_version(version: &str) -> bool {
    let Some(numeric) = version.split(['-', '+']).next() else {
        return false;
    };
    let components: Vec<_> = numeric.split('.').collect();
    components.len() == 3
        && components[0] == "1"
        && components[1].parse::<u32>().is_ok()
        && components[2].parse::<u32>().is_ok()
}
