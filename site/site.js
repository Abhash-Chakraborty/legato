// legato.abhashchakraborty.tech: theme, header, reveals, downloads for this device,
// the three demos on the home page, and the docs' outline, copy buttons and search.
(() => {
  "use strict";
  const root = document.documentElement;
  root.classList.add("js");
  const $ = (s, el = document) => el.querySelector(s);
  const $$ = (s, el = document) => [...el.querySelectorAll(s)];
  const still = matchMedia("(prefers-reduced-motion: reduce)");
  const darkMq = matchMedia("(prefers-color-scheme: dark)");
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const store = {
    get: (k) => {
      try {
        return localStorage.getItem(k);
      } catch {
        return null;
      }
    },
    set: (k, v) => {
      try {
        localStorage.setItem(k, v);
      } catch {}
    },
  };
  // The site's root, from the icon link every page has.
  const base = new URL(".", $('link[rel="icon"]').href).href;

  // ---------- theme ----------
  const isDark = () =>
    root.dataset.theme === "dark" || (!root.dataset.theme && darkMq.matches);
  const syncDark = () => {
    root.toggleAttribute("data-dark", isDark());
    // The hero picture follows a theme picked here, not only the system's.
    for (const src of $$('picture source[srcset*="dark"]')) {
      src.media = root.dataset.theme
        ? isDark()
          ? "all"
          : "not all"
        : "(prefers-color-scheme: dark)";
    }
  };
  syncDark();
  darkMq.addEventListener("change", syncDark);
  for (const b of $$("[data-theme-toggle]")) {
    b.addEventListener("click", () => {
      const next = isDark() ? "light" : "dark";
      root.dataset.theme = next;
      store.set("legato-theme", next);
      syncDark();
    });
  }

  // ---------- header ----------
  const top = $("[data-top]");
  const onScroll = () => top && top.classList.toggle("scrolled", scrollY > 8);
  addEventListener("scroll", onScroll, { passive: true });
  onScroll();
  const menu = $("[data-menu]");
  const sheet = $("#sheet");
  if (menu && sheet) {
    const set = (open) => {
      menu.setAttribute("aria-expanded", String(open));
      sheet.hidden = !open;
    };
    menu.addEventListener("click", () => set(sheet.hidden));
    sheet.addEventListener("click", (e) => e.target.closest("a") && set(false));
    addEventListener(
      "keydown",
      (e) => e.key === "Escape" && !sheet.hidden && set(false),
    );
  }

  // ---------- reveal on scroll ----------
  const reveal = $$("[data-reveal]");
  if ("IntersectionObserver" in window) {
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (e.isIntersecting) {
            e.target.classList.add("in");
            io.unobserve(e.target);
          }
        }
      },
      { rootMargin: "0px 0px -8% 0px" },
    );
    reveal.forEach((el) => io.observe(el));
  } else reveal.forEach((el) => el.classList.add("in"));

  // ---------- downloads for this device ----------
  const ua = navigator.userAgent;
  const android = /Android/i.test(ua);
  const mark = (kind) => {
    const card = $(`[data-dl="${kind}"]`);
    if (card) card.classList.add("mine");
  };
  if (android) {
    mark("android");
    const get = $("#get");
    const other = $("#get-other");
    if (get) {
      get.href = get.dataset.android;
      $("span", get).textContent = "Download for Android";
    }
    if (other) $("span", other).textContent = "Windows and other downloads";
    const note = $("#get-note");
    if (note)
      note.textContent =
        "Android 10 or later on a 64-bit Arm phone · free and open source";
  } else if (/Windows/i.test(ua)) {
    mark("win");
    navigator.userAgentData?.getHighEntropyValues?.(["architecture"]).then(
      (v) => {
        if (v.architecture === "arm") {
          $('[data-dl="win"]')?.classList.remove("mine");
          mark("arm");
          const get = $("#get");
          const arm = $('[data-dl="arm"]');
          if (get && arm) {
            get.href = arm.href;
            $("span", get).textContent = "Download for Windows on Arm";
          }
        }
      },
      () => {},
    );
  }

  // A demo runs only while on screen and the tab is visible; reduced motion shows its end.
  function onScreen(el, run) {
    let visible = false;
    let running = false;
    const kick = async () => {
      if (running || !visible || document.hidden) return;
      running = true;
      try {
        await run(() => visible && !document.hidden);
      } finally {
        running = false;
      }
      if (visible && !document.hidden) setTimeout(kick, 400);
    };
    new IntersectionObserver(
      ([e]) => {
        visible = e.isIntersecting;
        kick();
      },
      { threshold: 0.35 },
    ).observe(el);
    document.addEventListener("visibilitychange", kick);
    return kick;
  }

  // Moves bar heights like a voice; returns a stop function.
  function wave(bars) {
    let t = 0;
    const id = setInterval(() => {
      t += 1;
      bars.forEach((b, i) => {
        const h =
          4 + Math.abs(Math.sin(t * 0.55 + i * 1.3)) * 10 + Math.random() * 4;
        b.style.height = `${h.toFixed(1)}px`;
      });
    }, 90);
    return () => {
      clearInterval(id);
      bars.forEach((b) => (b.style.height = ""));
    };
  }

  // ---------- the dictation demo ----------
  const dictate = $("[data-dictate]");
  if (dictate) {
    const lbar = $("[data-lbar]", dictate);
    const label = $("[data-lbar-label]", dictate);
    const keys = $("[data-keys]", dictate);
    const heard = $("[data-heard]", dictate);
    const typed = $("[data-typed]", dictate);
    const hint = $("[data-hint]", dictate);
    const sent = $("[data-sent]", dictate);
    const sentText = $("[data-sent-text]", dictate);
    const spoken = [
      "um",
      "so",
      "the",
      "Arm",
      "build",
      "is",
      "uh",
      "ready,",
      "we",
      "can",
      "ship",
      "it",
      "on",
      "Thursday",
    ];
    const fillers = new Set([0, 1, 6]);
    const result = "The Arm build is ready, so we can ship it on Thursday.";
    const state = (s, text) => {
      lbar.dataset.state = s;
      label.textContent = text;
      dictate.dataset.step = s === "done" ? "done" : s;
    };
    const reset = () => {
      state("idle", "Ready");
      keys.classList.remove("down");
      heard.textContent = "";
      typed.textContent = "";
      typed.classList.remove("flash");
      hint.hidden = false;
      sent.hidden = true;
    };
    const end = () => {
      state("done", "Pasted");
      heard.innerHTML = spoken
        .map((w, i) =>
          fillers.has(i) ? `<span class="filler">${w}</span>` : w,
        )
        .join(" ");
      typed.textContent = result;
      hint.hidden = true;
    };
    const run = async (ok) => {
      reset();
      await sleep(900);
      if (!ok()) return;
      keys.classList.add("down");
      state("listening", "Listening");
      const stop = wave($$(".lbar-wave i", lbar));
      const words = [];
      for (let i = 0; i < spoken.length; i++) {
        if (!ok()) return stop();
        words.push(
          fillers.has(i)
            ? `<span class="filler-soon">${spoken[i]}</span>`
            : spoken[i],
        );
        heard.innerHTML = words.join(" ");
        await sleep(fillers.has(i) ? 330 : 230 + spoken[i].length * 22);
      }
      await sleep(500);
      stop();
      keys.classList.remove("down");
      state("working", "Writing up");
      heard.innerHTML = spoken
        .map((w, i) =>
          fillers.has(i) ? `<span class="filler">${w}</span>` : w,
        )
        .join(" ");
      await sleep(1100);
      if (!ok()) return;
      state("paste", "Pasted");
      lbar.dataset.state = "done";
      hint.hidden = true;
      typed.textContent = result;
      typed.classList.add("flash");
      await sleep(1600);
      dictate.dataset.step = "done";
      await sleep(1400);
      if (!ok()) return;
      // Enter sends it, as it would in a chat app.
      sentText.textContent = result;
      sent.hidden = false;
      typed.textContent = "";
      hint.hidden = false;
      state("idle", "Ready");
      await sleep(3200);
    };
    if (still.matches) end();
    else {
      const kick = onScreen(dictate, run);
      $("[data-replay]", dictate)?.addEventListener("click", kick);
    }
  }

  // ---------- the meeting demo ----------
  const meet = $("[data-meet]");
  if (meet) {
    const words = [];
    for (const p of $$("[data-words]", meet)) {
      let t = Number(p.dataset.words);
      const nodes = [...p.childNodes];
      p.textContent = "";
      for (const n of nodes) {
        if (n.nodeType === 3) {
          for (const part of n.textContent.split(/(\s+)/)) {
            if (!part) continue;
            if (/^\s+$/.test(part)) {
              p.append(part);
              continue;
            }
            const s = document.createElement("span");
            s.className = "w";
            s.textContent = part;
            p.append(s);
            words.push({ el: s, t, line: p.parentElement });
            t += 140 + part.length * 45;
          }
        } else {
          n.classList.add("w");
          p.append(n);
          words.push({ el: n, t, line: p.parentElement });
          t += 140 + n.textContent.length * 45;
        }
      }
    }
    const total = words[words.length - 1].t + 900;
    const fill = $("[data-scrub-fill]", meet);
    const time = $("[data-time]", meet);
    const toast = $("[data-meet-toast]", meet);
    const play = $("[data-play]", meet);
    let pos = 0;
    let playing = false;
    let last = 0;
    let raf = 0;
    const fmt = (ms) => `0:${String(Math.floor(ms / 1000)).padStart(2, "0")}`;
    const paint = () => {
      fill.style.width = `${(pos / total) * 100}%`;
      time.textContent = fmt(pos);
      let current = null;
      for (const w of words) {
        const said = w.t <= pos;
        w.el.classList.toggle("said", said);
        if (said) current = w;
      }
      for (const w of words)
        w.el.classList.toggle("on", w === current && pos > 0);
      for (const l of $$(".mline", meet))
        l.classList.toggle("now", !!current && l === current.line && pos > 0);
    };
    const tick = (now) => {
      pos += now - last;
      last = now;
      if (pos >= total) {
        pos = total;
        setPlaying(false);
      }
      paint();
      if (playing) raf = requestAnimationFrame(tick);
    };
    function setPlaying(on) {
      playing = on;
      meet.classList.toggle("playing", on);
      play.setAttribute("aria-label", on ? "Pause" : "Play");
      cancelAnimationFrame(raf);
      if (on) {
        if (pos >= total) pos = 0;
        last = performance.now();
        raf = requestAnimationFrame(tick);
      }
    }
    play.addEventListener("click", () => setPlaying(!playing));
    const seek = (ms) => {
      pos = Math.max(0, Math.min(total, ms));
      paint();
      if (!playing) setPlaying(true);
    };
    $("[data-scrub]", meet).addEventListener("click", (e) => {
      const r = e.currentTarget.getBoundingClientRect();
      seek(((e.clientX - r.left) / r.width) * total);
    });
    let toastTimer = 0;
    const say = (text) => {
      toast.textContent = text;
      toast.classList.add("show");
      clearTimeout(toastTimer);
      toastTimer = setTimeout(() => toast.classList.remove("show"), 2600);
    };
    const fix = (el) => {
      el.textContent = el.dataset.fix;
      el.classList.remove("misheard");
      el.removeAttribute("role");
      el.removeAttribute("tabindex");
      el.removeAttribute("aria-label");
      el.classList.add("fixed");
      say("Fixed, and still tied to 0:03. The summary catches up in 20 s.");
    };
    meet.addEventListener("click", (e) => {
      const w = e.target.closest(".w");
      if (!w) return;
      if (w.classList.contains("misheard")) return fix(w);
      const hit = words.find((x) => x.el === w);
      if (hit) seek(hit.t);
    });
    meet.addEventListener("keydown", (e) => {
      const w = e.target.closest(".misheard");
      if (w && (e.key === "Enter" || e.key === " ")) {
        e.preventDefault();
        fix(w);
      }
    });
    // Plays once by itself the first time it comes into view.
    if (!still.matches) {
      const io = new IntersectionObserver(
        ([e]) => {
          if (e.isIntersecting) {
            io.disconnect();
            setTimeout(() => setPlaying(true), 600);
          }
        },
        { threshold: 0.5 },
      );
      io.observe(meet);
    }
    paint();
  }

  // ---------- the phone demo ----------
  const phone = $("[data-phone]");
  if (phone) {
    const lb = $("[data-p-bubble]", phone);
    const typed = $("[data-p-typed]", phone);
    const hint = $("[data-p-hint]", phone);
    const sent = $("[data-p-sent]", phone);
    const text = "Yes, see you at eight. I'll book the table.";
    const reset = () => {
      lb.dataset.state = "idle";
      typed.textContent = "";
      hint.hidden = false;
      sent.hidden = true;
    };
    const end = () => {
      lb.dataset.state = "idle";
      typed.textContent = text;
      hint.hidden = true;
    };
    const tap = async (cls = "ripple") => {
      lb.classList.remove(cls);
      void lb.offsetWidth;
      lb.classList.add(cls);
      await sleep(cls === "tap" ? 180 : 60);
      if (cls === "tap") lb.classList.remove("tap");
    };
    const run = async (ok) => {
      reset();
      await sleep(1000);
      if (!ok()) return;
      lb.dataset.state = "press";
      await tap();
      await sleep(160);
      lb.dataset.state = "listening";
      const stop = wave($$(".lb-wave i", lb));
      await sleep(2600);
      stop();
      if (!ok()) return;
      await tap("tap");
      lb.dataset.state = "working";
      await sleep(1000);
      if (!ok()) return;
      lb.dataset.state = "idle";
      hint.hidden = true;
      typed.textContent = text;
      await sleep(2200);
      if (!ok()) return;
      sent.textContent = text;
      sent.hidden = false;
      typed.textContent = "";
      hint.hidden = false;
      await sleep(3000);
    };
    if (still.matches) end();
    else onScreen(phone, run);
  }

  // ---------- copy buttons ----------
  for (const b of $$("[data-copy]")) {
    b.addEventListener("click", async () => {
      const code = b.parentElement.querySelector("code");
      try {
        await navigator.clipboard.writeText(code.innerText.replace(/\n$/, ""));
        b.classList.add("done");
        b.setAttribute("aria-label", "Copied");
        setTimeout(() => {
          b.classList.remove("done");
          b.setAttribute("aria-label", "Copy");
        }, 1600);
      } catch {}
    });
  }

  // ---------- docs: the menu on small screens, and the outline ----------
  const sideToggle = $("[data-side-toggle]");
  const sideNav = $("[data-side-nav]");
  if (sideToggle && sideNav) {
    sideToggle.addEventListener("click", () => {
      const open = !sideNav.classList.contains("open");
      sideNav.classList.toggle("open", open);
      sideToggle.setAttribute("aria-expanded", String(open));
    });
  }
  const toc = $("[data-toc]");
  if (toc) {
    const links = $$("a", toc);
    const heads = links
      .map((a) => document.getElementById(decodeURIComponent(a.hash.slice(1))))
      .filter(Boolean);
    const spy = () => {
      const line = 120;
      let current = heads[0];
      for (const h of heads)
        if (h.getBoundingClientRect().top <= line) current = h;
      if (innerHeight + scrollY >= document.body.scrollHeight - 4)
        current = heads[heads.length - 1];
      links.forEach((a) =>
        a.classList.toggle("on", a.hash.slice(1) === current?.id),
      );
    };
    addEventListener("scroll", spy, { passive: true });
    spy();
  }

  // ---------- search ----------
  const dialog = $("[data-search]");
  if (dialog) {
    const input = $("[data-search-input]", dialog);
    const list = $("[data-search-results]", dialog);
    const empty = $("[data-search-empty]", dialog);
    let index = null;
    let results = [];
    let sel = 0;
    let opener = null;
    const load = async () => {
      if (index) return index;
      try {
        const res = await fetch(`${base}search.json`);
        index = await res.json();
      } catch {
        index = [];
      }
      return index;
    };
    const escHtml = (s) =>
      s.replace(
        /[&<>"]/g,
        (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c],
      );
    const highlight = (s, terms) => {
      let out = escHtml(s);
      for (const t of terms)
        out = out.replace(
          new RegExp(`(${t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`, "ig"),
          "<mark>$1</mark>",
        );
      return out;
    };
    const snippet = (text, terms) => {
      const lower = text.toLowerCase();
      const at = Math.max(
        0,
        Math.min(...terms.map((t) => lower.indexOf(t)).filter((i) => i >= 0)) -
          40,
      );
      return (at > 0 ? "…" : "") + text.slice(at, at + 160);
    };
    const render = () => {
      list.innerHTML = results
        .map(
          (r, i) =>
            `<li><a href="${base}${r.u}" role="option" aria-selected="${i === sel}"><small>${escHtml(r.p)}</small><strong>${highlight(r.h, r.terms)}</strong><span>${highlight(snippet(r.t, r.terms), r.terms)}</span></a></li>`,
        )
        .join("");
      list
        .querySelector('[aria-selected="true"]')
        ?.scrollIntoView({ block: "nearest" });
    };
    const search = async () => {
      const q = input.value.trim().toLowerCase();
      const data = await load();
      if (!q) {
        results = [];
        empty.hidden = true;
        return render();
      }
      const terms = q.split(/\s+/).filter(Boolean);
      results = data
        .map((r) => {
          const h = r.h.toLowerCase();
          const p = r.p.toLowerCase();
          const t = r.t.toLowerCase();
          let score = 0;
          for (const term of terms) {
            if (h.includes(term)) score += h.startsWith(term) ? 12 : 8;
            else if (p.includes(term)) score += 5;
            else if (t.includes(term)) score += 2;
            else return null;
          }
          return { ...r, terms, score };
        })
        .filter(Boolean)
        .sort((a, b) => b.score - a.score)
        .slice(0, 12);
      sel = 0;
      empty.hidden = results.length > 0;
      render();
    };
    const open = () => {
      opener = document.activeElement;
      dialog.hidden = false;
      dialog.classList.remove("closing");
      input.value = "";
      results = [];
      empty.hidden = true;
      render();
      input.focus();
      load();
      document.body.style.overflow = "hidden";
    };
    const close = async () => {
      if (dialog.hidden) return;
      dialog.classList.add("closing");
      await sleep(100);
      dialog.hidden = true;
      document.body.style.overflow = "";
      opener?.focus?.();
    };
    $$("[data-search-open]").forEach((b) => b.addEventListener("click", open));
    dialog.addEventListener("click", (e) => e.target === dialog && close());
    input.addEventListener("input", search);
    addEventListener("keydown", (e) => {
      const typing = /INPUT|TEXTAREA/.test(
        document.activeElement?.tagName ?? "",
      );
      if (
        (e.key === "k" && (e.ctrlKey || e.metaKey)) ||
        (e.key === "/" && !typing && dialog.hidden)
      ) {
        e.preventDefault();
        dialog.hidden ? open() : close();
        return;
      }
      if (dialog.hidden) return;
      if (e.key === "Escape") close();
      else if (e.key === "ArrowDown" && results.length) {
        e.preventDefault();
        sel = (sel + 1) % results.length;
        render();
      } else if (e.key === "ArrowUp" && results.length) {
        e.preventDefault();
        sel = (sel - 1 + results.length) % results.length;
        render();
      } else if (e.key === "Enter" && results[sel]) {
        e.preventDefault();
        location.href = `${base}${results[sel].u}`;
        close();
      }
    });
  }
})();
