import { useFocusEffect, useLocalSearchParams } from 'expo-router';
import * as ScreenCapture from 'expo-screen-capture';
import { Check, Clock, Hourglass, Layers, List as ListIcon, MapPin, PartyPopper, RotateCcw, Search, X } from 'lucide-react-native';
import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { Pressable, StyleSheet, Text, TextInput, View } from 'react-native';
import Toast from 'react-native-toast-message';

import { apiErrorMessage } from '../../../api/client';
import { RosterStudent } from '../../../api/types';
import { Badge } from '../../../components/ui/Badge';
import { Card } from '../../../components/ui/Card';
import { EmptyState } from '../../../components/ui/EmptyState';
import { IconButton } from '../../../components/ui/IconButton';
import { LoadingScreen } from '../../../components/ui/LoadingScreen';
import { ProgressBar } from '../../../components/ui/ProgressBar';
import { RosterListRow } from '../../../components/RosterListRow';
import { Screen } from '../../../components/Screen';
import { SwipeStack } from '../../../components/SwipeStack';
import { useMarkAttendance, useRoster, useUndoAttendance } from '../../../hooks/useRoster';
import { useTheme } from '../../../theme/ThemeProvider';
import { formatDateTime } from '../../../utils/formatTime';

const countdownLabel = (iso: string, now: number) => {
  const diffMin = Math.round((new Date(iso).getTime() - now) / 60000);
  if (diffMin <= 0) return 'Starting now';
  const h = Math.floor(diffMin / 60);
  const m = diffMin % 60;
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
};

type ViewMode = 'stack' | 'list';

export default function SessionDetail() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const sessionId = id ?? '';
  const { colors, radii, font } = useTheme();
  const { data, isLoading, isError } = useRoster(sessionId);
  const markMutation = useMarkAttendance(sessionId);
  const undoMutation = useUndoAttendance(sessionId);
  const [viewMode, setViewMode] = useState<ViewMode>('stack');
  const [searchQuery, setSearchQuery] = useState('');
  const [now, setNow] = useState(() => Date.now());

  // Ticks so a mentor who opens this screen before the session starts sees
  // the countdown live-update and gets the marking UI automatically once the
  // start time passes, with no manual refresh needed.
  useEffect(() => {
    const interval = setInterval(() => setNow(Date.now()), 30000);
    return () => clearInterval(interval);
  }, []);

  useFocusEffect(
    useCallback(() => {
      ScreenCapture.preventScreenCaptureAsync();
      return () => {
        ScreenCapture.allowScreenCaptureAsync();
      };
    }, [])
  );

  React.useEffect(() => {
    if (isError) Toast.show({ type: 'error', text1: 'Failed to load the roster for this session' });
  }, [isError]);

  const unmarked = useMemo(() => data?.students.filter((s) => s.status === 'unmarked') ?? [], [data]);
  const markedList = useMemo(() => data?.students.filter((s) => s.status !== 'unmarked') ?? [], [data]);
  const filteredStudents = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    const all = data?.students ?? [];
    if (!q) return all;
    return all.filter((s) => s.name.toLowerCase().includes(q) || s.rollNumber.toLowerCase().includes(q));
  }, [data, searchQuery]);

  const handleMark = (student: RosterStudent, status: 'present' | 'absent') => {
    markMutation.mutate(
      { student, status },
      {
        onSuccess: () => {
          Toast.show({ type: 'success', text1: `${student.name} marked ${status === 'present' ? 'Present' : 'Absent'}` });
        },
        onError: (error) => {
          Toast.show({ type: 'error', text1: apiErrorMessage(error, `Failed to mark ${student.name}`) });
        },
      }
    );
  };

  const handleUndo = (student: RosterStudent) => {
    undoMutation.mutate(student, {
      onSuccess: () => Toast.show({ type: 'info', text1: `Undid mark for ${student.name}` }),
      onError: () => Toast.show({ type: 'error', text1: 'Failed to undo — please try again' }),
    });
  };

  if (isLoading) return <LoadingScreen fullScreen={false} />;
  if (!data) return null;

  const { session, summary } = data;
  const top = unmarked[0];
  const progressPct = summary.total === 0 ? 0 : Math.round((summary.marked / summary.total) * 100);
  const pending = markMutation.isPending;
  // A session with no startsAt (a normal self-check-in session, marked
  // manually only as a fallback) has always been markable — the gate only
  // applies to sessions explicitly scheduled for the future. Marking opens
  // manualMarkEarlyWindowMinutes (super-admin configurable, Settings page)
  // before the scheduled start so a mentor can be ready right as the exam
  // begins. Comes from the roster response rather than a hardcoded value so
  // it stays correct without an app release when the setting changes.
  const hasStarted =
    !session.startsAt ||
    new Date(session.startsAt).getTime() - session.manualMarkEarlyWindowMinutes * 60000 <= now;
  const hasEnded = new Date(session.expiresAt).getTime() <= now;

  return (
    <Screen mode="back">
      <Text style={[styles.title, { color: colors.text, fontFamily: font.extrabold }]}>{session.collegeName || 'Session'}</Text>
      <View style={styles.metaRow}>
        {session.batchName ? (
          <View style={styles.metaItem}>
            <MapPin size={13} color={colors.muted} />
            <Text style={{ color: colors.muted, fontSize: 13 }}>{session.batchName}</Text>
          </View>
        ) : null}
        {session.startsAt ? (
          <View style={styles.metaItem}>
            <Clock size={13} color={colors.muted} />
            <Text style={{ color: colors.muted, fontSize: 13 }}>{formatDateTime(session.startsAt)}</Text>
          </View>
        ) : null}
      </View>

      <Card style={{ padding: 14 }}>
        <View style={styles.progressHeader}>
          <Text style={{ color: colors.text, fontSize: 13, fontWeight: '700' }}>
            {summary.marked}/{summary.total} marked
          </Text>
          <Text style={{ color: colors.muted, fontSize: 13 }}>
            <Text style={{ color: colors.successTxt }}>{summary.present} present</Text>
            {'  ·  '}
            <Text style={{ color: colors.dangerTxt }}>{summary.absent} absent</Text>
          </Text>
        </View>
        <ProgressBar percent={progressPct} />
      </Card>

      {!hasStarted ? (
        <Card style={{ marginTop: 20, paddingVertical: 40, paddingHorizontal: 24 }}>
          <View style={styles.startsSoonContent}>
            <View style={[styles.startsSoonIcon, { backgroundColor: colors.warningBg }]}>
              <Hourglass size={26} color={colors.warningTxt} />
            </View>
            <Text style={[styles.startsSoonTitle, { color: colors.text, fontFamily: font.bold }]}>
              This session hasn&apos;t started yet
            </Text>
            <View style={[styles.startsSoonPill, { backgroundColor: colors.warningBg, borderRadius: radii.pill }]}>
              <Text style={{ color: colors.warningTxt, fontSize: 15, fontFamily: font.extrabold }}>{countdownLabel(session.startsAt!, now)}</Text>
            </View>
            <Text style={[styles.startsSoonHint, { color: colors.muted }]}>
              Attendance marking opens automatically once it begins.
            </Text>
          </View>
        </Card>
      ) : hasEnded ? (
        <Card style={{ marginTop: 20, paddingVertical: 40, paddingHorizontal: 24 }}>
          <View style={styles.startsSoonContent}>
            <View style={[styles.startsSoonIcon, { backgroundColor: colors.dangerBg }]}>
              <Hourglass size={26} color={colors.dangerTxt} />
            </View>
            <Text style={[styles.startsSoonTitle, { color: colors.text, fontFamily: font.bold }]}>
              This session has ended
            </Text>
            <Text style={[styles.startsSoonHint, { color: colors.muted }]}>
              Attendance marking closed when the scheduled session time ended.
            </Text>
          </View>
        </Card>
      ) : summary.total === 0 ? (
        <EmptyState title="This session has no roster attached." />
      ) : (
        <>
          <View style={[styles.toggle, { backgroundColor: colors.bgSubtle, borderRadius: radii.md }]}>
            <ToggleButton label="Stack" icon={<Layers size={14} color={viewMode === 'stack' ? colors.primary : colors.muted} />} active={viewMode === 'stack'} onPress={() => setViewMode('stack')} />
            <ToggleButton label="List" icon={<ListIcon size={14} color={viewMode === 'list' ? colors.primary : colors.muted} />} active={viewMode === 'list'} onPress={() => setViewMode('list')} />
          </View>

          {viewMode === 'stack' ? (
            <>
              {!top ? (
                <EmptyState icon={<PartyPopper size={32} color={colors.success} />} title="All students marked!" />
              ) : (
                <>
                  <SwipeStack students={unmarked} onMark={handleMark} />

                  <View style={styles.swipeHint}>
                    <Text style={{ color: colors.successTxt, fontSize: 12, fontWeight: '700' }}>← Swipe left: Present</Text>
                    <Text style={{ color: colors.dangerTxt, fontSize: 12, fontWeight: '700' }}>Swipe right: Absent →</Text>
                  </View>

                  <View style={styles.actionRow}>
                    <ActionButton label="Present" tone="success" icon={<Check size={20} color={colors.successTxt} />} onPress={() => handleMark(top, 'present')} disabled={pending} />
                    <ActionButton label="Absent" tone="danger" icon={<X size={20} color={colors.dangerTxt} />} onPress={() => handleMark(top, 'absent')} disabled={pending} />
                  </View>
                </>
              )}

              {markedList.length > 0 && (
                <>
                  <Text style={[styles.sectionHeading, { color: colors.muted }]}>MARKED ({markedList.length})</Text>
                  <Card padded={false}>
                    {markedList.map((s, idx) => (
                      <View
                        key={s.rollNumber}
                        style={[styles.markedRow, idx < markedList.length - 1 && { borderBottomWidth: 1, borderBottomColor: colors.border }]}
                      >
                        <View style={{ flex: 1 }}>
                          <Text style={{ color: colors.text, fontWeight: '600' }}>{s.name}</Text>
                          <Text style={{ color: colors.muted, fontSize: 12, marginTop: 2 }}>
                            {s.rollNumber}
                            {s.source === 'self_submitted' ? ' · Self-submitted' : ''}
                          </Text>
                        </View>
                        <View style={styles.markedRowRight}>
                          <Badge label={s.status} tone={s.status === 'present' ? 'success' : 'danger'} />
                          {s.source === 'manual' ? (
                            <IconButton accessibilityLabel="Undo mark" onPress={() => handleUndo(s)} size={30}>
                              <RotateCcw size={13} color={colors.muted} />
                            </IconButton>
                          ) : null}
                        </View>
                      </View>
                    ))}
                  </Card>
                </>
              )}
            </>
          ) : (
            <>
              <View style={[styles.searchWrap, { borderColor: colors.border, backgroundColor: colors.bgSubtle, borderRadius: radii.sm }]}>
                <Search size={15} color={colors.faint} />
                <TextInput
                  style={[styles.searchInput, { color: colors.text }]}
                  placeholder="Search by name or roll number…"
                  placeholderTextColor={colors.faint}
                  value={searchQuery}
                  onChangeText={setSearchQuery}
                />
              </View>

              {filteredStudents.length === 0 ? (
                <EmptyState title="No students match your search." />
              ) : (
                <View style={{ marginTop: 4 }}>
                  {filteredStudents.map((s) => (
                    <RosterListRow key={s.rollNumber} student={s} disabled={pending} onMark={handleMark} onUndo={handleUndo} />
                  ))}
                </View>
              )}

              <Text style={[styles.hint, { color: colors.muted }]}>Pull the green arrow for present, the red arrow for absent — any student, any time.</Text>
            </>
          )}
        </>
      )}
    </Screen>
  );
}

const ToggleButton = ({ label, icon, active, onPress }: { label: string; icon: React.ReactNode; active: boolean; onPress: () => void }) => {
  const { colors, radii, shadows } = useTheme();
  return (
    <Pressable
      onPress={onPress}
      style={[
        styles.toggleBtn,
        { borderRadius: radii.md - 4 },
        active && { backgroundColor: colors.surface, ...shadows.card },
      ]}
    >
      {icon}
      <Text style={{ color: active ? colors.primary : colors.muted, fontSize: 13, fontWeight: '700' }}>{label}</Text>
    </Pressable>
  );
};

const ActionButton = ({
  label,
  tone,
  icon,
  onPress,
  disabled,
}: {
  label: string;
  tone: 'success' | 'danger';
  icon: React.ReactNode;
  onPress: () => void;
  disabled: boolean;
}) => {
  const { colors, radii } = useTheme();
  const textColor = tone === 'success' ? colors.successTxt : colors.dangerTxt;
  return (
    <Pressable
      onPress={onPress}
      disabled={disabled}
      style={[styles.actionBtn, { borderColor: colors.border, backgroundColor: colors.surface, borderRadius: radii.md, opacity: disabled ? 0.55 : 1 }]}
    >
      {icon}
      <Text style={{ color: textColor, fontWeight: '700', fontSize: 13 }}>{label}</Text>
    </Pressable>
  );
};

const styles = StyleSheet.create({
  title: { fontSize: 21, marginBottom: 4 },
  metaRow: { flexDirection: 'row', flexWrap: 'wrap', gap: 16, marginBottom: 16 },
  metaItem: { flexDirection: 'row', alignItems: 'center', gap: 5 },
  progressHeader: { flexDirection: 'row', justifyContent: 'space-between', marginBottom: 8 },
  startsSoonContent: { alignItems: 'center' },
  startsSoonIcon: { width: 56, height: 56, borderRadius: 28, alignItems: 'center', justifyContent: 'center', marginBottom: 14 },
  startsSoonTitle: { fontSize: 16, marginBottom: 4, textAlign: 'center' },
  startsSoonPill: { paddingVertical: 6, paddingHorizontal: 16, marginTop: 4 },
  startsSoonHint: { fontSize: 12.5, textAlign: 'center', marginTop: 10 },
  toggle: { flexDirection: 'row', gap: 6, marginVertical: 16, padding: 4 },
  toggleBtn: { flex: 1, flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: 6, paddingVertical: 9 },
  swipeHint: { flexDirection: 'row', justifyContent: 'space-between', marginTop: 10, paddingHorizontal: 4 },
  actionRow: { flexDirection: 'row', gap: 12, marginTop: 16 },
  actionBtn: { flex: 1, alignItems: 'center', gap: 6, paddingVertical: 14, borderWidth: 1.5 },
  sectionHeading: { fontSize: 13, fontWeight: '700', textTransform: 'uppercase', letterSpacing: 0.4, marginTop: 24, marginBottom: 10 },
  markedRow: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', padding: 14 },
  markedRowRight: { flexDirection: 'row', alignItems: 'center', gap: 10 },
  searchWrap: { flexDirection: 'row', alignItems: 'center', gap: 8, marginTop: 14, marginBottom: 12, paddingHorizontal: 13, borderWidth: 1.5 },
  searchInput: { flex: 1, paddingVertical: 11, fontSize: 14 },
  hint: { fontSize: 12, textAlign: 'center', marginTop: 12 },
});
