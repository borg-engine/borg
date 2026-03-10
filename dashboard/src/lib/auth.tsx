import type { ReactNode } from "react";
import { createContext, useCallback, useContext, useEffect, useState } from "react";
import type { AuthUser } from "./api";
import {
  fetchAuthStatus,
  fetchMe,
  getSelectedWorkspaceId,
  loginUser,
  setAuthToken,
  setSelectedWorkspaceId,
  setupAdmin,
  tokenReady,
} from "./api";

interface AuthState {
  ready: boolean;
  needsSetup: boolean;
  user: AuthUser | null;
  login: (username: string, password: string) => Promise<string | null>;
  setup: (username: string, password: string, displayName?: string) => Promise<string | null>;
  logout: () => void;
}

const AuthContext = createContext<AuthState>({
  ready: false,
  needsSetup: false,
  user: null,
  login: async () => "not ready",
  setup: async () => "not ready",
  logout: () => {},
});

export function AuthProvider({ children }: { children: ReactNode }) {
  const [ready, setReady] = useState(false);
  const [needsSetup, setNeedsSetup] = useState(false);
  const [user, setUser] = useState<AuthUser | null>(null);

  useEffect(() => {
    (async () => {
      await tokenReady;

      // Check if users exist
      const status = await fetchAuthStatus();
      if (status.auth_disabled) {
        setUser({ id: 0, username: "admin", display_name: "Admin", is_admin: true, default_workspace_id: 0 });
        setReady(true);
        return;
      }
      if (status.auth_mode === "cloudflare_access") {
        const me = await fetchMe();
        if (me) {
          if (me.default_workspace_id && !getSelectedWorkspaceId()) {
            setSelectedWorkspaceId(me.default_workspace_id);
          }
          setUser(me);
        }
        setReady(true);
        return;
      }
      if (status.needs_setup) {
        setNeedsSetup(true);
        setReady(true);
        return;
      }

      // Try to get current user (works with JWT or shared token)
      const me = await fetchMe();
      if (me) {
        if (me.default_workspace_id && !me.is_admin && !getSelectedWorkspaceId()) {
          setSelectedWorkspaceId(me.default_workspace_id);
        }
        setUser(me);
      }
      setReady(true);
    })();
  }, []);

  const login = useCallback(async (username: string, password: string): Promise<string | null> => {
    const res = await loginUser(username, password);
    if (res.error) return res.error;
    if (!res.token) return "login failed";
    setAuthToken(res.token);
    if (res.user.default_workspace_id && !res.user.is_admin) {
      setSelectedWorkspaceId(res.user.default_workspace_id);
    }
    setUser(res.user);
    setNeedsSetup(false);
    return null;
  }, []);

  const setup = useCallback(
    async (username: string, password: string, displayName?: string): Promise<string | null> => {
      const res = await setupAdmin(username, password, displayName);
      if (res.error) return res.error;
      if (!res.token) return "setup failed";
      setAuthToken(res.token);
      if (res.user.default_workspace_id && !res.user.is_admin) {
        setSelectedWorkspaceId(res.user.default_workspace_id);
      }
      setUser(res.user);
      setNeedsSetup(false);
      return null;
    },
    [],
  );

  const logout = useCallback(() => {
    setAuthToken(null);
    setSelectedWorkspaceId(null);
    setUser(null);
    window.location.reload();
  }, []);

  return (
    <AuthContext.Provider value={{ ready, needsSetup, user, login, setup, logout }}>{children}</AuthContext.Provider>
  );
}

export function useAuth() {
  return useContext(AuthContext);
}
