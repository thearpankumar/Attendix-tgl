import { useFocusEffect, useRouter } from 'expo-router';
import * as ScreenCapture from 'expo-screen-capture';
import { Activity, CalendarClock, ClipboardX, Clock, GraduationCap, MapPin, PieChart, Users } from 'lucide-react-native';
import React, { useCallback, useMemo } from 'react';
import { Pressable, StyleSheet, Text, View } from 'react-native';
import Toast from 'react-native-toast-message';

import { Badge } from '../../components/ui/Badge';
import { Card } from '../../components/ui/Card';
import { EmptyState } from '../../components/ui/EmptyState';
import { LoadingScreen } from '../../components/ui/LoadingScreen';
import { Panel } from '../../components/ui/Panel';
import { StatCard } from '../../components/ui/StatCard';
import { DonutChart } from '../../components/DonutChart';
import { Screen } from '../../components/Screen';
import { Timeline, TimelineEntry } from '../../components/Timeline';
import { useAuth } from '../../context/AuthContext';
import { sessionState, useDashboard } from '../../hooks/useDashboard';
import { useTheme } from '../../theme/ThemeProvider';
import { Session } from '../../api/types';
import { formatClock, formatSlot, timeUntil } from '../../utils/formatTime';
import { greetingWord } from '../../utils/greeting';

export default function Dashboard() {
  const { colors, font } = useTheme();
  const { admin } = useAuth();
  const router = useRouter();
  const { data, isLoading, isError, isFetching, refetch } = useDashboard();

  useFocusEffect(
    useCallback(() => {
      ScreenCapture.preventScreenCaptureAsync();
      return () => {
        ScreenCapture.allowScreenCaptureAsync();
      };
    }, [])
  );

  React.useEffect(() => {
    if (isError) Toast.show({ type: 'error', text1: 'Failed to load your sessions' });
  }, [isError]);

  const displayName = admin?.fullName || admin?.username || '';

  const { sessions, rosterMap, openSessions, liveSessions, upcomingSessions, closedSessions, scheduled, rosterTotals } = useMemo(() => {
    const sessions = data?.sessions ?? [];
    const rosterMap = data?.rosterMap ?? {};
    const open = sessions.filter((s) => sessionState(s) !== 'closed');
    const live = open.filter((s) => sessionState(s) === 'live');
    const upcoming = open.filter((s) => sessionState(s) === 'upcoming');
    const closed = sessions.filter((s) => sessionState(s) === 'closed');
    const scheduledList = open
      .filter((s) => s.startsAt)
      .sort((a, b) => new Date(a.startsAt!).getTime() - new Date(b.startsAt!).getTime());
    const totals = open.reduce(
      (acc, s) => {
        const r = rosterMap[s._id];
        if (r) {
          acc.total += r.total;
          acc.present += r.present;
          acc.marked += r.marked;
        }
        return acc;
      },
      { total: 0, present: 0, marked: 0 }
    );
    return { sessions, rosterMap, openSessions: open, liveSessions: live, upcomingSessions: upcoming, closedSessions: closed, scheduled: scheduledList, rosterTotals: totals };
  }, [data]);

  const attendancePct = rosterTotals.marked === 0 ? null : Math.round((rosterTotals.present / rosterTotals.marked) * 100);

  const timelineEntries: TimelineEntry[] = scheduled.map((s) => {
    const state = sessionState(s);
    return {
      key: s._id,
      time: formatClock(s.startsAt!),
      title: s.batchName || s.collegeName || 'Session',
      live: state === 'live',
      upcoming: state === 'upcoming',
    };
  });

  if (isLoading) return <LoadingScreen fullScreen={false} />;

  return (
    <Screen mode="brand" refreshing={isFetching} onRefresh={refetch}>
      <View style={{ marginBottom: 4 }}>
        <Text style={[styles.greeting, { color: colors.text, fontFamily: font.extrabold }]}>
          {greetingWord()}
          {displayName ? `, ${displayName}` : ''} 👋
        </Text>
        <Text style={[styles.subGreeting, { color: colors.muted }]}>Here&apos;s what&apos;s happening today</Text>
      </View>

      {sessions.length === 0 ? (
        <EmptyState
          icon={<ClipboardX size={32} color={colors.faint} />}
          title="No sessions assigned to you yet."
          subtitle="Ask your super admin to assign you to a session."
        />
      ) : (
        <>
          <View style={styles.statGrid}>
            <StatCard icon={Activity} label="Active Sessions" value={liveSessions.length} tone="primary" />
            <StatCard icon={CalendarClock} label="Upcoming" value={upcomingSessions.length} tone="warning" />
            <StatCard icon={GraduationCap} label="Students" value={rosterTotals.total} tone="success" />
            <StatCard icon={PieChart} label="Attendance" value={attendancePct === null ? '—' : `${attendancePct}%`} tone="primary" />
          </View>

          {openSessions.length > 0 && (
            <Panel header="Your Sessions Today">
              {openSessions.map((s) => {
                const state = sessionState(s);
                const roster = rosterMap[s._id];
                return (
                  <Pressable
                    key={s._id}
                    style={[styles.sessionRow, { borderBottomColor: colors.border }]}
                    onPress={() => router.push(`/sessions/${s._id}`)}
                  >
                    <View style={{ flex: 1 }}>
                      <Text style={[styles.sessionTitle, { color: colors.text, fontFamily: font.bold }]}>
                        {s.batchName || s.collegeName || 'Session'}
                      </Text>
                      <Text style={[styles.sessionSub, { color: colors.muted }]}>
                        {[s.collegeName, s.locationName].filter(Boolean).join(' · ') || 'No location'}
                      </Text>
                    </View>
                    <View style={styles.sessionMeta}>
                      <Badge label={state === 'live' ? 'LIVE' : s.startsAt ? timeUntil(s.startsAt) : 'Upcoming'} tone={state === 'live' ? 'success' : 'warning'} />
                      <Text style={[styles.sessionCount, { color: colors.muted }]}>
                        {roster ? `${roster.marked}/${roster.total}` : `${s.attendanceCount}`} Marked
                      </Text>
                    </View>
                  </Pressable>
                );
              })}
            </Panel>
          )}

          {timelineEntries.length > 0 && (
            <Panel header="Today's Schedule">
              <Timeline entries={timelineEntries} />
            </Panel>
          )}

          {rosterTotals.marked > 0 && attendancePct !== null && (
            <Panel header="Attendance Overview">
              <View style={styles.donutWrap}>
                <DonutChart percent={attendancePct} />
                <View style={styles.legend}>
                  <LegendRow color={colors.success} label="Present" value={rosterTotals.present} textColor={colors.text} />
                  <LegendRow color={colors.danger} label="Absent" value={rosterTotals.marked - rosterTotals.present} textColor={colors.text} />
                </View>
              </View>
            </Panel>
          )}

          {closedSessions.length > 0 && (
            <>
              <Text style={[styles.sectionHeading, { color: colors.muted }]}>PAST / INACTIVE</Text>
              {closedSessions.map((s) => (
                <ClosedSessionCard key={s._id} session={s} onPress={() => router.push(`/sessions/${s._id}`)} />
              ))}
            </>
          )}
        </>
      )}
    </Screen>
  );
}

const LegendRow = ({ color, label, value, textColor }: { color: string; label: string; value: number; textColor: string }) => (
  <View style={styles.legendRow}>
    <View style={[styles.legendDot, { backgroundColor: color }]} />
    <Text style={{ color: textColor, fontSize: 13 }}>
      {label} <Text style={{ fontWeight: '800' }}>{value}</Text>
    </Text>
  </View>
);

const ClosedSessionCard = ({ session, onPress }: { session: Session; onPress: () => void }) => {
  const { colors, font } = useTheme();
  const slot = formatSlot(session.startsAt);

  return (
    <Card onPress={onPress} style={{ marginBottom: 12, opacity: 0.65 }}>
      <View style={styles.closedHeader}>
        <View style={{ flex: 1 }}>
          <Text style={[styles.closedTitle, { color: colors.text, fontFamily: font.bold }]}>
            {session.collegeName || session.locationName || 'Session'}
          </Text>
          {session.batchName ? <Text style={[styles.closedSub, { color: colors.muted }]}>{session.batchName}</Text> : null}
        </View>
        <Badge label="Closed" tone="neutral" />
      </View>
      <View style={styles.closedMetaRow}>
        {slot ? <MetaItem icon={<Clock size={14} color={colors.muted} />} text={slot} /> : null}
        {session.locationName ? <MetaItem icon={<MapPin size={14} color={colors.muted} />} text={session.locationName} /> : null}
        <MetaItem icon={<Users size={14} color={colors.muted} />} text={`${session.attendanceCount} marked`} />
      </View>
    </Card>
  );
};

const MetaItem = ({ icon, text }: { icon: React.ReactNode; text: string }) => {
  const { colors } = useTheme();
  return (
    <View style={styles.metaItem}>
      {icon}
      <Text style={{ color: colors.muted, fontSize: 13 }}>{text}</Text>
    </View>
  );
};

const styles = StyleSheet.create({
  greeting: { fontSize: 22 },
  subGreeting: { fontSize: 13, marginTop: 4 },
  statGrid: { flexDirection: 'row', flexWrap: 'wrap', gap: 10, marginTop: 16 },
  sessionRow: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', gap: 10, padding: 14, borderBottomWidth: 1 },
  sessionTitle: { fontSize: 14 },
  sessionSub: { fontSize: 12, marginTop: 2 },
  sessionMeta: { alignItems: 'flex-end', gap: 5 },
  sessionCount: { fontSize: 11, fontWeight: '600' },
  donutWrap: { flexDirection: 'row', alignItems: 'center', gap: 20, padding: 16 },
  legend: { gap: 10 },
  legendRow: { flexDirection: 'row', alignItems: 'center', gap: 8 },
  legendDot: { width: 10, height: 10, borderRadius: 5 },
  sectionHeading: { fontSize: 13, fontWeight: '700', textTransform: 'uppercase', letterSpacing: 0.4, marginTop: 24, marginBottom: 10 },
  closedHeader: { flexDirection: 'row', alignItems: 'flex-start', justifyContent: 'space-between', gap: 8 },
  closedTitle: { fontSize: 17 },
  closedSub: { fontSize: 13, marginTop: 2 },
  closedMetaRow: { flexDirection: 'row', flexWrap: 'wrap', gap: 12, marginTop: 14 },
  metaItem: { flexDirection: 'row', alignItems: 'center', gap: 5 },
});
