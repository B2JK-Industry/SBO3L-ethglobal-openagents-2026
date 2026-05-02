// OAuth via expo-auth-session against the hosted-app /api/mobile/auth
// endpoint. The hosted-app forwards to GitHub OAuth (NextAuth flow)
// and returns a session token that lives in expo-secure-store.

import * as AuthSession from "expo-auth-session";
import * as LocalAuthentication from "expo-local-authentication";
import { setToken, clearToken } from "./api";

const REDIRECT_URI = AuthSession.makeRedirectUri({ scheme: "sbo3l", path: "auth/callback" });

export async function signIn(): Promise<boolean> {
  const baseUrl = process.env.EXPO_PUBLIC_API_BASE_URL ?? "https://sbo3l-app.vercel.app";
  const authUrl = `${baseUrl}/api/mobile/auth/start?redirect=${encodeURIComponent(REDIRECT_URI)}`;
  const result = await AuthSession.startAsync({ authUrl });
  if (result.type !== "success" || !result.params.token) return false;
  await setToken(result.params.token);
  return true;
}

export async function signOut(): Promise<void> {
  await clearToken();
}

export async function biometricGate(reason = "Confirm decision approval"): Promise<boolean> {
  const supported = await LocalAuthentication.hasHardwareAsync();
  if (!supported) return true;
  const enrolled = await LocalAuthentication.isEnrolledAsync();
  if (!enrolled) return true;
  const result = await LocalAuthentication.authenticateAsync({ promptMessage: reason });
  return result.success;
}
