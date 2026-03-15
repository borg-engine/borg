import React, { useState, useCallback } from "react";
import {
  View,
  Text,
  TextInput,
  Pressable,
  ScrollView,
  StyleSheet,
  ActivityIndicator,
  Alert,
} from "react-native";
import { router, Stack } from "expo-router";
import { Ionicons } from "@expo/vector-icons";
import { useCreateTask, useModes, useProjects } from "@/lib/query";
import { colors, spacing, radius, common } from "@/lib/theme";

export default function CreateTaskScreen() {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [selectedMode, setSelectedMode] = useState<string | undefined>();
  const [selectedProject, setSelectedProject] = useState<number | undefined>();
  const createMutation = useCreateTask();
  const { data: modes } = useModes();
  const { data: projects } = useProjects();

  const handleCreate = useCallback(async () => {
    if (!title.trim()) {
      Alert.alert("Error", "Title is required");
      return;
    }
    try {
      const task = await createMutation.mutateAsync({
        title: title.trim(),
        description: description.trim(),
        mode: selectedMode,
        project_id: selectedProject,
      });
      router.replace(`/task/${task.id}` as any);
    } catch (err: any) {
      Alert.alert("Error", err?.message || "Failed to create task");
    }
  }, [title, description, selectedMode, selectedProject, createMutation]);

  return (
    <>
      <Stack.Screen
        options={{
          headerShown: true,
          headerTitle: "New Task",
          headerStyle: { backgroundColor: colors.bg },
          headerTintColor: colors.text,
          headerShadowVisible: false,
        }}
      />
      <ScrollView
        style={common.screen}
        contentContainerStyle={styles.content}
        keyboardShouldPersistTaps="handled"
      >
        <View style={styles.field}>
          <Text style={styles.label}>Title</Text>
          <TextInput
            style={common.input}
            placeholder="What needs to be done?"
            placeholderTextColor={colors.textTertiary}
            value={title}
            onChangeText={setTitle}
            autoFocus
            returnKeyType="next"
          />
        </View>

        <View style={styles.field}>
          <Text style={styles.label}>Description</Text>
          <TextInput
            style={[common.input, styles.multilineInput]}
            placeholder="Detailed description, acceptance criteria..."
            placeholderTextColor={colors.textTertiary}
            value={description}
            onChangeText={setDescription}
            multiline
            numberOfLines={4}
            textAlignVertical="top"
          />
        </View>

        {projects && projects.length > 0 && (
          <View style={styles.field}>
            <Text style={styles.label}>Project</Text>
            <ScrollView
              horizontal
              showsHorizontalScrollIndicator={false}
              contentContainerStyle={styles.optionRow}
            >
              <Pressable
                style={[styles.option, !selectedProject && styles.optionActive]}
                onPress={() => setSelectedProject(undefined)}
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
                  onPress={() => setSelectedProject(p.id)}
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
        )}

        {modes && modes.length > 0 && (
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
                  onPress={() =>
                    setSelectedMode(selectedMode === m.name ? undefined : m.name)
                  }
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
        )}

        <Pressable
          style={[common.buttonPrimary, styles.createButton, createMutation.isPending && styles.disabled]}
          onPress={handleCreate}
          disabled={createMutation.isPending}
        >
          {createMutation.isPending ? (
            <ActivityIndicator size="small" color={colors.textInverse} />
          ) : (
            <>
              <Ionicons name="rocket" size={18} color={colors.textInverse} />
              <Text style={common.buttonPrimaryText}>Create Task</Text>
            </>
          )}
        </Pressable>
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
  multilineInput: {
    minHeight: 100,
    paddingTop: spacing.md,
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
    flexDirection: "row",
    gap: spacing.sm,
    paddingVertical: 16,
    marginTop: spacing.md,
  },
  disabled: {
    opacity: 0.6,
  },
});
