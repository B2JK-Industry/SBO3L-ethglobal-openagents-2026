import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "SBO3L — Hosted preview",
  description:
    "Hosted free-tier SBO3L. Login with GitHub, get a daemon, sign your first APRP envelope.",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
