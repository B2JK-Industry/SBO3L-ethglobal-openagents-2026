export { auth as middleware } from "@/auth";

// Auth-protect /dashboard/* and /agents/* and /audit/* and /capsules/*.
// Unauthed visits redirect to NextAuth sign-in page (then back).
export const config = {
  matcher: [
    "/dashboard/:path*",
    "/agents/:path*",
    "/audit/:path*",
    "/capsules/:path*",
  ],
};
