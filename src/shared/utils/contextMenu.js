// 右键菜单工具函数
import {
  showContextMenu,
  createMenuPlacementFromEvent,
  createMenuItem,
  createSeparator,
} from "@/plugins/context_menu/index.js";
import { openUrl, openPath } from "@tauri-apps/plugin-opener";
import i18n from "@shared/i18n";
import { extractAllLinks, normalizeUrl } from "./linkUtils";
import { settingsStore } from "@shared/store/settingsStore";
import {
  toast,
  toastStore,
  TOAST_SIZES,
  TOAST_POSITIONS,
} from "@shared/store/toastStore";
import {
  addClipboardToFavorites,
  pinImageToScreen,
  clearClipboardHistory,
  moveFavoriteToGroup,
  deleteFavorite,
  saveImageFromPath,
  copyTextToClipboard,
  recognizeImageOcr,
  moveClipboardItemToTop,
  copyClipboardItem,
  getClipboardItemPasteOptions,
  listTransferShelves,
  addPathsToTransferShelf,
} from "@shared/api";
import { getFavoriteItemPasteOptions } from "@shared/api/favorites";
import { clipboardStore } from "@shared/store/clipboardStore";
import { getOneTimePasteEnabled } from "@shared/services/oneTimePaste";
const TOAST_CONFIG = {
  size: TOAST_SIZES.EXTRA_SMALL,
  position: TOAST_POSITIONS.BOTTOM_RIGHT,
};

// 获取搜索引擎列表
function getSearchEngines() {
  return [
    {
      id: "bing",
      name: "Bing",
      favicon: "https://www.bing.com/favicon.ico",
      url: "https://www.bing.com/search?q=",
    },
    {
      id: "baidu",
      name: "百度",
      favicon: "https://www.baidu.com/favicon.ico",
      url: "https://www.baidu.com/s?wd=",
    },
    {
      id: "google",
      name: "Google",
      favicon: "https://www.google.com/favicon.ico",
      url: "https://www.google.com/search?q=",
    },
  ];
}

// 获取当前搜索引擎
function getCurrentSearchEngine() {
  const engines = getSearchEngines();
  const savedEngineId = localStorage.getItem("current-search-engine");
  return (
    engines.find((e) => e.id === savedEngineId) ||
    engines.find((e) => e.id === "bing") ||
    engines[0]
  );
}

// 设置当前搜索引擎
function setCurrentSearchEngine(engineId) {
  localStorage.setItem("current-search-engine", engineId);
}

// 在浏览器中搜索文本
async function searchTextInBrowser(text, engineId = null) {
  try {
    const engine = engineId
      ? getSearchEngines().find((e) => e.id === engineId)
      : getCurrentSearchEngine();
    if (!engine) return;
    const url = engine.url + encodeURIComponent(text);
    await openUrl(url);
    if (engineId) {
      setCurrentSearchEngine(engineId);
    }
  } catch (error) {
    console.error("搜索失败:", error);
  }
}

// 打开链接
async function openLink(url) {
  try {
    const normalizedUrl = normalizeUrl(url);
    await openUrl(normalizedUrl);
  } catch (error) {
    console.error("打开链接失败:", error);
  }
}

// 创建链接菜单项
function createLinkMenuItems(item) {
  const links = extractAllLinks({
    content: item.content,
    html_content: item.html_content,
  });
  if (links.length === 0)
    return {
      menuItems: [],
      links,
    };
  const menuItems = [];
  if (links.length === 1) {
    menuItems.push(
      createMenuItem({
        id: "open-link",
        label: i18n.t("contextMenu.openInBrowser"),
        icon: "ti ti-external-link",
      }),
    );
  } else {
    const linkMenuItem = createMenuItem({
      id: "open-links",
      label: i18n.t("contextMenu.openLinks", {
        count: links.length,
      }),
      icon: "ti ti-external-link",
    });
    linkMenuItem.children = [
      ...links.map((link, idx) => {
        const displayText =
          link.length > 50 ? link.substring(0, 50) + "..." : link;
        return createMenuItem({
          id: `open-link-${idx}`,
          label: displayText,
          icon: "ti ti-link",
        });
      }),
      createSeparator(),
      createMenuItem({
        id: "open-all-links",
        label: i18n.t("contextMenu.openAll"),
        icon: "ti ti-external-link",
      }),
    ];
    menuItems.push(linkMenuItem);
  }
  return {
    menuItems,
    links,
  };
}

// 创建搜索菜单项
function createSearchMenuItems(plainText, contentType) {
  if (
    !plainText ||
    contentType.includes("image") ||
    contentType.includes("file")
  ) {
    return [];
  }
  const searchEngines = getSearchEngines();
  const currentEngine = getCurrentSearchEngine();
  if (!currentEngine || searchEngines.length === 0) {
    return [];
  }
  const searchMenuItem = createMenuItem({
    id: "search-current",
    label: i18n.t("contextMenu.searchWith", {
      engine: currentEngine.name,
    }),
    favicon: currentEngine.favicon,
  });
  searchMenuItem.children = searchEngines.map((engine) =>
    createMenuItem({
      id: `search-${engine.id}`,
      label: engine.name,
      favicon: engine.favicon,
      icon: engine.id === currentEngine.id ? "ti ti-check" : undefined,
    }),
  );
  return [searchMenuItem];
}

// 创建粘贴菜单项
function createPasteMenuItem(pasteOptions = []) {
  const pasteMenuItem = createMenuItem({
    id: "paste",
    label: i18n.t("contextMenu.paste"),
    icon: "ti ti-clipboard",
  });
  if (!Array.isArray(pasteOptions) || pasteOptions.length <= 1) {
    return pasteMenuItem;
  }
  pasteMenuItem.children = pasteOptions.map((option, index) =>
    createMenuItem({
      id: `paste-option-${index}`,
      label: getPasteOptionLabel(option),
      icon: getPasteOptionIcon(option),
    }),
  );
  return pasteMenuItem;
}
function resolvePasteAction(result, pasteOptions = []) {
  if (result === "paste") {
    return null;
  }
  if (result.startsWith("paste-option-")) {
    const index = Number.parseInt(result.substring("paste-option-".length), 10);
    if (Number.isInteger(index) && index >= 0 && index < pasteOptions.length) {
      return pasteOptions[index].id;
    }
    return null;
  }
  return undefined;
}
function getPasteOptionIcon(option) {
  switch (option?.kind) {
    case "image_bundle":
      return "ti ti-photo";
    case "file":
      return "ti ti-files";
    case "plain_text":
      return "ti ti-text-size";
    case "html":
    case "rtf":
      return "ti ti-typography";
    case "all_formats":
      return "ti ti-stack";
    default:
      return "ti ti-clipboard";
  }
}
function getPasteOptionLabel(option) {
  const sourceFormatName = option?.source_format_name;
  switch (option?.kind) {
    case "all_formats":
      return i18n.t("contextMenu.pasteAllFormats");
    case "plain_text":
      return i18n.t("contextMenu.pastePlainText");
    case "html":
    case "rtf":
      return sourceFormatName
        ? `${i18n.t("contextMenu.formatRichText")} (${sourceFormatName})`
        : i18n.t("contextMenu.formatRichText");
    case "image_bundle":
      return i18n.t("contextMenu.formatImage");
    case "file":
      return i18n.t("contextMenu.formatFile");
    default:
      return i18n.t("contextMenu.paste");
  }
}
function isFileOrImageContentType(contentType) {
  const value = String(contentType || "").toLowerCase();
  return value.includes("file") || value.includes("image");
}
async function getTransferShelvesForContentType(contentType) {
  if (!isFileOrImageContentType(contentType)) return [];
  return await listTransferShelves().catch(() => []);
}
function createTransferShelfMenuItem(transferShelves = []) {
  const shelves = Array.isArray(transferShelves) ? transferShelves : [];
  const item = createMenuItem({
    id: "add-to-transfer-shelf",
    label: i18n.t("contextMenu.addToTransferShelf"),
    icon: "ti ti-package",
    disabled: shelves.length === 0,
  });
  if (shelves.length > 0) {
    item.children = shelves.map((shelf) =>
      createMenuItem({
        id: `add-to-transfer-shelf-${shelf.id}`,
        label: shelf.name || i18n.t("transferShelf.defaultName"),
        icon: "ti ti-package",
      }),
    );
  }
  return item;
}
function createContentTypeMenuItems(contentType, transferShelves = []) {
  const value = String(contentType || "").toLowerCase();
  const transferShelfItem = createTransferShelfMenuItem(transferShelves);
  if (value.includes("image")) {
    return [
      createMenuItem({
        id: "open-file",
        label: i18n.t("contextMenu.openWithDefault"),
        icon: "ti ti-external-link",
      }),
      createMenuItem({
        id: "pin-image",
        label: i18n.t("contextMenu.pinToScreen"),
        icon: "ti ti-window-maximize",
      }),
      createMenuItem({
        id: "save-image",
        label: i18n.t("contextMenu.saveImage"),
        icon: "ti ti-download",
      }),
      createMenuItem({
        id: "extract-text",
        label: i18n.t("contextMenu.extractText"),
        icon: "ti ti-text-scan-2",
      }),
      transferShelfItem,
    ];
  }
  if (value.includes("file")) {
    return [
      createMenuItem({
        id: "open-file",
        label: i18n.t("contextMenu.openWithDefault"),
        icon: "ti ti-external-link",
      }),
      createMenuItem({
        id: "open-location",
        label: i18n.t("contextMenu.openLocation"),
        icon: "ti ti-folder-open",
      }),
      createMenuItem({
        id: "copy-path",
        label: i18n.t("contextMenu.copyPath"),
        icon: "ti ti-copy",
      }),
      transferShelfItem,
    ];
  }
  const isRichText = contentType.includes("rich_text");
  return [
    createMenuItem({
      id: "edit-text",
      label: isRichText
        ? i18n.t("contextMenu.editPlainText")
        : i18n.t("contextMenu.editText"),
      icon: "ti ti-edit",
    }),
  ];
}
async function resolveItemFilePaths(item) {
  if (typeof item.content !== "string" || !item.content.startsWith("files:"))
    return [];
  try {
    const filesData = JSON.parse(item.content.substring(6));
    const files = Array.isArray(filesData?.files) ? filesData.files : [];
    if (files.length === 0) return [];
    const { invoke } = await import("@tauri-apps/api/core");
    const paths = [];
    for (const file of files) {
      const actualPath =
        typeof file?.actual_path === "string" ? file.actual_path.trim() : "";
      if (actualPath) {
        paths.push(actualPath);
        continue;
      }
      const storedPath = typeof file?.path === "string" ? file.path.trim() : "";
      if (!storedPath) continue;
      try {
        paths.push(
          await invoke("resolve_image_path", {
            storedPath,
          }),
        );
      } catch (_) {
        paths.push(storedPath);
      }
    }
    return [...new Set(paths.filter(Boolean))];
  } catch (_) {
    return [];
  }
}

// 处理链接相关操作
async function handleLinkActions(result, links) {
  if (result === "open-link" && links.length === 1) {
    await openLink(links[0]);
    return true;
  }
  if (result.startsWith("open-link-")) {
    const linkIndex = parseInt(result.substring(10));
    if (linkIndex >= 0 && linkIndex < links.length) {
      await openLink(links[linkIndex]);
    }
    return true;
  }
  if (result === "open-all-links") {
    for (const link of links) {
      await openLink(link);
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    return true;
  }
  return false;
}

// 处理搜索相关操作
async function handleSearchActions(result, plainText) {
  if (result === "search-current") {
    await searchTextInBrowser(plainText);
    return true;
  }
  if (result.startsWith("search-")) {
    const engineId = result.substring(7);
    await searchTextInBrowser(plainText, engineId);
    return true;
  }
  return false;
}

// 处理粘贴操作
async function handlePasteActions(
  result,
  item,
  pasteOptions = [],
  isClipboard = true,
) {
  const action = resolvePasteAction(result, pasteOptions);
  if (action === undefined) return false;
  if (isClipboard) {
    const { pasteClipboardItem } = await import("@shared/api/clipboard");
    await pasteClipboardItem(item.id, action);
  } else {
    const { pasteFavorite } = await import("@shared/api/favorites");
    await pasteFavorite(item.id, action);
  }

  // 粘贴后置顶
  if (isClipboard) {
    if (
      !getOneTimePasteEnabled() &&
      settingsStore.pasteToTop &&
      item.id &&
      !item.is_pinned
    ) {
      try {
        await moveClipboardItemToTop(item.id);
      } finally {
        clipboardStore.items = {};
      }
    }
  }
  return true;
}

// 处理内容类型操作
async function handleContentTypeActions(result, item, index) {
  if (result === "edit-text") {
    const { openEditorForClipboard } = await import("@shared/api/textEditor");
    await openEditorForClipboard(item, index);
    return true;
  }
  if (result === "edit-item") {
    const { openEditorForFavorite } = await import("@shared/api/textEditor");
    await openEditorForFavorite(item);
    return true;
  }
  const contentType = item.content_type || "text";
  if (!isFileOrImageContentType(contentType)) return false;
  const filePaths = await resolveItemFilePaths(item);
  if (filePaths.length === 0) return false;
  const filePath = filePaths[0];
  const dirPath = filePath.substring(
    0,
    Math.max(filePath.lastIndexOf("\\"), filePath.lastIndexOf("/")),
  );
  const actions = {
    "add-to-transfer-shelf": async () => {},
    "pin-image": async () => {
      try {
        await pinImageToScreen(filePath);
        toast.success(i18n.t("contextMenu.imagePinned"), TOAST_CONFIG);
      } catch (error) {
        console.error("贴图到屏幕失败:", error);
        toast.error(i18n.t("common.operationFailed"), TOAST_CONFIG);
      }
    },
    "save-image": async () => {
      await saveImageFromPath(filePath);
      toast.success(i18n.t("contextMenu.imageSaved"), TOAST_CONFIG);
    },
    "extract-text": async () => {
      const loadingToastId = toast.info(i18n.t("contextMenu.extractingText"), {
        duration: 0,
        ...TOAST_CONFIG,
      });
      try {
        const result = await recognizeImageOcr(filePath);
        toastStore.removeToast(loadingToastId);
        if (result.text && result.text.trim()) {
          await copyTextToClipboard(result.text);
          toast.success(i18n.t("contextMenu.textExtracted"), TOAST_CONFIG);
        } else {
          toast.error(i18n.t("contextMenu.extractTextFailed"), TOAST_CONFIG);
        }
      } catch (error) {
        console.error("OCR识别失败:", error);
        toastStore.removeToast(loadingToastId);
        toast.error(i18n.t("contextMenu.extractTextFailed"), TOAST_CONFIG);
      }
    },
    "open-file": async () => {
      try {
        await openPath(filePath);
        toast.success(i18n.t("contextMenu.fileOpened"), TOAST_CONFIG);
      } catch (error) {
        console.error("打开文件失败:", error);
        toast.error(i18n.t("common.operationFailed"), TOAST_CONFIG);
      }
    },
    "open-location": async () => {
      try {
        await openPath(dirPath);
        toast.success(i18n.t("contextMenu.locationOpened"), TOAST_CONFIG);
      } catch (error) {
        console.error("打开文件位置失败:", error);
        toast.error(i18n.t("common.operationFailed"), TOAST_CONFIG);
      }
    },
    "copy-path": async () => {
      try {
        await copyTextToClipboard(filePath);
        toast.success(i18n.t("contextMenu.pathCopied"), TOAST_CONFIG);
      } catch (error) {
        console.error("复制文件路径失败:", error);
        toast.error(i18n.t("common.operationFailed"), TOAST_CONFIG);
      }
    },
  };
  if (result.startsWith("add-to-transfer-shelf-")) {
    const shelfId = result.substring("add-to-transfer-shelf-".length);
    await addPathsToTransferShelf(shelfId, filePaths);
    toast.success(i18n.t("contextMenu.addedToTransferShelf"), TOAST_CONFIG);
    return true;
  }
  if (actions[result]) {
    await actions[result]();
    return true;
  }
  return false;
}

// 显示剪贴板项的右键菜单
export async function showClipboardItemContextMenu(event, item, index) {
  const menuItems = [];
  const contentType = item.content_type || "text";
  const plainText = typeof item.content === "string" ? item.content.trim() : "";
  const ct = String(contentType || "")
    .trim()
    .toLowerCase();
  const pasteOptions = await getClipboardItemPasteOptions(item.id).catch(
    () => [],
  );
  const transferShelves = await getTransferShelvesForContentType(contentType);
  const pasteMenuItem = createPasteMenuItem(pasteOptions);
  menuItems.push(pasteMenuItem);
  menuItems.push(
    createMenuItem({
      id: "copy-item",
      label: i18n.t("contextMenu.copy"),
      icon: "ti ti-copy",
    }),
  );
  menuItems.push(createSeparator());
  const { menuItems: linkMenuItems, links } = createLinkMenuItems(item);
  if (linkMenuItems.length > 0) {
    menuItems.push(...linkMenuItems, createSeparator());
  }
  const searchMenuItems = createSearchMenuItems(plainText, contentType);
  if (searchMenuItems.length > 0) {
    menuItems.push(...searchMenuItems, createSeparator());
  }
  const contentMenuItems = createContentTypeMenuItems(
    contentType,
    transferShelves,
  );
  if (contentMenuItems.length > 0) {
    menuItems.push(...contentMenuItems);
  }

  // 添加分隔线
  if (menuItems.length > 0 && menuItems[menuItems.length - 1].type !== "separator") {
    menuItems.push(createSeparator());
  }

  // 添加"添加到收藏"菜单
  const { groupsStore } = await import("@shared/store/groupsStore");
  const groups = groupsStore.groups || [];
  const addToFavoritesItem = createMenuItem({
    id: "add-to-favorites",
    label: i18n.t("contextMenu.addToFavorites"),
    icon: "ti ti-star",
  });
  if (groups.length > 0) {
    addToFavoritesItem.children = groups.map((group) =>
      createMenuItem({
        id: `add-to-group-${group.name}`,
        label: group.name,
        icon: group.icon || "ti ti-folder",
        iconColor: group.name === "全部" ? null : group.color || "#dc2626",
      }),
    );
  }

  // 添加通用菜单项
  menuItems.push(
    addToFavoritesItem,
    createMenuItem({
      id: "delete-item",
      label: i18n.t("contextMenu.deleteItem"),
      icon: "ti ti-trash",
    }),
    createSeparator(),
    createMenuItem({
      id: "clear-all",
      label: i18n.t("contextMenu.clearAll"),
      icon: "ti ti-trash-x",
    }),
  );

  // 显示菜单并处理结果
  const result = await showContextMenu({
    items: menuItems,
    placement: createMenuPlacementFromEvent(event),
    appearance: {
      theme: settingsStore.theme,
      lightThemeStyle: settingsStore.lightThemeStyle,
      darkThemeStyle: settingsStore.darkThemeStyle,
      uiAnimationEnabled: settingsStore.uiAnimationEnabled,
    },
  });
  if (!result) return;
  try {
    // 处理复制操作
    if (result === "copy-item") {
      await copyClipboardItem(item.id);
      toast.success(i18n.t("contextMenu.copied"), TOAST_CONFIG);
      return;
    }

    // 处理粘贴操作
    if (await handlePasteActions(result, item, pasteOptions, true)) return;

    // 处理链接操作
    if (await handleLinkActions(result, links)) {
      toast.success(i18n.t("contextMenu.linkOpened"), TOAST_CONFIG);
      return;
    }

    // 处理搜索操作
    if (await handleSearchActions(result, plainText)) {
      toast.success(i18n.t("contextMenu.searchOpened"), TOAST_CONFIG);
      return;
    }

    // 处理添加到收藏
    if (result.startsWith("add-to-group-")) {
      const groupName = result.substring(13);
      await addClipboardToFavorites(item.id, groupName);
      toast.success(i18n.t("contextMenu.addedToFavorites"), TOAST_CONFIG);
      return;
    }
    if (result === "add-to-favorites") {
      await addClipboardToFavorites(item.id);
      toast.success(i18n.t("contextMenu.addedToFavorites"), TOAST_CONFIG);
      return;
    }

    // 处理内容类型操作
    if (await handleContentTypeActions(result, item, index)) return;

    // 处理其他操作
    switch (result) {
      case "delete-item":
        const { deleteClipboardItem } =
          await import("@shared/store/clipboardStore");
        await deleteClipboardItem(item.id);
        toast.success(i18n.t("common.deleted"), TOAST_CONFIG);
        break;
      case "clear-all":
        const { showConfirm } = await import("@shared/utils/dialog");
        const confirmed = await showConfirm(
          i18n.t("contextMenu.clearAllConfirm"),
          i18n.t("contextMenu.clearAllConfirmTitle"),
        );
        if (confirmed) {
          await clearClipboardHistory();
          const { loadClipboardItems } =
            await import("@shared/store/clipboardStore");
          await loadClipboardItems();
          toast.success(i18n.t("contextMenu.allCleared"), TOAST_CONFIG);
        }
        break;
    }
  } catch (error) {
    console.error("处理菜单操作失败:", error);
    toast.error(i18n.t("common.operationFailed"), TOAST_CONFIG);
  }
}

// 显示收藏项的右键菜单
export async function showFavoriteItemContextMenu(event, item, index) {
  const menuItems = [];
  const contentType = item.content_type || "text";
  const ct = String(contentType || "")
    .trim()
    .toLowerCase();
  const isFileType = ct === "file" || ct.startsWith("file/");
  const isImageType = ct === "image" || ct.startsWith("image/");
  const pasteOptions = await getFavoriteItemPasteOptions(item.id).catch(
    () => [],
  );
  const transferShelves = await getTransferShelvesForContentType(contentType);
  const pasteMenuItem = createPasteMenuItem(pasteOptions);
  menuItems.push(pasteMenuItem);
  menuItems.push(
    createMenuItem({
      id: "copy-item",
      label: i18n.t("contextMenu.copy"),
      icon: "ti ti-copy",
    }),
  );
  menuItems.push(createSeparator());

  // 添加链接菜单
  const { menuItems: linkMenuItems, links } = createLinkMenuItems(item);
  if (linkMenuItems.length > 0) {
    menuItems.push(...linkMenuItems, createSeparator());
  }

  // 添加内容类型特定菜单项（图片、文件等）
  const contentMenuItems = createContentTypeMenuItems(
    contentType,
    transferShelves,
  );
  if (contentMenuItems.length > 0) {
    menuItems.push(...contentMenuItems, createSeparator());
  }

  // 添加"移动到分组"菜单
  const { groupsStore } = await import("@shared/store/groupsStore");
  const groups = groupsStore.groups || [];
  const moveToGroupItem = createMenuItem({
    id: "move-to-group",
    label: i18n.t("contextMenu.moveToGroup"),
    icon: "ti ti-folder",
  });
  if (groups.length > 0) {
    moveToGroupItem.children = groups
      .filter((group) => group.name !== item.group_name)
      .map((group) =>
        createMenuItem({
          id: `move-to-group-${group.name}`,
          label: group.name,
          icon: group.icon || "ti ti-folder",
          iconColor: group.name === "全部" ? null : group.color || "#dc2626",
        }),
      );
  }

  // 添加通用菜单项
  menuItems.push(
    moveToGroupItem,
    createSeparator(),
    createMenuItem({
      id: "delete-item",
      label: i18n.t("contextMenu.delete"),
      icon: "ti ti-trash",
    }),
  );
  const result = await showContextMenu({
    items: menuItems,
    placement: createMenuPlacementFromEvent(event),
    appearance: {
      theme: settingsStore.theme,
      lightThemeStyle: settingsStore.lightThemeStyle,
      darkThemeStyle: settingsStore.darkThemeStyle,
      uiAnimationEnabled: settingsStore.uiAnimationEnabled,
    },
  });
  if (!result) return;
  try {
    // 处理复制操作
    if (result === "copy-item") {
      const { copyFavoriteItem } = await import("@shared/api/favorites");
      await copyFavoriteItem(item.id);
      toast.success(i18n.t("contextMenu.copied"), TOAST_CONFIG);
      return;
    }

    // 处理粘贴操作
    if (await handlePasteActions(result, item, pasteOptions, false)) return;
    if (await handleLinkActions(result, links)) {
      toast.success(i18n.t("contextMenu.linkOpened"), TOAST_CONFIG);
      return;
    }
    if (result === "edit-text") {
      const { openEditorForFavorite } = await import("@shared/api/textEditor");
      await openEditorForFavorite(item);
      return;
    }

    // 处理移动到分组
    if (result.startsWith("move-to-group-")) {
      const groupName = result.substring(14);
      await moveFavoriteToGroup(item.id, groupName);
      const { refreshFavorites } = await import("@shared/store/favoritesStore");
      await refreshFavorites();
      toast.success(i18n.t("contextMenu.movedToGroup"), TOAST_CONFIG);
      return;
    }

    // 处理内容类型操作
    if (await handleContentTypeActions(result, item, index)) return;

    // 处理删除操作
    if (result === "delete-item") {
      const { showConfirm } = await import("@shared/utils/dialog");
      const confirmed = await showConfirm(
        i18n.t("favorites.confirmDelete"),
        i18n.t("favorites.confirmDeleteTitle"),
      );
      if (confirmed) {
        await deleteFavorite(item.id);
        const { refreshFavorites } =
          await import("@shared/store/favoritesStore");
        await refreshFavorites();
        toast.success(i18n.t("common.deleted"), TOAST_CONFIG);
      }
    }
  } catch (error) {
    console.error("处理菜单操作失败:", error);
    toast.error(i18n.t("common.operationFailed"), TOAST_CONFIG);
  }
}
