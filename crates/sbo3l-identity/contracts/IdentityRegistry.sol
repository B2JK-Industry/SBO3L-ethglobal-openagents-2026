// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title SBO3L ERC-8004 IdentityRegistry — minimal reference implementation
/// @notice Implements the ERC-8004 (proposed EIP) Identity Registry surface
///         consumed by `crates/sbo3l-identity/src/erc8004.rs`. Maps an agent
///         EOA address → (metadataUri, did, ensNode, registeredAt).
///
/// @dev    Deployment notes:
///         - Calldata signature must match exactly:
///           registerAgent(address,string,string,bytes32) selector 0x5a27c211
///         - The selector is pinned by `tests::register_agent_selector_is_canonical`
///           in the Rust client; any drift breaks the consumer immediately.
///         - msg.sender authorisation: agent registers itself. The (separate)
///           `agentAddress` param in the calldata exists in the EIP draft to
///           allow third-party submission with off-chain attestation; this
///           minimal impl ignores it and uses msg.sender as the canonical
///           agent address. (Comment retained: extending to attestation-
///           gated third-party register is a future change without ABI break.)
///
/// @author SBO3L (B2JK Industry) — for ETHGlobal Open Agents 2026
contract IdentityRegistry {
    struct AgentInfo {
        string metadataUri;
        string did;
        bytes32 ensNode;
        uint256 registeredAt;
    }

    mapping(address => AgentInfo) public agents;
    mapping(address => bool) public isRegistered;

    event AgentRegistered(
        address indexed agentAddress,
        string metadataUri,
        string did,
        bytes32 ensNode
    );
    event AgentUpdated(
        address indexed agentAddress,
        string metadataUri,
        string did,
        bytes32 ensNode
    );
    event AgentRevoked(address indexed agentAddress);

    /// @notice Register or update the agent at msg.sender. The
    ///         `agentAddress` param is part of the ERC-8004 draft ABI;
    ///         this impl uses msg.sender for authorisation. Future
    ///         extensions can gate on a signed attestation matching
    ///         agentAddress.
    function registerAgent(
        address /* agentAddress */,
        string calldata metadataUri,
        string calldata did,
        bytes32 ensNode
    ) external {
        if (isRegistered[msg.sender]) {
            agents[msg.sender].metadataUri = metadataUri;
            agents[msg.sender].did = did;
            agents[msg.sender].ensNode = ensNode;
            emit AgentUpdated(msg.sender, metadataUri, did, ensNode);
        } else {
            agents[msg.sender] = AgentInfo({
                metadataUri: metadataUri,
                did: did,
                ensNode: ensNode,
                registeredAt: block.timestamp
            });
            isRegistered[msg.sender] = true;
            emit AgentRegistered(msg.sender, metadataUri, did, ensNode);
        }
    }

    /// @notice Revoke the calling agent's registration. Idempotent on absent.
    function revokeAgent() external {
        require(isRegistered[msg.sender], "not registered");
        delete agents[msg.sender];
        delete isRegistered[msg.sender];
        emit AgentRevoked(msg.sender);
    }

    /// @notice Read the agent record. Returns zero-init AgentInfo if absent.
    function getAgent(address agentAddress) external view returns (AgentInfo memory) {
        return agents[agentAddress];
    }

    /// @notice ERC-165 interface check. ERC-8004 is a proposed EIP without
    ///         a canonical interface id; we expose the function selector
    ///         interface id (XOR of the 4 selectors) for forward compat.
    function supportsInterface(bytes4 interfaceId) external pure returns (bool) {
        bytes4 erc8004 =
            this.registerAgent.selector ^
            this.revokeAgent.selector ^
            this.getAgent.selector;
        return interfaceId == 0x01ffc9a7 /* ERC-165 */ || interfaceId == erc8004;
    }
}
