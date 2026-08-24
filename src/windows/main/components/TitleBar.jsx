import "@tabler/icons-webfont/dist/tabler-icons.min.css";
import { useTranslation } from "react-i18next";
import {
  useEffect,
  useRef,
  useState,
  forwardRef,
  useImperativeHandle,
} from "react";
import { useSnapshot } from "valtio";
import { useWindowDrag } from "@shared/hooks/useWindowDrag";
import {
  toggleWindowPin,
  getWindowPinState,
  openAppSettings,
} from "@shared/services/titleBarActions";
import { clipboardStore } from "@shared/store/clipboardStore";
import { favoritesStore } from "@shared/store/favoritesStore";
import { settingsStore } from "@shared/store/settingsStore";
import { navigationStore } from "@shared/store/navigationStore";
import {
  showContextMenu,
  createMenuPlacementFromEvent,
  createMenuItem,
  createSeparator,
} from "@/plugins/context_menu/index.js";
import { invoke } from "@tauri-apps/api/core";
import { clearClipboardHistory } from "@shared/api";
import { createTransferShelf } from "@shared/api/transferShelf";
import { openReceiveBox } from "@shared/api/receiveBox";
import { downloadWebdav, uploadWebdav } from "@shared/api/webdavSync";
import { toast, TOAST_SIZES, TOAST_POSITIONS } from "@shared/store/toastStore";
import {
  getOneTimePasteEnabled,
  toggleOneTimePasteEnabled,
  getOneTimePasteEventName,
} from "@shared/services/oneTimePaste";
import { normalizeDisplayPriorityValue } from "@shared/utils/displayFormatPriority";
import { formatUserMessage } from "@shared/utils/userMessages";
import logoIcon from "@/assets/icon32.png";
import TitleBarSearch from "./TitleBarSearch";
import Tooltip from "@shared/components/common/Tooltip.jsx";
const ACTIVE_ICON_BUTTON_CLASS =
  "bg-blue-500 bg-dynamic-primary text-white hover:bg-blue-600";
const TITLE_BAR_IMAGE_ICON_STYLE = {
  imageRendering: "pixelated",
};
const TITLE_BAR_FONT_ICON_STYLE = {
  WebkitFontSmoothing: "none",
};
const TITLE_BAR_BUTTON_CLASS =
  "w-7 h-7 flex items-center justify-center rounded-lg transition-all duration-200";
const TITLE_BAR_ICON_STYLE = {
  fontSize: 16,
  ...TITLE_BAR_FONT_ICON_STYLE,
};
const TITLE_BAR_GRID_BUTTON_CLASS =
  "flex h-full w-full items-center justify-center transition-all duration-200";
const TITLE_BAR_GRID_ICON_STYLE = {
  fontSize: 14,
  ...TITLE_BAR_FONT_ICON_STYLE,
};
const TOAST_CONFIG = {
  size: TOAST_SIZES.EXTRA_SMALL,
  position: TOAST_POSITIONS.BOTTOM_RIGHT,
};
const TitleBar = forwardRef(
  (
    {
      searchQuery,
      onSearchChange,
      searchPlaceholder,
      position = "top",
      activeTab = "clipboard",
      updateBannerState = null,
      compactActions = false,
    },
    ref,
  ) => {
    const { t } = useTranslation();
    const clipboardSnap = useSnapshot(clipboardStore);
    const favoritesSnap = useSnapshot(favoritesStore);
    const settingsSnap = useSnapshot(settingsStore);
    const navigationSnap = useSnapshot(navigationStore);
    const searchRef = useRef(null);
    const [isPinned, setIsPinned] = useState(() =>
      Boolean(getWindowPinState()),
    );
    const [oneTimePasteEnabled, setOneTimePasteEnabledState] = useState(() =>
      getOneTimePasteEnabled(),
    );
    const [webdavBusy, setWebdavBusy] = useState("");
    const isVertical = position === "left" || position === "right";
    const isCompactActions = compactActions && !isVertical;
    const actionContainerClass = isCompactActions
      ? "grid grid-cols-2 grid-rows-2 flex-shrink-0 overflow-hidden rounded-md border border-qc-border h-8 w-14"
      : `flex flex-shrink-0 ${isVertical ? "flex-col items-center" : "items-center"} gap-1`;
    const actionButtonClass = isCompactActions
      ? TITLE_BAR_GRID_BUTTON_CLASS
      : TITLE_BAR_BUTTON_CLASS;
    const actionIconStyle = isCompactActions
      ? TITLE_BAR_GRID_ICON_STYLE
      : TITLE_BAR_ICON_STYLE;
    const tooltipPlacement = isVertical
      ? position === "left"
        ? "right"
        : "left"
      : "bottom";
    const showUpdateHint = Boolean(
      settingsSnap.disableUpdatePopup === true &&
      updateBannerState?.currentVersion &&
      updateBannerState?.latestVersion,
    );
    const currentStore =
      activeTab === "clipboard"
        ? clipboardStore
        : activeTab === "favorites"
          ? favoritesStore
          : null;
    const isMultiSelectMode =
      activeTab === "clipboard"
        ? clipboardSnap.isMultiSelectMode
        : activeTab === "favorites"
          ? favoritesSnap.isMultiSelectMode
          : false;
    const dragRef = useWindowDrag({
      excludeSelectors: [
        "[data-no-drag]",
        "button",
        '[role="button"]',
        "input",
        "textarea",
      ],
      allowChildren: true,
    });
    useEffect(() => {
      const handlePinStateChanged = (event) => {
        const pinned = Boolean(event?.detail?.pinned);
        setIsPinned(pinned);
      };
      window.addEventListener(
        "window-pin-state-changed",
        handlePinStateChanged,
      );
      return () => {
        window.removeEventListener(
          "window-pin-state-changed",
          handlePinStateChanged,
        );
      };
    }, []);
    useEffect(() => {
      const syncState = () => {
        setOneTimePasteEnabledState(getOneTimePasteEnabled());
      };
      const eventName = getOneTimePasteEventName();
      window.addEventListener(eventName, syncState);
      return () => {
        window.removeEventListener(eventName, syncState);
      };
    }, []);
    const handleTogglePin = async (event) => {
      event.preventDefault();
      event.stopPropagation();
      try {
        const result = await toggleWindowPin();
        setIsPinned(Boolean(result));
      } catch (error) {
        console.error("标题栏固定窗口失败:", error);
      }
    };
    const handleToggleEdgeHide = async (event) => {
      event.preventDefault();
      event.stopPropagation();
      const previousValue = Boolean(settingsStore.edgeHideEnabled);
      const nextValue = !previousValue;
      try {
        const result = await settingsStore.saveSetting(
          "edgeHideEnabled",
          nextValue,
        );
        if (result?.success === false) {
          settingsStore.edgeHideEnabled = previousValue;
        }
      } catch (error) {
        settingsStore.edgeHideEnabled = previousValue;
        console.error("标题栏切换贴边隐藏失败:", error);
      }
    };
    const handleOpenSettings = async (event) => {
      event.preventDefault();
      event.stopPropagation();
      try {
        await openAppSettings();
      } catch (error) {
        console.error("标题栏打开设置失败:", error);
      }
    };
    const handleOpenUpdater = async (event) => {
      event.preventDefault();
      event.stopPropagation();
      try {
        const opened = await invoke("open_cached_update_window");
        if (!opened) {
          await invoke("check_updates_and_open_window");
        }
      } catch (error) {
        console.error("标题栏打开更新窗口失败:", error);
        toast.error(
          t("updater.checkFailed", {
            msg: formatUserMessage(error, t, "errors.operationFailed"),
          }),
          TOAST_CONFIG,
        );
      }
    };
    const handleOpenTransferShelf = async (event) => {
      event.preventDefault();
      event.stopPropagation();
      try {
        await createTransferShelf();
      } catch (error) {
        console.error("标题栏新建文件盒失败:", error);
        toast.error(
          t("transferShelf.createFailed", {
            reason: formatUserMessage(error, t, "errors.operationFailed"),
          }),
          TOAST_CONFIG,
        );
      }
    };
    const handleOpenReceiveBox = async (event) => {
      event.preventDefault();
      event.stopPropagation();
      try {
        await openReceiveBox();
      } catch (error) {
        console.error("标题栏打开收件盒失败:", error);
        toast.error(
          t("receiveBox.openFailed", {
            reason: formatUserMessage(error, t, "errors.operationFailed"),
          }),
          TOAST_CONFIG,
        );
      }
    };
    const formatWebdavReport = (result, mode) => {
      const total = mode === "push" ? result?.pushed || 0 : result?.pulled || 0;
      const clipboard =
        mode === "push"
          ? result?.pushed_clipboard || 0
          : result?.pulled_clipboard || 0;
      const favorites =
        mode === "push"
          ? result?.pushed_favorites || 0
          : result?.pulled_favorites || 0;
      const groups =
        mode === "push"
          ? result?.pushed_groups || 0
          : result?.pulled_groups || 0;
      return t("settings.webdav.syncResultDetail", {
        total,
        clipboard,
        favorites,
        groups,
      });
    };
    const handleWebdavAction = async (event, mode) => {
      event.preventDefault();
      event.stopPropagation();
      if (webdavBusy) {
        return;
      }
      if (
        !settingsSnap.webdavEnabled ||
        !String(settingsSnap.webdavUrl || "").trim()
      ) {
        toast.warning(t("settings.webdav.notReady"), TOAST_CONFIG);
        return;
      }
      const action = mode === "push" ? uploadWebdav : downloadWebdav;
      const successKey =
        mode === "push"
          ? "settings.webdav.pushComplete"
          : "settings.webdav.pullComplete";
      try {
        setWebdavBusy(mode);
        const result = await action();
        toast.success(
          t("settings.webdav.successWithDetail", {
            title: t(successKey),
            detail: formatWebdavReport(result, mode),
          }),
          {
            ...TOAST_CONFIG,
            duration: 5000,
          },
        );
      } catch (error) {
        console.error(
          `标题栏 WebDAV ${mode === "push" ? "推送" : "拉取"}失败:`,
          error,
        );
        toast.error(
          formatUserMessage(error, t, "errors.webdav.operationFailed"),
          {
            ...TOAST_CONFIG,
            duration: 6000,
          },
        );
      } finally {
        setWebdavBusy("");
      }
    };
    const handleToggleMultiSelect = (event) => {
      event.preventDefault();
      event.stopPropagation();
      if (!currentStore) {
        return;
      }
      if (isMultiSelectMode) {
        currentStore.exitMultiSelectMode();
      } else {
        const activeIndex = navigationSnap.currentSelectedIndex;
        const activeItem = typeof activeIndex === "number" && activeIndex >= 0
          ? currentStore.getItem(activeIndex)
          : null;
        if (!activeItem?.id) {
          toast.warning(t("multiSelect.selectFirst"), TOAST_CONFIG);
          return;
        }
        currentStore.enterMultiSelectMode({
          id: activeItem.id,
          index: activeIndex,
          contentType: activeItem.content_type,
        });
      }
    };
    const handleMoreMenu = async (event) => {
      event.preventDefault();
      event.stopPropagation();
      const checkIcon = (enabled) => (enabled ? "ti ti-check" : undefined);
      const normalizedDisplayPriority = normalizeDisplayPriorityValue(
        settingsSnap.displayPriorityOrder,
      );
      const displayPriorityOptions = [
        {
          id: "text-html-image",
          value: "text,html,image",
          label: t("settings.clipboard.displayPriorityTextHtmlImage"),
        },
        {
          id: "text-image-html",
          value: "text,image,html",
          label: t("settings.clipboard.displayPriorityTextImageHtml"),
        },
        {
          id: "html-text-image",
          value: "html,text,image",
          label: t("settings.clipboard.displayPriorityHtmlTextImage"),
        },
        {
          id: "html-image-text",
          value: "html,image,text",
          label: t("settings.clipboard.displayPriorityHtmlImageText"),
        },
        {
          id: "image-text-html",
          value: "image,text,html",
          label: t("settings.clipboard.displayPriorityImageTextHtml"),
        },
        {
          id: "image-html-text",
          value: "image,html,text",
          label: t("settings.clipboard.displayPriorityImageHtmlText"),
        },
      ];
      const displayPriorityValueByMenuId = Object.fromEntries(
        displayPriorityOptions.map((option) => [
          `menu-display-priority-${option.id}`,
          option.value,
        ]),
      );
      const previewItem = createMenuItem({
        id: "menu-preview-group",
        label: t("tools.moreMenu.contentPreview"),
        icon: "ti ti-eye",
      });
      previewItem.children = [
        createMenuItem({
          id: "menu-preview-text",
          label: t("settings.clipboard.textPreview"),
          icon: checkIcon(settingsSnap.textPreview !== false),
        }),
        createMenuItem({
          id: "menu-preview-image",
          label: t("settings.clipboard.imagePreview"),
          icon: checkIcon(settingsSnap.imagePreview !== false),
        }),
        createMenuItem({
          id: "menu-preview-file",
          label: t("settings.clipboard.filePreview"),
          icon: checkIcon(settingsSnap.filePreview !== false),
        }),
      ];
      const displayPriorityItem = createMenuItem({
        id: "menu-display-priority-group",
        label: t("settings.clipboard.displayPriority"),
        icon: "ti ti-sort-descending-2",
      });
      displayPriorityItem.children = displayPriorityOptions.map((option) =>
        createMenuItem({
          id: `menu-display-priority-${option.id}`,
          label: option.label,
          icon: checkIcon(normalizedDisplayPriority === option.value),
        }),
      );
      const pasteItem = createMenuItem({
        id: "menu-paste-group",
        label: t("tools.moreMenu.globalPaste"),
        icon: "ti ti-clipboard",
      });
      pasteItem.children = [
        createMenuItem({
          id: "menu-paste-format",
          label: t("tools.formatToggle"),
          icon: checkIcon(settingsSnap.pasteWithFormat !== false),
        }),
        createMenuItem({
          id: "menu-paste-to-top",
          label: t("settings.clipboard.pasteToTop"),
          icon: checkIcon(settingsSnap.pasteToTop === true),
        }),
        createMenuItem({
          id: "menu-paste-one-time",
          label: t("tools.oneTimePaste"),
          icon: checkIcon(oneTimePasteEnabled),
        }),
      ];
      const fileHubItem = createMenuItem({
        id: "menu-file-hub-group",
        label: t("tools.moreMenu.fileHub", "文件中转"),
        icon: "ti ti-transfer",
      });
      fileHubItem.children = [
        createMenuItem({
          id: "menu-open-transfer-shelf",
          label: t("tools.moreMenu.newTransferShelf", "新建文件盒"),
          icon: "ti ti-package",
        }),
        createMenuItem({
          id: "menu-open-receive-box",
          label: t("tools.moreMenu.openReceiveBox", "打开收件盒"),
          icon: "ti ti-inbox",
        }),
      ];
      const webdavItem = createMenuItem({
        id: "menu-webdav-group",
        label: t("tools.moreMenu.webdav", "WebDAV 同步"),
        icon: webdavBusy ? "ti ti-loader-2" : "ti ti-cloud",
      });
      webdavItem.children = [
        createMenuItem({
          id: "menu-webdav-upload",
          label: t("settings.webdav.upload"),
          icon: webdavBusy === "push" ? "ti ti-loader-2" : "ti ti-cloud-up",
          disabled: Boolean(webdavBusy),
        }),
        createMenuItem({
          id: "menu-webdav-download",
          label: t("settings.webdav.download"),
          icon: webdavBusy === "pull" ? "ti ti-loader-2" : "ti ti-cloud-down",
          disabled: Boolean(webdavBusy),
        }),
      ];
      const menuItems = [
        fileHubItem,
        webdavItem,
        createSeparator(),
        previewItem,
        displayPriorityItem,
        pasteItem,
        createSeparator(),
        createMenuItem({
          id: "menu-clear-clipboard-history",
          label: t("contextMenu.clearAll"),
          icon: "ti ti-trash-x",
        }),
        createMenuItem({
          id: "menu-open-settings",
          label: t("tools.moreMenu.settings"),
          icon: "ti ti-settings",
        }),
      ];
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
      if (!result) {
        return;
      }
      const displayPriorityValue = displayPriorityValueByMenuId[result];
      if (displayPriorityValue) {
        try {
          await settingsStore.saveSetting(
            "displayPriorityOrder",
            displayPriorityValue,
          );
        } catch (error) {
          console.error("切换展示优先级失败:", error);
        }
        return;
      }
      switch (result) {
        case "menu-preview-text":
          try {
            await settingsStore.saveSetting(
              "textPreview",
              settingsSnap.textPreview === false,
            );
          } catch (error) {
            console.error("切换文本预览失败:", error);
          }
          break;
        case "menu-preview-image":
          try {
            await settingsStore.saveSetting(
              "imagePreview",
              settingsSnap.imagePreview === false,
            );
          } catch (error) {
            console.error("切换图片预览失败:", error);
          }
          break;
        case "menu-preview-file":
          try {
            await settingsStore.saveSetting(
              "filePreview",
              settingsSnap.filePreview === false,
            );
          } catch (error) {
            console.error("切换文件预览失败:", error);
          }
          break;
        case "menu-paste-format":
          try {
            await settingsStore.saveSetting(
              "pasteWithFormat",
              settingsSnap.pasteWithFormat === false,
            );
          } catch (error) {
            console.error("切换格式粘贴失败:", error);
          }
          break;
        case "menu-paste-to-top":
          try {
            await settingsStore.saveSetting(
              "pasteToTop",
              !Boolean(settingsStore.pasteToTop),
            );
          } catch (error) {
            console.error("切换粘贴后置顶失败:", error);
          }
          break;
        case "menu-paste-one-time":
          try {
            setOneTimePasteEnabledState(await toggleOneTimePasteEnabled());
          } catch (error) {
            console.error("切换一次性粘贴失败:", error);
          }
          break;
        case "menu-open-transfer-shelf":
          await handleOpenTransferShelf(event);
          break;
        case "menu-open-receive-box":
          await handleOpenReceiveBox(event);
          break;
        case "menu-webdav-upload":
          await handleWebdavAction(event, "push");
          break;
        case "menu-webdav-download":
          await handleWebdavAction(event, "pull");
          break;
        case "menu-clear-clipboard-history":
          try {
            const { showConfirm } = await import("@shared/utils/dialog");
            const confirmed = await showConfirm(
              t("contextMenu.clearAllConfirm"),
              t("contextMenu.clearAllConfirmTitle"),
            );
            if (!confirmed) {
              break;
            }
            await clearClipboardHistory();
            const { loadClipboardItems } =
              await import("@shared/store/clipboardStore");
            await loadClipboardItems();
            toast.success(t("contextMenu.allCleared"), TOAST_CONFIG);
          } catch (error) {
            console.error("标题栏清空剪贴板失败:", error);
            toast.error(t("common.operationFailed"), TOAST_CONFIG);
          }
          break;
        case "menu-open-settings":
          try {
            await openAppSettings();
          } catch (error) {
            console.error("标题栏打开设置失败:", error);
          }
          break;
        default:
          break;
      }
    };
    useImperativeHandle(ref, () => ({
      focus: () => {
        if (searchRef.current?.focus) {
          searchRef.current.focus();
        }
      },
      blur: () => {
        searchRef.current?.blur?.();
      },
      toggleFocus: () => {
        searchRef.current?.toggleFocus?.();
      },
      isFocused: () => {
        return searchRef.current?.isFocused?.() === true;
      },
    }));
    return (
      <div
        ref={dragRef}
        className={`title-bar flex-shrink-0 flex ${isVertical ? `w-10 h-full flex-col items-center justify-between py-2 bg-qc-panel ${position === "left" ? "border-r border-qc-border" : "border-l border-qc-border"}` : `h-9 flex-row items-center justify-between px-2 bg-qc-panel ${position === "top" ? "border-b border-qc-border" : "border-t border-qc-border"}`} relative overflow-hidden shadow-sm transition-colors duration-500`}
      >
        <div className="flex items-center gap-1.5 flex-shrink-0">
          {showUpdateHint ? (
            <Tooltip
              content={t("updater.newVersionFound", {
                version: updateBannerState.latestVersion,
              })}
              placement={tooltipPlacement}
              asChild
            >
              <button
                type="button"
                className="relative flex h-7 w-7 items-center justify-center rounded-full border-2 border-emerald-500 bg-emerald-50/80 text-emerald-600 shadow-sm transition-all duration-200 hover:bg-emerald-100 dark:bg-emerald-500/12 dark:text-emerald-400"
                onClick={handleOpenUpdater}
                aria-label={t("updater.newVersionFound", {
                  version: updateBannerState.latestVersion,
                })}
              >
                <img
                  src={logoIcon}
                  alt="QuickClipboard"
                  className="h-4.5 w-4.5 rounded-sm"
                  style={TITLE_BAR_IMAGE_ICON_STYLE}
                />
                <span className="pointer-events-none absolute inset-0 flex items-center justify-center">
                  <i
                    className="ti ti-arrow-big-up text-[12px] leading-none text-emerald-600 dark:text-emerald-400"
                    style={TITLE_BAR_FONT_ICON_STYLE}
                  />
                </span>
              </button>
            </Tooltip>
          ) : (
            <div className="w-6 h-6 flex items-center justify-center pointer-events-none">
              <img
                src={logoIcon}
                alt="QuickClipboard"
                className="w-5 h-5"
                style={TITLE_BAR_IMAGE_ICON_STYLE}
              />
            </div>
          )}
        </div>

        <div
          className={`flex ${isVertical ? "flex-col items-center gap-2" : "ml-2 min-w-0 flex-1 flex-row items-center justify-end gap-1"}`}
        >
          <TitleBarSearch
            ref={searchRef}
            value={searchQuery}
            onChange={onSearchChange}
            placeholder={searchPlaceholder}
            isVertical={isVertical}
            position={position}
          />

          <div className={actionContainerClass}>
            <Tooltip
              content={
                isMultiSelectMode
                  ? t("multiSelect.exitMode")
                  : t("multiSelect.enterMode")
              }
              placement={tooltipPlacement}
              asChild
            >
              <button
                className={`${actionButtonClass} ${isCompactActions ? "border-r border-b border-qc-border" : ""} ${!currentStore ? "text-qc-fg-subtle opacity-60 cursor-not-allowed" : isMultiSelectMode ? ACTIVE_ICON_BUTTON_CLASS : "hover:bg-qc-hover text-qc-fg-muted"}`}
                aria-label={
                  isMultiSelectMode
                    ? t("multiSelect.exitMode")
                    : t("multiSelect.enterMode")
                }
                type="button"
                onClick={handleToggleMultiSelect}
                disabled={!currentStore}
              >
                <i
                  className={
                    isMultiSelectMode ? "ti ti-list" : "ti ti-list-check"
                  }
                  style={actionIconStyle}
                  data-stroke="1.5"
                ></i>
              </button>
            </Tooltip>

            <Tooltip
              content={t("settings.clipboard.edgeHideEnabled")}
              placement={tooltipPlacement}
              asChild
            >
              <button
                className={`${actionButtonClass} ${isCompactActions ? "border-b border-qc-border" : ""} ${settingsSnap.edgeHideEnabled ? ACTIVE_ICON_BUTTON_CLASS : "hover:bg-qc-hover text-qc-fg-muted"}`}
                role="switch"
                aria-checked={Boolean(settingsSnap.edgeHideEnabled)}
                aria-label={t("settings.clipboard.edgeHideEnabled")}
                type="button"
                onClick={handleToggleEdgeHide}
              >
                <i
                  className={settingsSnap.edgeHideEnabled ? "ti ti-browser-check" : "ti ti-browser"}
                  style={actionIconStyle}
                  data-stroke="1.5"
                ></i>
              </button>
            </Tooltip>

            <Tooltip
              content={t("tools.pin")}
              placement={tooltipPlacement}
              asChild
            >
              <button
                className={`${actionButtonClass} ${isCompactActions ? "border-r border-qc-border" : ""} ${isPinned ? ACTIVE_ICON_BUTTON_CLASS : "hover:bg-qc-hover text-qc-fg-muted"}`}
                onClick={handleTogglePin}
                aria-label={t("tools.pin")}
              >
                <i
                  className="ti ti-pin"
                  style={actionIconStyle}
                  data-stroke="1.5"
                ></i>
              </button>
            </Tooltip>

            <Tooltip
              content={t("tools.more")}
              placement={tooltipPlacement}
              asChild
            >
              <button
                className={`${actionButtonClass} hover:bg-qc-hover text-qc-fg-muted`}
                aria-label={t("tools.more")}
                type="button"
                onClick={handleMoreMenu}
              >
                <i
                  className="ti ti-dots"
                  style={actionIconStyle}
                  data-stroke="1.5"
                ></i>
              </button>
            </Tooltip>
          </div>
        </div>
      </div>
    );
  },
);
TitleBar.displayName = "TitleBar";
export default TitleBar;
