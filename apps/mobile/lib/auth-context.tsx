import React, { createContext, useContext, useEffect, useState, useCallback } from "react";
import { isAuthenticated, getStoredUser, logout as doLogout, login as doLogin } from "./auth";
import type { StoredUser, LoginResult } from "./auth";

interface AuthState {
  ready: boolean;
  authenticated: boolean;
  user: StoredUser | null;
  login: (serverUrl: string, username: string, password: string) => Promise<LoginResult>;
  logout: () => Promise<void>;
  refresh: () => Promise<void>;
}

const AuthContext = createContext<AuthState>({
  ready: false,
  authenticated: false,
  user: null,
  login: async () => ({ success: false, error: "Not initialized" }),
  logout: async () => {},
  refresh: async () => {},
});

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [ready, setReady] = useState(false);
  const [authenticated, setAuthenticated] = useState(false);
  const [user, setUser] = useState<StoredUser | null>(null);

  const refresh = useCallback(async () => {
    const authed = await isAuthenticated();
    setAuthenticated(authed);
    if (authed) {
      const u = await getStoredUser();
      setUser(u);
    } else {
      setUser(null);
    }
    setReady(true);
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const login = useCallback(
    async (serverUrl: string, username: string, password: string): Promise<LoginResult> => {
      const result = await doLogin(serverUrl, username, password);
      if (result.success) {
        setAuthenticated(true);
        setUser(result.user ?? null);
      }
      return result;
    },
    [],
  );

  const logout = useCallback(async () => {
    await doLogout();
    setAuthenticated(false);
    setUser(null);
  }, []);

  return (
    <AuthContext.Provider value={{ ready, authenticated, user, login, logout, refresh }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  return useContext(AuthContext);
}
