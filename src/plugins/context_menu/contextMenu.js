import "@tabler/icons-webfont/dist/tabler-icons.min.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";

document.addEventListener("contextmenu", (e) => e.preventDefault());

const currentWindow = getCurrentWindow();
const menuContainer = document.getElementById("menuContainer");
const systemThemeMediaQuery = window.matchMedia("(prefers-color-scheme: dark)");

const LAYOUT = {
  trayBottomMargin: 50,
  bodyPadding: 8,
  shadowMargin: 5,
  maxOriginSpace: 210,
  submenuMaxWidth: 200,
  submenuMaxHeight: 400,
  resizePadding: 10,
  submenuHideDelay: 200,
  previewDelay: 300,
};

function cssToLogical(value, textScale) {
  return value * textScale;
}

function logicalToCss(value, textScale) {
  return value / textScale;
}

function normalizeItems(items) {
  return Array.isArray(items) ? items : [];
}

function updateScrollIndicator(el) {
  if (el)
    el.classList.toggle("has-scroll", el.scrollHeight - el.clientHeight > 4);
}

function applyContainerLayout(container, layout = {}) {
  const width = Number(layout?.width);
  container.style.minWidth =
    Number.isFinite(width) && width > 0 ? `${Math.round(width)}px` : "";
}

const themeController = {
  currentThemeSetting: null,
  customFontStyleEl: null,

  apply(appearance = {}) {
    const theme = appearance?.theme ?? null;
    const lightThemeStyle = appearance?.lightThemeStyle ?? null;
    const darkThemeStyle = appearance?.darkThemeStyle ?? null;
    const uiAnimationEnabled = appearance?.uiAnimationEnabled !== false;

    this.currentThemeSetting = theme;
    const isDark =
      theme === "dark" || (theme !== "light" && systemThemeMediaQuery.matches);

    document.body.classList.toggle("dark-theme", isDark);
    document.body.classList.toggle(
      "dark-classic",
      isDark && darkThemeStyle === "classic",
    );
    document.body.classList.toggle(
      "theme-light-wireframe",
      !isDark && lightThemeStyle === "wireframe",
    );
    document.body.classList.toggle(
      "theme-dark-sketch",
      isDark && darkThemeStyle === "sketch",
    );
    document.body.classList.toggle("no-ui-animations", !uiAnimationEnabled);
    this.applyCustomFont(appearance);
  },

  syncSystemTheme(isDark) {
    if (!this.currentThemeSetting || this.currentThemeSetting === "auto") {
      document.body.classList.toggle("dark-theme", isDark);
    }
  },

  applyCustomFont(appearance = {}) {
    const enabled = appearance?.customFontEnabled === true;
    const type = appearance?.customFontType || "file";
    const url = appearance?.customFontUrl || "";
    const family =
      type === "file"
        ? "CustomFont"
        : appearance?.customFontFamily || "CustomFont";

    if (this.customFontStyleEl) {
      document.head.removeChild(this.customFontStyleEl);
      this.customFontStyleEl = null;
    }

    const existingLink = document.getElementById("custom-font-link");
    if (existingLink) document.head.removeChild(existingLink);

    if (!enabled) return;

    if (type === "url" && url && family) {
      const link = document.createElement("link");
      link.rel = "stylesheet";
      link.href = url;
      link.id = "custom-font-link";
      document.head.appendChild(link);
    }

    this.customFontStyleEl = document.createElement("style");
    this.customFontStyleEl.id = "custom-font-override";
    this.customFontStyleEl.textContent = `*{font-family:"${family}","Segoe UI","Microsoft YaHei",sans-serif!important}`;
    document.head.appendChild(this.customFontStyleEl);
  },
};

systemThemeMediaQuery.addEventListener("change", (e) => {
  themeController.syncSystemTheme(e.matches);
});

class PreviewController {
  constructor() {
    this.timers = new WeakMap();
  }

  bind(menuItem, item) {
    if (!item.previewImage) return;

    menuItem.addEventListener("mouseenter", () => {
      const timer = setTimeout(async () => {
        try {
          await invoke("show_native_image_preview", {
            filePath: item.previewImage,
          });
        } catch (nativeError) {
          console.error("原生预览失败，回退到 Tauri WebView 版:", nativeError);
          invoke("pin_image_from_file", {
            filePath: item.previewImage,
            previewMode: true,
          }).catch((fallbackError) => { console.error("WebView 预览也失败:", fallbackError); });
        }
      }, LAYOUT.previewDelay);
      this.timers.set(menuItem, timer);
    });

    menuItem.addEventListener("mouseleave", () => {
      this.clear(menuItem);
      invoke("close_native_image_preview").catch(() => {});
      invoke("close_image_preview").catch(() => {});
    });
  }

  clear(menuItem) {
    const timer = this.timers.get(menuItem);
    if (timer) clearTimeout(timer);
    this.timers.delete(menuItem);
  }
}

class LayoutManager {
  constructor(container) {
    this.container = container;
    this.monitorInfo = { x: 0, y: 0, width: 1920, height: 1080 };
    this.windowOrigin = { x: 0, y: 0 };
    this.scaleFactor = 1;
    this.textScale = 1;
    this.isTrayMenu = false;
  }

  async configure(options = {}) {
    this.scaleFactor = await currentWindow.scaleFactor().catch(() => 1);
    this.textScale = await invoke("get_system_text_scale").catch(() => 1);
    this.isTrayMenu = options.behavior?.isTrayMenu || false;
    this.monitorInfo = {
      x: options.monitor?.x || 0,
      y: options.monitor?.y || 0,
      width: options.monitor?.width || 1920,
      height: options.monitor?.height || 1080,
    };
    this.windowOrigin = { x: 0, y: 0 };
  }

  positionMainMenu(options = {}) {
    const menuRect = this.container.getBoundingClientRect();
    const screenWidth = this.monitorInfo.width;
    const screenHeight =
      this.monitorInfo.height - (this.isTrayMenu ? LAYOUT.trayBottomMargin : 0);

    const menuCssWidth =
      menuRect.width + LAYOUT.bodyPadding * 2 + LAYOUT.shadowMargin;
    const menuCssHeight =
      menuRect.height + LAYOUT.bodyPadding * 2 + LAYOUT.shadowMargin;
    const menuLogicalW = cssToLogical(menuCssWidth, this.textScale);
    const menuLogicalH = cssToLogical(menuCssHeight, this.textScale);

    const cursor = options.placement?.cursor ?? { x: 0, y: 0 };
    const left = Math.max(0, Math.min(cursor.x, screenWidth - menuLogicalW));
    const top = this.isTrayMenu
      ? Math.max(0, cursor.y - menuLogicalH)
      : Math.max(0, Math.min(cursor.y, screenHeight - menuLogicalH));

    const leftSpace = Math.min(
      logicalToCss(left, this.textScale),
      LAYOUT.maxOriginSpace,
    );
    const topSpace = Math.min(
      logicalToCss(top, this.textScale),
      LAYOUT.maxOriginSpace,
    );

    this.windowOrigin = {
      x: left - cssToLogical(leftSpace, this.textScale),
      y: top - cssToLogical(topSpace, this.textScale),
    };

    this.container.style.left = `${Math.round(leftSpace)}px`;
    this.container.style.top = `${Math.round(topSpace)}px`;
  }

  positionSubmenu(submenu, parentItem) {
    const menuRect = this.container.getBoundingClientRect();
    const parentRect = parentItem.getBoundingClientRect();
    const screenWidth = this.monitorInfo.width - this.windowOrigin.x;
    const bottomMargin = this.isTrayMenu ? LAYOUT.trayBottomMargin : 0;
    const screenHeight =
      this.monitorInfo.height - bottomMargin - this.windowOrigin.y;

    submenu.style.cssText = `max-width:${LAYOUT.submenuMaxWidth}px;max-height:${LAYOUT.submenuMaxHeight}px;left:${menuRect.width}px;top:0`;

    const submenuRect = submenu.getBoundingClientRect();
    const spaceRight =
      screenWidth -
      cssToLogical(menuRect.left + menuRect.width, this.textScale);
    const spaceLeft =
      cssToLogical(menuRect.left, this.textScale) + this.windowOrigin.x;

    if (
      spaceRight >= cssToLogical(submenuRect.width, this.textScale) ||
      spaceLeft < cssToLogical(submenuRect.width, this.textScale)
    ) {
      submenu.style.left = `${menuRect.width}px`;
      submenu.style.right = "auto";
    } else {
      submenu.style.left = "auto";
      submenu.style.right = `${menuRect.width}px`;
    }

    let top = parentRect.top - menuRect.top;
    const bottomSpace =
      screenHeight -
      cssToLogical(menuRect.top + top + submenuRect.height, this.textScale);
    if (bottomSpace < 0) top += logicalToCss(bottomSpace, this.textScale);
    const topSpace =
      cssToLogical(menuRect.top, this.textScale) +
      this.windowOrigin.y +
      cssToLogical(top, this.textScale);
    if (topSpace < 0) top -= logicalToCss(topSpace, this.textScale);

    submenu.style.top = `${top}px`;
    updateScrollIndicator(submenu);
  }

  getWindowPhysicalOrigin() {
    return {
      x: this.monitorInfo.x + this.windowOrigin.x * this.scaleFactor,
      y: this.monitorInfo.y + this.windowOrigin.y * this.scaleFactor,
    };
  }

  toPhysicalRegion(rect) {
    const totalScale = this.scaleFactor * this.textScale;
    const origin = this.getWindowPhysicalOrigin();
    return {
      x: Math.round(origin.x + rect.left * totalScale),
      y: Math.round(origin.y + rect.top * totalScale),
      width: Math.round(rect.width * totalScale),
      height: Math.round(rect.height * totalScale),
    };
  }

  getMenuBounds() {
    const mainRect = this.container.getBoundingClientRect();
    let maxX = mainRect.right;
    let maxY = mainRect.bottom;

    document.querySelectorAll(".submenu-container.show").forEach((sub) => {
      const rect = sub.getBoundingClientRect();
      maxX = Math.max(maxX, rect.right);
      maxY = Math.max(maxY, rect.bottom);
    });

    return {
      width: Math.ceil(maxX + LAYOUT.resizePadding),
      height: Math.ceil(maxY + LAYOUT.resizePadding),
      ...this.getWindowPhysicalOrigin(),
    };
  }
}

class RegionReporter {
  constructor(container, layoutManager) {
    this.container = container;
    this.layoutManager = layoutManager;
  }

  async resizeWindowToFitMenu() {
    const bounds = this.layoutManager.getMenuBounds();
    await invoke("resize_context_menu", {
      width: bounds.width,
      height: bounds.height,
      x: bounds.x,
      y: bounds.y,
    }).catch(() => {});
  }

  async sendRegions() {
    const mainMenu = this.layoutManager.toPhysicalRegion(
      this.container.getBoundingClientRect(),
    );
    const submenus = Array.from(
      document.querySelectorAll(".submenu-container.show"),
    ).map((s) =>
      this.layoutManager.toPhysicalRegion(s.getBoundingClientRect()),
    );

    await invoke("update_context_menu_regions", { mainMenu, submenus }).catch(
      () => {},
    );
  }

  async sync() {
    await this.resizeWindowToFitMenu();
    await this.sendRegions();
  }
}

class MenuRenderer {
  constructor(
    container,
    layoutManager,
    regionReporter,
    previewController,
    onSelect,
  ) {
    this.container = container;
    this.layoutManager = layoutManager;
    this.regionReporter = regionReporter;
    this.previewController = previewController;
    this.onSelect = onSelect;
  }

  render(items) {
    this.container.innerHTML = "";
    normalizeItems(items).forEach((item) => {
      this.container.appendChild(this.createNode(item));
    });
  }

  createNode(item) {
    if (item.type === "button_row" && Array.isArray(item.buttons)) {
      return this.createButtonRow(item);
    }

    if (item.type === "separator") {
      const separator = document.createElement("div");
      separator.className = "menu-separator";
      return separator;
    }

    return this.createMenuItem(item);
  }

  createButtonRow(item) {
    const row = document.createElement("div");
    row.className = "menu-button-row";

    item.buttons.slice(0, 3).forEach((btn) => {
      const el = document.createElement("button");
      el.type = "button";
      el.className = "menu-button-row-btn";
      el.dataset.itemId = btn.id;
      el.disabled = !!btn.disabled;
      el.title = btn.label || "";
      el.setAttribute("aria-label", btn.label || "");

      this.appendIcon(el, btn, "menu-button-row-btn-icon");

      el.addEventListener("click", (e) => {
        e.stopPropagation();
        if (btn.disabled) return;
        this.onSelect(btn.id);
      });

      row.appendChild(el);
    });

    return row;
  }

  createMenuItem(item) {
    const menuItem = document.createElement("div");
    menuItem.className = "menu-item";
    if (item.disabled) menuItem.classList.add("disabled");
    menuItem.dataset.itemId = item.id;

    const hasChildren = item.children?.length > 0;
    if (hasChildren) menuItem.classList.add("has-submenu");

    this.appendIconOrPlaceholder(menuItem, item);
    this.appendLabel(menuItem, item);

    if (hasChildren) {
      this.attachSubmenu(menuItem, item);
    } else if (!item.disabled) {
      menuItem.addEventListener("click", (e) => {
        e.stopPropagation();
        this.onSelect(item.id);
      });
    }

    this.previewController.bind(menuItem, item);
    return menuItem;
  }

  appendIcon(parent, item, className) {
    if (item.favicon) {
      const img = document.createElement("img");
      img.src = item.favicon;
      img.className = className;
      parent.appendChild(img);
      return true;
    }

    if (item.icon) {
      const icon = document.createElement("i");
      icon.className = `${className} ${item.icon}`;
      if (item.iconColor) icon.style.color = item.iconColor;
      parent.appendChild(icon);
      return true;
    }

    return false;
  }

  appendIconOrPlaceholder(parent, item) {
    if (item.favicon) {
      const iconContainer = document.createElement("div");
      iconContainer.className = "menu-item-icon";
      const img = document.createElement("img");
      img.src = item.favicon;
      img.style.cssText = "width:16px;height:16px;object-fit:contain";
      iconContainer.appendChild(img);
      parent.appendChild(iconContainer);
      return;
    }

    if (item.icon) {
      const icon = document.createElement("i");
      icon.className = `menu-item-icon ${item.icon}`;
      if (item.iconColor) icon.style.color = item.iconColor;
      parent.appendChild(icon);
      return;
    }

    const placeholder = document.createElement("div");
    placeholder.className = "menu-item-icon";
    parent.appendChild(placeholder);
  }

  appendLabel(parent, item) {
    const label = document.createElement("div");
    label.className = "menu-item-label";
    label.textContent = item.label || "";
    label.style.cssText =
      "max-width:200px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap";
    parent.appendChild(label);
  }

  createSubmenu(items) {
    const submenu = document.createElement("div");
    submenu.className = "submenu-container";
    submenu.style.maxWidth = `${LAYOUT.submenuMaxWidth}px`;
    normalizeItems(items).forEach((item) =>
      submenu.appendChild(this.createNode(item)),
    );
    submenu.addEventListener("scroll", () => updateScrollIndicator(submenu));
    return submenu;
  }

  attachSubmenu(menuItem, item) {
    const indicator = document.createElement("i");
    indicator.className = "menu-item-submenu-indicator ti ti-chevron-right";
    menuItem.appendChild(indicator);

    const submenu = this.createSubmenu(item.children);
    this.container.appendChild(submenu);
    menuItem.submenuElement = submenu;

    if (!item.disabled) {
      menuItem.addEventListener("click", (e) => {
        if (!e.target.classList.contains("menu-item-submenu-indicator")) {
          e.stopPropagation();
          this.onSelect(item.id);
        }
      });
    }

    let hideTimeout = null;
    const showSubmenu = async () => {
      document.querySelectorAll(".submenu-container.show").forEach((s) => {
        if (s !== submenu) s.classList.remove("show");
      });
      submenu.classList.add("show");
      this.layoutManager.positionSubmenu(submenu, menuItem);
      await this.regionReporter.sync();
    };

    const hideSubmenu = async () => {
      submenu.classList.remove("show");
      await this.regionReporter.sync();
    };

    menuItem.addEventListener("mouseenter", () => {
      if (item.disabled) return;
      if (hideTimeout) clearTimeout(hideTimeout);
      showSubmenu();
    });

    menuItem.addEventListener("mouseleave", (e) => {
      if (!submenu.contains(e.relatedTarget)) {
        hideTimeout = setTimeout(hideSubmenu, LAYOUT.submenuHideDelay);
      }
    });

    submenu.addEventListener("mouseenter", () => {
      if (hideTimeout) clearTimeout(hideTimeout);
    });

    submenu.addEventListener("mouseleave", (e) => {
      if (!menuItem.contains(e.relatedTarget)) {
        hideTimeout = setTimeout(hideSubmenu, LAYOUT.submenuHideDelay);
      }
    });

    submenu.addEventListener("click", (e) => e.stopPropagation());
  }
}

class MenuSession {
  constructor(container) {
    this.container = container;
    this.layoutManager = new LayoutManager(container);
    this.regionReporter = new RegionReporter(container, this.layoutManager);
    this.previewController = new PreviewController();
    this.renderer = new MenuRenderer(
      container,
      this.layoutManager,
      this.regionReporter,
      this.previewController,
      (itemId) => this.hide(itemId),
    );
    this.isClosing = false;
  }

  async loadAndRender() {
    this.isClosing = false;
    const options = await invoke("get_context_menu_options").catch(() => null);
    if (options) await this.render(options);
  }

  async render(options = {}) {
    await this.layoutManager.configure(options);
    themeController.apply(options.appearance);
    applyContainerLayout(this.container, options.layout);
    this.renderer.render(options.items);
    this.layoutManager.positionMainMenu(options);
    this.container.style.visibility = "visible";
    await this.regionReporter.sync();
  }

  async hide(itemId = null) {
    if (this.isClosing) return;
    this.isClosing = true;
    await invoke("submit_context_menu", { itemId: itemId || null }).catch(
      () => {},
    );
    document
      .querySelectorAll(".submenu-container")
      .forEach((s) => s.classList.remove("show"));
    this.container.style.visibility = "hidden";
    await currentWindow.hide();
  }
}

const session = new MenuSession(menuContainer);

session.loadAndRender();
currentWindow.listen("reload-menu", () => session.loadAndRender());
currentWindow.listen("close-context-menu", () => session.hide(null));
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") session.hide(null);
});
