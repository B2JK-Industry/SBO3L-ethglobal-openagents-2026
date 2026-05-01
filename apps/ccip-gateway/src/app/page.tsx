/**
 * Landing page for the CCIP-Read gateway. Tells operators what they
 * landed on (since some judge will inevitably visit the root URL),
 * points at the API endpoint and the design doc. No styling beyond
 * inline minimal — this is an API host, not a marketing surface.
 */
export default function Home() {
  return (
    <main
      style={{
        fontFamily: "system-ui, sans-serif",
        maxWidth: "42rem",
        margin: "4rem auto",
        padding: "0 1rem",
        lineHeight: 1.5,
      }}
    >
      <h1 style={{ fontSize: "1.5rem" }}>SBO3L CCIP-Read gateway</h1>
      <p>
        ENSIP-25 / EIP-3668 gateway for off-chain SBO3L text records.
        Resolves <code>sbo3l:*</code> records published by SBO3L agents
        whose ENS names point at an OffchainResolver contract.
      </p>
      <p>
        This URL is consumed automatically by{" "}
        <code>viem.getEnsText</code>, <code>ethers.js</code>, and any
        ENSIP-10-aware client. No SBO3L-specific code on the client side.
      </p>
      <h2 style={{ fontSize: "1.1rem", marginTop: "2rem" }}>API</h2>
      <pre
        style={{
          background: "#f5f5f5",
          padding: "0.75rem",
          overflowX: "auto",
          fontSize: "0.85rem",
        }}
      >
        GET /api/{"{sender}"}/{"{data}"}.json
      </pre>
      <h2 style={{ fontSize: "1.1rem", marginTop: "2rem" }}>Status</h2>
      <p>
        Pre-scaffold. Returns <code>501 Not Implemented</code> until
        the T-4-1 main PR ships the record source + signing logic.
      </p>
      <h2 style={{ fontSize: "1.1rem", marginTop: "2rem" }}>References</h2>
      <ul>
        <li>
          <a href="https://eips.ethereum.org/EIPS/eip-3668">EIP-3668 (CCIP-Read)</a>
        </li>
        <li>
          <a href="https://docs.ens.domains/ensip/10">ENSIP-10</a>
        </li>
        <li>
          <a href="https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/docs/design/T-4-1-ccip-read-prep.md">
            T-4-1 design doc
          </a>
        </li>
      </ul>
    </main>
  );
}
