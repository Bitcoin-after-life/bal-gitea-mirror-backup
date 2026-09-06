// Add a visible text link to the main site next to the copyright line.
// The theme's extra.social icon already links to bitcoin-after.life, but
// as an icon-only button (title tooltip, no visible text) it's easy to
// miss — this makes the link readable as text too.
(function () {
  function decorate() {
    var highlight = document.querySelector('.md-copyright__highlight');
    if (!highlight || highlight.querySelector('.footer-home-link')) return;
    highlight.appendChild(document.createTextNode(' — '));
    var link = document.createElement('a');
    link.href = 'https://bitcoin-after.life';
    link.className = 'footer-home-link';
    link.textContent = 'bitcoin-after.life';
    highlight.appendChild(link);
  }
  if (document.readyState !== 'loading') decorate();
  else document.addEventListener('DOMContentLoaded', decorate);
})();
