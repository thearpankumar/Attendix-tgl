// NOTE: this hook is not imported by any page — it was written and never wired
// up. It is kept rather than deleted because the multi-sample baseline and
// mock-pattern heuristics below are real detection logic with their own test
// suite, and removing working anti-spoofing code is the wrong direction.
//
// It is a *client-side* signal and can only ever add suspicion: the server
// re-derives GPS anomalies itself in
// backend-rust/src/middleware/security_extractors.rs, which is the check that
// actually gates attendance. Wiring this up would enrich the telemetry sent
// with a submission, not replace the server-side verdict.

import { useState, useCallback } from 'react';

export interface GPSMetadata {
  accuracy: number;
  altitude: number | null;
  altitudeAccuracy: number | null;
  speed: number | null;
  heading: number | null;
  timestamp: number;
  isMockLocation?: boolean;
  provider: string;
}

export interface PositionSample {
  latitude: number;
  longitude: number;
  metadata: GPSMetadata;
  collectedAt: number;
}

export interface GPSValidationResult {
  isValid: boolean;
  metadata: GPSMetadata | null;
  positionHistory: PositionSample[];
  baselineCollected: boolean;
  warnings: string[];
}

const detectGPSProvider = (position: GeolocationPosition): string => {
  const accuracy = position.coords.accuracy;
  
  if (accuracy < 10) {
    return 'gps';
  } else if (accuracy < 100) {
    return 'fused';
  } else if (accuracy < 1000) {
    return 'network';
  }
  
  return 'unknown';
};

export function useGPSValidation() {
  const [positionHistory, setPositionHistory] = useState<PositionSample[]>([]);
  const [baselineCollected, setBaselineCollected] = useState(false);
  const [collecting, setCollecting] = useState(false);
  const [warnings, setWarnings] = useState<string[]>([]);

  const collectPosition = useCallback((): Promise<PositionSample> => {
    return new Promise((resolve, reject) => {
      if (!navigator.geolocation) {
        reject(new Error('Geolocation not supported'));
        return;
      }

      navigator.geolocation.getCurrentPosition(
        (position) => {
          const metadata: GPSMetadata = {
            accuracy: position.coords.accuracy,
            altitude: position.coords.altitude,
            altitudeAccuracy: position.coords.altitudeAccuracy,
            speed: position.coords.speed,
            heading: position.coords.heading,
            timestamp: position.timestamp,
            isMockLocation: (position as unknown as { isMockLocation?: boolean }).isMockLocation || false,
            provider: detectGPSProvider(position),
          };

          const sample: PositionSample = {
            latitude: position.coords.latitude,
            longitude: position.coords.longitude,
            metadata,
            collectedAt: Date.now(),
          };

          resolve(sample);
        },
        (error) => {
          reject(error);
        },
        {
          enableHighAccuracy: true,
          timeout: 15000,
          maximumAge: 0,
        }
      );
    });
  }, []);

  const collectBaseline = useCallback(async (): Promise<PositionSample[]> => {
    setCollecting(true);
    const samples: PositionSample[] = [];

    try {
      const sample1 = await collectPosition();
      samples.push(sample1);

      await new Promise((r) => setTimeout(r, 2000));

      const sample2 = await collectPosition();
      samples.push(sample2);

      await new Promise((r) => setTimeout(r, 2000));

      const sample3 = await collectPosition();
      samples.push(sample3);

      setPositionHistory(samples);
      setBaselineCollected(true);

      const analysis = analyzePositions(samples);
      if (analysis.warnings.length > 0) {
        setWarnings(analysis.warnings);
      }

      return samples;
    } catch {
      setCollecting(false);
      throw new Error('Failed to collect GPS baseline');
    } finally {
      setCollecting(false);
    }
  }, [collectPosition]);

  const addPosition = useCallback((sample: PositionSample) => {
    setPositionHistory((prev) => {
      const updated = [...prev, sample];
      if (updated.length > 10) {
        return updated.slice(-10);
      }
      return updated;
    });
  }, []);

  const getLatestMetadata = useCallback((): GPSMetadata | null => {
    if (positionHistory.length === 0) return null;
    return positionHistory[positionHistory.length - 1].metadata;
  }, [positionHistory]);

  const clearHistory = useCallback(() => {
    setPositionHistory([]);
    setBaselineCollected(false);
    setWarnings([]);
  }, []);

  return {
    positionHistory,
    baselineCollected,
    collecting,
    warnings,
    collectBaseline,
    collectPosition,
    addPosition,
    getLatestMetadata,
    clearHistory,
  };
}

function analyzePositions(positions: PositionSample[]): { warnings: string[] } {
  const warnings: string[] = [];

  if (positions.length < 3) {
    return { warnings };
  }

  const accuracies = positions.map((p) => p.metadata.accuracy).filter((a) => a != null);
  if (accuracies.length >= 3) {
    const allSame = accuracies.every((a) => a === accuracies[0]);
    if (allSame && accuracies[0] < 10) {
      warnings.push(`Consistent accuracy of ${accuracies[0]}m (mock GPS pattern)`);
    }
  }

  const altitudes = positions.map((p) => p.metadata.altitude).filter((a) => a != null);
  if (altitudes.length > 0) {
    const allZeroOrNull = altitudes.every((a) => a === 0);
    if (allZeroOrNull) {
      warnings.push('All altitude readings are zero (mock GPS pattern)');
    }
  }

  const timestamps = positions.map((p) => p.metadata.timestamp);
  for (let i = 1; i < timestamps.length; i++) {
    const diff = timestamps[i] - timestamps[i - 1];
    if (diff < 100) {
      warnings.push('Timestamps too close together (suspicious timing)');
      break;
    }
  }

  const mockFlags = positions.filter((p) => p.metadata.isMockLocation === true);
  if (mockFlags.length > 0) {
    warnings.push('Mock location detected by browser');
  }

  return { warnings };
}

export default useGPSValidation;
