import * as Notifications from "expo-notifications";
import { Platform } from "react-native";
import { getServerUrl, getToken } from "./auth";

export async function registerForPushNotifications(): Promise<string | null> {
  const { status } = await Notifications.requestPermissionsAsync();
  if (status !== "granted") return null;
  const tokenData = await Notifications.getExpoPushTokenAsync();
  const pushToken = tokenData.data;

  const serverUrl = await getServerUrl();
  const authToken = await getToken();
  if (!serverUrl || !authToken) return pushToken;

  await fetch(`${serverUrl}/api/push/register`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${authToken}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ token: pushToken, platform: Platform.OS }),
  });

  return pushToken;
}

export async function unregisterPushToken(pushToken: string): Promise<void> {
  const serverUrl = await getServerUrl();
  const authToken = await getToken();
  if (!serverUrl || !authToken) return;

  await fetch(`${serverUrl}/api/push/unregister`, {
    method: "DELETE",
    headers: {
      Authorization: `Bearer ${authToken}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ token: pushToken }),
  });
}

export function setupNotificationHandler() {
  Notifications.setNotificationHandler({
    handleNotification: async () => ({
      shouldShowAlert: true,
      shouldPlaySound: true,
      shouldSetBadge: true,
    }),
  });
}
