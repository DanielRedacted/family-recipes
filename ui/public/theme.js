// Apply the saved theme before first paint (external file: the CSP forbids inline scripts).
try { var t = localStorage.getItem('theme'); if (t === 'light' || t === 'dark') document.documentElement.setAttribute('data-theme', t); } catch (e) {}
