import React, { createContext, useContext, useState, useCallback, useRef } from 'react';
import { Text, StyleSheet } from 'react-native';
import Animated, {
  useSharedValue,
  useAnimatedStyle,
  withTiming,
  withDelay,
  runOnJS,
  SlideInUp,
  SlideOutUp,
} from 'react-native-reanimated';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { Ionicons } from '@expo/vector-icons';
import { colors, spacing, radius, fontSize } from './theme';

type ToastType = 'success' | 'error' | 'info';

interface ToastData {
  id: number;
  message: string;
  type: ToastType;
}

interface ToastContextType {
  show: (message: string, type?: ToastType) => void;
}

const ToastContext = createContext<ToastContextType>({
  show: () => {},
});

export function useToast() {
  return useContext(ToastContext);
}

const TOAST_COLORS: Record<ToastType, { bg: string; fg: string; icon: keyof typeof Ionicons.glyphMap }> = {
  success: { bg: 'rgba(34, 197, 94, 0.15)', fg: colors.success, icon: 'checkmark-circle' },
  error: { bg: 'rgba(239, 68, 68, 0.15)', fg: colors.error, icon: 'alert-circle' },
  info: { bg: 'rgba(59, 130, 246, 0.15)', fg: colors.info, icon: 'information-circle' },
};

function ToastItem({ toast, onDismiss }: { toast: ToastData; onDismiss: (id: number) => void }) {
  const c = TOAST_COLORS[toast.type];

  React.useEffect(() => {
    const timer = setTimeout(() => onDismiss(toast.id), 3000);
    return () => clearTimeout(timer);
  }, [toast.id, onDismiss]);

  return (
    <Animated.View
      entering={SlideInUp.duration(300).springify().damping(15)}
      exiting={SlideOutUp.duration(200)}
      style={[styles.toast, { backgroundColor: c.bg, borderColor: c.fg + '30' }]}
    >
      <Ionicons name={c.icon} size={18} color={c.fg} />
      <Text style={[styles.toastText, { color: c.fg }]} numberOfLines={2}>
        {toast.message}
      </Text>
    </Animated.View>
  );
}

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<ToastData[]>([]);
  const nextId = useRef(0);
  const insets = useSafeAreaInsets();

  const dismiss = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const show = useCallback((message: string, type: ToastType = 'info') => {
    const id = nextId.current++;
    setToasts((prev) => [...prev.slice(-2), { id, message, type }]);
  }, []);

  return (
    <ToastContext.Provider value={{ show }}>
      {children}
      <Animated.View
        style={[styles.container, { top: insets.top + spacing.sm }]}
        pointerEvents="box-none"
      >
        {toasts.map((toast) => (
          <ToastItem key={toast.id} toast={toast} onDismiss={dismiss} />
        ))}
      </Animated.View>
    </ToastContext.Provider>
  );
}

const styles = StyleSheet.create({
  container: {
    position: 'absolute',
    left: spacing.lg,
    right: spacing.lg,
    zIndex: 9999,
    gap: spacing.sm,
  },
  toast: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: spacing.lg,
    paddingVertical: spacing.md,
    borderRadius: radius.md,
    borderWidth: 1,
    gap: spacing.sm,
    shadowColor: '#000',
    shadowOffset: { width: 0, height: 4 },
    shadowOpacity: 0.3,
    shadowRadius: 8,
    elevation: 5,
  },
  toastText: {
    flex: 1,
    fontSize: fontSize.sm,
    fontWeight: '500',
    lineHeight: 18,
  },
});
