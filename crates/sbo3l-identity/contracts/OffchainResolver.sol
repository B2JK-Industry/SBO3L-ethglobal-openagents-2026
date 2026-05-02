// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title SBO3L OffchainResolver
/// @notice ENSIP-10 / EIP-3668 OffchainResolver. Reverts with
///         OffchainLookup so ENSIP-10-aware clients (viem, ethers.js,
///         etc.) fetch text records from the SBO3L CCIP-Read gateway
///         (`apps/ccip-gateway/`, deployed at sbo3l-ccip.vercel.app)
///         and re-submit through `resolveCallback` for on-chain
///         signature verification.
///
/// @dev    Pinned to the ENS Labs reference impl shape (see
///         https://github.com/ensdomains/offchain-resolver). Minimal
///         on-chain footprint: one `gatewaySigner` address baked at
///         deploy time, one URL list, one signature-verifying callback.
///         The off-chain gateway holds the `GATEWAY_PRIVATE_KEY`
///         whose address must match `gatewaySigner` here.
///
///         Trust model: an attacker who controls the gateway URL but
///         NOT the gateway private key cannot return tampered records
///         — the callback's EIP-191 "intended validator" signature
///         check fails. Compromise = wrong record served to the
///         caller, NEVER fund movement (this contract handles no
///         funds).

/// @dev EIP-3668 OffchainLookup error. Clients catching this error
///      should fetch from `urls` and call `callbackFunction`.
error OffchainLookup(
    address sender,
    string[] urls,
    bytes callData,
    bytes4 callbackFunction,
    bytes extraData
);

error InvalidSignerLength();
error SignatureExpired(uint64 expires, uint256 currentTime);
error UnauthorizedSigner(address recovered, address expected);

interface IExtendedResolver {
    function resolve(bytes calldata name, bytes calldata data)
        external
        view
        returns (bytes memory);
}

contract OffchainResolver is IExtendedResolver {
    /// @notice Address whose private key the off-chain gateway uses
    ///         to sign responses. Pinned at deploy time; rotation is
    ///         redeploy.
    address public immutable gatewaySigner;

    /// @notice Gateway URL templates. Clients pick one from this list
    ///         and substitute `{sender}` and `{data}` per ENSIP-10.
    string[] public urls;

    constructor(address _gatewaySigner, string[] memory _urls) {
        if (_gatewaySigner == address(0)) revert InvalidSignerLength();
        gatewaySigner = _gatewaySigner;
        urls = _urls;
    }

    /// @notice ENSIP-10 entry point. Always reverts with OffchainLookup
    ///         so the caller fetches from `urls` and re-submits via
    ///         `resolveCallback`.
    /// @dev    `name` is DNS-encoded; we don't decode it here because
    ///         the gateway is namehash-driven. We do pass the original
    ///         `data` (the inner `text(node, key)` / `addr(node)`
    ///         calldata) plus the same data as `extraData` so the
    ///         callback can re-hash it for signature verification.
    function resolve(bytes calldata, /* name */ bytes calldata data)
        external
        view
        override
        returns (bytes memory)
    {
        revert OffchainLookup(
            address(this),
            urls,
            data,
            this.resolveCallback.selector,
            data
        );
    }

    /// @notice Verifies the gateway signature on the response and
    ///         returns the decoded record value.
    /// @param  response abi.encode(bytes value, uint64 expires, bytes signature)
    /// @param  extraData the original `data` passed to resolve()
    /// @return result The ABI-encoded record value (e.g. `(string)`
    ///         tuple for text records).
    function resolveCallback(bytes calldata response, bytes calldata extraData)
        external
        view
        returns (bytes memory result)
    {
        (bytes memory value, uint64 expires, bytes memory signature) =
            abi.decode(response, (bytes, uint64, bytes));

        if (block.timestamp > expires) {
            revert SignatureExpired(expires, block.timestamp);
        }

        bytes32 digest = keccak256(
            abi.encodePacked(
                hex"1900",
                address(this),
                expires,
                keccak256(extraData),
                keccak256(value)
            )
        );

        address recovered = recoverSigner(digest, signature);
        if (recovered != gatewaySigner) {
            revert UnauthorizedSigner(recovered, gatewaySigner);
        }

        return value;
    }

    /// @notice ECDSA recovery for a 65-byte (r, s, v) signature over a
    ///         32-byte digest. Pure, no state, no events.
    function recoverSigner(bytes32 digest, bytes memory sig)
        public
        pure
        returns (address)
    {
        if (sig.length != 65) revert InvalidSignerLength();

        bytes32 r;
        bytes32 s;
        uint8 v;
        // solhint-disable-next-line no-inline-assembly
        assembly {
            r := mload(add(sig, 0x20))
            s := mload(add(sig, 0x40))
            v := byte(0, mload(add(sig, 0x60)))
        }
        if (v < 27) v += 27;
        return ecrecover(digest, v, r, s);
    }

    /// @notice Number of gateway URLs configured.
    function urlsLength() external view returns (uint256) {
        return urls.length;
    }

    /// @notice ENSIP-10 / ERC-165 interface advertisement.
    function supportsInterface(bytes4 interfaceId) external pure returns (bool) {
        return
            interfaceId == type(IExtendedResolver).interfaceId ||
            interfaceId == 0x01ffc9a7; // ERC-165
    }
}
