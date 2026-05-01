export default function RootLayout({ children }: { children: React.ReactNode }): JSX.Element {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}

export const metadata = {
  title: "SBO3L × Vercel AI SDK demo",
  description: "Minimal Next.js + Vercel AI SDK + @sbo3l/vercel-ai example.",
};
