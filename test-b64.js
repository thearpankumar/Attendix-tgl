const buf = new Uint8Array(80000); // 80KB array
try {
  const str = btoa(String.fromCharCode(...buf));
  console.log('Success');
} catch (e) {
  console.log('Error:', e.name, e.message);
}
