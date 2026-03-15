import * as SecureStore from "expo-secure-store";

const SERVER_URL_KEY = "borg_server_url";
const JWT_KEY = "borg_jwt";
const USER_KEY = "borg_user";

export interface StoredUser {
  id: number;
  username: string;
  display_name?: string;
  is_admin: boolean;
  default_workspace_id?: number;
}

export async function getServerUrl(): Promise<string | null> {
  return SecureStore.getItemAsync(SERVER_URL_KEY);
}

export async function setServerUrl(url: string): Promise<void> {
  const normalized = url.replace(/\/+$/, "");
  await SecureStore.setItemAsync(SERVER_URL_KEY, normalized);
}

export async function getToken(): Promise<string | null> {
  return SecureStore.getItemAsync(JWT_KEY);
}

export async function setToken(token: string): Promise<void> {
  await SecureStore.setItemAsync(JWT_KEY, token);
}

export async function getStoredUser(): Promise<StoredUser | null> {
  const raw = await SecureStore.getItemAsync(USER_KEY);
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

export async function setStoredUser(user: StoredUser): Promise<void> {
  await SecureStore.setItemAsync(USER_KEY, JSON.stringify(user));
}

export async function isAuthenticated(): Promise<boolean> {
  const [url, token] = await Promise.all([getServerUrl(), getToken()]);
  return !!(url && token);
}

export interface LoginResult {
  success: boolean;
  error?: string;
  user?: StoredUser;
}

export async function login(
  serverUrl: string,
  username: string,
  password: string,
): Promise<LoginResult> {
  const base = serverUrl.replace(/\/+$/, "");

  try {
    const res = await fetch(`${base}/api/auth/login`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ username, password }),
    });

    const data = await res.json();

    if (!res.ok || data.error) {
      return { success: false, error: data.error || `HTTP ${res.status}` };
    }

    if (!data.token) {
      return { success: false, error: "No token received" };
    }

    await setServerUrl(base);
    await setToken(data.token);

    const user: StoredUser = {
      id: data.user?.id ?? 0,
      username: data.user?.username ?? username,
      display_name: data.user?.display_name,
      is_admin: data.user?.is_admin ?? false,
      default_workspace_id: data.user?.default_workspace_id,
    };

    await setStoredUser(user);

    return { success: true, user };
  } catch (err: any) {
    return {
      success: false,
      error: err?.message || "Connection failed",
    };
  }
}

export async function logout(): Promise<void> {
  await Promise.all([
    SecureStore.deleteItemAsync(JWT_KEY),
    SecureStore.deleteItemAsync(USER_KEY),
  ]);
}

export async function checkConnection(serverUrl: string): Promise<boolean> {
  const base = serverUrl.replace(/\/+$/, "");
  try {
    const res = await fetch(`${base}/api/auth/status`, {
      signal: AbortSignal.timeout(5000),
    });
    return res.ok;
  } catch {
    return false;
  }
}
