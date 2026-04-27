//! Agent Payment Request Protocol (APRP) v1 types.
//! Mirrors `schemas/aprp_v1.json` and §2 of `docs/spec/17_interface_contracts.md`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaymentRequest {
    pub agent_id: String,
    pub task_id: String,
    pub intent: Intent,
    pub amount: Money,
    pub token: String,
    pub destination: Destination,
    pub payment_protocol: PaymentProtocol,
    pub chain: String,
    pub provider_url: String,
    #[serde(default)]
    pub x402_payload: Option<serde_json::Value>,
    pub expiry: chrono::DateTime<chrono::Utc>,
    pub nonce: String,
    #[serde(default)]
    pub expected_result: Option<ExpectedResult>,
    pub risk_class: RiskClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    PurchaseApiCall,
    PurchaseDataset,
    PayComputeJob,
    PayAgentService,
    Tip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Money {
    pub value: String,
    pub currency: Currency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Currency {
    USD,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Destination {
    X402Endpoint {
        url: String,
        method: HttpMethod,
        #[serde(default)]
        expected_recipient: Option<String>,
    },
    Eoa {
        address: String,
    },
    SmartAccount {
        address: String,
    },
    Erc20Transfer {
        token_address: String,
        recipient: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentProtocol {
    X402,
    L402,
    Erc20Transfer,
    SmartAccountSession,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedResult {
    pub kind: ExpectedResultKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedResultKind {
    Json,
    File,
    Receipt,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskClass {
    Low,
    Medium,
    High,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialise_golden_minimal_fixture() {
        let raw = include_str!("../../../test-corpus/aprp/golden_001_minimal.json");
        let value: serde_json::Value = serde_json::from_str(raw).unwrap();
        let typed: PaymentRequest = serde_json::from_value(value).unwrap();
        assert_eq!(typed.agent_id, "research-agent-01");
        assert_eq!(typed.intent, Intent::PurchaseApiCall);
        assert_eq!(typed.token, "USDC");
    }

    #[test]
    fn deserialise_prompt_injection_fixture() {
        let raw = include_str!("../../../test-corpus/aprp/deny_prompt_injection_request.json");
        let value: serde_json::Value = serde_json::from_str(raw).unwrap();
        let typed: PaymentRequest = serde_json::from_value(value).unwrap();
        assert_eq!(typed.risk_class, RiskClass::Critical);
        match typed.destination {
            Destination::Erc20Transfer { recipient, .. } => {
                assert_eq!(recipient, "0x9999999999999999999999999999999999999999");
            }
            _ => panic!("expected erc20_transfer destination"),
        }
    }

    #[test]
    fn unknown_field_is_rejected_by_serde() {
        let raw = include_str!("../../../test-corpus/aprp/adversarial_unknown_field.json");
        let value: serde_json::Value = serde_json::from_str(raw).unwrap();
        let result: std::result::Result<PaymentRequest, _> = serde_json::from_value(value);
        assert!(result.is_err(), "deny_unknown_fields must reject");
    }
}
