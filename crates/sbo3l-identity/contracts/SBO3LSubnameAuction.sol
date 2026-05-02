// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

// SBO3LSubnameAuction — English auction for premium SBO3L subnames.
// Author: SBO3L (Round 13 P3).
//
// 24-hour English auction with a 5% minimum bid increment and
// optional reserve price. The contract escrows bids, refunds
// outbid bidders automatically, and on settlement transfers
// the won subname's right-of-issuance to the winning bidder.
//
// Right-of-issuance, NOT the subname itself: this contract
// doesn't take custody of `sbo3lagent.eth` or perform the
// actual `setSubnodeRecord` call. After the auction settles,
// the operator (off-chain) executes the issuance against the
// winning bidder using the existing `sbo3l agent register`
// flow. The auction's on-chain commitment is the **economic
// right** + the proof that this bidder won.
//
// This split keeps the contract small, no-fund-movement
// beyond bid escrow, no integration with the ENS Registry's
// ownership model. Operators who want the contract to also
// issue the name can wrap it; we don't bake that in.
//
// Lifecycle
//
// 1. Operator calls `createAuction(label, reserve, duration)`.
//    `label` is the agent slug (e.g. `"premium-trader"`); the auction
//    binds it as `<label>.sbo3lagent.eth`. `duration` is in seconds,
//    bounded `[1 hour, 7 days]`.
// 2. Bidders call `bid(auctionId)` with `msg.value`. First bid must
//    be >= reserve. Subsequent bids must beat the current high by
//    at least `MIN_INCREMENT_BPS` (5%). Outbid bidders' funds are
//    queued for `withdrawRefund(auctionId)`.
// 3. After `endTime`, anyone can call `settle(auctionId)`. The
//    operator collects the winning bid; the winner emits a
//    `AuctionSettled(auctionId, winner, label, finalBid)` event.
// 4. If no bid clears the reserve, the auction settles with no
//    winner and the operator emits `AuctionUnsold(auctionId)`.

error InvalidLabel(string reason);
error InvalidReserve(uint256 reserve);
error InvalidDuration(uint64 duration);
error AuctionNotFound(uint256 id);
error AuctionAlreadyEnded(uint256 id);
error AuctionStillRunning(uint256 id);
error AuctionAlreadySettled(uint256 id);
error BidBelowReserve(uint256 bid, uint256 reserve);
error BidIncrementTooSmall(uint256 bid, uint256 minBidRequired);
error CallerNotOperator(address caller, address expected);
error NoRefundOwed(address bidder, uint256 id);
error RefundFailed(uint256 amount);
error PayoutFailed(uint256 amount);
error NoOperatorProceedsOwed(address operator);

event AuctionCreated(
    uint256 indexed id,
    string label,
    uint256 reserve,
    uint64 endTime,
    address operator
);

event BidPlaced(
    uint256 indexed id,
    address indexed bidder,
    uint256 bid,
    address previousHighBidder,
    uint256 previousHighBid
);

event AuctionSettled(
    uint256 indexed id,
    address indexed winner,
    string label,
    uint256 winningBid
);

event AuctionUnsold(uint256 indexed id, string label);

event RefundWithdrawn(uint256 indexed id, address indexed bidder, uint256 amount);

event OperatorProceedsAccrued(uint256 indexed id, address indexed operator, uint256 amount);

event OperatorProceedsWithdrawn(address indexed operator, uint256 amount);

contract SBO3LSubnameAuction {
    /// @notice Minimum bid increment over the current high bid, in
    ///         basis points (1 bp = 0.01%). 500 bp = 5%.
    uint256 public constant MIN_INCREMENT_BPS = 500;

    /// @notice Bound on `duration` passed to `createAuction`.
    uint64 public constant MIN_DURATION = 1 hours;
    uint64 public constant MAX_DURATION = 7 days;

    /// @notice Maximum subname-label length. ENS labels are bounded
    ///         to 255 bytes by the registry; we further cap at 64
    ///         bytes here to keep gas costs predictable.
    uint256 public constant MAX_LABEL_BYTES = 64;

    struct Auction {
        string label;
        uint256 reserve;
        uint64 endTime;
        address operator;
        address highBidder;
        uint256 highBid;
        bool settled;
    }

    /// @notice Per-auction state. Sequence is the auction id; ids
    ///         start at 0 and increment per `createAuction` call.
    mapping(uint256 => Auction) internal _auctions;
    uint256 public auctionCount;

    /// @notice Per-(auction, bidder) refund balance. When a bidder
    ///         is outbid, their stake moves here. Pull-pattern (vs
    ///         push-pattern transfer) so a malicious previous bidder
    ///         can't lock the auction with a revert-on-receive
    ///         contract.
    mapping(uint256 => mapping(address => uint256)) internal _refundsOwed;

    /// @notice Per-operator settled-auction proceeds. After `settle`
    ///         credits the winning bid here, the operator pulls via
    ///         `withdrawOperatorProceeds`. Pull-pattern protects
    ///         settlement from a misconfigured operator address (e.g.
    ///         a contract with a reverting `receive`) — without this,
    ///         a push-style transfer in `settle` would brick the
    ///         auction permanently.
    mapping(address => uint256) internal _operatorProceeds;

    /// @notice Create an auction. Caller becomes the operator entitled
    ///         to the winning bid on settlement.
    function createAuction(
        string calldata label,
        uint256 reserve,
        uint64 duration
    ) external returns (uint256 id) {
        bytes memory labelBytes = bytes(label);
        if (labelBytes.length == 0) revert InvalidLabel("empty");
        if (labelBytes.length > MAX_LABEL_BYTES) revert InvalidLabel("too long");
        for (uint256 i = 0; i < labelBytes.length; i++) {
            bytes1 c = labelBytes[i];
            // Allow lowercase a-z, 0-9, '-'. ENS DNS-label conventions.
            bool ok = (c >= 0x61 && c <= 0x7A) // a-z
                || (c >= 0x30 && c <= 0x39)    // 0-9
                || c == 0x2D;                  // '-'
            if (!ok) revert InvalidLabel("char");
        }
        // Reject leading/trailing hyphens for a hint of DNS hygiene.
        if (labelBytes[0] == 0x2D || labelBytes[labelBytes.length - 1] == 0x2D) {
            revert InvalidLabel("hyphen-edge");
        }

        if (reserve == 0) revert InvalidReserve(reserve);
        if (duration < MIN_DURATION || duration > MAX_DURATION) {
            revert InvalidDuration(duration);
        }

        id = auctionCount;
        auctionCount = id + 1;
        _auctions[id] = Auction({
            label: label,
            reserve: reserve,
            endTime: uint64(block.timestamp) + duration,
            operator: msg.sender,
            highBidder: address(0),
            highBid: 0,
            settled: false
        });
        emit AuctionCreated(id, label, reserve, uint64(block.timestamp) + duration, msg.sender);
    }

    /// @notice Place a bid. Pay msg.value. Must be >= reserve for the
    ///         first bid; subsequent bids must beat the current high
    ///         by at least `MIN_INCREMENT_BPS` basis points.
    function bid(uint256 id) external payable {
        Auction storage a = _auctions[id];
        if (a.endTime == 0) revert AuctionNotFound(id);
        if (block.timestamp >= a.endTime) revert AuctionAlreadyEnded(id);

        if (a.highBid == 0) {
            // First bid — must clear reserve.
            if (msg.value < a.reserve) revert BidBelowReserve(msg.value, a.reserve);
        } else {
            // Subsequent bid — must beat current high by min increment.
            // Floor the increment at 1 wei so tiny bids (highBid <
            // 10_000 / MIN_INCREMENT_BPS) can't be replaced by an
            // equal bid; otherwise integer truncation makes the
            // computed increment 0 and `minRequired == highBid`.
            uint256 increment = (a.highBid * MIN_INCREMENT_BPS) / 10_000;
            if (increment == 0) {
                increment = 1;
            }
            uint256 minRequired = a.highBid + increment;
            if (msg.value < minRequired) {
                revert BidIncrementTooSmall(msg.value, minRequired);
            }
        }

        address prevBidder = a.highBidder;
        uint256 prevBid = a.highBid;

        a.highBidder = msg.sender;
        a.highBid = msg.value;

        // Queue refund for the outbid bidder. Pull-pattern.
        if (prevBidder != address(0)) {
            _refundsOwed[id][prevBidder] += prevBid;
        }

        emit BidPlaced(id, msg.sender, msg.value, prevBidder, prevBid);
    }

    /// @notice Withdraw a queued refund. Pull-pattern protects the
    ///         auction from a malicious previous bidder using a
    ///         revert-on-receive contract.
    function withdrawRefund(uint256 id) external returns (uint256 amount) {
        amount = _refundsOwed[id][msg.sender];
        if (amount == 0) revert NoRefundOwed(msg.sender, id);
        _refundsOwed[id][msg.sender] = 0;
        (bool ok, ) = payable(msg.sender).call{value: amount}("");
        if (!ok) revert RefundFailed(amount);
        emit RefundWithdrawn(id, msg.sender, amount);
    }

    /// @notice Settle an auction. Anyone can call after the end time;
    ///         the operator collects the winning bid (if any).
    ///         Idempotent — second call reverts with
    ///         `AuctionAlreadySettled`.
    function settle(uint256 id) external {
        Auction storage a = _auctions[id];
        if (a.endTime == 0) revert AuctionNotFound(id);
        if (block.timestamp < a.endTime) revert AuctionStillRunning(id);
        if (a.settled) revert AuctionAlreadySettled(id);

        a.settled = true;

        if (a.highBidder == address(0)) {
            // No bids cleared the reserve.
            emit AuctionUnsold(id, a.label);
            return;
        }

        uint256 winningBid = a.highBid;
        emit AuctionSettled(id, a.highBidder, a.label, winningBid);

        // Pull-pattern proceeds — settle is permissionless ("anyone can
        // call after end time") so a push transfer to a contract-typed
        // operator with a reverting `receive` would otherwise brick the
        // settle path forever and trap the winning bid in this contract.
        _operatorProceeds[a.operator] += winningBid;
        emit OperatorProceedsAccrued(id, a.operator, winningBid);
    }

    /// @notice Withdraw queued settlement proceeds for the calling
    ///         operator. Pull-pattern; mirrors `withdrawRefund`.
    function withdrawOperatorProceeds() external returns (uint256 amount) {
        amount = _operatorProceeds[msg.sender];
        if (amount == 0) revert NoOperatorProceedsOwed(msg.sender);
        _operatorProceeds[msg.sender] = 0;
        (bool ok, ) = payable(msg.sender).call{value: amount}("");
        if (!ok) revert PayoutFailed(amount);
        emit OperatorProceedsWithdrawn(msg.sender, amount);
    }

    /// @notice Read an auction's full state. Returns the zero-shape
    ///         for an unknown id (callers compare `endTime != 0`).
    function getAuction(uint256 id) external view returns (Auction memory) {
        return _auctions[id];
    }

    /// @notice Read the queued refund balance for (auction, bidder).
    function refundOwed(uint256 id, address bidder) external view returns (uint256) {
        return _refundsOwed[id][bidder];
    }

    /// @notice Read the queued settlement-proceeds balance for an
    ///         operator. Pull-pattern; operator calls
    ///         `withdrawOperatorProceeds` to claim.
    function operatorProceeds(address operator) external view returns (uint256) {
        return _operatorProceeds[operator];
    }

    /// @notice ERC-165 advertisement. Currently advertises only IERC165;
    ///         specific auction interface id is reserved for future
    ///         standardisation.
    function supportsInterface(bytes4 interfaceId) external pure returns (bool) {
        return interfaceId == 0x01ffc9a7;
    }
}
