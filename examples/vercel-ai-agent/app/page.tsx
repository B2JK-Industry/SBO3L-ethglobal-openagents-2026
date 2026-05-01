"use client";

import { useChat } from "ai/react";

export default function Page(): JSX.Element {
  const { messages, input, handleInputChange, handleSubmit } = useChat({
    api: "/api/chat",
  });

  return (
    <main style={{ maxWidth: 720, margin: "2rem auto", padding: "1rem", fontFamily: "system-ui" }}>
      <h1>SBO3L × Vercel AI SDK demo</h1>
      <p style={{ color: "#666" }}>
        Try: <em>&quot;Pay 0.05 USDC for an inference call to api.example.com.&quot;</em>
      </p>

      <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem", margin: "1rem 0" }}>
        {messages.map((m) => (
          <div key={m.id} style={{ padding: "0.5rem", borderRadius: 4, background: m.role === "user" ? "#eef" : "#efe" }}>
            <strong>{m.role}:</strong> {m.content}
          </div>
        ))}
      </div>

      <form onSubmit={handleSubmit}>
        <input
          value={input}
          onChange={handleInputChange}
          placeholder="Ask the agent to pay..."
          style={{ width: "100%", padding: "0.5rem", border: "1px solid #ccc", borderRadius: 4 }}
        />
      </form>
    </main>
  );
}
