// GitHub's `/releases/latest/download/<asset>` alias always redirects to the
// newest release's asset with this exact filename — no API call, no version
// number to keep in sync. Asset name is fixed by
// .github/workflows/mobile-release.yml's build step. iOS is deliberately
// omitted here: the .ipa in that same release is built with EAS's default
// "store" distribution profile, so it downloads but won't install on a
// device without an App Store/TestFlight submission or an ad-hoc profile —
// linking it today would look broken to anyone who tries it.
export const MENTOR_APK_URL =
  "https://github.com/thearpankumar/Attendix-tgl/releases/latest/download/attendix-mentor-android.apk";
