const express = require('express');
const router = express.Router();

router.post('/verify', (req, res) => {
  const { metrics } = req.body;
  
  if (!metrics) {
    return res.status(400).json({ 
      valid: false, 
      message: 'Device metrics required' 
    });
  }
  
  const userAgent = req.headers['user-agent'] || '';
  const clientHintMobile = req.headers['sec-ch-ua-mobile'];
  const clientHintPlatform = req.headers['sec-ch-ua-platform'] || '';
  
  const inconsistencies = [];
  const serverSideChecks = [];
  
  const uaClaimsMobile = /Mobi|Android|iPhone|iPad|iPod/i.test(userAgent);
  const isChromium = /Chrome|Chromium|Edg|Opera|Brave/i.test(userAgent);
  
  // Chromium browsers send Client Hints
  if (isChromium && clientHintMobile !== undefined) {
    const chSaysMobile = clientHintMobile === '?1';
    
    if (uaClaimsMobile && !chSaysMobile) {
      inconsistencies.push('Server: UA claims mobile but Sec-CH-UA-Mobile disagrees');
    }
    
    const isDesktopPlatform = /Windows|macOS|Linux|Chrome OS/i.test(clientHintPlatform);
    if (isDesktopPlatform && uaClaimsMobile) {
      inconsistencies.push('Server: Desktop platform header with mobile UA');
    }
  }
  
  // Safari and other browsers: check platform consistency
  if (!isChromium && clientHintPlatform) {
    const isDesktopPlatform = /Windows|macOS|Linux|Chrome OS/i.test(clientHintPlatform);
    if (isDesktopPlatform && uaClaimsMobile) {
      inconsistencies.push('Server: Desktop platform header with mobile UA');
    }
  }
  
  if (metrics.isEmulation) {
    serverSideChecks.push('Client-side emulation patterns detected');
  }
  
  if (metrics.inconsistencies && metrics.inconsistencies.length > 0) {
    serverSideChecks.push(...metrics.inconsistencies.map(i => `Client: ${i}`));
  }
  
  if (metrics.webglRenderer) {
    const desktopGPUPatterns = /NVIDIA|AMD|Radeon|GeForce|Intel.*Graphics|RTX|GTX|Arc/i;
    if (desktopGPUPatterns.test(metrics.webglRenderer) && uaClaimsMobile) {
      inconsistencies.push('Server: Desktop GPU detected with mobile UA');
    }
  }
  
  if (metrics.hardwareConcurrency && metrics.hardwareConcurrency >= 16 && uaClaimsMobile) {
    inconsistencies.push('Server: Very high CPU cores with mobile UA (desktop pattern)');
  }
  
  if (metrics.deviceMemory && metrics.deviceMemory >= 16 && uaClaimsMobile) {
    inconsistencies.push('Server: Very high memory with mobile UA (desktop pattern)');
  }
  
  const allIssues = [...inconsistencies, ...serverSideChecks];
  const isValid = allIssues.length === 0;
  
  res.json({
    valid: isValid,
    isEmulation: !isValid,
    inconsistencies: allIssues,
    message: isValid ? 'Device verified' : 'Device emulation detected. Please use a real mobile device.',
  });
});

module.exports = router;
