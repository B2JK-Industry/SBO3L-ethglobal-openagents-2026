import type { Metadata } from "next";
import type { ReactNode } from "react";

export const metadata: Metadata = {
  title: "SBO3L CCIP-Read gateway",
  description:
    "ENSIP-25 / EIP-3668 gateway for off-chain SBO3L text records.",
  robots: {
    // Robots can index the landing page, but the API endpoints aren't
    // useful to crawl.
    index: true,
    follow: true,
  },
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body style={{ margin: 0, background: "#fff", color: "#222" }}>
        {children}
      </body>
    </html>
  );
}
