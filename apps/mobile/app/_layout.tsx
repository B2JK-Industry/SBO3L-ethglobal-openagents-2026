import { Stack } from "expo-router";
import { StatusBar } from "expo-status-bar";
import { tokens } from "~/theme";

export default function RootLayout(): JSX.Element {
  return (
    <>
      <StatusBar style="light" />
      <Stack
        screenOptions={{
          headerStyle: { backgroundColor: tokens.bg },
          headerTitleStyle: { color: tokens.fg },
          headerTintColor: tokens.accent,
          contentStyle: { backgroundColor: tokens.bg },
        }}
      >
        <Stack.Screen name="(tabs)" options={{ headerShown: false }} />
        <Stack.Screen name="approval/[id]" options={{ title: "Approval" }} />
        <Stack.Screen name="scan" options={{ title: "Scan capsule" }} />
        <Stack.Screen name="signin" options={{ title: "Sign in", presentation: "modal" }} />
      </Stack>
    </>
  );
}
