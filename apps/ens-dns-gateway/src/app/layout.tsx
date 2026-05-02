export const metadata = {
  title: 'SBO3L ENS DNS gateway',
  description:
    'DNS-over-HTTPS bridge from legacy DNS clients to SBO3L ENS agent identity records.',
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
