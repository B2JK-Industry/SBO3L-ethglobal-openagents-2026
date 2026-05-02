// Push registration. Called once on first sign-in + whenever the
// user enables push in /settings. Token is registered against the
// active tenant so the daemon can fan-out human_2fa events to the
// right device set.

import * as Notifications from "expo-notifications";
import { Platform } from "react-native";
import { api } from "./api";

Notifications.setNotificationHandler({
  handleNotification: async () => ({
    shouldShowAlert: true,
    shouldPlaySound: true,
    shouldSetBadge: true,
    shouldShowBanner: true,
    shouldShowList: true,
  }),
});

export async function registerForPushNotifications(tenantSlug: string): Promise<string | null> {
  if (Platform.OS === "web") return null;

  const existing = await Notifications.getPermissionsAsync();
  let status = existing.status;
  if (status !== "granted") {
    const req = await Notifications.requestPermissionsAsync();
    status = req.status;
  }
  if (status !== "granted") return null;

  if (Platform.OS === "android") {
    await Notifications.setNotificationChannelAsync("approvals", {
      name: "Approval requests",
      importance: Notifications.AndroidImportance.HIGH,
      lightColor: "#4ade80",
    });
  }

  const tokenResult = await Notifications.getExpoPushTokenAsync();
  await api.registerPushToken(tenantSlug, tokenResult.data);
  return tokenResult.data;
}
