(function () {
  var t = "system";
  try {
    t = localStorage.getItem("kppcnTheme") || "system";
  } catch (e) {}
  if (t !== "light" && t !== "dark")
    t = window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
  document.documentElement.setAttribute("data-theme", t);
})();
