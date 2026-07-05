const API_BASE = window.location.origin;
const shortCode = window.location.pathname.split('/').pop();

// Helper for fetching from local API with ngrok warning bypass
async function apiFetch(url, options = {}) {
    if (url.startsWith('/') || url.startsWith(window.location.origin)) {
        options.headers = {
            ...options.headers,
            'ngrok-skip-browser-warning': 'true'
        };
    }
    return fetch(url, options);
}

let sessionData = null;
let userLocation = null;
let photoData = null;
let mediaStream = null;
let storageInfo = null;
let faceModelLoaded = false;
let faceDetectedVal = true;

async function loadFaceModel() {
    try {
        console.log('Loading face detection model (SSD MobileNet)...');
        await faceapi.nets.ssdMobilenetv1.loadFromUri('/models');
        faceModelLoaded = true;
        console.log('Face detection model loaded successfully');
    } catch (err) {
        console.warn('Failed to load face detection model:', err);
    }
}
loadFaceModel();


const loadingView = document.getElementById('loadingView');
const errorView = document.getElementById('errorView');
const formView = document.getElementById('formView');
const successView = document.getElementById('successView');
const errorMessage = document.getElementById('errorMessage');

const video = document.getElementById('video');
const canvas = document.getElementById('canvas');
const photoPreview = document.getElementById('photoPreview');
const captureBtn = document.getElementById('captureBtn');
const captchaInput = document.getElementById('captchaInput');
const captchaSvgContainer = document.getElementById('captchaSvgContainer');
const refreshCaptchaBtn = document.getElementById('refreshCaptchaBtn');
const nameInput = document.getElementById('name');
const rollNumberInput = document.getElementById('rollNumber');
const submitBtn = document.getElementById('submitBtn');
const gpsStatus = document.getElementById('gpsStatus');
const locationName = document.getElementById('locationName');
const expiresAt = document.getElementById('expiresAt');

let captchaId = '';

async function loadCaptcha() {
    try {
        captchaSvgContainer.innerHTML = '<div style="font-size: 14px; color: #666;">Loading code...</div>';
        const res = await apiFetch(`${API_BASE}/s/${shortCode}/captcha`);
        if (!res.ok) throw new Error('Failed to fetch captcha');
        const data = await res.json();
        
        // Parse SVG to avoid Safari rendering namespace bugs
        const parser = new DOMParser();
        const doc = parser.parseFromString(data.captchaSvg, 'image/svg+xml');
        const svgElement = doc.documentElement;
        
        captchaSvgContainer.innerHTML = '';
        if (svgElement) {
            svgElement.removeAttribute('width');
            svgElement.removeAttribute('height');
            svgElement.style.height = '40px';
            svgElement.style.width = 'auto';
            svgElement.style.display = 'block';
            captchaSvgContainer.appendChild(svgElement);
        } else {
            captchaSvgContainer.innerHTML = data.captchaSvg;
        }

        captchaId = data.captchaId;
        captchaInput.value = '';
        checkFormValidity();
    } catch (err) {
        console.warn('Failed to load captcha:', err);
        captchaSvgContainer.innerHTML = '<div style="font-size: 12px; color: #d9534f; cursor: pointer;">Failed to load. Click 🔄 to retry.</div>';
    }
}

function showView(view) {
    loadingView.classList.add('hidden');
    errorView.classList.add('hidden');
    formView.classList.add('hidden');
    successView.classList.add('hidden');
    view.classList.remove('hidden');
}

function showError(message) {
    errorMessage.textContent = message;
    showView(errorView);
}

function cleanupCamera() {
    if (mediaStream) {
        mediaStream.getTracks().forEach(track => track.stop());
        mediaStream = null;
    }
}

window.addEventListener('beforeunload', cleanupCamera);

// Nominatim (OpenStreetMap) reverse geocoding — free, no API key
async function fetchPlaceName(lat, lon) {
    try {
        const res = await fetch(
            `https://nominatim.openstreetmap.org/reverse?format=json&lat=${lat}&lon=${lon}&zoom=16&addressdetails=1`,
            { headers: { 'Accept-Language': 'en' } }
        );
        if (!res.ok) return null;
        const data = await res.json();
        // Build a short readable name: building/road + city
        const a = data.address || {};
        const parts = [
            a.building || a.amenity || a.road || a.pedestrian,
            a.suburb || a.neighbourhood,
            a.city || a.town || a.village || a.county
        ].filter(Boolean);
        return parts.length ? parts.join(', ') : data.display_name || null;
    } catch {
        return null;
    }
}

async function init() {
    if (!shortCode || !/^[a-zA-Z0-9_-]+$/.test(shortCode)) {
        showError('Invalid attendance link format');
        return;
    }

    const isLocalhost = location.hostname === 'localhost' || location.hostname === '127.0.0.1' || location.hostname === '[::1]';
    const isSecure = location.protocol === 'https:' || isLocalhost;
    
    if (!isSecure) {
        showError('⚠ HTTPS Required\n\nGeolocation and Camera require a secure connection (HTTPS).\n\nIf you\'re accessing this from a phone:\n1. Ask your administrator for the HTTPS link\n2. Or use http://localhost from a computer\n\nCurrent URL: ' + location.href);
        return;
    }

    console.log('[Init] Starting attendance flow...');
    console.log('[Init] Protocol:', location.protocol);
    console.log('[Init] Hostname:', location.hostname);
    console.log('[Init] Is secure context:', window.isSecureContext);

    try {
        const storageRes = await apiFetch(`${API_BASE}/api/storage-info`);
        storageInfo = await storageRes.json();

        const res = await apiFetch(`${API_BASE}/s/${shortCode}/session`);
        const data = await res.json();

        if (!res.ok || !data.valid) {
            showError(data.message || 'Invalid or expired attendance link');
            return;
        }

        sessionData = data.session;
        locationName.textContent = sessionData.locationName;
        expiresAt.textContent = new Date(sessionData.expiresAt).toLocaleString();

        const expiryTime = new Date(sessionData.expiresAt);
        if (expiryTime < new Date()) {
            showError('This attendance session has expired');
            return;
        }

        showView(formView);
        loadCaptcha();
        
        console.log('[Init] Initializing geolocation...');
        await initGeolocation();
        
        console.log('[Init] Initializing camera...');
        await initCamera();
        
        console.log('[Init] All systems ready');
    } catch (error) {
        console.error('[Init] Error:', error);
        showError('Failed to load attendance page. Please check your internet connection.');
    }
}

async function initGeolocation() {
    gpsStatus.className = 'gps-status gps-loading';
    gpsStatus.innerHTML = '<span>Requesting Location...</span><small>Your browser will ask for location permission. Please click "Allow" when prompted.</small>';

    return new Promise((resolve) => {
        if (!navigator.geolocation) {
            gpsStatus.className = 'gps-status gps-error';
            gpsStatus.innerHTML = '<span>Not Supported</span><small>Geolocation is not supported by this browser. Please use a modern browser like Chrome, Firefox, or Safari.</small>';
            resolve();
            return;
        }

        const options = {
            enableHighAccuracy: true,
            timeout: 60000,
            maximumAge: 0
        };

        console.log('[Geolocation] Requesting position with options:', options);

        navigator.geolocation.getCurrentPosition(
            (position) => {
                console.log('[Geolocation] Position received:', position.coords);
                userLocation = {
                    latitude: position.coords.latitude,
                    longitude: position.coords.longitude,
                    accuracy: position.coords.accuracy
                };
                gpsStatus.className = 'gps-status gps-ok';
                gpsStatus.innerHTML = `
                    <span>✓ Location Detected</span>
                    <small>Accuracy: ${Math.round(position.coords.accuracy)} meters</small>
                    <div class="coords-display">${userLocation.latitude.toFixed(6)}, ${userLocation.longitude.toFixed(6)}</div>
                    <div class="place-name-display" id="placeNameDisplay">Fetching place name...</div>
                `;
                checkFormValidity();
                resolve();

                fetchPlaceName(userLocation.latitude, userLocation.longitude).then(name => {
                    const el = document.getElementById('placeNameDisplay');
                    if (el) el.textContent = name || 'Place name unavailable';
                });
            },
            (error) => {
                console.error('[Geolocation] Error:', error.code, error.message);
                let msg = 'Unknown error occurred.';
                let instructions = 'Please refresh the page and try again.';
                
                switch (error.code) {
                    case error.PERMISSION_DENIED:
                        msg = '⚠ Location Permission Denied';
                        instructions = 'You must allow location access to mark attendance. Instructions:\n\n';
                        instructions += '📱 iPhone/iPad: Settings > Privacy & Security > Location Services > Safari > Allow\n';
                        instructions += '📱 Android: Settings > Apps > Chrome/Safari > Permissions > Location > Allow\n';
                        instructions += 'Or tap the lock/info icon in your browser address bar, enable location, and refresh.';
                        break;
                    case error.POSITION_UNAVAILABLE:
                        msg = '⚠ Location Unavailable';
                        instructions = 'GPS signal could not be obtained. Please:\n';
                        instructions += '1. Enable Location/GPS in device settings\n';
                        instructions += '2. Try outdoors for better GPS signal\n';
                        instructions += '3. Ensure no VPN/proxy is blocking location';
                        break;
                    case error.TIMEOUT:
                        msg = '⚠ Location Timeout';
                        instructions = 'Location request timed out. Please:\n';
                        instructions += '1. Check if GPS/Location is enabled\n';
                        instructions += '2. Move to an open area\n';
                        instructions += '3. Refresh and try again';
                        break;
                    default:
                        msg = '⚠ Location Error';
                        instructions = 'An unknown error occurred. Please refresh and try again.';
                }
                
                gpsStatus.className = 'gps-status gps-error';
                gpsStatus.innerHTML = `
                    <span>${msg}</span>
                    <small style="white-space: pre-line;">${instructions}</small>
                `;
                
                const retryBtn = document.getElementById('retryLocationBtn');
                if (retryBtn) {
                    retryBtn.classList.remove('hidden');
                }
                
                checkFormValidity();
                resolve();
            },
            options
        );
    });
}

async function initCamera() {
    try {
        console.log('[Camera] Requesting camera access...');
        
        const constraints = {
            video: {
                facingMode: 'user',
                width: { ideal: 640 },
                height: { ideal: 480 }
            },
            audio: false
        };
        
        console.log('[Camera] Constraints:', constraints);
        
        mediaStream = await navigator.mediaDevices.getUserMedia(constraints);
        
        console.log('[Camera] Stream obtained:', mediaStream.getTracks());
        
        video.srcObject = mediaStream;
        
        video.onloadedmetadata = () => {
            console.log('[Camera] Video metadata loaded:', video.videoWidth, 'x', video.videoHeight);
            video.play().catch(err => console.warn('[Camera] Autoplay failed:', err));
        };
        
    } catch (error) {
        console.error('[Camera] Error:', error.name, error.message);
        
        if (error.name === 'NotAllowedError' || error.name === 'PermissionDeniedError') {
            showError('⚠ Camera Permission Denied\n\nTo enable camera:\n\n📱 iPhone: Settings > Safari > Camera > Allow\n📱 Android: Settings > Apps > Browser > Permissions > Camera > Allow\n\nOr tap the lock icon in your address bar and enable camera permissions, then refresh.');
        } else if (error.name === 'NotFoundError' || error.name === 'DevicesNotFoundError') {
            showError('⚠ No Camera Found\n\nNo camera detected on this device. A camera is required for attendance verification.');
        } else if (error.name === 'NotReadableError' || error.name === 'TrackStartError') {
            showError('⚠ Camera In Use\n\nCamera is being used by another application. Please close other apps using the camera and refresh.');
        } else if (error.name === 'OverconstrainedError') {
            console.warn('[Camera] Constraints not met, trying fallback...');
            try {
                mediaStream = await navigator.mediaDevices.getUserMedia({ video: true });
                video.srcObject = mediaStream;
            } catch (fallbackError) {
                showError('Camera access required. Please enable camera permissions and refresh.');
            }
        } else {
            showError(`⚠ Camera Error: ${error.message || 'Unknown error'}\n\nPlease enable camera permissions and refresh the page.`);
        }
    }
}

captureBtn.addEventListener('click', async () => {
    if (photoData) {
        photoPreview.style.display = 'none';
        video.style.display = 'block';
        captureBtn.textContent = 'Capture';
        photoData = null;
        checkFormValidity();
        return;
    }

    if (!video.videoWidth || !video.videoHeight) {
        alert('Camera not ready. Please wait and try again.');
        return;
    }

    let targetWidth = video.videoWidth;
    let targetHeight = video.videoHeight;
    const MAX_DIMENSION = 800;
    
    if (targetWidth > MAX_DIMENSION || targetHeight > MAX_DIMENSION) {
        if (targetWidth > targetHeight) {
            targetHeight = Math.round((targetHeight * MAX_DIMENSION) / targetWidth);
            targetWidth = MAX_DIMENSION;
        } else {
            targetWidth = Math.round((targetWidth * MAX_DIMENSION) / targetHeight);
            targetHeight = MAX_DIMENSION;
        }
    }

    canvas.width = targetWidth;
    canvas.height = targetHeight;
    const ctx = canvas.getContext('2d');
    ctx.drawImage(video, 0, 0, targetWidth, targetHeight);
    
    try {
        photoData = canvas.toDataURL('image/jpeg', 0.75);
    } catch (e) {
        alert('Failed to capture photo. Please try again.');
        return;
    }

    if (!faceModelLoaded) {
        try {
            captureBtn.disabled = true;
            captureBtn.textContent = 'Loading face engine...';
            await loadFaceModel();
        } catch (err) {
            console.warn('Could not load face model on capture:', err);
        }
    }

    faceDetectedVal = true;
    if (faceModelLoaded) {
        try {
            captureBtn.disabled = true;
            captureBtn.textContent = 'Scanning...';
            const options = new faceapi.SsdMobilenetv1Options({ minConfidence: 0.5 });
            const detections = await faceapi.detectAllFaces(canvas, options);
            faceDetectedVal = detections.length > 0;
            if (!faceDetectedVal) {
                alert("Warning: We couldn't detect a face in the photo. Please make sure your face is clearly visible in the camera before submitting.");
            }
        } catch (err) {
            console.warn('Face detection failed:', err);
            faceDetectedVal = false;
        } finally {
            captureBtn.disabled = false;
        }
    } else {
        console.warn('Face model not loaded. Marking as undetected for safety.');
        faceDetectedVal = false;
        alert("Warning: Face verification engine is not ready. Your attendance will be marked with a 'No Face' warning.");
    }

    photoPreview.src = photoData;
    photoPreview.style.display = 'block';
    video.style.display = 'none';
    captureBtn.textContent = 'Retake';
    
    checkFormValidity();
});

photoPreview.addEventListener('click', () => {
    photoPreview.style.display = 'none';
    video.style.display = 'block';
    captureBtn.textContent = 'Capture';
    photoData = null;
    checkFormValidity();
});

nameInput.addEventListener('input', checkFormValidity);
rollNumberInput.addEventListener('input', checkFormValidity);
captchaInput.addEventListener('input', checkFormValidity);
refreshCaptchaBtn.addEventListener('click', loadCaptcha);

function checkFormValidity() {
    const nameValid = nameInput.value.trim().length >= 2;
    const rollValid = rollNumberInput.value.trim().length >= 1 && /^[a-zA-Z0-9]+$/.test(rollNumberInput.value.trim());
    const photoValid = photoData !== null;
    const locationValid = userLocation !== null;
    const captchaValid = captchaInput.value.trim().length >= 4;

    submitBtn.disabled = !(nameValid && rollValid && photoValid && locationValid && captchaValid);
}

async function uploadDirectToS3(blob) {
    try {
        const uploadUrlRes = await apiFetch(`${API_BASE}/s/${shortCode}/upload-url`);
        const uploadUrlData = await uploadUrlRes.json();
        
        if (!uploadUrlRes.ok) {
            throw new Error(uploadUrlData.message || 'Failed to get upload URL');
        }

        const uploadRes = await fetch(uploadUrlData.uploadUrl, {
            method: uploadUrlData.method,
            headers: uploadUrlData.headers || {},
            body: blob
        });

        if (!uploadRes.ok) {
            throw new Error('Direct upload failed');
        }

        return uploadUrlData.publicId;
    } catch (error) {
        throw error;
    }
}

async function dataURLtoBlob(dataURL) {
    const response = await fetch(dataURL);
    return await response.blob();
}

submitBtn.addEventListener('click', async () => {
    if (!userLocation) {
        alert('Location is required to submit attendance. Please allow location access.');
        return;
    }

    submitBtn.disabled = true;
    submitBtn.textContent = 'Submitting...';

    try {
        let requestBody;
        
        if (storageInfo?.provider === 's3' && storageInfo?.supportsDirectUpload) {
            submitBtn.textContent = 'Uploading photo...';
            const blob = await dataURLtoBlob(photoData);
            const publicId = await uploadDirectToS3(blob);
            
            submitBtn.textContent = 'Submitting attendance...';
            requestBody = {
                studentName: nameInput.value.trim(),
                rollNumber: rollNumberInput.value.trim().toUpperCase(),
                directUpload: true,
                publicId,
                latitude: userLocation.latitude,
                longitude: userLocation.longitude,
                faceDetected: faceDetectedVal,
                captchaAnswer: captchaInput.value.trim(),
                captchaId
            };
        } else {
            requestBody = {
                studentName: nameInput.value.trim(),
                rollNumber: rollNumberInput.value.trim().toUpperCase(),
                photo: photoData,
                latitude: userLocation.latitude,
                longitude: userLocation.longitude,
                faceDetected: faceDetectedVal,
                captchaAnswer: captchaInput.value.trim(),
                captchaId
            };
        }

        const res = await apiFetch(`${API_BASE}/s/${shortCode}/submit`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(requestBody)
        });

        const data = await res.json();

        if (!res.ok) {
            throw new Error(data.message || 'Failed to submit attendance');
        }

        cleanupCamera();

        const verifiedText = data.attendance.verified 
            ? 'VERIFIED - You are within the allowed area'
            : 'NOT VERIFIED - You are outside the allowed area';
        
        document.getElementById('distanceInfo').textContent = 
            `Distance: ${data.attendance.distanceFromLocation}m | ${verifiedText}`;
        
        showView(successView);
    } catch (error) {
        alert(error.message);
        loadCaptcha();
        submitBtn.disabled = false;
        submitBtn.textContent = 'Submit Attendance';
        checkFormValidity();
    }
});

function retryLocation() {
    document.getElementById('retryLocationBtn')?.classList.add('hidden');
    initGeolocation();
}

init();
