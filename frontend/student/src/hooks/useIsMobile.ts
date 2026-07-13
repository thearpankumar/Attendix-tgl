import { useState, useEffect } from 'react';
import { useDeviceVerification } from './useDeviceVerification';

export function useIsMobile() {
  const { isValid, isEmulation, checking } = useDeviceVerification();
  const [isMobile, setIsMobile] = useState(true);

  useEffect(() => {
    if (checking) return;
    
    setIsMobile(isValid && !isEmulation);
  }, [checking, isValid, isEmulation]);

  return isMobile;
}

export function useMobileVerification() {
  const { isValid, isEmulation, inconsistencies, checking, metrics, recheck } = useDeviceVerification();
  
  return {
    isMobile: !checking && isValid && !isEmulation,
    isEmulation,
    inconsistencies,
    checking,
    metrics,
    recheck,
  };
}
