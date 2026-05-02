import { Tabs } from "expo-router";
import { tokens } from "~/theme";

export default function TabsLayout(): JSX.Element {
  return (
    <Tabs
      screenOptions={{
        tabBarStyle: { backgroundColor: tokens.bg, borderTopColor: tokens.border },
        tabBarActiveTintColor: tokens.accent,
        tabBarInactiveTintColor: tokens.muted,
        headerStyle: { backgroundColor: tokens.bg },
        headerTitleStyle: { color: tokens.fg },
        headerTintColor: tokens.accent,
      }}
    >
      <Tabs.Screen name="index" options={{ title: "Dashboard" }} />
      <Tabs.Screen name="agents" options={{ title: "Agents" }} />
      <Tabs.Screen name="audit" options={{ title: "Audit" }} />
      <Tabs.Screen name="approvals" options={{ title: "Approvals" }} />
      <Tabs.Screen name="settings" options={{ title: "Settings" }} />
    </Tabs>
  );
}
