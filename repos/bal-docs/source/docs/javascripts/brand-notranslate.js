// Keep the brand name "Bitcoin After Life" untranslated by Chrome / Google
// Translate, while the rest of the page still translates normally.
(function () {
  function mark() {
    document
      .querySelectorAll('.md-header__topic .md-ellipsis, .bal-hero .name')
      .forEach(function (el) {
        el.setAttribute('translate', 'no');
        el.classList.add('notranslate');
      });
  }
  if (document.readyState !== 'loading') mark();
  else document.addEventListener('DOMContentLoaded', mark);
})();
