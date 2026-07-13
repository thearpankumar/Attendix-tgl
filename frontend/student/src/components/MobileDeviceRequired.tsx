import React from 'react';

interface MobileDeviceRequiredProps {
  isEmulation?: boolean;
  inconsistencies?: string[];
}

const MobileDeviceRequired: React.FC<MobileDeviceRequiredProps> = ({ 
  isEmulation = false, 
  inconsistencies = [] 
}) => {
  return (
    <div className="min-h-screen bg-gradient-to-br from-red-50 via-white to-orange-50 flex flex-col items-center justify-center p-4 sm:p-6 text-center overflow-hidden">
      <div className="bg-white rounded-2xl shadow-xl shadow-red-100/50 p-8 sm:p-10 w-full max-w-md mx-auto border border-red-100/50 flex flex-col items-center">

        <div className="relative mb-6">
          <div className="absolute inset-0 bg-gradient-to-br from-red-400 to-red-600 rounded-2xl blur-lg opacity-30 animate-pulse"></div>
          <div className={`relative p-4 sm:p-5 rounded-xl shadow-lg flex items-center justify-center text-white ${isEmulation ? 'bg-gradient-to-br from-orange-500 to-red-600' : 'bg-gradient-to-br from-red-500 to-red-600'}`}>
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="40"
              height="40"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <rect width="14" height="20" x="5" y="2" rx="2" ry="2" />
              <path d="M12 18h.01" />
            </svg>

            <div className="absolute -top-2 -right-2 bg-white p-1.5 rounded-full shadow-md border border-red-100">
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke={isEmulation ? "#EA580C" : "#DC2626"} strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M18 6 6 18" />
                <path d="m6 6 12 12" />
              </svg>
            </div>
          </div>
        </div>

        <h1 className="text-xl sm:text-2xl font-bold text-gray-900 mb-2">
          {isEmulation ? (
            <>
              Device Emulation <span className="text-orange-600">Detected</span>
            </>
          ) : (
            <>
              Mobile Access <span className="text-red-600">Required</span>
            </>
          )}
        </h1>

        {isEmulation ? (
          <>
            <p className="text-sm text-gray-600 mb-4 leading-relaxed max-w-xs">
              Your browser appears to be emulating a mobile device. Attendance can only be marked from a real smartphone or tablet.
            </p>
            
            <div className="bg-orange-50 border border-orange-200 rounded-lg p-4 mb-4 w-full text-left">
              <p className="text-xs font-medium text-orange-800 mb-2">
                Common emulation methods detected:
              </p>
              <ul className="text-xs text-orange-700 space-y-1">
                <li>• Chrome DevTools device mode</li>
                <li>• Browser extension user-agent spoofing</li>
                <li>• Desktop browser with mobile emulation</li>
              </ul>
            </div>
            
            {inconsistencies.length > 0 && (
              <div className="bg-gray-50 border border-gray-200 rounded-lg p-4 mb-4 w-full text-left">
                <p className="text-xs font-medium text-gray-700 mb-2">Technical details:</p>
                <ul className="text-xs text-gray-600 space-y-1">
                  {inconsistencies.slice(0, 5).map((issue, idx) => (
                    <li key={idx}>• {issue}</li>
                  ))}
                </ul>
              </div>
            )}
            
            <p className="text-xs text-gray-500 leading-relaxed max-w-xs">
              Please open this link directly on your smartphone to mark attendance.
            </p>
          </>
        ) : (
          <p className="text-sm text-gray-500 mb-8 leading-relaxed max-w-xs">
            For security and GPS verification, attendance can only be marked using a smartphone.
          </p>
        )}

      </div>

      <div className="fixed top-0 left-0 w-full h-full pointer-events-none overflow-hidden -z-10">
        <div className="absolute top-1/4 -left-20 w-72 h-72 bg-red-200 rounded-full mix-blend-multiply filter blur-3xl opacity-30"></div>
        <div className="absolute bottom-1/4 -right-20 w-72 h-72 bg-orange-200 rounded-full mix-blend-multiply filter blur-3xl opacity-30"></div>
      </div>
    </div>
  );
};

export default MobileDeviceRequired;
