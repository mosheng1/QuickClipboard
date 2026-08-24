import "@tabler/icons-webfont/dist/tabler-icons.min.css";
import { useTranslation } from "react-i18next";
import { useSnapshot } from "valtio";
import Tooltip from "@shared/components/common/Tooltip.jsx";
import { showConfirm } from "@shared/utils/dialog";
import { clipboardStore } from "@shared/store/clipboardStore";
import { favoritesStore, refreshFavorites } from "@shared/store/favoritesStore";
import { groupsStore } from "@shared/store/groupsStore";
import { settingsStore } from "@shared/store/settingsStore";
import {
  addClipboardToFavorites,
  deleteClipboardItems,
  mergeCopyClipboardItems,
} from "@shared/api/clipboard";
import {
  deleteFavoriteItems,
  mergeCopyFavoriteItems,
} from "@shared/api/favorites";
import { moveFavoriteToGroup } from "@shared/api/groups";
import {
  createMenuItem,
  createMenuPlacementFromEvent,
  showContextMenu,
} from "@/plugins/context_menu/index.js";
import { toast, TOAST_POSITIONS, TOAST_SIZES } from "@shared/store/toastStore";
import { getSelectionMergeState, mergePasteSelectedItems } from "../utils/multiSelect";
function MultiSelectActionBar({ activeTab }) {
  const { t } = useTranslation();
  const clipboardSnap = useSnapshot(clipboardStore);
  const favoritesSnap = useSnapshot(favoritesStore);
  const groupsSnap = useSnapshot(groupsStore);
  const settingsSnap = useSnapshot(settingsStore);
  const currentSnap =
    activeTab === "clipboard"
      ? clipboardSnap
      : activeTab === "favorites"
        ? favoritesSnap
        : null;
  const currentStore =
    activeTab === "clipboard"
      ? clipboardStore
      : activeTab === "favorites"
        ? favoritesStore
        : null;
  if (!currentSnap?.isMultiSelectMode || !currentStore) {
    return null;
  }
  const selectedEntries = currentSnap.selectedEntries || [];
  const selectedCount = selectedEntries.length;
  const selectedIds = currentStore.getSelectedIds();
  const mergeState = getSelectionMergeState(selectedEntries);
  const getDisabledTooltip = (reasonKey) => t(`multiSelect.${reasonKey}`);
  const makeActionButtonClasses = (disabled) =>
    `
    flex items-center justify-center
    w-7 h-7
    rounded-md border
    transition-colors duration-200
    ${disabled ? "cursor-not-allowed border-qc-border bg-qc-panel text-qc-fg-subtle opacity-60" : "border-qc-border bg-qc-panel text-qc-fg-muted hover:bg-qc-hover hover:text-qc-fg"}
  `
      .trim()
      .replace(/\s+/g, " ");
  const withToastConfig = {
    size: TOAST_SIZES.EXTRA_SMALL,
    position: TOAST_POSITIONS.BOTTOM_RIGHT,
  };
  const handleMergeCopy = async () => {
    if (!selectedCount || !mergeState.canMerge) return;
    try {
      if (activeTab === "clipboard") {
        await mergeCopyClipboardItems(selectedIds);
      } else {
        await mergeCopyFavoriteItems(selectedIds);
      }
      toast.success(t("multiSelect.mergeCopied"), withToastConfig);
    } catch (error) {
      console.error("合并复制失败:", error);
      toast.error(error?.message || t("common.copyFailed"), withToastConfig);
    }
  };
  const handleMergePaste = async () => {
    if (!selectedCount || !mergeState.canMerge) return;
    try {
      if (await mergePasteSelectedItems(activeTab)) {
        toast.success(t("multiSelect.mergePasted"), withToastConfig);
      }
    } catch (error) {
      console.error("合并粘贴失败:", error);
      toast.error(error?.message || t("common.pasteFailed"), withToastConfig);
    }
  };
  const handleDelete = async () => {
    if (!selectedCount) return;
    const confirmed = await showConfirm(
      t("multiSelect.confirmDelete", {
        count: selectedCount,
      }),
      t("multiSelect.confirmDeleteTitle"),
    );
    if (!confirmed) {
      return;
    }
    try {
      if (activeTab === "clipboard") {
        await deleteClipboardItems(selectedIds);
        clipboardStore.removeItems(selectedIds);
      } else {
        await deleteFavoriteItems(selectedIds);
        favoritesStore.removeItems(selectedIds);
      }
      currentStore.exitMultiSelectMode();
      toast.success(t("common.deleted"), withToastConfig);
    } catch (error) {
      console.error("批量删除失败:", error);
      toast.error(error?.message || t("common.deleteFailed"), withToastConfig);
    }
  };
  const handleGroupAction = async (event) => {
    if (!selectedCount) return;
    const groups = groupsSnap.groups || [];
    const availableGroups =
      activeTab === "favorites" && groupsSnap.currentGroup !== "全部"
        ? groups.filter((group) => group.name !== groupsSnap.currentGroup)
        : groups;
    if (!availableGroups.length) {
      return;
    }
    const actionPrefix =
      activeTab === "clipboard"
        ? "multi-select-add-group-"
        : "multi-select-move-group-";
    const menuItems = availableGroups.map((group) =>
      createMenuItem({
        id: `${actionPrefix}${group.name}`,
        label: group.name,
        icon: group.icon || "ti ti-folder",
        iconColor: group.name === "全部" ? null : group.color || "#dc2626",
      }),
    );
    const result = await showContextMenu({
      items: menuItems,
      placement: createMenuPlacementFromEvent(event),
      appearance: {
        theme: settingsSnap.theme,
        lightThemeStyle: settingsSnap.lightThemeStyle,
        darkThemeStyle: settingsSnap.darkThemeStyle,
        uiAnimationEnabled: settingsSnap.uiAnimationEnabled,
      },
    });
    if (!result || !result.startsWith(actionPrefix)) {
      return;
    }
    const groupName = result.substring(actionPrefix.length);
    try {
      if (activeTab === "clipboard") {
        await Promise.all(
          selectedIds.map((id) => addClipboardToFavorites(id, groupName)),
        );
      } else {
        await Promise.all(
          selectedIds.map((id) => moveFavoriteToGroup(id, groupName)),
        );
        await refreshFavorites(groupsSnap.currentGroup);
      }
      currentStore.exitMultiSelectMode();
      toast.success(
        activeTab === "clipboard"
          ? t("contextMenu.addedToFavorites")
          : t("contextMenu.movedToGroup"),
        withToastConfig,
      );
    } catch (error) {
      console.error(
        activeTab === "clipboard"
          ? "批量添加到分组失败:"
          : "批量移动到分组失败:",
        error,
      );
      toast.error(
        error?.message || t("common.operationFailed"),
        withToastConfig,
      );
    }
  };
  return (
    <div className="multi-select-action-bar flex-shrink-0 h-9 px-2 border-t border-qc-border bg-qc-panel backdrop-blur-sm rounded-bl-[8px] rounded-br-[8px]">
      <div className="h-full flex items-center justify-between gap-2">
        <div className="flex min-w-0 flex-col items-start leading-tight">
          <div className="whitespace-nowrap text-xs font-medium text-qc-fg">
            {t("multiSelect.selectedLabel")}{" "}
            <span className="font-semibold text-blue-500">{selectedCount}</span>{" "}
            {t("multiSelect.selectedUnit")}
          </div>
          <div className="mt-0.5 whitespace-nowrap text-[10px] font-normal text-qc-fg-subtle">
            {t("multiSelect.operationHint")}
          </div>
        </div>

        <div className="flex items-center gap-1.5">
          <Tooltip
            content={
              !selectedCount
                ? getDisabledTooltip("selectFirst")
                : !mergeState.canMerge
                  ? getDisabledTooltip(mergeState.reasonKey)
                  : t("multiSelect.mergeCopy")
            }
            placement="top"
            asChild
          >
            <button
              className={makeActionButtonClasses(
                !selectedCount || !mergeState.canMerge,
              )}
              onClick={handleMergeCopy}
              aria-disabled={!selectedCount || !mergeState.canMerge}
            >
              <i
                className="ti ti-copy"
                style={{
                  fontSize: 14,
                }}
              ></i>
            </button>
          </Tooltip>

          <Tooltip
            content={
              !selectedCount
                ? getDisabledTooltip("selectFirst")
                : !mergeState.canMerge
                  ? getDisabledTooltip(mergeState.reasonKey)
                  : t("multiSelect.mergePaste")
            }
            placement="top"
            asChild
          >
            <button
              className={makeActionButtonClasses(
                !selectedCount || !mergeState.canMerge,
              )}
              onClick={handleMergePaste}
              aria-disabled={!selectedCount || !mergeState.canMerge}
            >
              <i
                className="ti ti-clipboard-list"
                style={{
                  fontSize: 14,
                }}
              ></i>
            </button>
          </Tooltip>

          <Tooltip
            content={
              selectedCount
                ? activeTab === "clipboard"
                  ? t("contextMenu.addToFavorites")
                  : t("contextMenu.moveToGroup")
                : getDisabledTooltip("selectFirst")
            }
            placement="top"
            asChild
          >
            <button
              className={makeActionButtonClasses(!selectedCount)}
              onClick={handleGroupAction}
              aria-disabled={!selectedCount}
            >
              <i
                className={
                  activeTab === "clipboard"
                    ? "ti ti-folder-plus"
                    : "ti ti-folder-share"
                }
                style={{
                  fontSize: 14,
                }}
              ></i>
            </button>
          </Tooltip>

          <Tooltip
            content={
              selectedCount
                ? t("multiSelect.deleteSelected")
                : getDisabledTooltip("selectFirst")
            }
            placement="top"
            asChild
          >
            <button
              className={makeActionButtonClasses(!selectedCount)}
              onClick={handleDelete}
              aria-disabled={!selectedCount}
            >
              <i
                className="ti ti-trash"
                style={{
                  fontSize: 14,
                }}
              ></i>
            </button>
          </Tooltip>

          <Tooltip content={t("multiSelect.exitMode")} placement="top" asChild>
            <button
              className={makeActionButtonClasses(false)}
              onClick={() => currentStore.exitMultiSelectMode()}
            >
              <i
                className="ti ti-x"
                style={{
                  fontSize: 14,
                }}
              ></i>
            </button>
          </Tooltip>
        </div>
      </div>
    </div>
  );
}
export default MultiSelectActionBar;
