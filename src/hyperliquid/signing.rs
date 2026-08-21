use anyhow::{Context, Result};
use k256::ecdsa::{RecoveryId, Signature, SigningKey};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LimitWire {
    pub tif: String, // "Gtc", "Alo" (Post-Only), "Ioc"
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OrderTypeWire {
    pub limit: LimitWire,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OrderWire {
    pub a: u32,           // asset index
    pub b: bool,          // is_buy
    pub p: String,        // price
    pub s: String,        // size
    pub r: bool,          // reduce_only
    pub t: OrderTypeWire, // order type
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CancelWire {
    pub a: u32, // asset index
    pub o: u64, // order id
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ExchangeAction {
    #[serde(rename = "order")]
    Order {
        orders: Vec<OrderWire>,
        grouping: String,
    },
    #[serde(rename = "cancel")]
    Cancel { cancels: Vec<CancelWire> },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SignaturePayload {
    pub r: String,
    pub s: String,
    pub v: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeRequestPayload {
    pub action: ExchangeAction,
    pub nonce: u64,
    pub signature: SignaturePayload,
    pub vault_address: Option<String>,
}

pub struct HyperliquidSigner;

impl HyperliquidSigner {
    /// 计算 EIP-712 Domain Separator for Hyperliquid Exchange
    pub fn compute_domain_separator(chain_id: u64) -> [u8; 32] {
        let domain_typehash = Keccak256::digest(
            b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
        );
        let name_hash = Keccak256::digest(b"Exchange");
        let version_hash = Keccak256::digest(b"1");

        let mut chain_id_bytes = [0u8; 32];
        chain_id_bytes[24..32].copy_from_slice(&chain_id.to_be_bytes());

        let verifying_contract_bytes = [0u8; 32]; // address(0)

        let mut hasher = Keccak256::new();
        hasher.update(domain_typehash);
        hasher.update(name_hash);
        hasher.update(version_hash);
        hasher.update(chain_id_bytes);
        hasher.update(verifying_contract_bytes);

        let result = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&result);
        out
    }

    /// 对 L1 Action 进行 MessagePack 编码并计算 connectionId
    pub fn compute_connection_id(action: &ExchangeAction, nonce: u64) -> Result<[u8; 32]> {
        let mut msgpack_bytes =
            rmp_serde::to_vec_named(action).context("Failed to serialize action to msgpack")?;
        msgpack_bytes.extend_from_slice(&nonce.to_be_bytes());
        // vault address 0 for direct trading
        msgpack_bytes.push(0x00);

        let hash = Keccak256::digest(&msgpack_bytes);
        let mut out = [0u8; 32];
        out.copy_from_slice(&hash);
        Ok(out)
    }

    /// 构建 EIP-712 Phantom Agent 消息哈希并签名
    pub fn sign_l1_action(
        action: &ExchangeAction,
        nonce: u64,
        private_key_hex: &str,
        is_mainnet: bool,
    ) -> Result<SignaturePayload> {
        let clean_pk = private_key_hex
            .trim()
            .trim_start_matches("0x")
            .trim_start_matches("0X");
        let pk_bytes = hex::decode(clean_pk).context("Invalid hex private key")?;

        let signing_key =
            SigningKey::from_bytes((&pk_bytes[..]).into()).context("Invalid secp256k1 key")?;

        let chain_id = if is_mainnet { 1337 } else { 421614 };
        let domain_separator = Self::compute_domain_separator(chain_id);

        let connection_id = Self::compute_connection_id(action, nonce)?;

        // Agent(string source,bytes32 connectionId)
        let agent_typehash = Keccak256::digest(b"Agent(string source,bytes32 connectionId)");
        let source_str = if is_mainnet { "a" } else { "b" };
        let source_hash = Keccak256::digest(source_str.as_bytes());

        let mut agent_hasher = Keccak256::new();
        agent_hasher.update(agent_typehash);
        agent_hasher.update(source_hash);
        agent_hasher.update(connection_id);
        let agent_struct_hash = agent_hasher.finalize();

        // EIP-712 digest: keccak256("\x19\x01" + domain_separator + struct_hash)
        let mut digest_hasher = Keccak256::new();
        digest_hasher.update([0x19, 0x01]);
        digest_hasher.update(domain_separator);
        digest_hasher.update(agent_struct_hash);
        let digest = digest_hasher.finalize();

        let (sig, recid): (Signature, RecoveryId) =
            signing_key
                .sign_prehash_recoverable(&digest)
                .context("Failed to sign prehash with secp256k1")?;

        let r_bytes = &sig.to_bytes()[..32];
        let s_bytes = &sig.to_bytes()[32..64];
        let v_val = 27 + recid.to_byte();

        Ok(SignaturePayload {
            r: format!("0x{}", hex::encode(r_bytes)),
            s: format!("0x{}", hex::encode(s_bytes)),
            v: v_val,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_separator() {
        let dom = HyperliquidSigner::compute_domain_separator(1337);
        assert_eq!(dom.len(), 32);
    }

    #[test]
    fn test_sign_order_action() {
        let action = ExchangeAction::Order {
            orders: vec![OrderWire {
                a: 0,
                b: true,
                p: "50000".to_string(),
                s: "0.1".to_string(),
                r: false,
                t: OrderTypeWire {
                    limit: LimitWire {
                        tif: "Alo".to_string(),
                    },
                },
            }],
            grouping: "na".to_string(),
        };

        // Dummy private key for testing (32 bytes)
        let test_pk = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let nonce = 1700000000000;
        let sig = HyperliquidSigner::sign_l1_action(&action, nonce, test_pk, true).unwrap();

        assert!(sig.r.starts_with("0x"));
        assert!(sig.s.starts_with("0x"));
        assert!(sig.v == 27 || sig.v == 28);
    }
}
