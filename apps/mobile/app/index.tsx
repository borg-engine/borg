import { Redirect } from "expo-router";
import { useAuth } from "@/lib/auth-context";
import { LoadingScreen } from "@/components/LoadingScreen";

export default function Index() {
  const { ready, authenticated } = useAuth();

  if (!ready) return <LoadingScreen />;

  if (authenticated) {
    return <Redirect href="/(tabs)" />;
  }

  return <Redirect href="/(auth)/login" />;
}
