// API-only project; root layout is required by Next.js App Router
// even when no pages exist. Keeps the bundle minimal.

import type { ReactElement, ReactNode } from "react";

export const metadata = {
  title: "SBO3L Playground API",
  description: "Tier-3 hosted daemon for the SBO3L playground.",
};

export default function RootLayout({ children }: { children: ReactNode }): ReactElement {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
