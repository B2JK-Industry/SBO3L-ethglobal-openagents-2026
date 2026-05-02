// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

// SBO3LReputationBond — bond + slashing for reputation publishers.
// Author: SBO3L (Round 13 P7).
//
// Game-theoretic gate on top of SBO3LReputationRegistry. A
// publisher posts a fixed bond before they're allowed to publish
// reputation attestations; if an attestation is later proven
// false, the bond is slashed and split between a challenger
// reward and an insurance pool. The Registry checks
// `hasActiveBond(publisher)` before accepting writes.
//
// ## Trust model
//
// `slasher` is a single account in this contract (the arbiter).
// Real-world deployment points it at a multisig or governance
// contract — the contract doesn't bake a dispute-resolution
// algorithm in. On-chain dispute resolution (commit/reveal,
// optimistic challenges, ZK fraud proofs) is documented as future
// work; the hackathon scope is the bond accounting + the
// permissioned slash hook.
//
// `insuranceBeneficiary` is a single account that the slasher
// configures at deploy time. Slashed funds split 50/50 between
// the challenger (the address that supplied the slash evidence)
// and the insurance pool, which the beneficiary withdraws.
//
// ## Lifecycle
//
// 1. Publisher calls `postBond()` with `msg.value == BOND_AMOUNT`.
//    Bond is locked for `LOCK_PERIOD` (7 days) — a cooling-off
//    window so a publisher can't slash-and-run.
// 2. Publisher calls SBO3LReputationRegistry.writeReputation;
//    consumers verify `bond.hasActiveBond(publisher)` is true.
// 3. If a fraud proof surfaces, the off-chain arbiter (slasher)
//    calls `slash(publisher, challenger, evidenceUri)`. Bond
//    splits 50/50 between challenger + insurance pool. Publisher's
//    `bondedUntil` resets to 0 — they have no active bond and
//    can't publish until they re-bond.
// 4. If no slash, the publisher calls `withdrawBond()` after
//    `LOCK_PERIOD` to reclaim funds.
// 5. The insurance pool is withdrawn by `insuranceBeneficiary` via
//    `withdrawInsurance()`. Pull-pattern.

error InvalidBondAmount(uint256 sent, uint256 required);
error AlreadyBonded(address publisher);
error NoBondToWithdraw(address publisher);
error BondStillLocked(uint256 unlocksAt);
error CallerNotSlasher(address caller, address expected);
error CallerNotBeneficiary(address caller, address expected);
error PublisherHasNoBond(address publisher);
error InvalidChallenger();
error WithdrawFailed(uint256 amount);
error InsurancePoolEmpty();

event BondPosted(address indexed publisher, uint256 amount, uint256 lockedUntil);
event BondWithdrawn(address indexed publisher, uint256 amount);
event BondSlashed(
    address indexed publisher,
    address indexed challenger,
    uint256 publisherBondLost,
    uint256 challengerReward,
    uint256 insurancePoolGain,
    string evidenceUri
);
event InsuranceWithdrawn(address indexed beneficiary, uint256 amount);

contract SBO3LReputationBond {
    /// @notice Fixed bond amount. Round 13 P7 spec: 0.01 ETH.
    uint256 public constant BOND_AMOUNT = 0.01 ether;

    /// @notice Lock period before a non-slashed publisher can
    ///         withdraw. Cooling-off window prevents
    ///         publish-then-immediately-withdraw attacks.
    uint64 public constant LOCK_PERIOD = 7 days;

    /// @notice Slasher (arbiter). Set at deploy, immutable.
    ///         Production deployments point at a multisig.
    address public immutable slasher;

    /// @notice Insurance pool beneficiary. Set at deploy, immutable.
    address public immutable insuranceBeneficiary;

    /// @notice Per-publisher bond state.
    /// @dev    `lockedUntil == 0` means no active bond; the
    ///         publisher can post.
    struct BondState {
        uint256 amount;
        uint64 lockedUntil;
    }

    mapping(address => BondState) internal _bonds;

    /// @notice Insurance pool balance. Slashed funds accumulate here
    ///         until `withdrawInsurance` drains them.
    uint256 public insurancePool;

    constructor(address slasher_, address insuranceBeneficiary_) {
        require(slasher_ != address(0), "slasher zero");
        require(insuranceBeneficiary_ != address(0), "beneficiary zero");
        slasher = slasher_;
        insuranceBeneficiary = insuranceBeneficiary_;
    }

    /// @notice Post a bond. msg.value MUST equal BOND_AMOUNT.
    ///         Publisher with an active bond cannot re-post until
    ///         the existing bond is withdrawn or slashed.
    function postBond() external payable {
        if (msg.value != BOND_AMOUNT) {
            revert InvalidBondAmount(msg.value, BOND_AMOUNT);
        }
        BondState storage b = _bonds[msg.sender];
        if (b.amount > 0) revert AlreadyBonded(msg.sender);
        b.amount = msg.value;
        b.lockedUntil = uint64(block.timestamp) + LOCK_PERIOD;
        emit BondPosted(msg.sender, msg.value, b.lockedUntil);
    }

    /// @notice Withdraw an unslashed bond after the lock period.
    function withdrawBond() external {
        BondState storage b = _bonds[msg.sender];
        if (b.amount == 0) revert NoBondToWithdraw(msg.sender);
        if (block.timestamp < b.lockedUntil) {
            revert BondStillLocked(b.lockedUntil);
        }
        uint256 amount = b.amount;
        b.amount = 0;
        b.lockedUntil = 0;
        (bool ok, ) = payable(msg.sender).call{value: amount}("");
        if (!ok) revert WithdrawFailed(amount);
        emit BondWithdrawn(msg.sender, amount);
    }

    /// @notice Slash a publisher's bond. Caller MUST be `slasher`.
    ///         Splits 50% to `challenger`, 50% to insurance pool.
    ///         Publisher's bond state resets — they need to
    ///         re-post to publish again.
    /// @param  publisher       Address whose bond is slashed.
    /// @param  challenger      Recipient of the challenger reward.
    /// @param  evidenceUri     Off-chain pointer to the fraud proof
    ///                         (IPFS / HTTPS / arbitrary). Logged
    ///                         in the slash event for auditability.
    function slash(
        address publisher,
        address challenger,
        string calldata evidenceUri
    ) external {
        if (msg.sender != slasher) {
            revert CallerNotSlasher(msg.sender, slasher);
        }
        if (challenger == address(0)) revert InvalidChallenger();
        BondState storage b = _bonds[publisher];
        if (b.amount == 0) revert PublisherHasNoBond(publisher);

        uint256 lost = b.amount;
        // 50/50 split. Integer division — for BOND_AMOUNT = 0.01
        // ETH = 1e16 wei, the split is exact.
        uint256 reward = lost / 2;
        uint256 poolGain = lost - reward;

        b.amount = 0;
        b.lockedUntil = 0;

        insurancePool += poolGain;

        (bool ok, ) = payable(challenger).call{value: reward}("");
        if (!ok) revert WithdrawFailed(reward);

        emit BondSlashed(publisher, challenger, lost, reward, poolGain, evidenceUri);
    }

    /// @notice Withdraw the entire insurance pool. Caller MUST be
    ///         `insuranceBeneficiary`. Pull-pattern.
    function withdrawInsurance() external {
        if (msg.sender != insuranceBeneficiary) {
            revert CallerNotBeneficiary(msg.sender, insuranceBeneficiary);
        }
        uint256 amount = insurancePool;
        if (amount == 0) revert InsurancePoolEmpty();
        insurancePool = 0;
        (bool ok, ) = payable(insuranceBeneficiary).call{value: amount}("");
        if (!ok) revert WithdrawFailed(amount);
        emit InsuranceWithdrawn(insuranceBeneficiary, amount);
    }

    /// @notice Has `publisher` posted an active (non-slashed) bond?
    ///         Consumers check this before accepting their
    ///         attestations. A bond just posted but still locked
    ///         counts as active — the lock prevents withdrawal,
    ///         not publishing.
    function hasActiveBond(address publisher) external view returns (bool) {
        return _bonds[publisher].amount > 0;
    }

    /// @notice Read bond state.
    function bondOf(address publisher) external view returns (BondState memory) {
        return _bonds[publisher];
    }

    /// @notice ERC-165.
    function supportsInterface(bytes4 interfaceId) external pure returns (bool) {
        return interfaceId == 0x01ffc9a7;
    }
}
