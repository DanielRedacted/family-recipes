// Apply the saved theme and colour before first paint (external file: the CSP forbids inline scripts).
// The browser's own bar (theme-color) follows it; #141319 is the dark --bg. The colour's CSS is
// written by lib/accent.ts; inline styles are allowed.
try { var t = localStorage.getItem('theme'); if (t === 'light' || t === 'dark') document.documentElement.setAttribute('data-theme', t); if (t === 'dark') document.querySelector('meta[name="theme-color"]').setAttribute('content', '#141319'); } catch (e) {}
try { var c = localStorage.getItem('accent-css'); if (c) { var s = document.createElement('style'); s.id = 'accent'; s.textContent = c; document.head.appendChild(s); } } catch (e) {}
