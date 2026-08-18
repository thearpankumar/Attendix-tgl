import { Stack } from 'expo-router';
import React from 'react';

import { UpdatePrompt } from '../../components/UpdatePrompt';

export default function AppLayout() {
  return (
    <>
      <Stack screenOptions={{ headerShown: false }} />
      <UpdatePrompt />
    </>
  );
}
