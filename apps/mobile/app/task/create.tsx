import React, { useState, useCallback } from "react";
import {
  View,
  Text,
  Pressable,
  ScrollView,
  StyleSheet,
} from "react-native";
import { router, Stack } from "expo-router";
import { Ionicons } from "@expo/vector-icons";
import Animated, { FadeInDown } from "react-native-reanimated";
import { useCreateTask, useModes, useProjects } from "@/lib/query";
import { Input } from "@/components/ui/Input";
import { Button } from "@/components/ui/Button";
import { useToast } from "@/components/ui/Toast";
import { lightImpact, successNotification, selectionFeedback } from "@/lib/haptics";
import { colors, spacing, radius, common } from "@/lib/theme";

export default function CreateTaskScreen() {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [selectedMode, setSelectedMode] = useState<string | undefined>();
  const [selectedProject, setSelectedProject] = useState<number | undefined>();
  const [titleError, setTitleError] = useState("");
  const createMutation = useCreateTask();
  const { data: modes } = useModes();
  const { data: projects } = useProjects();
  const toast = useToast();

  const handleCreate = useCallback(async () => {
    if (!title.trim()) {
      setTitleError("Title is required");
      return;
    }
    setTitleError("");
    lightImpact();
    try {
      const task = await createMutation.mutateAsync({
        title: title.trim(),
        description: description.trim(),
        mode: selectedMode,
        project_id: selectedProject,
      });
      successNotification();
      toast.show("Task created successfully", "success");
      router.replace(`/task/${task.id}` as any);
    } catch (err: any) {
      toast.show(err?.message || "Failed to create task", "error");
    }
  }, [title, description, selectedMode, selectedProject, createMutation, toast]);

  return (
    <>
      <Stack.Screen options={{ headerTitle: "New Task" }} />
      <ScrollView
        style={common.screen}
        contentContainerStyle={styles.content}
        keyboardShouldPersistTaps="handled"
        showsVerticalScrollIndicator={false}
      >
        <Animated.View entering={FadeInDown.duration(300).delay(0)}>
          <Input
            label="Title"
            placeholder="What needs to be done?"
            value={title}
            onChangeText={(t) => {
              setTitle(t);
              if (titleError) setTitleError("");
            }}
            error={titleError}
            autoFocus
            returnKeyType="next"
          />
        </Animated.View>

        <Animated.View entering={FadeInDown.duration(300).delay(50)}>
          <Input
            label="Description"
            placeholder="Detailed description, acceptance criteria..."
            value={description}
            onChangeText={setDescription}
            multiline
            numberOfLines={4}
          />
        </Animated.View>

        {projects && projects.length > 0 && (
          <Animated.View entering={FadeInDown.duration(300).delay(100)}>
            <View style={styles.field}>
              <Text style={styles.label}>Project</Text>
              <ScrollView
                horizontal
                showsHorizontalScrollIndicator={false}
                contentContainerStyle={styles.optionRow}
              >
                <Pressable
                  style={[styles.option, !selectedProject && styles.optionActive]}
                  onPress={() => { setSelectedProject(undefined); selectionFeedback(); }}
                >
                  <Text
                    style={[styles.optionText, !selectedProject && styles.optionTextActive]}
                  >
                    None
                  </Text>
                </Pressable>
                {projects.map((p) => (
                  <Pressable
                    key={p.id}
                    style={[styles.option, selectedProject === p.id && styles.optionActive]}
                    onPress={() => { setSelectedProject(p.id); selectionFeedback(); }}
                  >
                    <Text
                      style={[
                        styles.optionText,
                        selectedProject === p.id && styles.optionTextActive,
                      ]}
                    >
                      {p.name}
                    </Text>
                  </Pressable>
                ))}
              </ScrollView>
            </View>
          </Animated.View>
        )}

        {modes && modes.length > 0 && (
          <Animated.View entering={FadeInDown.duration(300).delay(150)}>
            <View style={styles.field}>
              <Text style={styles.label}>Mode</Text>
              <ScrollView
                horizontal
                showsHorizontalScrollIndicator={false}
                contentContainerStyle={styles.optionRow}
              >
                {modes.map((m) => (
                  <Pressable
                    key={m.name}
                    style={[styles.option, selectedMode === m.name && styles.optionActive]}
                    onPress={() => {
                      setSelectedMode(selectedMode === m.name ? undefined : m.name);
                      selectionFeedback();
                    }}
                  >
                    <Text
                      style={[
                        styles.optionText,
                        selectedMode === m.name && styles.optionTextActive,
                      ]}
                    >
                      {m.label}
                    </Text>
                  </Pressable>
                ))}
              </ScrollView>
            </View>
          </Animated.View>
        )}

        <Animated.View entering={FadeInDown.duration(300).delay(200)}>
          <Button
            title="Create Task"
            onPress={handleCreate}
            loading={createMutation.isPending}
            icon={<Ionicons name="rocket" size={18} color={colors.textInverse} />}
            style={styles.createButton}
          />
        </Animated.View>
      </ScrollView>
    </>
  );
}

const styles = StyleSheet.create({
  content: {
    padding: spacing.lg,
    gap: spacing.xl,
    paddingBottom: 100,
  },
  field: {
    gap: spacing.sm,
  },
  label: {
    fontSize: 13,
    fontWeight: "600",
    color: colors.textSecondary,
    textTransform: "uppercase",
    letterSpacing: 0.5,
  },
  optionRow: {
    gap: spacing.sm,
  },
  option: {
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
    borderRadius: radius.md,
    borderWidth: 1,
    borderColor: colors.border,
    backgroundColor: colors.bgElevated,
  },
  optionActive: {
    borderColor: colors.accent,
    backgroundColor: colors.accentBg,
  },
  optionText: {
    fontSize: 13,
    color: colors.textSecondary,
    fontWeight: "500",
  },
  optionTextActive: {
    color: colors.accent,
  },
  createButton: {
    marginTop: spacing.md,
    shadowColor: colors.accent,
    shadowOffset: { width: 0, height: 4 },
    shadowOpacity: 0.25,
    shadowRadius: 8,
    elevation: 3,
  },
});
